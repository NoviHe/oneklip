use crate::error::{AppError, AppResult};
use crate::services::ai_viral::AiConfig;
use crate::services::binaries::{check_binaries, ensure_all_binaries, BinariesStatus};
use crate::services::encoder::{detect_best_encoder, EncoderInfo};
use crate::services::innertube::{fetch_metadata_innertube, Transcript, VideoMetadata};
use crate::services::render::hook_style::HookStyle;
use crate::services::render::session::{
    execute_session_pipeline, load_ai_config, rerender_single_clip, StartPipelineRequest,
};
use crate::services::render::subtitle_style::SubtitleStyle;
use crate::services::render::worker::ClipRenderSpec;
use crate::services::waveform::extract_waveform_peaks;
use crate::services::ytdlp::{fetch_metadata_ytdlp, probe_local_video};
use crate::state::AppState;
use crate::storage::{ClipRecord, SessionRecord};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use sysinfo::System;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemDiagnostics {
    pub binaries: BinariesStatus,
    pub encoder: EncoderInfo,
    pub ai_config: AiConfig,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub total_ram_gb: f32,
    pub app_data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetailResponse {
    pub session: SessionRecord,
    pub clips: Vec<ClipRecord>,
    pub transcript: Option<Transcript>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipEditorData {
    pub clip: ClipRecord,
    pub render_spec: ClipRenderSpec,
    pub waveform_peaks: Vec<f32>,
    pub raw_video_path: String,
}

#[tauri::command]
pub async fn get_system_diagnostics(state: State<'_, AppState>) -> AppResult<SystemDiagnostics> {
    let st = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let binaries = check_binaries(&st);
        let encoder = detect_best_encoder(&st.ffmpeg_path());
        let ai_config = load_ai_config(&st);

        let mut sys = System::new_all();
        sys.refresh_all();
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "x86_64 Processor".to_string());
        let cpu_cores = sys.cpus().len().max(1);
        let total_ram_gb = (sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0)) as f32;

        Ok(SystemDiagnostics {
            binaries,
            encoder,
            ai_config,
            cpu_brand,
            cpu_cores,
            total_ram_gb,
            app_data_dir: st.app_data_dir.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| AppError::custom("join_error", e.to_string()))?
}

#[tauri::command]
pub async fn install_missing_binaries(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<BinariesStatus> {
    ensure_all_binaries(state.inner(), Some(&app)).await
}

#[tauri::command]
pub fn get_ai_config(state: State<'_, AppState>) -> AppResult<AiConfig> {
    Ok(load_ai_config(state.inner()))
}

#[tauri::command]
pub fn save_ai_config(state: State<'_, AppState>, config: AiConfig) -> AppResult<AiConfig> {
    let json_str = serde_json::to_string(&config)?;
    state.db.set_kv("ai_config", &json_str)?;
    Ok(config)
}

#[tauri::command]
pub fn save_youtube_cookies(state: State<'_, AppState>, cookies_text: String) -> AppResult<bool> {
    let path = state.cookies_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, cookies_text)?;
    Ok(true)
}

#[tauri::command]
pub async fn probe_video_source(
    state: State<'_, AppState>,
    video_url: Option<String>,
    local_path: Option<String>,
) -> AppResult<VideoMetadata> {
    if let Some(url) = video_url.filter(|u| !u.trim().is_empty()) {
        if let Ok(meta) = fetch_metadata_innertube(&url).await {
            return Ok(meta);
        }
        let st = state.inner().clone();
        return tokio::task::spawn_blocking(move || {
            fetch_metadata_ytdlp(&st, &url, st.cookies_path().exists())
        })
        .await
        .map_err(|e| AppError::custom("join_error", e.to_string()))?;
    }

    if let Some(lpath) = local_path.filter(|p| !p.trim().is_empty()) {
        let st = state.inner().clone();
        return tokio::task::spawn_blocking(move || probe_local_video(&st, Path::new(&lpath)))
            .await
            .map_err(|e| AppError::custom("join_error", e.to_string()))?;
    }

    Err(AppError::custom(
        "missing_source",
        "Masukkan URL video atau pilih file video lokal",
    ))
}

#[tauri::command]
pub async fn start_clipper_session(
    state: State<'_, AppState>,
    app: AppHandle,
    request: StartPipelineRequest,
) -> AppResult<String> {
    let session_id = Uuid::new_v4().to_string();
    let st = state.inner().clone();
    let sid_clone = session_id.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(err) = execute_session_pipeline(st.clone(), app.clone(), sid_clone.clone(), request).await {
            let err_msg = err.to_string();
            let _ = st.db.update_session_status(&sid_clone, "failed", Some(&err_msg));
            let _ = tauri::Emitter::emit(
                &app,
                "pipeline:error",
                serde_json::json!({
                    "sessionId": sid_clone,
                    "error": err_msg,
                }),
            );
        }
    });

    Ok(session_id)
}

#[tauri::command]
pub fn cancel_clipper_session(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    state.cancel_task(&session_id);
    let _ = state
        .db
        .update_session_status(&session_id, "cancelled", Some("Dibatalkan oleh pengguna"));
    Ok(())
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> AppResult<Vec<SessionRecord>> {
    state.db.list_sessions()
}

#[tauri::command]
pub fn get_session_detail(
    state: State<'_, AppState>,
    session_id: String,
) -> AppResult<SessionDetailResponse> {
    let session = state
        .db
        .get_session(&session_id)?
        .ok_or_else(|| AppError::custom("not_found", "Sesi tidak ditemukan"))?;
    let clips = state.db.list_clips_for_session(&session_id)?;

    let captions_path = PathBuf::from(&session.folder_path).join("captions.json");
    let transcript = if captions_path.exists() {
        std::fs::read_to_string(&captions_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Transcript>(&s).ok())
    } else {
        None
    };

    Ok(SessionDetailResponse {
        session,
        clips,
        transcript,
    })
}

#[tauri::command]
pub fn delete_session(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    if let Some(folder) = state.db.delete_session(&session_id)? {
        let p = PathBuf::from(folder);
        if p.exists() {
            let _ = std::fs::remove_dir_all(p);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_clip_editor_data(
    state: State<'_, AppState>,
    clip_id: String,
) -> AppResult<ClipEditorData> {
    let st = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let clip = st
            .db
            .get_clip(&clip_id)?
            .ok_or_else(|| AppError::custom("clip_not_found", "Klip tidak ditemukan"))?;

        let final_path = PathBuf::from(&clip.file_path);
        let clip_dir = final_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let raw_mp4 = clip_dir.join("raw.mp4");
        let media_for_waveform = if raw_mp4.exists() {
            raw_mp4.clone()
        } else {
            final_path.clone()
        };

        let render_spec: ClipRenderSpec = clip
            .editor_state_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| ClipRenderSpec {
                clip_id: clip.id.clone(),
                video_layout: clip.video_layout.clone(),
                subtitle_style: SubtitleStyle::default(),
                hook_style: HookStyle::default(),
                hook_text: clip.hook_text.clone(),
                hook_kicker: clip.hook_kicker.clone(),
                hook_tag: clip.hook_tag.clone(),
                has_captions: clip.has_captions,
                has_hook: clip.has_hook,
                has_watermark: clip.has_watermark,
                watermark_text: None,
                words: Vec::new(),
            });

        let waveform_peaks =
            extract_waveform_peaks(&st, &media_for_waveform, 160).unwrap_or_else(|_| vec![0.2; 160]);

        Ok(ClipEditorData {
            clip,
            render_spec,
            waveform_peaks,
            raw_video_path: raw_mp4.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|e| AppError::custom("join_error", e.to_string()))?
}

#[tauri::command]
pub async fn rerender_clip(
    state: State<'_, AppState>,
    clip_id: String,
    spec: ClipRenderSpec,
    new_start_seconds: Option<f64>,
    new_end_seconds: Option<f64>,
) -> AppResult<ClipRecord> {
    let st = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        rerender_single_clip(&st, &clip_id, spec, new_start_seconds, new_end_seconds)
    })
    .await
    .map_err(|e| AppError::custom("join_error", e.to_string()))?
}

#[tauri::command]
pub fn get_subtitle_presets() -> Vec<SubtitleStyle> {
    SubtitleStyle::builtin_presets()
}

#[tauri::command]
pub fn export_clip_to_path(
    state: State<'_, AppState>,
    clip_id: String,
    dest_path: String,
) -> AppResult<ClipRecord> {
    let mut clip = state
        .db
        .get_clip(&clip_id)?
        .ok_or_else(|| AppError::custom("clip_not_found", "Klip tidak ditemukan"))?;

    let src = PathBuf::from(&clip.file_path);
    let dst = PathBuf::from(&dest_path);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&src, &dst)?;
    clip.exported_to = Some(dest_path);
    state.db.upsert_clip(&clip)?;
    Ok(clip)
}

#[tauri::command]
pub fn open_path_in_explorer(path: String) -> AppResult<()> {
    let p = PathBuf::from(&path);
    #[cfg(target_os = "windows")]
    {
        if p.is_file() {
            let _ = Command::new("explorer")
                .arg(format!("/select,{}", p.to_string_lossy()))
                .spawn();
        } else if p.exists() {
            let _ = Command::new("explorer").arg(&p).spawn();
        }
    }
    Ok(())
}

use crate::error::{AppError, AppResult};
use crate::services::ai_viral::{detect_viral_moments, AiConfig};
use crate::services::innertube::{fetch_metadata_innertube, Transcript};
use crate::services::render::hook_style::HookStyle;
use crate::services::render::subtitle_style::SubtitleStyle;
use crate::services::render::worker::{render_clip_master, ClipRenderSpec};
use crate::services::subtitles::slice_words_for_clip;
use crate::services::waveform::detect_audio_energy_spikes;
use crate::services::whisper::resolve_transcript_4tier;
use crate::services::ytdlp::{
    download_video_ytdlp, emit_pipeline_progress, extract_raw_clip, fetch_metadata_ytdlp,
    probe_local_video,
};
use crate::state::AppState;
use crate::storage::{ClipRecord, SessionRecord};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartPipelineRequest {
    pub video_url: Option<String>,
    pub local_video_path: Option<String>,
    pub video_language: String,
    pub content_type: String,     // "podcast" | "gaming" | "education" | "general"
    pub default_layout: String,   // "auto_banter" | "smart_speaker" | "split_screen" | "fill" | "fit_blur" | "fit_black"
    pub subtitle_preset: String,  // e.g. "hormozi_pop"
    pub hook_preset: String,      // e.g. "punchline"
    pub max_clips: Option<usize>,
}

pub fn load_ai_config(state: &AppState) -> AiConfig {
    state
        .db
        .get_kv("ai_config")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<AiConfig>(&s).ok())
        .unwrap_or_default()
}

pub async fn execute_session_pipeline(
    state: AppState,
    app: AppHandle,
    session_id: String,
    req: StartPipelineRequest,
) -> AppResult<SessionRecord> {
    let cancel_flag = state.register_cancel_token(&session_id);
    let session_dir = state.sessions_dir.join(&session_id);
    std::fs::create_dir_all(&session_dir)?;

    let now = Utc::now().to_rfc3339();
    let mut session_rec = SessionRecord {
        id: session_id.clone(),
        folder_path: session_dir.to_string_lossy().to_string(),
        video_url: req.video_url.clone(),
        video_id: None,
        local_video_path: req.local_video_path.clone(),
        video_title: Some("Memuat metadata video...".to_string()),
        video_channel: None,
        video_duration: None,
        video_language: req.video_language.clone(),
        content_type: req.content_type.clone(),
        status: "processing".to_string(),
        highlights_count: 0,
        clips_processed: 0,
        total_size_bytes: 0,
        last_error: None,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
    };
    state.db.upsert_session(&session_rec)?;

    // Step 1: Fetch Video Metadata
    emit_pipeline_progress(
        Some(&app),
        &session_id,
        "metadata",
        "Mengambil informasi metadata video...",
        5.0,
    );

    let meta = if let Some(ref url) = req.video_url {
        match fetch_metadata_innertube(url).await {
            Ok(m) => m,
            Err(_) => fetch_metadata_ytdlp(&state, url, state.cookies_path().exists())?,
        }
    } else if let Some(ref lpath) = req.local_video_path {
        probe_local_video(&state, Path::new(lpath))?
    } else {
        return Err(AppError::custom(
            "missing_input",
            "Harap masukkan URL YouTube atau pilih file video lokal",
        ));
    };

    session_rec.video_id = meta.video_id.clone();
    session_rec.video_title = Some(meta.title.clone());
    session_rec.video_channel = Some(meta.channel.clone());
    session_rec.video_duration = Some(meta.duration_seconds);
    session_rec.updated_at = Utc::now().to_rfc3339();
    state.db.upsert_session(&session_rec)?;

    // Step 2: Ensure local source video is available
    let source_video_path: PathBuf = if let Some(ref lpath) = req.local_video_path {
        PathBuf::from(lpath)
    } else if let Some(ref url) = req.video_url {
        let dl_dest = if let Some(ref vid) = meta.video_id {
            state
                .cache_dir
                .join("downloads")
                .join(format!("{}.mp4", vid))
        } else {
            session_dir.join("source.mp4")
        };
        emit_pipeline_progress(
            Some(&app),
            &session_id,
            "downloading_video",
            format!("Mengunduh video \"{}\"...", meta.title),
            12.0,
        );
        download_video_ytdlp(
            &state,
            Some(&app),
            &session_id,
            url,
            &dl_dest,
            false,
            Some(&cancel_flag),
        )?
    } else {
        unreachable!()
    };

    session_rec.local_video_path = Some(source_video_path.to_string_lossy().to_string());
    state.db.upsert_session(&session_rec)?;

    if cancel_flag.load(Ordering::SeqCst) {
        return Err(AppError::custom("cancelled", "Dibatalkan oleh pengguna"));
    }

    // Step 3: 4-Tier Transcript Ladder
    let transcript: Transcript = resolve_transcript_4tier(
        &state,
        Some(&app),
        &session_id,
        req.video_url.as_deref(),
        Some(&source_video_path),
        &req.video_language,
        &session_dir,
    )
    .await?;

    if session_rec.video_duration.unwrap_or(0.0) <= 1.0 {
        session_rec.video_duration = Some(transcript.duration_seconds);
        state.db.upsert_session(&session_rec)?;
    }

    // Step 4: Audio RMS Energy Spike Analysis + AI Viral Moment Engine
    emit_pipeline_progress(
        Some(&app),
        &session_id,
        "analyzing_viral",
        "Menganalisis lonjakan audio & mencari momen berpotensi viral...",
        45.0,
    );

    let audio_spikes = detect_audio_energy_spikes(&state, &source_video_path);
    let mut ai_config = load_ai_config(&state);
    if let Some(mc) = req.max_clips {
        ai_config.max_clips = mc.clamp(1, 20);
    }

    let highlights = detect_viral_moments(
        &transcript,
        &audio_spikes,
        &req.content_type,
        &ai_config,
    )
    .await?;

    if highlights.is_empty() {
        return Err(AppError::custom(
            "no_highlights_found",
            "Tidak ditemukan segmen klip yang memenuhi kriteria durasi",
        ));
    }

    session_rec.highlights_count = highlights.len() as i64;
    session_rec.updated_at = Utc::now().to_rfc3339();
    state.db.upsert_session(&session_rec)?;

    // Step 5: Cut & Render Each Viral Clip (Smart Crop 9:16 + Subtitles .ass + Hook PNG)
    let total_clips = highlights.len();
    let mut total_bytes: i64 = 0;

    for (idx, hl) in highlights.into_iter().enumerate() {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(AppError::custom("cancelled", "Dibatalkan oleh pengguna"));
        }

        let clip_num = idx + 1;
        let pct = 50.0 + ((idx as f32 / total_clips as f32) * 48.0);
        emit_pipeline_progress(
            Some(&app),
            &session_id,
            "rendering_clip",
            format!(
                "Merender klip {}/{}: \"{}\" (Skor Viral {})...",
                clip_num, total_clips, hl.title, hl.virality_score
            ),
            pct,
        );

        let clip_id = Uuid::new_v4().to_string();
        let clip_dir = session_dir.join(format!("clip_{}", clip_num));
        std::fs::create_dir_all(&clip_dir)?;

        let raw_mp4 = clip_dir.join("raw.mp4");
        let final_mp4 = clip_dir.join("final.mp4");
        let preview_jpg = clip_dir.join("preview.jpg");

        extract_raw_clip(
            &state,
            &source_video_path,
            hl.start_seconds,
            hl.end_seconds,
            &raw_mp4,
        )?;

        let words = slice_words_for_clip(&transcript, hl.start_seconds, hl.end_seconds);
        let subtitle_style = SubtitleStyle::from_preset(&req.subtitle_preset);
        let hook_style = HookStyle::from_preset(&req.hook_preset);

        let mut render_spec = ClipRenderSpec {
            clip_id: clip_id.clone(),
            video_layout: req.default_layout.clone(),
            subtitle_style,
            hook_style,
            hook_text: Some(hl.hook_text.clone()),
            hook_kicker: Some(hl.hook_kicker.clone()),
            hook_tag: Some(hl.hook_tag.clone()),
            has_captions: true,
            has_hook: true,
            has_watermark: false,
            watermark_text: None,
            words,
        };

        let effective_layout =
            render_clip_master(&state, &raw_mp4, &clip_dir, &render_spec, &final_mp4)?;
        render_spec.video_layout = effective_layout.clone();

        let file_size = std::fs::metadata(&final_mp4)
            .map(|m| m.len() as i64)
            .unwrap_or(0);
        total_bytes += file_size;

        let clip_rec = ClipRecord {
            id: clip_id,
            session_id: session_id.clone(),
            position: clip_num as i64,
            start_seconds: hl.start_seconds,
            end_seconds: hl.end_seconds,
            duration_seconds: hl.duration_seconds,
            title: hl.title,
            hook_text: Some(hl.hook_text),
            hook_kicker: Some(hl.hook_kicker),
            hook_tag: Some(hl.hook_tag),
            description: Some(hl.description),
            category: hl.category,
            virality_score: hl.virality_score,
            video_layout: effective_layout,
            preview_path: if preview_jpg.exists() {
                Some(preview_jpg.to_string_lossy().to_string())
            } else {
                None
            },
            file_path: final_mp4.to_string_lossy().to_string(),
            file_size_bytes: Some(file_size),
            resolution: Some("1080x1920".to_string()),
            has_hook: true,
            has_captions: true,
            has_watermark: false,
            editor_state_json: serde_json::to_string(&render_spec).ok(),
            exported_to: None,
            created_at: Utc::now().to_rfc3339(),
        };

        state.db.upsert_clip(&clip_rec)?;
        session_rec.clips_processed = clip_num as i64;
        session_rec.total_size_bytes = total_bytes;
        session_rec.updated_at = Utc::now().to_rfc3339();
        state.db.upsert_session(&session_rec)?;

        let _ = app.emit("pipeline:clip_ready", &clip_rec);
    }

    session_rec.status = "completed".to_string();
    session_rec.completed_at = Some(Utc::now().to_rfc3339());
    session_rec.updated_at = Utc::now().to_rfc3339();
    state.db.upsert_session(&session_rec)?;

    emit_pipeline_progress(
        Some(&app),
        &session_id,
        "completed",
        "Semua klip viral selesai diproses!",
        100.0,
    );
    let _ = app.emit("pipeline:completed", &session_rec);

    Ok(session_rec)
}

/// Re-renders a single clip when edited in the Studio Editor (e.g., new subtitle preset, edited words, new layout, or new trim).
pub fn rerender_single_clip(
    state: &AppState,
    clip_id: &str,
    updated_spec: ClipRenderSpec,
    new_start_seconds: Option<f64>,
    new_end_seconds: Option<f64>,
) -> AppResult<ClipRecord> {
    let mut clip = state
        .db
        .get_clip(clip_id)?
        .ok_or_else(|| AppError::custom("clip_not_found", "Data klip tidak ditemukan"))?;

    let session = state
        .db
        .get_session(&clip.session_id)?
        .ok_or_else(|| AppError::custom("session_not_found", "Data sesi tidak ditemukan"))?;

    let final_mp4 = PathBuf::from(&clip.file_path);
    let clip_dir = final_mp4
        .parent()
        .ok_or_else(|| AppError::custom("invalid_clip_dir", "Folder klip tidak valid"))?
        .to_path_buf();
    let raw_mp4 = clip_dir.join("raw.mp4");

    // If start/end trim timestamps changed, re-extract raw.mp4 from source video
    if let (Some(ns), Some(ne)) = (new_start_seconds, new_end_seconds) {
        if (ns - clip.start_seconds).abs() > 0.05 || (ne - clip.end_seconds).abs() > 0.05 {
            if let Some(ref src_video) = session.local_video_path {
                let src_path = PathBuf::from(src_video);
                if src_path.exists() {
                    extract_raw_clip(state, &src_path, ns, ne, &raw_mp4)?;
                    clip.start_seconds = ns;
                    clip.end_seconds = ne;
                    clip.duration_seconds = (ne - ns).max(1.0);
                }
            }
        }
    }

    let effective_layout = render_clip_master(state, &raw_mp4, &clip_dir, &updated_spec, &final_mp4)?;

    clip.video_layout = effective_layout;
    clip.hook_text = updated_spec.hook_text.clone();
    clip.hook_kicker = updated_spec.hook_kicker.clone();
    clip.hook_tag = updated_spec.hook_tag.clone();
    clip.has_captions = updated_spec.has_captions;
    clip.has_hook = updated_spec.has_hook;
    clip.has_watermark = updated_spec.has_watermark;
    clip.file_size_bytes = std::fs::metadata(&final_mp4)
        .map(|m| m.len() as i64)
        .ok();
    clip.editor_state_json = serde_json::to_string(&updated_spec).ok();

    state.db.upsert_clip(&clip)?;
    Ok(clip)
}

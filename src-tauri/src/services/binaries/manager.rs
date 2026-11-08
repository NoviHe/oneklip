use crate::error::{AppError, AppResult};
use crate::state::AppState;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatus {
    pub name: String,
    pub installed: bool,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinariesStatus {
    pub all_ready: bool,
    pub ffmpeg: ComponentStatus,
    pub ffprobe: ComponentStatus,
    pub ytdlp: ComponentStatus,
    pub deno: ComponentStatus,
    pub whisper_cli: ComponentStatus,
    pub onnxruntime: ComponentStatus,
    pub whisper_model: ComponentStatus,
    pub yunet_model: ComponentStatus,
    pub silero_vad_model: ComponentStatus,
    pub cookies_present: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryDownloadProgress {
    pub component: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: f32,
    pub status: String,
}

fn stat_component(name: &str, path: &Path, min_bytes: u64) -> ComponentStatus {
    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    ComponentStatus {
        name: name.to_string(),
        installed: size_bytes >= min_bytes,
        path: path.to_string_lossy().to_string(),
        size_bytes,
    }
}

/// Attempts to hardlink or copy a file from `src` to `dst` if `dst` does not exist or is too small.
fn link_or_copy_file(src: &Path, dst: &Path, min_bytes: u64) -> std::io::Result<bool> {
    if !src.exists() {
        return Ok(false);
    }
    let src_len = std::fs::metadata(src)?.len();
    if src_len < min_bytes {
        return Ok(false);
    }
    if let Ok(dst_meta) = std::fs::metadata(dst) {
        if dst_meta.len() >= min_bytes {
            return Ok(false);
        }
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if dst.exists() {
        let _ = std::fs::remove_file(dst);
    }
    if std::fs::hard_link(src, dst).is_err() {
        std::fs::copy(src, dst)?;
    }
    Ok(true)
}

/// Copies recursive directory contents using hard_link with copy fallback.
fn sync_directory(src_dir: &Path, dst_dir: &Path) -> std::io::Result<()> {
    if !src_dir.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst_dir)?;
    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst_dir.join(entry.file_name());
        if file_type.is_dir() {
            sync_directory(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            let _ = link_or_copy_file(&src_path, &dst_path, 1);
        }
    }
    Ok(())
}

/// Ensures vcruntime DLLs are placed next to sidecar binaries so clean Windows machines work out of the box.
pub fn ensure_vcruntime_dlls(state: &AppState) {
    if !state.vcruntime_dir.exists() {
        return;
    }
    let whisper_dir = state.bin_dir.join("whisper");
    let _ = std::fs::create_dir_all(&whisper_dir);
    if let Ok(entries) = std::fs::read_dir(&state.vcruntime_dir) {
        for entry in entries.flatten() {
            let src = entry.path();
            if src.is_file() {
                let fname = entry.file_name();
                let _ = link_or_copy_file(&src, &state.bin_dir.join(&fname), 100);
                let _ = link_or_copy_file(&src, &whisper_dir.join(&fname), 100);
            }
        }
    }
}

/// Bootstraps sidecar binaries and ONNX/GGML models from any existing installation on the machine
/// (e.g., `%APPDATA%/com.lokaclip.desktop`) before falling back to network downloads.
pub fn bootstrap_local_sidecars(state: &AppState) {
    ensure_vcruntime_dlls(state);

    let mut candidate_roots: Vec<PathBuf> = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        candidate_roots.push(PathBuf::from(&appdata).join("com.lokaclip.desktop"));
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        candidate_roots.push(PathBuf::from(&localappdata).join("com.lokaclip.desktop"));
    }

    for root in candidate_roots {
        if !root.exists() {
            continue;
        }
        let src_bin = root.join("bin");
        if src_bin.exists() {
            let _ = sync_directory(&src_bin, &state.bin_dir);
        }
        let src_models = root.join("models");
        if src_models.exists() {
            let _ = sync_directory(&src_models, &state.models_dir);
        }
        let src_cookies = root.join("secrets").join("cookies.txt");
        if src_cookies.exists() && !state.cookies_path().exists() {
            let _ = link_or_copy_file(&src_cookies, &state.cookies_path(), 10);
        }
    }
}

pub fn check_binaries(state: &AppState) -> BinariesStatus {
    bootstrap_local_sidecars(state);

    let ffmpeg = stat_component("ffmpeg", &state.ffmpeg_path(), 1_000_000);
    let ffprobe = stat_component("ffprobe", &state.ffprobe_path(), 1_000_000);
    let ytdlp = stat_component("yt-dlp", &state.ytdlp_path(), 1_000_000);
    let deno = stat_component("deno", &state.deno_path(), 1_000_000);
    let whisper_cli = stat_component("whisper-cli", &state.whisper_cli_path(), 100_000);
    let onnxruntime = stat_component("onnxruntime", &state.onnxruntime_dll_path(), 1_000_000);
    let whisper_model = stat_component(
        "ggml-large-v3-turbo-q5_0.bin",
        &state.whisper_model_path(),
        100_000_000,
    );
    let yunet_model = stat_component(
        "face_detection_yunet_2023mar.onnx",
        &state.yunet_model_path(),
        100_000,
    );
    let silero_vad_model = stat_component(
        "silero_vad.onnx",
        &state.silero_vad_model_path(),
        100_000,
    );

    let all_ready = ffmpeg.installed
        && ffprobe.installed
        && ytdlp.installed
        && deno.installed
        && whisper_cli.installed
        && onnxruntime.installed
        && whisper_model.installed
        && yunet_model.installed
        && silero_vad_model.installed;

    let cookies_present = std::fs::metadata(state.cookies_path())
        .map(|m| m.len() > 20)
        .unwrap_or(false);

    BinariesStatus {
        all_ready,
        ffmpeg,
        ffprobe,
        ytdlp,
        deno,
        whisper_cli,
        onnxruntime,
        whisper_model,
        yunet_model,
        silero_vad_model,
        cookies_present,
    }
}

async fn download_file_with_progress(
    app: Option<&AppHandle>,
    component: &str,
    url: &str,
    dest: &Path,
) -> AppResult<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_dest = dest.with_extension("part");
    let client = reqwest::Client::builder()
        .user_agent("OneKlip/0.1.0")
        .build()?;

    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::custom(
            "download_failed",
            format!("Gagal mengunduh {} (HTTP {})", component, resp.status()),
        ));
    }

    let total_bytes = resp.content_length();
    let mut downloaded: u64 = 0;
    let mut file = std::fs::File::create(&tmp_dest)?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        std::io::Write::write_all(&mut file, &chunk)?;
        downloaded += chunk.len() as u64;

        if let Some(app_handle) = app {
            let percent = total_bytes
                .map(|t| (downloaded as f32 / t as f32) * 100.0)
                .unwrap_or(0.0);
            let _ = app_handle.emit(
                "binaries:progress",
                BinaryDownloadProgress {
                    component: component.to_string(),
                    downloaded_bytes: downloaded,
                    total_bytes,
                    percent,
                    status: "downloading".to_string(),
                },
            );
        }
    }

    drop(file);
    if dest.exists() {
        let _ = std::fs::remove_file(dest);
    }
    std::fs::rename(&tmp_dest, dest)?;
    Ok(())
}

pub async fn ensure_all_binaries(
    state: &AppState,
    app: Option<&AppHandle>,
) -> AppResult<BinariesStatus> {
    let status = check_binaries(state);
    if status.all_ready {
        return Ok(status);
    }

    if !status.ytdlp.installed {
        download_file_with_progress(
            app,
            "yt-dlp",
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
            &state.ytdlp_path(),
        )
        .await?;
    }

    if !status.yunet_model.installed {
        download_file_with_progress(
            app,
            "face_detection_yunet_2023mar.onnx",
            "https://huggingface.co/opencv/face_detection_yunet/resolve/main/face_detection_yunet_2023mar.onnx",
            &state.yunet_model_path(),
        )
        .await?;
    }

    if !status.silero_vad_model.installed {
        download_file_with_progress(
            app,
            "silero_vad.onnx",
            "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx",
            &state.silero_vad_model_path(),
        )
        .await?;
    }

    if !status.whisper_model.installed {
        download_file_with_progress(
            app,
            "ggml-large-v3-turbo-q5_0.bin",
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
            &state.whisper_model_path(),
        )
        .await?;
    }

    Ok(check_binaries(state))
}

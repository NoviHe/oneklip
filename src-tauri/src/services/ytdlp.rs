use crate::error::{AppError, AppResult};
use crate::services::innertube::{
    extract_youtube_id, parse_json3_transcript, Transcript, VideoMetadata,
};
use crate::state::AppState;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineProgressPayload {
    pub session_id: String,
    pub stage: String,
    pub message: String,
    pub percent: f32,
}

pub fn emit_pipeline_progress(
    app: Option<&AppHandle>,
    session_id: &str,
    stage: &str,
    message: impl Into<String>,
    percent: f32,
) {
    if let Some(h) = app {
        let _ = h.emit(
            "pipeline:progress",
            PipelineProgressPayload {
                session_id: session_id.to_string(),
                stage: stage.to_string(),
                message: message.into(),
                percent,
            },
        );
    }
}

fn apply_deno_and_ffmpeg_args(cmd: &mut Command, state: &AppState, use_cookies: bool) {
    if state.ffmpeg_path().exists() {
        cmd.arg("--ffmpeg-location").arg(&state.bin_dir);
    }
    if state.deno_path().exists() {
        let deno_arg = format!("deno:{}", state.deno_path().to_string_lossy());
        cmd.arg("--js-runtimes").arg(deno_arg);
    }
    if use_cookies && state.cookies_path().exists() {
        cmd.arg("--cookies").arg(state.cookies_path());
    }
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
}

pub fn probe_local_video(state: &AppState, video_path: &Path) -> AppResult<VideoMetadata> {
    if !video_path.exists() {
        return Err(AppError::custom(
            "file_not_found",
            format!("File video tidak ditemukan: {}", video_path.display()),
        ));
    }

    let file_stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Local Video")
        .to_string();

    let mut duration_seconds = 0.0;
    if state.ffprobe_path().exists() {
        let mut cmd = Command::new(state.ffprobe_path());
        cmd.args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(video_path);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(out) = cmd.output() {
            if let Ok(val) = serde_json::from_slice::<Value>(&out.stdout) {
                duration_seconds = val
                    .pointer("/format/duration")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
            }
        }
    }

    Ok(VideoMetadata {
        video_id: None,
        title: file_stem,
        channel: "Local File".to_string(),
        duration_seconds,
        thumbnail_url: None,
        source_url: None,
        local_path: Some(video_path.to_string_lossy().to_string()),
    })
}

pub fn fetch_metadata_ytdlp(
    state: &AppState,
    url: &str,
    use_cookies: bool,
) -> AppResult<VideoMetadata> {
    let mut cmd = Command::new(state.ytdlp_path());
    apply_deno_and_ffmpeg_args(&mut cmd, state, use_cookies);
    cmd.args([
        "--dump-single-json",
        "--no-playlist",
        "--no-warnings",
        url,
    ]);

    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::custom(
            "ytdlp_metadata_failed",
            format!("yt-dlp metadata gagal: {}", stderr.trim()),
        ));
    }

    let val: Value = serde_json::from_slice(&out.stdout)?;
    let video_id = val
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| extract_youtube_id(url));
    let title = val
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Online Video")
        .to_string();
    let channel = val
        .get("uploader")
        .or_else(|| val.get("channel"))
        .and_then(|v| v.as_str())
        .unwrap_or("Creator")
        .to_string();
    let duration_seconds = val
        .get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let thumbnail_url = val
        .get("thumbnail")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(VideoMetadata {
        video_id,
        title,
        channel,
        duration_seconds,
        thumbnail_url,
        source_url: Some(url.to_string()),
        local_path: None,
    })
}

pub fn fetch_subtitles_ytdlp(
    state: &AppState,
    url: &str,
    preferred_lang: &str,
    use_cookies: bool,
    work_dir: &Path,
) -> AppResult<Transcript> {
    std::fs::create_dir_all(work_dir)?;
    let out_tmpl = work_dir.join("sub_probe");

    // Clean up any old .json3 files in work_dir
    if let Ok(entries) = std::fs::read_dir(work_dir) {
        for e in entries.flatten() {
            if e.path().extension().and_then(|x| x.to_str()) == Some("json3") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    let sub_langs = format!("{},id,en,id.*,en.*", preferred_lang);
    let mut cmd = Command::new(state.ytdlp_path());
    apply_deno_and_ffmpeg_args(&mut cmd, state, use_cookies);
    cmd.args([
        "--skip-download",
        "--write-subs",
        "--write-auto-subs",
        "--sub-format",
        "json3",
        "--sub-langs",
        &sub_langs,
        "--no-playlist",
        "-o",
    ])
    .arg(out_tmpl.to_string_lossy().to_string())
    .arg(url);

    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::custom(
            "ytdlp_subs_failed",
            format!("yt-dlp subtitle gagal: {}", stderr.trim()),
        ));
    }

    // Find downloaded .json3 file
    let mut json3_files: Vec<PathBuf> = std::fs::read_dir(work_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json3"))
        .collect();

    if json3_files.is_empty() {
        return Err(AppError::custom(
            "no_json3_written",
            "yt-dlp tidak menghasilkan file subtitle .json3",
        ));
    }

    // Prefer file matching preferred_lang
    json3_files.sort_by_key(|p| {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.contains(&format!(".{}.", preferred_lang)) {
            0
        } else if name.contains(".id.") {
            1
        } else {
            2
        }
    });

    let chosen = &json3_files[0];
    let raw = std::fs::read_to_string(chosen)?;
    let json_val: Value = serde_json::from_str(&raw)?;
    let tier_name = if use_cookies {
        "tier3_ytdlp_cookies"
    } else {
        "tier1_ytdlp"
    };

    parse_json3_transcript(&json_val, preferred_lang, tier_name, 0.0)
}

pub fn download_video_ytdlp(
    state: &AppState,
    app: Option<&AppHandle>,
    session_id: &str,
    url: &str,
    dest_path: &Path,
    use_cookies: bool,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> AppResult<PathBuf> {
    if dest_path.exists()
        && std::fs::metadata(dest_path)
            .map(|m| m.len() > 500_000)
            .unwrap_or(false)
    {
        return Ok(dest_path.to_path_buf());
    }

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cmd = Command::new(state.ytdlp_path());
    apply_deno_and_ffmpeg_args(&mut cmd, state, use_cookies);
    cmd.args([
        "-f",
        "bestvideo[height<=1080][ext=mp4]+bestaudio[ext=m4a]/best[height<=1080][ext=mp4]/best",
        "--merge-output-format",
        "mp4",
        "--no-playlist",
        "--newline",
        "-o",
    ])
    .arg(dest_path.to_string_lossy().to_string())
    .arg(url)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::custom("stdout_error", "Gagal membaca output yt-dlp"))?;
    let reader = BufReader::new(stdout);
    let pct_re = Regex::new(r"\[download\]\s+(\d+(?:\.\d+)?)%").unwrap();

    for line in reader.lines().flatten() {
        if let Some(flag) = cancel_flag {
            if flag.load(Ordering::SeqCst) {
                let _ = child.kill();
                return Err(AppError::custom("cancelled", "Proses dibatalkan oleh pengguna"));
            }
        }
        if let Some(caps) = pct_re.captures(&line) {
            if let Some(pct_str) = caps.get(1) {
                if let Ok(pct) = pct_str.as_str().parse::<f32>() {
                    emit_pipeline_progress(
                        app,
                        session_id,
                        "downloading_video",
                        format!("Mengunduh video sumber ({:.1}%)...", pct),
                        pct,
                    );
                }
            }
        }
    }

    let status = child.wait()?;
    if !status.success() || !dest_path.exists() {
        if !use_cookies && state.cookies_path().exists() {
            // Automatic retry with cookies.txt
            return download_video_ytdlp(
                state,
                app,
                session_id,
                url,
                dest_path,
                true,
                cancel_flag,
            );
        }
        return Err(AppError::custom(
            "download_video_failed",
            "Gagal mengunduh video dari URL tersebut",
        ));
    }

    Ok(dest_path.to_path_buf())
}

pub fn extract_raw_clip(
    state: &AppState,
    source_video: &Path,
    start_sec: f64,
    end_sec: f64,
    out_raw_mp4: &Path,
) -> AppResult<()> {
    if let Some(parent) = out_raw_mp4.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let duration = (end_sec - start_sec).max(1.0);

    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-ss",
        &format!("{:.3}", start_sec),
        "-i",
    ])
    .arg(source_video)
    .args([
        "-t",
        &format!("{:.3}", duration),
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "18",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
    ])
    .arg(out_raw_mp4);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::custom(
            "extract_raw_clip_failed",
            format!("FFmpeg gagal memotong klip mentah: {}", stderr.trim()),
        ));
    }
    Ok(())
}

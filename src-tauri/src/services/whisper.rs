use crate::error::{AppError, AppResult};
use crate::services::innertube::{
    fetch_transcript_innertube, Transcript, TranscriptSegment, TranscriptWord,
};
use crate::services::ytdlp::{emit_pipeline_progress, fetch_subtitles_ytdlp};
use crate::state::AppState;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::AppHandle;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn extract_16k_wav(state: &AppState, input_media: &Path, out_wav: &Path) -> AppResult<()> {
    if let Some(parent) = out_wav.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(input_media)
        .args(["-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(out_wav);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::custom(
            "wav_extract_failed",
            format!("Gagal mengekstrak audio 16kHz WAV: {}", stderr.trim()),
        ));
    }
    Ok(())
}

pub fn transcribe_with_whisper_cli(
    state: &AppState,
    audio_wav: &Path,
    language: &str,
    work_dir: &Path,
) -> AppResult<Transcript> {
    if !state.whisper_cli_path().exists() {
        return Err(AppError::custom(
            "whisper_missing",
            "Binary whisper-cli.exe belum terinstal",
        ));
    }
    if !state.whisper_model_path().exists() {
        return Err(AppError::custom(
            "whisper_model_missing",
            "Model ggml-large-v3-turbo-q5_0.bin belum terinstal",
        ));
    }

    std::fs::create_dir_all(work_dir)?;
    let out_prefix = work_dir.join("whisper_out");
    let out_json = work_dir.join("whisper_out.json");
    if out_json.exists() {
        let _ = std::fs::remove_file(&out_json);
    }

    let lang_arg = if language.trim().is_empty() {
        "auto"
    } else {
        language.trim()
    };

    let threads = std::thread::available_parallelism()
        .map(|n| n.get().clamp(2, 8))
        .unwrap_or(4);

    let mut cmd = Command::new(state.whisper_cli_path());
    cmd.current_dir(
        state
            .whisper_cli_path()
            .parent()
            .unwrap_or(&state.bin_dir),
    );
    cmd.args(["-m"])
        .arg(state.whisper_model_path())
        .args(["-f"])
        .arg(audio_wav)
        .args([
            "-l",
            lang_arg,
            "-t",
            &threads.to_string(),
            "-ojf",
            "-of",
        ])
        .arg(&out_prefix);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output()?;
    if !out.status.success() || !out_json.exists() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(AppError::custom(
            "whisper_exec_failed",
            format!("Whisper lokal gagal mentranskripsi audio: {}", stderr.trim()),
        ));
    }

    let raw = std::fs::read_to_string(&out_json)?;
    let val: Value = serde_json::from_str(&raw)?;
    parse_whisper_json(&val, language)
}

pub fn parse_whisper_json(val: &Value, fallback_lang: &str) -> AppResult<Transcript> {
    let detected_lang = val
        .pointer("/result/language")
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_lang)
        .to_string();

    let items = val
        .get("transcription")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            AppError::custom(
                "invalid_whisper_json",
                "Output JSON Whisper tidak memiliki field transcription",
            )
        })?;

    let mut segments: Vec<TranscriptSegment> = Vec::new();
    let mut max_end = 0.0_f64;

    for item in items {
        let seg_start_ms = item
            .pointer("/offsets/from")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let seg_end_ms = item
            .pointer("/offsets/to")
            .and_then(|v| v.as_f64())
            .unwrap_or(seg_start_ms + 2000.0);
        let seg_start = seg_start_ms / 1000.0;
        let seg_end = (seg_end_ms / 1000.0).max(seg_start + 0.15);

        let raw_text = item
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if raw_text.is_empty()
            || raw_text.starts_with('[') && raw_text.ends_with(']')
            || raw_text.starts_with('(') && raw_text.ends_with(')')
        {
            continue;
        }

        let mut words: Vec<TranscriptWord> = Vec::new();
        if let Some(tokens) = item.get("tokens").and_then(|v| v.as_array()) {
            for tok in tokens {
                let t_text = tok.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if t_text.is_empty()
                    || (t_text.starts_with('[') && t_text.ends_with(']'))
                    || (t_text.starts_with("<|") && t_text.ends_with("|>"))
                {
                    continue;
                }

                let t_from = tok
                    .pointer("/offsets/from")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(seg_start_ms)
                    / 1000.0;
                let t_to = (tok
                    .pointer("/offsets/to")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(seg_end_ms)
                    / 1000.0)
                    .max(t_from + 0.06);
                let prob = tok.get("p").and_then(|v| v.as_f64());

                let starts_with_space = t_text.starts_with(' ');
                let clean_piece = t_text.trim();
                if clean_piece.is_empty() {
                    continue;
                }

                // Punctuation-only or subword continuation merges into previous word
                let is_punct = clean_piece
                    .chars()
                    .all(|c| c.is_ascii_punctuation());

                if (!starts_with_space || is_punct) && !words.is_empty() {
                    if let Some(last) = words.last_mut() {
                        last.word.push_str(clean_piece);
                        last.end = t_to.max(last.end);
                        continue;
                    }
                }

                words.push(TranscriptWord {
                    word: clean_piece.to_string(),
                    start: (t_from * 1000.0).round() / 1000.0,
                    end: (t_to * 1000.0).round() / 1000.0,
                    probability: prob,
                });
            }
        }

        // Fallback if tokens array was not populated
        if words.is_empty() {
            let split_words: Vec<&str> = raw_text.split_whitespace().collect();
            if !split_words.is_empty() {
                let step = ((seg_end - seg_start) / split_words.len() as f64).max(0.08);
                for (i, w) in split_words.iter().enumerate() {
                    let ws = seg_start + i as f64 * step;
                    let we = (ws + step).min(seg_end.max(ws + 0.08));
                    words.push(TranscriptWord {
                        word: (*w).to_string(),
                        start: (ws * 1000.0).round() / 1000.0,
                        end: (we * 1000.0).round() / 1000.0,
                        probability: None,
                    });
                }
            }
        }

        if words.is_empty() {
            continue;
        }

        let actual_start = words.first().map(|w| w.start).unwrap_or(seg_start);
        let actual_end = words
            .last()
            .map(|w| w.end)
            .unwrap_or(seg_end)
            .max(actual_start + 0.15);
        if actual_end > max_end {
            max_end = actual_end;
        }

        segments.push(TranscriptSegment {
            id: segments.len(),
            start: actual_start,
            end: actual_end,
            text: words
                .iter()
                .map(|w| w.word.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            words,
        });
    }

    if segments.is_empty() {
        return Err(AppError::custom(
            "whisper_empty_segments",
            "Hasil transkripsi Whisper tidak menghasilkan segmen percakapan",
        ));
    }

    Ok(Transcript {
        language: detected_lang,
        source_tier: "tier4_whisper_local".to_string(),
        duration_seconds: max_end,
        segments,
    })
}

/// Executes the 4-Tier Transcript Ladder:
/// - Tier 0: YouTube Innertube API (`json3` word-level)
/// - Tier 1: `yt-dlp` subtitle extraction (without cookies)
/// - Tier 3: `yt-dlp` subtitle extraction (with `cookies.txt` if present)
/// - Tier 4: Local `whisper-cli.exe` (`ggml-large-v3-turbo-q5_0.bin`) on 16kHz WAV
pub async fn resolve_transcript_4tier(
    state: &AppState,
    app: Option<&AppHandle>,
    session_id: &str,
    video_url: Option<&str>,
    local_media_path: Option<&Path>,
    language: &str,
    session_dir: &Path,
) -> AppResult<Transcript> {
    std::fs::create_dir_all(session_dir)?;
    let captions_json_path = session_dir.join("captions.json");

    if let Some(url) = video_url {
        // Tier 0: Innertube
        emit_pipeline_progress(
            app,
            session_id,
            "transcript_tier0",
            "Mencoba Tier 0: Mengambil transkrip cepat via YouTube Innertube...",
            10.0,
        );
        if let Ok(t) = fetch_transcript_innertube(url, language).await {
            let _ = std::fs::write(&captions_json_path, serde_json::to_string_pretty(&t)?);
            return Ok(t);
        }

        // Tier 1: yt-dlp without cookies
        emit_pipeline_progress(
            app,
            session_id,
            "transcript_tier1",
            "Mencoba Tier 1: Mengekstrak subtitle word-level via yt-dlp...",
            18.0,
        );
        if let Ok(t) = fetch_subtitles_ytdlp(state, url, language, false, session_dir) {
            let _ = std::fs::write(&captions_json_path, serde_json::to_string_pretty(&t)?);
            return Ok(t);
        }

        // Tier 3: yt-dlp with cookies.txt
        if state.cookies_path().exists() {
            emit_pipeline_progress(
                app,
                session_id,
                "transcript_tier3",
                "Mencoba Tier 3: Mengekstrak subtitle dengan autentikasi cookies.txt...",
                25.0,
            );
            if let Ok(t) = fetch_subtitles_ytdlp(state, url, language, true, session_dir) {
                let _ = std::fs::write(&captions_json_path, serde_json::to_string_pretty(&t)?);
                return Ok(t);
            }
        }
    }

    // Tier 4: Local Whisper.cpp
    emit_pipeline_progress(
        app,
        session_id,
        "transcript_tier4",
        "Menjalankan Tier 4: Transkripsi AI lokal dengan Whisper Large-v3-Turbo...",
        32.0,
    );

    let media_path: PathBuf = match local_media_path {
        Some(p) if p.exists() => p.to_path_buf(),
        _ => {
            return Err(AppError::custom(
                "tier4_requires_local_media",
                "Transkripsi Whisper lokal membutuhkan file video/audio yang sudah terunduh",
            ))
        }
    };

    let wav_path = session_dir.join("audio_16k.wav");
    extract_16k_wav(state, &media_path, &wav_path)?;
    let transcript = transcribe_with_whisper_cli(state, &wav_path, language, session_dir)?;
    let _ = std::fs::remove_file(&wav_path);

    std::fs::write(
        &captions_json_path,
        serde_json::to_string_pretty(&transcript)?,
    )?;

    Ok(transcript)
}

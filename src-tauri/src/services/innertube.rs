use crate::error::{AppError, AppResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probability: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: usize,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<TranscriptWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub language: String,
    pub source_tier: String,
    pub duration_seconds: f64,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
    pub video_id: Option<String>,
    pub title: String,
    pub channel: String,
    pub duration_seconds: f64,
    pub thumbnail_url: Option<String>,
    pub source_url: Option<String>,
    pub local_path: Option<String>,
}

pub fn extract_youtube_id(url_or_id: &str) -> Option<String> {
    let trimmed = url_or_id.trim();
    if trimmed.len() == 11
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some(trimmed.to_string());
    }
    let re = Regex::new(
        r"(?:youtube\.com/(?:watch\?.*v=|shorts/|live/|embed/)|youtu\.be/)([A-Za-z0-9_-]{11})",
    )
    .ok()?;
    re.captures(trimmed)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

async fn fetch_innertube_player(video_id: &str) -> AppResult<Value> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let clients = [
        (
            "ANDROID_VR",
            "1.60.19",
            "com.google.android.apps.youtube.vr.oculus/1.60.19 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
        ),
        (
            "ANDROID",
            "19.29.37",
            "com.google.android.youtube/19.29.37 (Linux; U; Android 14) gzip",
        ),
    ];

    let mut last_err = String::from("Innertube player request failed");

    for (client_name, client_version, ua) in clients {
        let body = json!({
            "videoId": video_id,
            "context": {
                "client": {
                    "clientName": client_name,
                    "clientVersion": client_version,
                    "androidSdkVersion": 32,
                    "hl": "id",
                    "gl": "ID"
                }
            },
            "racyCheckOk": true,
            "contentCheckOk": true
        });

        let res = client
            .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header("User-Agent", ua)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        if let Ok(resp) = res {
            if resp.status().is_success() {
                if let Ok(json_val) = resp.json::<Value>().await {
                    if json_val.get("videoDetails").is_some() {
                        return Ok(json_val);
                    }
                    last_err = format!(
                        "Innertube {} response missing videoDetails",
                        client_name
                    );
                }
            }
        }
    }

    Err(AppError::custom("innertube_failed", last_err))
}

pub async fn fetch_metadata_innertube(url: &str) -> AppResult<VideoMetadata> {
    let video_id = extract_youtube_id(url).ok_or_else(|| {
        AppError::custom("invalid_youtube_url", "URL YouTube tidak valid")
    })?;

    let player = fetch_innertube_player(&video_id).await?;
    let details = player
        .get("videoDetails")
        .ok_or_else(|| AppError::custom("missing_details", "videoDetails tidak ditemukan"))?;

    let title = details
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("YouTube Video")
        .to_string();
    let channel = details
        .get("author")
        .and_then(|v| v.as_str())
        .unwrap_or("YouTube Channel")
        .to_string();
    let duration_seconds = details
        .get("lengthSeconds")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let thumbnail_url = Some(format!(
        "https://i.ytimg.com/vi/{}/maxresdefault.jpg",
        video_id
    ));

    Ok(VideoMetadata {
        video_id: Some(video_id),
        title,
        channel,
        duration_seconds,
        thumbnail_url,
        source_url: Some(url.to_string()),
        local_path: None,
    })
}

pub async fn fetch_transcript_innertube(
    url: &str,
    preferred_lang: &str,
) -> AppResult<Transcript> {
    let video_id = extract_youtube_id(url).ok_or_else(|| {
        AppError::custom("invalid_youtube_url", "URL YouTube tidak valid")
    })?;

    let player = fetch_innertube_player(&video_id).await?;
    let duration_seconds = player
        .pointer("/videoDetails/lengthSeconds")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let tracks = player
        .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            AppError::custom(
                "no_caption_tracks",
                "Video ini tidak memiliki caption track di Innertube",
            )
        })?;

    if tracks.is_empty() {
        return Err(AppError::custom(
            "empty_caption_tracks",
            "Daftar caption track kosong",
        ));
    }

    // Choose best track: exact preferred_lang (manual first, then asr), then "id", then "en", then first available
    let pick_track = || -> Option<&Value> {
        for lang in [preferred_lang, "id", "en"] {
            // Try manual track first
            if let Some(t) = tracks.iter().find(|t| {
                t.get("languageCode").and_then(|l| l.as_str()) == Some(lang)
                    && t.get("kind").and_then(|k| k.as_str()) != Some("asr")
            }) {
                return Some(t);
            }
            // Then auto-generated ASR
            if let Some(t) = tracks.iter().find(|t| {
                t.get("languageCode")
                    .and_then(|l| l.as_str())
                    .map(|l| l.starts_with(lang))
                    .unwrap_or(false)
            }) {
                return Some(t);
            }
        }
        tracks.first()
    };

    let selected = pick_track().ok_or_else(|| {
        AppError::custom("no_track_selected", "Tidak ada track subtitle yang cocok")
    })?;

    let track_lang = selected
        .get("languageCode")
        .and_then(|v| v.as_str())
        .unwrap_or(preferred_lang)
        .to_string();

    let base_url = selected
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::custom("missing_base_url", "baseUrl subtitle tidak ditemukan"))?;

    let json3_url = if base_url.contains("fmt=") {
        let re = Regex::new(r"fmt=[^&]+").unwrap();
        re.replace(base_url, "fmt=json3").to_string()
    } else {
        format!("{}&fmt=json3", base_url)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let json3_val: Value = client.get(&json3_url).send().await?.json().await?;

    parse_json3_transcript(&json3_val, &track_lang, "tier0_innertube", duration_seconds)
}

pub fn parse_json3_transcript(
    json3: &Value,
    language: &str,
    source_tier: &str,
    fallback_duration: f64,
) -> AppResult<Transcript> {
    let events = json3
        .get("events")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::custom("invalid_json3", "Format json3 tidak memiliki array events"))?;

    let mut segments: Vec<TranscriptSegment> = Vec::new();
    let mut max_end = fallback_duration;

    for ev in events {
        let segs = match ev.get("segs").and_then(|s| s.as_array()) {
            Some(s) if !s.is_empty() => s,
            _ => continue,
        };

        let event_start_ms = ev.get("tStartMs").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let event_dur_ms = ev
            .get("dDurationMs")
            .and_then(|v| v.as_f64())
            .unwrap_or(3000.0);
        let event_start = event_start_ms / 1000.0;
        let event_end = (event_start_ms + event_dur_ms) / 1000.0;

        let mut words: Vec<TranscriptWord> = Vec::new();
        let mut full_text = String::new();

        for (i, seg) in segs.iter().enumerate() {
            let raw_utf8 = seg.get("utf8").and_then(|v| v.as_str()).unwrap_or("");
            let cleaned = raw_utf8.replace('\n', " ");
            full_text.push_str(&cleaned);

            let trimmed_word = cleaned.trim();
            if trimmed_word.is_empty() {
                continue;
            }

            let offset_ms = seg.get("tOffsetMs").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let w_start = (event_start_ms + offset_ms) / 1000.0;

            let next_offset_ms = segs
                .get(i + 1)
                .and_then(|next_seg| next_seg.get("tOffsetMs").and_then(|v| v.as_f64()));

            let w_end = match next_offset_ms {
                Some(no) if no > offset_ms => (event_start_ms + no) / 1000.0,
                _ => (w_start + 0.35).min(event_end.max(w_start + 0.12)),
            };

            // Split multi-word tokens if needed
            let split_tokens: Vec<&str> = trimmed_word.split_whitespace().collect();
            if split_tokens.len() == 1 {
                words.push(TranscriptWord {
                    word: split_tokens[0].to_string(),
                    start: (w_start * 1000.0).round() / 1000.0,
                    end: (w_end * 1000.0).round() / 1000.0,
                    probability: None,
                });
            } else if split_tokens.len() > 1 {
                let step = ((w_end - w_start) / split_tokens.len() as f64).max(0.08);
                for (k, tok) in split_tokens.iter().enumerate() {
                    let ts = w_start + k as f64 * step;
                    let te = (ts + step).min(event_end.max(ts + 0.08));
                    words.push(TranscriptWord {
                        word: (*tok).to_string(),
                        start: (ts * 1000.0).round() / 1000.0,
                        end: (te * 1000.0).round() / 1000.0,
                        probability: None,
                    });
                }
            }
        }

        let clean_text = full_text.split_whitespace().collect::<Vec<_>>().join(" ");
        if clean_text.is_empty() || clean_text == "[Music]" || clean_text == "[Musik]" {
            continue;
        }

        let seg_start = words.first().map(|w| w.start).unwrap_or(event_start);
        let seg_end = words
            .last()
            .map(|w| w.end)
            .unwrap_or(event_end)
            .max(seg_start + 0.2);

        if seg_end > max_end {
            max_end = seg_end;
        }

        segments.push(TranscriptSegment {
            id: segments.len(),
            start: (seg_start * 1000.0).round() / 1000.0,
            end: (seg_end * 1000.0).round() / 1000.0,
            text: clean_text,
            words,
        });
    }

    if segments.is_empty() {
        return Err(AppError::custom(
            "empty_parsed_transcript",
            "Transkrip json3 kosong setelah diparsing",
        ));
    }

    Ok(Transcript {
        language: language.to_string(),
        source_tier: source_tier.to_string(),
        duration_seconds: max_end,
        segments,
    })
}

use crate::error::{AppError, AppResult};
use crate::services::innertube::{Transcript, TranscriptWord};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub provider: String, // "gemini" | "groq" | "openai" | "openrouter" | "ollama" | "heuristic"
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
    pub target_clip_min_secs: f64,
    pub target_clip_max_secs: f64,
    pub max_clips: usize,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "gemini".to_string(),
            api_key: String::new(),
            model: "gemini-2.5-flash".to_string(),
            base_url: None,
            target_clip_min_secs: 30.0,
            target_clip_max_secs: 75.0,
            max_clips: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViralHighlight {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub duration_seconds: f64,
    pub title: String,
    pub hook_text: String,
    pub hook_kicker: String,
    pub hook_tag: String,
    pub description: String,
    pub category: String,
    pub virality_score: i64,
}

struct TranscriptChunk {
    start_sec: f64,
    end_sec: f64,
    formatted_text: String,
}

fn build_chunks(transcript: &Transcript, audio_spikes: &[f64]) -> Vec<TranscriptChunk> {
    let window_secs = 1200.0; // 20 minutes per window
    let overlap_secs = 120.0; // 2 minutes overlap
    let total_dur = transcript.duration_seconds.max(60.0);

    let mut chunks = Vec::new();
    let mut cursor = 0.0;

    while cursor < total_dur {
        let win_end = (cursor + window_secs).min(total_dur + 10.0);
        let mut lines = Vec::new();

        for seg in &transcript.segments {
            if seg.end >= cursor && seg.start <= win_end {
                let has_spike = audio_spikes
                    .iter()
                    .any(|&sp| sp >= (seg.start - 1.0) && sp <= (seg.end + 1.0));
                let spike_tag = if has_spike { " [PEAK_AUDIO]" } else { "" };
                lines.push(format!(
                    "[{:.2}s - {:.2}s]{} {}",
                    seg.start, seg.end, spike_tag, seg.text
                ));
            }
        }

        if !lines.is_empty() {
            chunks.push(TranscriptChunk {
                start_sec: cursor,
                end_sec: win_end,
                formatted_text: lines.join("\n"),
            });
        }

        if win_end >= total_dur {
            break;
        }
        cursor += window_secs - overlap_secs;
    }

    chunks
}

fn build_system_prompt(content_type: &str, min_secs: f64, max_secs: f64, max_clips: usize) -> String {
    let domain_focus = match content_type {
        "gaming" => "Fokus pada momen clutch, reaksi kaget/teriakan ([PEAK_AUDIO]), kejadian kocak, fail epik, dan banter antar pemain.",
        "education" => "Fokus pada penjelasan konsep paling daging, analogi brilian, tips praktis yang langsung bisa dipakai, dan koreksi miskonsepsi umum.",
        "podcast" => "Fokus pada opini kontroversial/berani, rahasia yang baru dibongkar, perdebatan seru antar pembicara, cerita emosional, dan punchline komedi.",
        _ => "Fokus pada momen paling menarik perhatian, pernyataan pembuka yang memicu rasa penasaran (curiosity gap), dan kesimpulan yang kuat.",
    };

    format!(
        r#"Kamu adalah Produser Eksekutif Konten Short-Form Viral kelas dunia (TikTok, YouTube Shorts, Instagram Reels).
Tugasmu adalah menganalisis transkrip ber-timestamp di bawah ini dan memilih maksimal {max_clips} segmen video paling berpotensi viral.

ATURAN WAJIB PEMILIHAN KLIP:
1. Durasi setiap klip WAJIB berada di rentang {min_secs:.0} hingga {max_secs:.0} detik (ideal 35-65 detik).
2. Struktur Tiga Babak:
   - Detik 0-3 (HOOK): Harus diawali kalimat pemicu penasaran, pertanyaan tajam, pernyataan mengejutkan, atau lonjakan emosi ([PEAK_AUDIO]). JANGAN memulai dari basa-basi atau kata sambung menggantung.
   - Bagian Tengah (ISI): Alur cerita atau argumen yang utuh dan padat.
   - Bagian Akhir (PAYOFF/PUNCHLINE): Berakhir tepat setelah kalimat klimaks, tawa, atau kesimpulan tegas. JANGAN memotong di tengah kalimat.
3. Konteks Konten ({content_type}): {domain_focus}
4. Untuk setiap klip, buat:
   - `start_seconds`: angka desimal detik mulai (sesuai timestamp transkrip).
   - `end_seconds`: angka desimal detik selesai (sesuai timestamp transkrip).
   - `title`: judul klip yang sangat menarik & memancing klik (maks 65 karakter).
   - `hook_text`: teks banner headline singkat & nendang untuk ditampilkan di 3 detik pertama video (maks 8 kata, huruf kapital pada kata kunci).
   - `hook_kicker`: sub-badge pendek di atas hook (2-3 kata, contoh: "FAKTA MENGEJUTKAN", "DEBAT PANAS", "MOMEN KOCAK").
   - `hook_tag`: tag kategori singkat (contoh: "PODCAST HIGHLIGHT", "VIRAL CLIP", "INSIGHT").
   - `description`: alasan singkat kenapa momen ini berpotensi viral + caption siap posting.
   - `category`: pilih salah satu dari ["insightful", "funny", "controversial", "emotional", "hype", "story"].
   - `virality_score`: skor integer 70 sampai 99 berdasarkan kekuatan hook dan payoff.

KELUARKAN HANYA JSON VALID DENGAN FORMAT PERSIS BERIKUT TANPA TEKS LAIN:
{{
  "clips": [
    {{
      "start_seconds": 120.5,
      "end_seconds": 172.8,
      "title": "Kenapa 90% Bisnis Baru Gagal di Tahun Pertama",
      "hook_text": "RAHASIA BISNIS YANG JARANG DIBahas!",
      "hook_kicker": "FAKTA PAHIT",
      "hook_tag": "INSIGHT",
      "description": "Pembicara membongkar kesalahan fatal founder pemula dengan analogi yang sangat jelas.",
      "category": "insightful",
      "virality_score": 94
    }}
  ]
}}"#
    )
}

async fn call_llm_chunk(
    config: &AiConfig,
    system_prompt: &str,
    user_transcript: &str,
) -> AppResult<Vec<ViralHighlight>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()?;

    let provider = config.provider.to_lowercase();
    let raw_json_text = if provider == "gemini" {
        let model = if config.model.trim().is_empty() {
            "gemini-2.5-flash"
        } else {
            config.model.trim()
        };
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model,
            config.api_key.trim()
        );
        let body = json!({
            "systemInstruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": format!("TRANSKRIP VIDEO:\n\n{}", user_transcript) }]
            }],
            "generationConfig": {
                "temperature": 0.35,
                "responseMimeType": "application/json"
            }
        });

        let resp = client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(AppError::custom(
                "gemini_api_error",
                format!("Gemini API error: {}", err_txt),
            ));
        }
        let val: Value = resp.json().await?;
        val.pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::custom("gemini_empty", "Respons Gemini kosong"))?
            .to_string()
    } else {
        // OpenAI-compatible endpoint (OpenAI, Groq, OpenRouter, Ollama)
        let endpoint = match provider.as_str() {
            "groq" => "https://api.groq.com/openai/v1/chat/completions".to_string(),
            "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
            "ollama" => config
                .base_url
                .clone()
                .unwrap_or_else(|| "http://127.0.0.1:11434/v1/chat/completions".to_string()),
            _ => config
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string()),
        };

        let model = if config.model.trim().is_empty() {
            match provider.as_str() {
                "groq" => "llama-3.3-70b-versatile",
                "openrouter" => "google/gemini-2.0-flash-001",
                "ollama" => "llama3.1:8b",
                _ => "gpt-4o-mini",
            }
        } else {
            config.model.trim()
        };

        let body = json!({
            "model": model,
            "temperature": 0.35,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": format!("TRANSKRIP VIDEO:\n\n{}", user_transcript) }
            ]
        });

        let mut req = client.post(&endpoint).json(&body);
        if !config.api_key.trim().is_empty() {
            req = req.bearer_auth(config.api_key.trim());
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(AppError::custom(
                "llm_api_error",
                format!("LLM API ({}) error: {}", provider, err_txt),
            ));
        }
        let val: Value = resp.json().await?;
        val.pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::custom("llm_empty", "Respons LLM kosong"))?
            .to_string()
    };

    parse_llm_highlights_json(&raw_json_text)
}

fn parse_llm_highlights_json(raw: &str) -> AppResult<Vec<ViralHighlight>> {
    // Strip markdown code fences if present
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let val: Value = serde_json::from_str(cleaned).or_else(|_| {
        // Fallback: extract substring from first '{' to last '}'
        if let (Some(s), Some(e)) = (cleaned.find('{'), cleaned.rfind('}')) {
            serde_json::from_str(&cleaned[s..=e])
        } else {
            Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid JSON",
            )))
        }
    })?;

    let arr = val
        .get("clips")
        .or_else(|| val.get("highlights"))
        .and_then(|v| v.as_array())
        .or_else(|| val.as_array())
        .ok_or_else(|| AppError::custom("invalid_clips_json", "JSON tidak memiliki array clips"))?;

    let mut result = Vec::new();
    for item in arr {
        let start = item
            .get("start_seconds")
            .or_else(|| item.get("startSeconds"))
            .or_else(|| item.get("start"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let end = item
            .get("end_seconds")
            .or_else(|| item.get("endSeconds"))
            .or_else(|| item.get("end"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if end <= start + 10.0 {
            continue;
        }

        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Momen Viral Pilihan")
            .to_string();
        let hook_text = item
            .get("hook_text")
            .or_else(|| item.get("hookText"))
            .and_then(|v| v.as_str())
            .unwrap_or(&title)
            .to_string();
        let hook_kicker = item
            .get("hook_kicker")
            .or_else(|| item.get("hookKicker"))
            .and_then(|v| v.as_str())
            .unwrap_or("HIGHLIGHT UTAMA")
            .to_string();
        let hook_tag = item
            .get("hook_tag")
            .or_else(|| item.get("hookTag"))
            .and_then(|v| v.as_str())
            .unwrap_or("VIRAL CLIP")
            .to_string();
        let description = item
            .get("description")
            .or_else(|| item.get("reason"))
            .and_then(|v| v.as_str())
            .unwrap_or("Segmen dengan potensi retensi tinggi.")
            .to_string();
        let category = item
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("insightful")
            .to_string();
        let virality_score = item
            .get("virality_score")
            .or_else(|| item.get("viralityScore"))
            .and_then(|v| v.as_i64())
            .unwrap_or(85)
            .clamp(50, 99);

        result.push(ViralHighlight {
            start_seconds: start,
            end_seconds: end,
            duration_seconds: end - start,
            title,
            hook_text,
            hook_kicker,
            hook_tag,
            description,
            category,
            virality_score,
        });
    }

    Ok(result)
}

/// Built-in Offline Smart Heuristic Viral Detector (works without any API key!).
/// Scores candidate windows using discourse hook words, question marks, speech density, and audio RMS spikes.
pub fn detect_highlights_heuristic(
    transcript: &Transcript,
    audio_spikes: &[f64],
    config: &AiConfig,
) -> Vec<ViralHighlight> {
    let min_dur = config.target_clip_min_secs.max(20.0);
    let max_dur = config.target_clip_max_secs.max(min_dur + 15.0);
    let ideal_dur = ((min_dur + max_dur) * 0.5).clamp(35.0, 60.0);

    let hook_keywords = [
        "rahasia", "ternyata", "masalahnya", "kenapa", "bayangin", "jujur", "fakta", "kesalahan",
        "bahaya", "gila", "parah", "kunci", "penting", "jangan", "harus", "pertama", "intinya",
        "secret", "actually", "problem", "why", "imagine", "honestly", "mistake", "crazy", "never",
    ];

    let mut candidates: Vec<ViralHighlight> = Vec::new();
    let segs = &transcript.segments;
    if segs.is_empty() {
        return candidates;
    }

    let mut i = 0;
    while i < segs.len() {
        let start_seg = &segs[i];
        let start_t = start_seg.start;

        // Find ending segment near ideal_dur that ends with sentence punctuation or pause
        let mut best_j = i;
        for (j, seg) in segs.iter().enumerate().skip(i) {
            let dur = seg.end - start_t;
            if dur >= min_dur && dur <= max_dur {
                best_j = j;
                if dur >= ideal_dur
                    && (seg.text.ends_with('.')
                        || seg.text.ends_with('!')
                        || seg.text.ends_with('?'))
                {
                    break;
                }
            } else if dur > max_dur {
                break;
            }
        }

        let end_seg = &segs[best_j];
        let end_t = (end_seg.end).max(start_t + min_dur.min(transcript.duration_seconds));
        let dur = end_t - start_t;

        if dur >= 15.0 {
            let window_text: String = segs[i..=best_j]
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let lower = window_text.to_lowercase();

            // Score calculation
            let mut score: i64 = 72;

            // 1. Audio RMS spikes inside window
            let spike_count = audio_spikes
                .iter()
                .filter(|&&sp| sp >= start_t && sp <= end_t)
                .count();
            score += (spike_count as i64 * 4).min(12);

            // 2. Hook keywords in first 2 segments
            let opening_lower = segs[i..(i + 2).min(segs.len())]
                .iter()
                .map(|s| s.text.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            for kw in hook_keywords {
                if opening_lower.contains(kw) {
                    score += 4;
                }
            }
            if opening_lower.contains('?') || opening_lower.contains('!') {
                score += 5;
            }

            // 3. Speech pace / word density
            let word_count = window_text.split_whitespace().count();
            let wps = word_count as f64 / dur.max(1.0);
            if (2.2..=4.2).contains(&wps) {
                score += 5;
            }

            let final_score = score.clamp(70, 97);

            // Build clean title & hook from opening segment
            let raw_opening = start_seg.text.trim();
            let title_words: Vec<&str> = raw_opening.split_whitespace().take(9).collect();
            let title = if title_words.is_empty() {
                format!("Momen Highlight #{:.0}s", start_t)
            } else {
                title_words.join(" ")
            };

            let hook_words: Vec<String> = raw_opening
                .split_whitespace()
                .take(6)
                .map(|w| w.to_uppercase())
                .collect();
            let hook_text = if hook_words.is_empty() {
                "MOMEN VIRAL PILIHAN!".to_string()
            } else {
                hook_words.join(" ")
            };

            let (category, kicker) = if spike_count >= 2 {
                ("hype", "MOMEN INTENS")
            } else if lower.contains('?') {
                ("insightful", "FAKTA PENTING")
            } else {
                ("story", "SOROTAN UTAMA")
            };

            candidates.push(ViralHighlight {
                start_seconds: start_t,
                end_seconds: end_t,
                duration_seconds: dur,
                title,
                hook_text,
                hook_kicker: kicker.to_string(),
                hook_tag: "AUTO HIGHLIGHT".to_string(),
                description: format!(
                    "Terdeteksi lonjakan retensi & struktur kalimat kuat ({} kata, {} lonjakan audio).",
                    word_count, spike_count
                ),
                category: category.to_string(),
                virality_score: final_score,
            });
        }

        // Advance cursor by ~25 seconds
        let next_target = start_t + 25.0;
        while i < segs.len() && segs[i].start < next_target {
            i += 1;
        }
    }

    candidates
}

/// Clamps clip `start_seconds` and `end_seconds` to exact word boundaries in `Transcript`
/// with natural breath lead-in (-0.12s) and tail decay (+0.25s).
pub fn clamp_to_word_boundaries(
    mut clip: ViralHighlight,
    all_words: &[TranscriptWord],
    total_duration: f64,
) -> ViralHighlight {
    if all_words.is_empty() {
        return clip;
    }

    // Find word nearest to start_seconds
    if let Some(first_w) = all_words
        .iter()
        .find(|w| w.end >= clip.start_seconds)
    {
        clip.start_seconds = (first_w.start - 0.12).max(0.0);
    }

    // Find word nearest to end_seconds
    if let Some(last_w) = all_words
        .iter()
        .rev()
        .find(|w| w.start <= clip.end_seconds)
    {
        let max_limit = if total_duration > 1.0 {
            total_duration
        } else {
            last_w.end + 1.0
        };
        clip.end_seconds = (last_w.end + 0.25).min(max_limit);
    }

    if clip.end_seconds <= clip.start_seconds + 5.0 {
        clip.end_seconds = clip.start_seconds + 30.0;
    }

    clip.start_seconds = (clip.start_seconds * 100.0).round() / 100.0;
    clip.end_seconds = (clip.end_seconds * 100.0).round() / 100.0;
    clip.duration_seconds =
        ((clip.end_seconds - clip.start_seconds) * 100.0).round() / 100.0;

    clip
}

fn deduplicate_and_rank(
    clips: Vec<ViralHighlight>,
    max_clips: usize,
) -> Vec<ViralHighlight> {
    let mut sorted = clips;
    sorted.sort_by(|a, b| b.virality_score.cmp(&a.virality_score));

    let mut accepted: Vec<ViralHighlight> = Vec::new();
    for cand in sorted {
        let overlaps = accepted.iter().any(|existing| {
            let inter_start = cand.start_seconds.max(existing.start_seconds);
            let inter_end = cand.end_seconds.min(existing.end_seconds);
            let inter = (inter_end - inter_start).max(0.0);
            let union = (cand.duration_seconds + existing.duration_seconds - inter).max(1.0);
            (inter / union) > 0.35
        });
        if !overlaps {
            accepted.push(cand);
            if accepted.len() >= max_clips {
                break;
            }
        }
    }

    // Sort final list chronologically or by virality score (virality score descending is best for studio)
    accepted.sort_by(|a, b| b.virality_score.cmp(&a.virality_score));
    accepted
}

pub async fn detect_viral_moments(
    transcript: &Transcript,
    audio_spikes: &[f64],
    content_type: &str,
    config: &AiConfig,
) -> AppResult<Vec<ViralHighlight>> {
    let all_words: Vec<TranscriptWord> = transcript
        .segments
        .iter()
        .flat_map(|s| s.words.clone())
        .collect();

    let use_llm = config.provider != "heuristic"
        && (!config.api_key.trim().is_empty() || config.provider == "ollama");

    let mut raw_clips: Vec<ViralHighlight> = Vec::new();

    if use_llm {
        let chunks = build_chunks(transcript, audio_spikes);
        let sys_prompt = build_system_prompt(
            content_type,
            config.target_clip_min_secs,
            config.target_clip_max_secs,
            config.max_clips,
        );

        for chunk in chunks {
            match call_llm_chunk(config, &sys_prompt, &chunk.formatted_text).await {
                Ok(mut chunk_clips) => raw_clips.append(&mut chunk_clips),
                Err(e) => {
                    tracing::warn!(
                        "LLM chunk [{:.0}-{:.0}] gagal ({}), fallback ke heuristik untuk chunk ini",
                        chunk.start_sec,
                        chunk.end_sec,
                        e
                    );
                }
            }
        }
    }

    // Fallback to smart local heuristic if LLM was not configured or returned empty
    if raw_clips.is_empty() {
        raw_clips = detect_highlights_heuristic(transcript, audio_spikes, config);
    }

    let clamped: Vec<ViralHighlight> = raw_clips
        .into_iter()
        .map(|c| clamp_to_word_boundaries(c, &all_words, transcript.duration_seconds))
        .collect();

    let final_clips = deduplicate_and_rank(clamped, config.max_clips.max(1));
    Ok(final_clips)
}

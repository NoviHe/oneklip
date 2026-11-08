use crate::error::AppResult;
use crate::services::innertube::{Transcript, TranscriptWord};
use crate::services::render::subtitle_style::{
    hex_to_ass_color, hex_to_ass_override_bgr, SubtitleStyle,
};
use std::path::Path;

fn format_ass_time(seconds: f64) -> String {
    let clamped = seconds.max(0.0);
    let total_cs = (clamped * 100.0).round() as u64;
    let cs = total_cs % 100;
    let total_secs = total_cs / 100;
    let s = total_secs % 60;
    let total_mins = total_secs / 60;
    let m = total_mins % 60;
    let h = total_mins / 60;
    format!("{}:{:02}:{:02}.{:02}", h, m, s, cs)
}

fn escape_ass_text(raw: &str, uppercase: bool) -> String {
    let cleaned = raw
        .replace('\\', "")
        .replace('{', "(")
        .replace('}', ")");
    if uppercase {
        cleaned.to_uppercase()
    } else {
        cleaned
    }
}

/// Extracts clip-local words shifted so `0.0` corresponds to `clip_start_sec`.
pub fn slice_words_for_clip(
    transcript: &Transcript,
    clip_start_sec: f64,
    clip_end_sec: f64,
) -> Vec<TranscriptWord> {
    let mut local_words = Vec::new();
    let clip_dur = (clip_end_sec - clip_start_sec).max(0.5);

    for seg in &transcript.segments {
        if seg.end < clip_start_sec || seg.start > clip_end_sec {
            continue;
        }
        for w in &seg.words {
            if w.end <= clip_start_sec || w.start >= clip_end_sec {
                continue;
            }
            let rel_start = (w.start - clip_start_sec).clamp(0.0, clip_dur);
            let rel_end = (w.end - clip_start_sec)
                .clamp(rel_start + 0.06, clip_dur);
            local_words.push(TranscriptWord {
                word: w.word.clone(),
                start: (rel_start * 1000.0).round() / 1000.0,
                end: (rel_end * 1000.0).round() / 1000.0,
                probability: w.probability,
            });
        }
    }

    // Prevent overlapping adjacent word timings
    for i in 0..local_words.len().saturating_sub(1) {
        if local_words[i].end > local_words[i + 1].start {
            local_words[i].end = local_words[i + 1]
                .start
                .max(local_words[i].start + 0.05);
        }
    }

    local_words
}

fn group_words_into_chunks(
    words: &[TranscriptWord],
    max_words_per_chunk: usize,
) -> Vec<Vec<TranscriptWord>> {
    let limit = max_words_per_chunk.clamp(1, 7);
    let mut chunks: Vec<Vec<TranscriptWord>> = Vec::new();
    let mut current: Vec<TranscriptWord> = Vec::new();

    for w in words {
        if !current.is_empty() {
            let prev = current.last().unwrap();
            let gap = w.start - prev.end;
            let prev_ends_sentence = prev.word.ends_with('.')
                || prev.word.ends_with('!')
                || prev.word.ends_with('?');

            if current.len() >= limit || gap > 0.45 || prev_ends_sentence {
                chunks.push(std::mem::take(&mut current));
            }
        }
        current.push(w.clone());
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn animation_entry_tag(animation: &str, style: &SubtitleStyle) -> String {
    let hi_bgr = hex_to_ass_override_bgr(&style.highlight_color);
    let box_bgr = hex_to_ass_override_bgr(&style.box_color);
    match animation {
        "slam" => r"{\fscx145\fscy145\t(0,85,\fscx100\fscy100)}".to_string(),
        "bump" => r"{\fscx92\fscy92\t(0,75,\fscx106\fscy106)\t(75,150,\fscx100\fscy100)}".to_string(),
        "glow" => format!(r"{{\blur4\3c{}\t(0,120,\blur1.5)}}", hi_bgr),
        "neon" => format!(r"{{\bord6\blur5\3c{}}}", box_bgr),
        "comic" => r"{\frz-3\fscx125\fscy125\t(0,95,\frz0\fscx100\fscy100)}".to_string(),
        "fade" => r"{\fad(90,90)}".to_string(),
        _ => r"{\fscx118\fscy118\t(0,80,\fscx100\fscy100)}".to_string(), // "pop" default
    }
}

pub fn generate_ass_content(words: &[TranscriptWord], style: &SubtitleStyle) -> String {
    let primary_ass = hex_to_ass_color(&style.primary_color, 0x00);
    let secondary_ass = hex_to_ass_color(&style.highlight_color, 0x00);
    let outline_ass = hex_to_ass_color(&style.outline_color, 0x00);
    let back_ass = if style.animation == "word_box" {
        hex_to_ass_color(&style.box_color, 0x20)
    } else {
        "&H80000000".to_string()
    };

    let border_style = if style.animation == "word_box" { 3 } else { 1 };
    let y_px = (style.vertical_position.clamp(0.15, 0.88) * 1920.0).round() as i32;
    let margin_v = (1920 - y_px).clamp(120, 1600);

    let mut ass = format!(
        "[Script Info]\n\
         Title: OneKlip Animated Subtitles\n\
         ScriptType: v4.00+\n\
         WrapStyle: 0\n\
         ScaledBorderAndShadow: yes\n\
         YCbCr Matrix: TV.709\n\
         PlayResX: 1080\n\
         PlayResY: 1920\n\n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Default,{font},{size},{pri},{sec},{out},{back},-1,0,0,0,100,100,1,0,{bstyle},{outline:.1},{shadow:.1},2,70,70,{margin_v},1\n\n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        font = style.font_family,
        size = style.font_size,
        pri = primary_ass,
        sec = secondary_ass,
        out = outline_ass,
        back = back_ass,
        bstyle = border_style,
        outline = style.outline_width,
        shadow = style.shadow_depth,
        margin_v = margin_v,
    );

    if words.is_empty() {
        return ass;
    }

    let pri_bgr = hex_to_ass_override_bgr(&style.primary_color);
    let hi_bgr = hex_to_ass_override_bgr(&style.highlight_color);
    let out_bgr = hex_to_ass_override_bgr(&style.outline_color);
    let box_bgr = hex_to_ass_override_bgr(&style.box_color);
    let entry_anim = animation_entry_tag(&style.animation, style);
    let pos_tag = format!(r"{{\an5\pos(540,{})}}", y_px);

    match style.reveal_mode.as_str() {
        "per_word" => {
            for w in words {
                let start_str = format_ass_time(w.start);
                let end_str = format_ass_time(w.end.max(w.start + 0.08));
                let txt = escape_ass_text(&w.word, style.uppercase);
                ass.push_str(&format!(
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}{}{{\\1c{}}}{}\n",
                    start_str, end_str, pos_tag, entry_anim, hi_bgr, txt
                ));
            }
        }
        "karaoke_sweep" => {
            let chunks = group_words_into_chunks(words, style.words_per_chunk);
            for chunk in chunks {
                let c_start = chunk.first().unwrap().start;
                let c_end = chunk.last().unwrap().end.max(c_start + 0.15);
                let mut line_parts = Vec::new();
                for w in &chunk {
                    let dur_cs = (((w.end - w.start).max(0.08)) * 100.0).round() as u32;
                    let txt = escape_ass_text(&w.word, style.uppercase);
                    line_parts.push(format!("{{\\kf{}}}{}", dur_cs, txt));
                }
                ass.push_str(&format!(
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}{}{}\n",
                    format_ass_time(c_start),
                    format_ass_time(c_end),
                    pos_tag,
                    entry_anim,
                    line_parts.join(" ")
                ));
            }
        }
        "cumulative" => {
            let chunks = group_words_into_chunks(words, style.words_per_chunk);
            for chunk in chunks {
                let chunk_end = chunk.last().unwrap().end;
                for idx in 0..chunk.len() {
                    let sub_start = chunk[idx].start;
                    let sub_end = if idx + 1 < chunk.len() {
                        chunk[idx + 1].start.max(sub_start + 0.06)
                    } else {
                        chunk_end.max(sub_start + 0.10)
                    };

                    let mut rendered_words = Vec::new();
                    for (j, w) in chunk.iter().take(idx + 1).enumerate() {
                        let txt = escape_ass_text(&w.word, style.uppercase);
                        if j == idx {
                            rendered_words.push(format!("{{\\1c{}}}{}{{\\1c{}}}", hi_bgr, txt, pri_bgr));
                        } else {
                            rendered_words.push(txt);
                        }
                    }

                    ass.push_str(&format!(
                        "Dialogue: 0,{},{},Default,,0,0,0,,{}{}{}\n",
                        format_ass_time(sub_start),
                        format_ass_time(sub_end),
                        pos_tag,
                        if idx == 0 { entry_anim.as_str() } else { "" },
                        rendered_words.join(" ")
                    ));
                }
            }
        }
        "segment_static" => {
            let chunks = group_words_into_chunks(words, style.words_per_chunk.max(4));
            for chunk in chunks {
                let c_start = chunk.first().unwrap().start;
                let c_end = chunk.last().unwrap().end.max(c_start + 0.15);
                let text = chunk
                    .iter()
                    .map(|w| escape_ass_text(&w.word, style.uppercase))
                    .collect::<Vec<_>>()
                    .join(" ");
                ass.push_str(&format!(
                    "Dialogue: 0,{},{},Default,,0,0,0,,{}{}{}\n",
                    format_ass_time(c_start),
                    format_ass_time(c_end),
                    pos_tag,
                    entry_anim,
                    text
                ));
            }
        }
        _ => {
            // Default: "chunk_karaoke" — shows N words at a time, highlighting the active spoken word
            let chunks = group_words_into_chunks(words, style.words_per_chunk);
            for chunk in chunks {
                let chunk_end = chunk.last().unwrap().end;
                for (active_idx, active_w) in chunk.iter().enumerate() {
                    let sub_start = active_w.start;
                    let sub_end = if active_idx + 1 < chunk.len() {
                        chunk[active_idx + 1].start.max(sub_start + 0.06)
                    } else {
                        chunk_end.max(sub_start + 0.10)
                    };

                    let mut tokens = Vec::new();
                    for (j, w) in chunk.iter().enumerate() {
                        let txt = escape_ass_text(&w.word, style.uppercase);
                        if j == active_idx {
                            if style.animation == "word_box" {
                                tokens.push(format!(
                                    "{{\\1c{}\\3c{}\\bord9}}{}{{\\1c{}\\3c{}\\bord{:.1}}}",
                                    hi_bgr,
                                    box_bgr,
                                    txt,
                                    pri_bgr,
                                    out_bgr,
                                    style.outline_width
                                ));
                            } else {
                                tokens.push(format!(
                                    "{{\\1c{}\\fscx110\\fscy110}}{}{{\\1c{}\\fscx100\\fscy100}}",
                                    hi_bgr, txt, pri_bgr
                                ));
                            }
                        } else {
                            tokens.push(format!("{{\\1c{}}}{}", pri_bgr, txt));
                        }
                    }

                    let anim_prefix = if active_idx == 0 {
                        entry_anim.as_str()
                    } else {
                        ""
                    };

                    ass.push_str(&format!(
                        "Dialogue: 0,{},{},Default,,0,0,0,,{}{}{}\n",
                        format_ass_time(sub_start),
                        format_ass_time(sub_end),
                        pos_tag,
                        anim_prefix,
                        tokens.join(" ")
                    ));
                }
            }
        }
    }

    ass
}

pub fn write_ass_file(
    words: &[TranscriptWord],
    style: &SubtitleStyle,
    out_path: &Path,
) -> AppResult<()> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = generate_ass_content(words, style);
    std::fs::write(out_path, content)?;
    Ok(())
}

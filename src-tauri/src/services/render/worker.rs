use crate::error::{AppError, AppResult};
use crate::services::encoder::{detect_best_encoder, encoder_ffmpeg_args};
use crate::services::innertube::TranscriptWord;
use crate::services::render::hook_overlay::render_hook_png;
use crate::services::render::hook_style::HookStyle;
use crate::services::render::subtitle_style::SubtitleStyle;
use crate::services::smartcrop::analyze_smart_crop;
use crate::services::subtitles::write_ass_file;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipRenderSpec {
    pub clip_id: String,
    pub video_layout: String, // "smart_speaker" | "split_screen" | "auto_banter" | "fill" | "fit_blur" | "fit_black"
    pub subtitle_style: SubtitleStyle,
    pub hook_style: HookStyle,
    pub hook_text: Option<String>,
    pub hook_kicker: Option<String>,
    pub hook_tag: Option<String>,
    pub has_captions: bool,
    pub has_hook: bool,
    pub has_watermark: bool,
    pub watermark_text: Option<String>,
    pub words: Vec<TranscriptWord>,
}

/// Escapes a Windows file path for use inside FFmpeg filtergraph single-quoted arguments.
pub fn escape_ffmpeg_filter_path(p: &Path) -> String {
    p.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "\\'")
}

pub fn generate_clip_thumbnail(
    state: &AppState,
    video_path: &Path,
    out_jpg: &Path,
    at_sec: f64,
) -> AppResult<()> {
    if let Some(parent) = out_jpg.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args([
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-ss",
        &format!("{:.2}", at_sec.max(0.2)),
        "-i",
    ])
    .arg(video_path)
    .args(["-frames:v", "1", "-vf", "scale=540:-2", "-q:v", "3"])
    .arg(out_jpg);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let _ = cmd.output();
    Ok(())
}

pub fn render_clip_master(
    state: &AppState,
    raw_clip_mp4: &Path,
    clip_dir: &Path,
    spec: &ClipRenderSpec,
    out_final_mp4: &Path,
) -> AppResult<String> {
    std::fs::create_dir_all(clip_dir)?;

    // 1. Analyze Smart Crop (ONNX YuNet + Active Speaker + Banter Funnel)
    let crop_plan = analyze_smart_crop(state, raw_clip_mp4, &spec.video_layout, clip_dir)?;

    // 2. Write animated .ass subtitle file if enabled
    let ass_path = clip_dir.join("captions.ass");
    let use_captions = spec.has_captions && !spec.words.is_empty();
    if use_captions {
        write_ass_file(&spec.words, &spec.subtitle_style, &ass_path)?;
    }

    // 3. Render 1080x1920 intro_hook.png if enabled
    let hook_png_path = clip_dir.join("intro_hook.png");
    let hook_headline = spec.hook_text.as_deref().unwrap_or("").trim();
    let use_hook = spec.has_hook && spec.hook_style.enabled && !hook_headline.is_empty();
    if use_hook {
        let _ = render_hook_png(
            &state.fonts_dir,
            hook_headline,
            spec.hook_kicker.as_deref(),
            spec.hook_tag.as_deref(),
            &spec.hook_style,
            &hook_png_path,
        );
    }

    // 4. Build base 9:16 (1080x1920) layout filter chain on [0:v] -> [v_base]
    let src_w = crop_plan.source_width.max(2) as f32;
    let src_h = crop_plan.source_height.max(2) as f32;

    let base_layout_filter = match crop_plan.effective_layout.as_str() {
        "split_screen" => {
            // Top half (1080x960) = Left speaker, Bottom half (1080x960) = Right speaker
            let tile_crop_w = ((src_h * 9.0 / 8.0).min(src_w * 0.48).round() as u32)
                .max(2)
                & !1;
            let tile_crop_h = ((tile_crop_w as f32 * 960.0 / 1080.0).min(src_h).round() as u32)
                .max(2)
                & !1;
            let max_x = (crop_plan.source_width.saturating_sub(tile_crop_w)) as f32;

            let left_x = (((crop_plan.left_speaker_x_norm * src_w) - (tile_crop_w as f32 * 0.5))
                .round()
                .clamp(0.0, max_x) as u32)
                & !1;
            let right_x = (((crop_plan.right_speaker_x_norm * src_w) - (tile_crop_w as f32 * 0.5))
                .round()
                .clamp(0.0, max_x) as u32)
                & !1;
            let crop_y = ((crop_plan.source_height.saturating_sub(tile_crop_h)) / 3) & !1;

            format!(
                "[0:v]split=2[sp_top][sp_bot];\
                 [sp_top]crop={w}:{h}:{lx}:{cy},scale=1080:960,setsar=1[top_v];\
                 [sp_bot]crop={w}:{h}:{rx}:{cy},scale=1080:960,setsar=1[bot_v];\
                 [top_v][bot_v]vstack=inputs=2[v_base]",
                w = tile_crop_w,
                h = tile_crop_h,
                lx = left_x,
                rx = right_x,
                cy = crop_y
            )
        }
        "fit_blur" => String::from(
            "[0:v]split=2[bg_in][fg_in];\
             [bg_in]scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,boxblur=24:4[bg_v];\
             [fg_in]scale=1080:-2:force_original_aspect_ratio=decrease[fg_v];\
             [bg_v][fg_v]overlay=(W-w)/2:(H-h)/2,setsar=1[v_base]",
        ),
        "fit_black" => String::from(
            "[0:v]scale=1080:1920:force_original_aspect_ratio=decrease,\
             pad=1080:1920:(ow-iw)/2:(oh-ih)/2:black,setsar=1[v_base]",
        ),
        "smart_speaker" => {
            let crop_w = ((src_h * 9.0 / 16.0).min(src_w).round() as u32).max(2) & !1;
            let crop_h = (crop_plan.source_height.max(2)) & !1;
            let default_x = ((crop_plan.source_width.saturating_sub(crop_w)) / 2) & !1;

            if let Some(ref cmd_file) = crop_plan.sendcmd_path {
                let esc_cmd = escape_ffmpeg_filter_path(Path::new(cmd_file));
                format!(
                    "[0:v]sendcmd=f='{}',crop={}:{}:{}:0,scale=1080:1920,setsar=1[v_base]",
                    esc_cmd, crop_w, crop_h, default_x
                )
            } else {
                format!(
                    "[0:v]crop={}:{}:{}:0,scale=1080:1920,setsar=1[v_base]",
                    crop_w, crop_h, default_x
                )
            }
        }
        _ => {
            // "fill" (center 9:16 crop)
            let crop_w = ((src_h * 9.0 / 16.0).min(src_w).round() as u32).max(2) & !1;
            let crop_h = (crop_plan.source_height.max(2)) & !1;
            let center_x = ((crop_plan.source_width.saturating_sub(crop_w)) / 2) & !1;
            format!(
                "[0:v]crop={}:{}:{}:0,scale=1080:1920,setsar=1[v_base]",
                crop_w, crop_h, center_x
            )
        }
    };

    // 5. Chain subtitles & hook overlay
    let mut filter_parts = vec![base_layout_filter];
    let mut current_pad = String::from("[v_base]");

    if use_captions && ass_path.exists() {
        let esc_ass = escape_ffmpeg_filter_path(&ass_path);
        let esc_fonts = escape_ffmpeg_filter_path(&state.fonts_dir);
        filter_parts.push(format!(
            "{}subtitles='{}':fontsdir='{}'[v_sub]",
            current_pad, esc_ass, esc_fonts
        ));
        current_pad = String::from("[v_sub]");
    }

    if use_hook && hook_png_path.exists() {
        let dur = spec.hook_style.duration_seconds.clamp(1.0, 8.0);
        filter_parts.push(format!(
            "{}[1:v]overlay=0:0:enable='between(t,0,{:.2})'[v_hook]",
            current_pad, dur
        ));
        current_pad = String::from("[v_hook]");
    }

    filter_parts.push(format!("{}format=yuv420p[v_out]", current_pad));
    let filter_complex = filter_parts.join(";");

    // 6. Execute FFmpeg encode
    let encoder = detect_best_encoder(&state.ffmpeg_path());
    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(raw_clip_mp4);

    if use_hook && hook_png_path.exists() {
        cmd.arg("-i").arg(&hook_png_path);
    }

    cmd.args(["-filter_complex", &filter_complex, "-map", "[v_out]", "-map", "0:a?"]);
    cmd.args(encoder_ffmpeg_args(&encoder.codec, 21));
    cmd.args(["-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"])
        .arg(out_final_mp4);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output()?;
    if !out.status.success() {
        // Fallback to software libx264 if hardware encoder failed
        if encoder.is_hardware {
            let mut sw_cmd = Command::new(state.ffmpeg_path());
            sw_cmd
                .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
                .arg(raw_clip_mp4);
            if use_hook && hook_png_path.exists() {
                sw_cmd.arg("-i").arg(&hook_png_path);
            }
            sw_cmd.args([
                "-filter_complex",
                &filter_complex,
                "-map",
                "[v_out]",
                "-map",
                "0:a?",
            ]);
            sw_cmd.args(encoder_ffmpeg_args("libx264", 21));
            sw_cmd
                .args(["-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"])
                .arg(out_final_mp4);
            #[cfg(target_os = "windows")]
            sw_cmd.creation_flags(CREATE_NO_WINDOW);

            let sw_out = sw_cmd.output()?;
            if !sw_out.status.success() {
                let stderr = String::from_utf8_lossy(&sw_out.stderr);
                return Err(AppError::custom(
                    "ffmpeg_render_failed",
                    format!("Render FFmpeg gagal: {}", stderr.trim()),
                ));
            }
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(AppError::custom(
                "ffmpeg_render_failed",
                format!("Render FFmpeg gagal: {}", stderr.trim()),
            ));
        }
    }

    // 7. Generate preview thumbnail
    let preview_jpg = clip_dir.join("preview.jpg");
    let _ = generate_clip_thumbnail(state, out_final_mp4, &preview_jpg, 1.2);

    Ok(crop_plan.effective_layout)
}

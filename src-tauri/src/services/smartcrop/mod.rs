use crate::error::AppResult;
use crate::state::AppState;
use ndarray::Array4;
use ort::environment::Environment;
use ort::session::Session;
use ort::value::TensorRef;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static ORT_ENV: OnceLock<Option<Environment>> = OnceLock::new();

fn get_ort_environment(state: &AppState) -> Option<&'static Environment> {
    ORT_ENV
        .get_or_init(|| {
            let dll = state.onnxruntime_dll_path();
            if dll.exists() {
                std::env::set_var("ORT_DYLIB_PATH", &dll);
                ort::init_from(&dll).ok().and_then(|b| b.build().ok())
            } else {
                None
            }
        })
        .as_ref()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedFace {
    pub center_x: f32, // Normalized 0.0..1.0
    pub center_y: f32, // Normalized 0.0..1.0
    pub width: f32,    // Normalized 0.0..1.0
    pub height: f32,   // Normalized 0.0..1.0
    pub score: f32,
    pub mouth_activity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrajectoryPoint {
    pub start_sec: f64,
    pub end_sec: f64,
    pub center_x_norm: f32, // 0.0..1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCropPlan {
    pub effective_layout: String, // "smart_speaker" | "split_screen" | "fill" | "fit_blur" | "fit_black"
    pub source_width: u32,
    pub source_height: u32,
    pub left_speaker_x_norm: f32,
    pub right_speaker_x_norm: f32,
    pub trajectory: Vec<TrajectoryPoint>,
    pub sendcmd_path: Option<String>,
}

fn probe_video_dimensions(state: &AppState, video_path: &Path) -> (u32, u32) {
    if !state.ffprobe_path().exists() {
        return (1920, 1080);
    }
    let mut cmd = Command::new(state.ffprobe_path());
    cmd.args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height",
        "-of",
        "csv=s=x:p=0",
    ])
    .arg(video_path);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(out) = cmd.output() {
        let s = String::from_utf8_lossy(&out.stdout);
        let parts: Vec<&str> = s.trim().split('x').collect();
        if parts.len() == 2 {
            if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                if w > 0 && h > 0 {
                    return (w, h);
                }
            }
        }
    }
    (1920, 1080)
}

/// Extracts 320x320 RGB24 frames at 4 FPS for fast YuNet face detection and mouth activity analysis.
fn extract_analysis_frames(state: &AppState, video_path: &Path, fps: u32) -> Vec<Vec<u8>> {
    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args(["-v", "error", "-i"])
        .arg(video_path)
        .args([
            "-vf",
            &format!("fps={},scale=320:320:flags=fast_bilinear", fps),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let frame_bytes = 320 * 320 * 3;
    match cmd.output() {
        Ok(out) if out.stdout.len() >= frame_bytes => out
            .stdout
            .chunks_exact(frame_bytes)
            .map(|c| c.to_vec())
            .collect(),
        _ => Vec::new(),
    }
}

/// Computes per-frame voice activity energy (0.0..1.0) at `fps` rate using 4kHz mono PCM pipe.
fn extract_frame_vad_energy(state: &AppState, video_path: &Path, fps: u32) -> Vec<f32> {
    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args(["-v", "error", "-i"])
        .arg(video_path)
        .args(["-vn", "-ac", "1", "-ar", "4000", "-f", "s16le", "-"]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let samples_per_frame = (4000 / fps.max(1)) as usize;
    match cmd.output() {
        Ok(out) if out.stdout.len() >= samples_per_frame * 2 => {
            let samples: Vec<i16> = out
                .stdout
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            samples
                .chunks(samples_per_frame)
                .map(|ch| {
                    let sum_sq: f64 = ch
                        .iter()
                        .map(|&s| {
                            let v = s as f64 / 32768.0;
                            v * v
                        })
                        .sum();
                    ((sum_sq / ch.len().max(1) as f64).sqrt() as f32 * 4.0).clamp(0.0, 1.0)
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn nms_faces(mut faces: Vec<DetectedFace>, iou_threshold: f32) -> Vec<DetectedFace> {
    faces.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<DetectedFace> = Vec::new();

    for cand in faces {
        let overlaps = kept.iter().any(|k| {
            let ax1 = cand.center_x - cand.width * 0.5;
            let ay1 = cand.center_y - cand.height * 0.5;
            let ax2 = cand.center_x + cand.width * 0.5;
            let ay2 = cand.center_y + cand.height * 0.5;

            let bx1 = k.center_x - k.width * 0.5;
            let by1 = k.center_y - k.height * 0.5;
            let bx2 = k.center_x + k.width * 0.5;
            let by2 = k.center_y + k.height * 0.5;

            let ix1 = ax1.max(bx1);
            let iy1 = ay1.max(by1);
            let ix2 = ax2.min(bx2);
            let iy2 = ay2.min(by2);

            let inter = (ix2 - ix1).max(0.0) * (iy2 - iy1).max(0.0);
            let area_a = (cand.width * cand.height).max(1e-5);
            let area_b = (k.width * k.height).max(1e-5);
            inter / (area_a + area_b - inter) > iou_threshold
        });
        if !overlaps {
            kept.push(cand);
        }
    }
    kept
}

fn detect_faces_yunet_frame(
    session: &mut Session,
    rgb_frame: &[u8],
    prev_frame: Option<&[u8]>,
) -> Vec<DetectedFace> {
    // Prepare [1, 3, 320, 320] BGR float32 tensor expected by OpenCV YuNet
    let mut input = Array4::<f32>::zeros((1, 3, 320, 320));
    for y in 0..320 {
        for x in 0..320 {
            let idx = (y * 320 + x) * 3;
            let r = rgb_frame[idx] as f32;
            let g = rgb_frame[idx + 1] as f32;
            let b = rgb_frame[idx + 2] as f32;
            input[[0, 0, y, x]] = b;
            input[[0, 1, y, x]] = g;
            input[[0, 2, y, x]] = r;
        }
    }

    let input_tensor = match TensorRef::from_array_view(&input) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let outputs = match session.run(ort::inputs![input_tensor]) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let mut raw_faces = Vec::new();
    let strides = [(8_usize, "cls_8", "obj_8", "bbox_8"), (16, "cls_16", "obj_16", "bbox_16"), (32, "cls_32", "obj_32", "bbox_32")];

    for (stride, cls_name, obj_name, bbox_name) in strides {
        let grid_w = 320 / stride;
        let grid_h = 320 / stride;

        let cls_slice = outputs
            .get(cls_name)
            .and_then(|v| v.try_extract_tensor::<f32>().ok())
            .map(|(_, data)| data.to_vec());
        let obj_slice = outputs
            .get(obj_name)
            .and_then(|v| v.try_extract_tensor::<f32>().ok())
            .map(|(_, data)| data.to_vec());
        let bbox_slice = outputs
            .get(bbox_name)
            .and_then(|v| v.try_extract_tensor::<f32>().ok())
            .map(|(_, data)| data.to_vec());

        if let (Some(cls), Some(obj), Some(bbox)) = (cls_slice, obj_slice, bbox_slice) {
            let cells = grid_w * grid_h;
            if cls.len() >= cells && obj.len() >= cells && bbox.len() >= cells * 4 {
                for idx in 0..cells {
                    let score = (cls[idx].clamp(0.0, 1.0) * obj[idx].clamp(0.0, 1.0)).sqrt();
                    if score < 0.52 {
                        continue;
                    }
                    let row = idx / grid_w;
                    let col = idx % grid_w;
                    let s = stride as f32;

                    let cx = (col as f32 + bbox[idx * 4]) * s / 320.0;
                    let cy = (row as f32 + bbox[idx * 4 + 1]) * s / 320.0;
                    let w = (bbox[idx * 4 + 2].exp() * s / 320.0).clamp(0.04, 0.90);
                    let h = (bbox[idx * 4 + 3].exp() * s / 320.0).clamp(0.04, 0.90);

                    if !(0.05..=0.95).contains(&cx) || !(0.05..=0.90).contains(&cy) {
                        continue;
                    }

                    let mouth_activity = compute_mouth_roi_motion(rgb_frame, prev_frame, cx, cy, w, h);

                    raw_faces.push(DetectedFace {
                        center_x: cx,
                        center_y: cy,
                        width: w,
                        height: h,
                        score,
                        mouth_activity,
                    });
                }
            }
        }
    }

    nms_faces(raw_faces, 0.35)
}

fn compute_mouth_roi_motion(
    curr_rgb: &[u8],
    prev_rgb: Option<&[u8]>,
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
) -> f32 {
    let prev = match prev_rgb {
        Some(p) => p,
        None => return 0.1,
    };

    // Mouth region is the bottom 40% of the face box, horizontally centered 60%
    let x1 = ((cx - w * 0.3) * 320.0).round().clamp(0.0, 319.0) as usize;
    let x2 = ((cx + w * 0.3) * 320.0).round().clamp(0.0, 319.0) as usize;
    let y1 = ((cy + h * 0.05) * 320.0).round().clamp(0.0, 319.0) as usize;
    let y2 = ((cy + h * 0.45) * 320.0).round().clamp(0.0, 319.0) as usize;

    if x2 <= x1 || y2 <= y1 {
        return 0.05;
    }

    let mut diff_sum = 0.0_f32;
    let mut count = 0_usize;
    for y in y1..=y2 {
        for x in x1..=x2 {
            let idx = (y * 320 + x) * 3;
            let lum_c = 0.299 * curr_rgb[idx] as f32
                + 0.587 * curr_rgb[idx + 1] as f32
                + 0.114 * curr_rgb[idx + 2] as f32;
            let lum_p = 0.299 * prev[idx] as f32
                + 0.587 * prev[idx + 1] as f32
                + 0.114 * prev[idx + 2] as f32;
            diff_sum += (lum_c - lum_p).abs();
            count += 1;
        }
    }

    if count == 0 {
        0.05
    } else {
        (diff_sum / (count as f32 * 35.0)).clamp(0.0, 1.0)
    }
}

/// Detects scene cut between consecutive 320x320 frames using mean absolute luminance difference.
fn is_scene_cut(curr_rgb: &[u8], prev_rgb: &[u8]) -> bool {
    let step = 48; // Sample every 16th pixel
    let mut sum_diff = 0.0_f32;
    let mut samples = 0_usize;
    let mut idx = 0;
    while idx + 2 < curr_rgb.len() && idx + 2 < prev_rgb.len() {
        let d = (curr_rgb[idx] as f32 - prev_rgb[idx] as f32).abs()
            + (curr_rgb[idx + 1] as f32 - prev_rgb[idx + 1] as f32).abs()
            + (curr_rgb[idx + 2] as f32 - prev_rgb[idx + 2] as f32).abs();
        sum_diff += d / (255.0 * 3.0);
        samples += 1;
        idx += step;
    }
    samples > 0 && (sum_diff / samples as f32) > 0.28
}

/// Analyzes a raw clip and produces a `SmartCropPlan` (including `crop_trajectory.cmd` for FFmpeg `sendcmd`).
pub fn analyze_smart_crop(
    state: &AppState,
    clip_raw_mp4: &Path,
    requested_layout: &str,
    clip_dir: &Path,
) -> AppResult<SmartCropPlan> {
    let (src_w, src_h) = probe_video_dimensions(state, clip_raw_mp4);

    // If source is already vertical (9:16) or user requested static layout ("fill", "fit_blur", "fit_black")
    if src_h >= src_w || matches!(requested_layout, "fill" | "fit_blur" | "fit_black") {
        let eff = if src_h >= src_w {
            "fill"
        } else {
            requested_layout
        };
        return Ok(SmartCropPlan {
            effective_layout: eff.to_string(),
            source_width: src_w,
            source_height: src_h,
            left_speaker_x_norm: 0.32,
            right_speaker_x_norm: 0.68,
            trajectory: Vec::new(),
            sendcmd_path: None,
        });
    }

    let fps = 4_u32;
    let frames = extract_analysis_frames(state, clip_raw_mp4, fps);
    let vad_scores = extract_frame_vad_energy(state, clip_raw_mp4, fps);

    let mut yunet_session = if state.yunet_model_path().exists() {
        get_ort_environment(state).and_then(|env| {
            Session::builder(env)
                .ok()
                .and_then(|mut b| b.commit_from_file(state.yunet_model_path()).ok())
        })
    } else {
        None
    };

    let mut frame_targets: Vec<f32> = Vec::with_capacity(frames.len().max(1));
    let mut dual_speaker_frames = 0_usize;
    let mut left_xs: Vec<f32> = Vec::new();
    let mut right_xs: Vec<f32> = Vec::new();
    let mut last_target = 0.50_f32;

    for (i, frame) in frames.iter().enumerate() {
        let prev = if i > 0 { Some(frames[i - 1].as_slice()) } else { None };
        let vad = vad_scores.get(i).copied().unwrap_or(0.5);

        let faces = if let Some(ref mut sess) = yunet_session {
            detect_faces_yunet_frame(sess, frame, prev)
        } else {
            Vec::new()
        };

        if faces.len() >= 2 {
            // Sort top 2 faces left-to-right
            let mut top2 = faces.iter().take(2).copied().collect::<Vec<_>>();
            top2.sort_by(|a, b| a.center_x.partial_cmp(&b.center_x).unwrap());
            let f_left = top2[0];
            let f_right = top2[1];

            if (f_right.center_x - f_left.center_x) >= 0.22 {
                dual_speaker_frames += 1;
                left_xs.push(f_left.center_x);
                right_xs.push(f_right.center_x);
            }
        }

        // Choose active speaker face: highest (score * (0.4 + 0.6 * mouth_activity * vad))
        if let Some(best) = faces.iter().max_by(|a, b| {
            let sa = a.score * (0.4 + 0.6 * a.mouth_activity * (0.5 + 0.5 * vad)) * a.width;
            let sb = b.score * (0.4 + 0.6 * b.mouth_activity * (0.5 + 0.5 * vad)) * b.width;
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        }) {
            last_target = best.center_x.clamp(0.18, 0.82);
        }

        frame_targets.push(last_target);
    }

    let left_avg = if left_xs.is_empty() {
        0.30
    } else {
        (left_xs.iter().sum::<f32>() / left_xs.len() as f32).clamp(0.18, 0.45)
    };
    let right_avg = if right_xs.is_empty() {
        0.70
    } else {
        (right_xs.iter().sum::<f32>() / right_xs.len() as f32).clamp(0.55, 0.82)
    };

    // Banter Funnel: If user explicitly requested "split_screen" OR ("auto" / "smart_speaker" with strong dual-speaker co-presence >= 45%)
    let dual_ratio = if frames.is_empty() {
        0.0
    } else {
        dual_speaker_frames as f32 / frames.len() as f32
    };

    if requested_layout == "split_screen"
        || (requested_layout == "auto_banter" && dual_ratio >= 0.40)
    {
        return Ok(SmartCropPlan {
            effective_layout: "split_screen".to_string(),
            source_width: src_w,
            source_height: src_h,
            left_speaker_x_norm: left_avg,
            right_speaker_x_norm: right_avg,
            trajectory: Vec::new(),
            sendcmd_path: None,
        });
    }

    // Smooth single-speaker trajectory with 30% deadzone + EMA + scene-cut snap
    let crop_w = ((src_h as f32 * 9.0 / 16.0).round() as u32)
        .min(src_w)
        .max(2);
    let max_crop_x = src_w.saturating_sub(crop_w) as f32;
    let deadzone_norm = (crop_w as f32 / src_w as f32) * 0.16; // ~30% inner deadzone

    let mut smoothed_x = frame_targets.first().copied().unwrap_or(0.50);
    let mut trajectory: Vec<TrajectoryPoint> = Vec::new();
    let frame_dt = 1.0 / fps as f64;

    for (i, &target_x) in frame_targets.iter().enumerate() {
        let cut = if i > 0 {
            is_scene_cut(&frames[i], &frames[i - 1])
        } else {
            false
        };

        if cut {
            smoothed_x = target_x;
        } else if (target_x - smoothed_x).abs() > deadzone_norm {
            smoothed_x = 0.78 * smoothed_x + 0.22 * target_x;
        }

        let t_start = i as f64 * frame_dt;
        let t_end = t_start + frame_dt;
        trajectory.push(TrajectoryPoint {
            start_sec: (t_start * 100.0).round() / 100.0,
            end_sec: (t_end * 100.0).round() / 100.0,
            center_x_norm: smoothed_x.clamp(0.15, 0.85),
        });
    }

    // Write FFmpeg sendcmd file (`crop_trajectory.cmd`)
    std::fs::create_dir_all(clip_dir)?;
    let cmd_path = clip_dir.join("crop_trajectory.cmd");
    let mut cmd_lines = Vec::new();

    for pt in &trajectory {
        let desired_left = (pt.center_x_norm * src_w as f32) - (crop_w as f32 * 0.5);
        let px_x = (desired_left.round().clamp(0.0, max_crop_x) as u32) & !1; // Even pixel alignment for YUV420p
        cmd_lines.push(format!(
            "{:.2}-{:.2} [enter] crop x {};",
            pt.start_sec, pt.end_sec, px_x
        ));
    }

    let sendcmd_path = if !cmd_lines.is_empty() {
        std::fs::write(&cmd_path, cmd_lines.join("\n"))?;
        Some(cmd_path.to_string_lossy().to_string())
    } else {
        None
    };

    Ok(SmartCropPlan {
        effective_layout: "smart_speaker".to_string(),
        source_width: src_w,
        source_height: src_h,
        left_speaker_x_norm: left_avg,
        right_speaker_x_norm: right_avg,
        trajectory,
        sendcmd_path,
    })
}

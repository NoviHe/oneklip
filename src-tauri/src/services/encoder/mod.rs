use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderInfo {
    pub codec: String,
    pub label: String,
    pub is_hardware: bool,
}

static DETECTED_ENCODER: OnceLock<EncoderInfo> = OnceLock::new();

fn test_encoder(ffmpeg_path: &Path, codec: &str) -> bool {
    let mut cmd = Command::new(ffmpeg_path);
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=256x256:d=0.1",
        "-frames:v",
        "1",
        "-c:v",
        codec,
        "-f",
        "null",
        "-",
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

pub fn detect_best_encoder(ffmpeg_path: &Path) -> EncoderInfo {
    DETECTED_ENCODER
        .get_or_init(|| {
            if ffmpeg_path.exists() {
                let candidates = [
                    ("h264_nvenc", "NVIDIA NVENC (Hardware GPU)"),
                    ("h264_amf", "AMD AMF (Hardware GPU)"),
                    ("h264_qsv", "Intel QuickSync (Hardware GPU)"),
                ];
                for (codec, label) in candidates {
                    if test_encoder(ffmpeg_path, codec) {
                        return EncoderInfo {
                            codec: codec.to_string(),
                            label: label.to_string(),
                            is_hardware: true,
                        };
                    }
                }
            }
            EncoderInfo {
                codec: "libx264".to_string(),
                label: "CPU x264 (Software)".to_string(),
                is_hardware: false,
            }
        })
        .clone()
}

pub fn encoder_ffmpeg_args(codec: &str, crf_or_cq: u8) -> Vec<String> {
    match codec {
        "h264_nvenc" => vec![
            "-c:v".into(),
            "h264_nvenc".into(),
            "-preset".into(),
            "p4".into(),
            "-rc".into(),
            "vbr".into(),
            "-cq".into(),
            crf_or_cq.to_string(),
            "-b:v".into(),
            "0".into(),
        ],
        "h264_amf" => vec![
            "-c:v".into(),
            "h264_amf".into(),
            "-quality".into(),
            "balanced".into(),
            "-rc".into(),
            "cqp".into(),
            "-qp_i".into(),
            crf_or_cq.to_string(),
            "-qp_p".into(),
            crf_or_cq.to_string(),
        ],
        "h264_qsv" => vec![
            "-c:v".into(),
            "h264_qsv".into(),
            "-preset".into(),
            "medium".into(),
            "-global_quality".into(),
            crf_or_cq.to_string(),
        ],
        _ => vec![
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "fast".into(),
            "-crf".into(),
            crf_or_cq.to_string(),
        ],
    }
}

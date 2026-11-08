use crate::error::AppResult;
use crate::state::AppState;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Extracts normalized audio waveform peaks (0.0..=1.0) using FFmpeg raw s16le pipe.
pub fn extract_waveform_peaks(
    state: &AppState,
    media_path: &Path,
    num_buckets: usize,
) -> AppResult<Vec<f32>> {
    let buckets = num_buckets.clamp(32, 2000);
    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args(["-v", "error", "-i"])
        .arg(media_path)
        .args(["-vn", "-ac", "1", "-ar", "4000", "-f", "s16le", "-"]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd.output()?;
    if !out.status.success() || out.stdout.len() < 4 {
        return Ok(vec![0.15; buckets]);
    }

    let samples: Vec<i16> = out
        .stdout
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();

    if samples.is_empty() {
        return Ok(vec![0.15; buckets]);
    }

    let chunk_size = (samples.len() / buckets).max(1);
    let mut peaks: Vec<f32> = Vec::with_capacity(buckets);
    let mut max_peak = 0.01_f32;

    for chunk in samples.chunks(chunk_size).take(buckets) {
        let sum_sq: f64 = chunk
            .iter()
            .map(|&s| {
                let v = s as f64 / 32768.0;
                v * v
            })
            .sum();
        let rms = (sum_sq / chunk.len() as f64).sqrt() as f32;
        if rms > max_peak {
            max_peak = rms;
        }
        peaks.push(rms);
    }

    for p in &mut peaks {
        *p = (*p / max_peak).clamp(0.03, 1.0);
    }

    while peaks.len() < buckets {
        peaks.push(0.05);
    }

    Ok(peaks)
}

/// Detects 1-second timestamps where audio RMS energy exceeds mean + 1.65 * std_dev.
/// Used to inject `[PEAK_AUDIO]` markers into the AI Viral Moment prompt.
pub fn detect_audio_energy_spikes(state: &AppState, media_path: &Path) -> Vec<f64> {
    if !media_path.exists() || !state.ffmpeg_path().exists() {
        return Vec::new();
    }

    let mut cmd = Command::new(state.ffmpeg_path());
    cmd.args(["-v", "error", "-i"])
        .arg(media_path)
        .args(["-vn", "-ac", "1", "-ar", "2000", "-f", "s16le", "-"]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = match cmd.output() {
        Ok(o) if o.status.success() && o.stdout.len() >= 4000 => o,
        _ => return Vec::new(),
    };

    // 2000 samples per second
    let samples: Vec<i16> = out
        .stdout
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();

    let per_sec: Vec<f64> = samples
        .chunks(2000)
        .map(|sec_samples| {
            let sum_sq: f64 = sec_samples
                .iter()
                .map(|&s| {
                    let v = s as f64 / 32768.0;
                    v * v
                })
                .sum();
            (sum_sq / sec_samples.len().max(1) as f64).sqrt()
        })
        .collect();

    if per_sec.len() < 5 {
        return Vec::new();
    }

    let mean = per_sec.iter().sum::<f64>() / per_sec.len() as f64;
    let variance = per_sec
        .iter()
        .map(|&x| (x - mean) * (x - mean))
        .sum::<f64>()
        / per_sec.len() as f64;
    let std_dev = variance.sqrt();
    let threshold = mean + 1.65 * std_dev;

    let mut spikes = Vec::new();
    let mut last_spike = -10.0_f64;

    for (sec_idx, &rms) in per_sec.iter().enumerate() {
        let ts = sec_idx as f64;
        if rms >= threshold && (ts - last_spike) >= 4.0 {
            spikes.push(ts);
            last_spike = ts;
        }
    }

    spikes
}

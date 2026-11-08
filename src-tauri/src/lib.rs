pub mod commands;
pub mod error;
pub mod services;
pub mod state;
pub mod storage;

use crate::services::binaries::bootstrap_local_sidecars;
use crate::state::AppState;
use std::path::PathBuf;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                std::env::var("APPDATA")
                    .map(|p| PathBuf::from(p).join("com.oneklip.desktop"))
                    .unwrap_or_else(|_| PathBuf::from("./oneklip_data"))
            });
            let resource_dir = app.path().resource_dir().ok();

            let state = AppState::new(app_data_dir, resource_dir)
                .expect("Failed to initialize OneKlip AppState");

            bootstrap_local_sidecars(&state);
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_system_diagnostics,
            commands::install_missing_binaries,
            commands::get_ai_config,
            commands::save_ai_config,
            commands::save_youtube_cookies,
            commands::probe_video_source,
            commands::start_clipper_session,
            commands::cancel_clipper_session,
            commands::list_sessions,
            commands::get_session_detail,
            commands::delete_session,
            commands::get_clip_editor_data,
            commands::rerender_clip,
            commands::get_subtitle_presets,
            commands::export_clip_to_path,
            commands::open_path_in_explorer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OneKlip Tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai_viral::{detect_viral_moments, AiConfig};
    use crate::services::binaries::check_binaries;
    use crate::services::encoder::detect_best_encoder;
    use crate::services::innertube::{Transcript, TranscriptSegment, TranscriptWord};
    use crate::services::render::hook_overlay::render_hook_png;
    use crate::services::render::hook_style::HookStyle;
    use crate::services::render::subtitle_style::SubtitleStyle;
    use crate::services::subtitles::generate_ass_content;

    #[tokio::test]
    async fn test_end_to_end_core_subsystems() {
        let tmp_dir = std::env::temp_dir().join("oneklip_test_appdata");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let state = AppState::new(tmp_dir.clone(), None).expect("AppState init");
        let bin_status = check_binaries(&state);
        println!("Binaries all_ready: {}", bin_status.all_ready);
        assert!(
            bin_status.ffmpeg.installed,
            "FFmpeg should be bootstrapped from local cache"
        );
        assert!(
            bin_status.whisper_model.installed,
            "Whisper Large-v3-Turbo model should be bootstrapped"
        );
        assert!(
            bin_status.yunet_model.installed,
            "YuNet ONNX model should be bootstrapped"
        );

        let enc = detect_best_encoder(&state.ffmpeg_path());
        println!("Detected encoder: {} ({})", enc.codec, enc.label);
        assert!(!enc.codec.is_empty());

        // Test Hook Overlay PNG generation (1080x1920)
        let hook_out = tmp_dir.join("test_hook.png");
        let hook_style = HookStyle::from_preset("punchline");
        render_hook_png(
            &state.fonts_dir,
            "RAHASIA BISNIS YANG JARANG DIBAHAS!",
            Some("FAKTA PAHIT"),
            Some("PODCAST HIGHLIGHT"),
            &hook_style,
            &hook_out,
        )
        .expect("render_hook_png should succeed");
        assert!(hook_out.exists());

        // Test Animated .ASS Subtitle Generation
        let sample_words = vec![
            TranscriptWord {
                word: "Ternyata".into(),
                start: 0.0,
                end: 0.45,
                probability: Some(0.99),
            },
            TranscriptWord {
                word: "rahasia".into(),
                start: 0.45,
                end: 0.90,
                probability: Some(0.98),
            },
            TranscriptWord {
                word: "sukses".into(),
                start: 0.90,
                end: 1.40,
                probability: Some(0.99),
            },
        ];
        let sub_style = SubtitleStyle::from_preset("hormozi_pop");
        let ass = generate_ass_content(&sample_words, &sub_style);
        assert!(ass.contains("PlayResX: 1080"));
        assert!(ass.contains("PlayResY: 1920"));
        assert!(ass.contains("TERNYATA"));

        // Test Standalone AI Viral Engine (Heuristic Offline Mode)
        let mut segments = Vec::new();
        for i in 0..20 {
            let s = i as f64 * 4.0;
            let e = s + 3.8;
            segments.push(TranscriptSegment {
                id: i,
                start: s,
                end: e,
                text: format!(
                    "Ternyata kenapa banyak orang gagal di bagian ke {} ini adalah rahasia penting!",
                    i + 1
                ),
                words: vec![
                    TranscriptWord {
                        word: "Ternyata".into(),
                        start: s,
                        end: s + 1.0,
                        probability: None,
                    },
                    TranscriptWord {
                        word: "penting!".into(),
                        start: s + 1.0,
                        end: e,
                        probability: None,
                    },
                ],
            });
        }
        let transcript = Transcript {
            language: "id".into(),
            source_tier: "tier0_innertube".into(),
            duration_seconds: 80.0,
            segments,
        };
        let mut cfg = AiConfig::default();
        cfg.provider = "heuristic".into();
        let highlights = detect_viral_moments(&transcript, &[12.0, 20.0], "podcast", &cfg)
            .await
            .expect("detect_viral_moments");
        assert!(!highlights.is_empty(), "Should detect at least 1 viral clip");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}

use crate::storage::Db;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub app_data_dir: PathBuf,
    pub bin_dir: PathBuf,
    pub models_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub secrets_dir: PathBuf,
    pub fonts_dir: PathBuf,
    pub vcruntime_dir: PathBuf,
    pub cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl AppState {
    pub fn new(app_data_dir: PathBuf, resource_dir: Option<PathBuf>) -> anyhow::Result<Self> {
        let bin_dir = app_data_dir.join("bin");
        let models_dir = app_data_dir.join("models");
        let sessions_dir = app_data_dir.join("sessions");
        let cache_dir = app_data_dir.join("cache");
        let secrets_dir = app_data_dir.join("secrets");

        for d in [
            &app_data_dir,
            &bin_dir,
            &models_dir,
            &sessions_dir,
            &cache_dir,
            &cache_dir.join("downloads"),
            &secrets_dir,
        ] {
            std::fs::create_dir_all(d)?;
        }

        let dev_res = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        let base_res = resource_dir
            .filter(|p| p.join("resources").exists() || p.join("fonts").exists())
            .unwrap_or(dev_res);

        let fonts_dir = if base_res.join("resources").join("fonts").exists() {
            base_res.join("resources").join("fonts")
        } else {
            base_res.join("fonts")
        };

        let vcruntime_dir = if base_res.join("resources").join("vcruntime").exists() {
            base_res.join("resources").join("vcruntime")
        } else {
            base_res.join("vcruntime")
        };

        let db = Db::open(&app_data_dir.join("oneklip.sqlite"))?;

        Ok(Self {
            db,
            app_data_dir,
            bin_dir,
            models_dir,
            sessions_dir,
            cache_dir,
            secrets_dir,
            fonts_dir,
            vcruntime_dir,
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn register_cancel_token(&self, id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flags.lock().insert(id.to_string(), flag.clone());
        flag
    }

    pub fn cancel_task(&self, id: &str) {
        if let Some(flag) = self.cancel_flags.lock().get(id) {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    pub fn ffmpeg_path(&self) -> PathBuf {
        self.bin_dir.join("ffmpeg.exe")
    }

    pub fn ffprobe_path(&self) -> PathBuf {
        self.bin_dir.join("ffprobe.exe")
    }

    pub fn ytdlp_path(&self) -> PathBuf {
        self.bin_dir.join("yt-dlp.exe")
    }

    pub fn deno_path(&self) -> PathBuf {
        self.bin_dir.join("deno.exe")
    }

    pub fn whisper_cli_path(&self) -> PathBuf {
        self.bin_dir.join("whisper").join("whisper-cli.exe")
    }

    pub fn onnxruntime_dll_path(&self) -> PathBuf {
        self.bin_dir.join("onnxruntime.dll")
    }

    pub fn whisper_model_path(&self) -> PathBuf {
        self.models_dir.join("ggml-large-v3-turbo-q5_0.bin")
    }

    pub fn yunet_model_path(&self) -> PathBuf {
        self.models_dir.join("face_detection_yunet_2023mar.onnx")
    }

    pub fn silero_vad_model_path(&self) -> PathBuf {
        self.models_dir.join("silero_vad.onnx")
    }

    pub fn cookies_path(&self) -> PathBuf {
        self.secrets_dir.join("cookies.txt")
    }
}

# OneKlip — AI Viral Podcast & Livestream Auto-Clipper

**OneKlip** adalah aplikasi desktop berbasis **Rust (Tauri v2)** + **React 19 + TypeScript + Tailwind CSS** untuk memotong momen berpotensi viral dari video podcast maupun livestream secara otomatis, mengubah rasio 16:9 menjadi vertikal 9:16 (*AI Active Speaker Tracking* & *Split-Screen 2 Pembicara*), serta membakar (*burn-in*) subtitle animasi tingkat kata (`.ass`) dan kartu *Intro Hook* 3 detik pertama.

---

## Fitur Utama

1. **4-Tier Transcript Ladder (Cepat & Tahan Banting)**
   - **Tier 0**: YouTube Innertube API (`ANDROID_VR` / `ANDROID`) untuk mengambil subtitle `json3` ber-timestamp per kata secara instan (< 2 detik).
   - **Tier 1**: Ekstraksi subtitle via `yt-dlp` + `deno` JS runtime.
   - **Tier 3**: Ekstraksi subtitle dengan autentikasi `cookies.txt` Netscape.
   - **Tier 4**: Transkripsi AI lokal menggunakan `whisper-cli.exe` (`ggml-large-v3-turbo-q5_0.bin`) pada audio 16kHz mono.

2. **Standalone AI Viral Moment Engine + Word-Boundary Clamping**
   - Mendukung **Google Gemini (`gemini-2.5-flash`)**, **Groq (`llama-3.3-70b-versatile`)**, **OpenAI (`gpt-4o-mini`)**, **OpenRouter**, **Ollama Lokal**, maupun **Smart Heuristic Offline (Tanpa API Key)**.
   - Menggabungkan analisis transkrip dengan deteksi lonjakan energi audio RMS (`[PEAK_AUDIO]`).
   - **Word-Boundary Clamping**: Menjamin waktu mulai dan selesai klip selalu jatuh tepat di batas awal dan akhir kalimat/kata.

3. **AI Smart Crop 9:16 (ONNX Runtime + YuNet + Active Speaker + Split-Screen)**
   - Deteksi wajah menggunakan model ONNX **YuNet (`face_detection_yunet_2023mar.onnx`)** dan analisis gerakan bibir (*Mouth ROI Motion*) + aktivitas suara (VAD).
   - **Banter Funnel**: Otomatis beralih ke layout **Split-Screen Atas-Bawah (`1080x960` + `1080x960`)** saat 2 pembicara berdialog aktif, atau menggunakan **Virtual Camera Tracking** (`sendcmd` FFmpeg) dengan *30% deadzone* & *EMA smoothing*.

4. **Mesin Subtitle Animasi `.ASS` & Kartu Intro Hook (`ab_glyph`)**
   - 9 preset subtitle animasi bawaan: `hormozi_pop`, `mrbeast_slam`, `karaoke_box`, `neon_cyber`, `podcast_clean`, `comic_punch`, `glow_viral`, `minimal_fade`, dan `retro_gold`.
   - Generator kartu **Intro Hook (`intro_hook.png` 1080x1920)** otomatis di 3 detik pertama video.

5. **Akselerasi Hardware GPU Encoder**
   - Otomatis mendeteksi dan menggunakan encoder GPU tercepat (`h264_nvenc` NVIDIA, `h264_amf` AMD, `h264_qsv` Intel) dengan fallback ke CPU `libx264`.

---

## Cara Menjalankan (Development & Production)

### Prasyarat
- **Rust** (`rustup`, `cargo`)
- **Node.js** (v20+) & `npm`

### Menjalankan Mode Development
```bash
npm install
npm run tauri dev
```

### Membangun Installer Produksi (`.exe` / `.msi`)
```bash
npm run tauri build
```

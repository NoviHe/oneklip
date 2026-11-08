import React, { useEffect, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open, save } from '@tauri-apps/plugin-dialog';
import {
  AlertCircle,
  CheckCircle2,
  Cpu,
  Download,
  Edit3,
  Film,
  FolderOpen,
  History,
  Loader2,
  Play,
  Scissors,
  Settings,
  Sparkles,
  Trash2,
  Upload,
  Wand2,
  Youtube,
} from 'lucide-react';
import { useStudioStore } from './store/useStudioStore';
import {
  AiConfig,
  ClipRecord,
  PipelineProgressPayload,
  SessionRecord,
  VideoMetadata,
} from './types';
import { ClipEditorModal } from './components/ClipEditorModal';

export const App: React.FC = () => {
  const {
    activeTab,
    diagnostics,
    sessions,
    currentSession,
    currentClips,
    pipelineProgress,
    isBusy,
    errorBanner,
    setActiveTab,
    setErrorBanner,
    setPipelineProgress,
    appendOrUpdateClip,
    fetchInitialData,
    refreshSessions,
    openSessionInStudio,
    deleteSessionById,
    startNewPipeline,
    cancelActivePipeline,
    openClipEditor,
    saveAiConfiguration,
    installMissingBinaries,
  } = useStudioStore();

  // Create form state
  const [sourceMode, setSourceMode] = useState<'url' | 'local'>('url');
  const [videoUrl, setVideoUrl] = useState<string>('');
  const [localVideoPath, setLocalVideoPath] = useState<string>('');
  const [probedMeta, setProbedMeta] = useState<VideoMetadata | null>(null);
  const [isProbing, setIsProbing] = useState<boolean>(false);

  const [videoLanguage, setVideoLanguage] = useState<string>('id');
  const [contentType, setContentType] = useState<string>('podcast');
  const [defaultLayout, setDefaultLayout] = useState<string>('auto_banter');
  const [subtitlePreset, setSubtitlePreset] = useState<string>('hormozi_pop');
  const [hookPreset, setHookPreset] = useState<string>('punchline');
  const [maxClips, setMaxClips] = useState<number>(6);

  // Settings form state
  const [aiForm, setAiForm] = useState<AiConfig>({
    provider: 'gemini',
    apiKey: '',
    model: 'gemini-2.5-flash',
    baseUrl: null,
    targetClipMinSecs: 30,
    targetClipMaxSecs: 75,
    maxClips: 8,
  });
  const [cookiesText, setCookiesText] = useState<string>('');
  const [settingsSavedMsg, setSettingsSavedMsg] = useState<string | null>(null);

  useEffect(() => {
    fetchInitialData();

    const unlistenProgress = listen<PipelineProgressPayload>(
      'pipeline:progress',
      (ev) => {
        setPipelineProgress(ev.payload);
      }
    );

    const unlistenClipReady = listen<ClipRecord>('pipeline:clip_ready', (ev) => {
      appendOrUpdateClip(ev.payload);
    });

    const unlistenCompleted = listen<SessionRecord>(
      'pipeline:completed',
      async (ev) => {
        setPipelineProgress(null);
        await refreshSessions();
        await openSessionInStudio(ev.payload.id);
      }
    );

    const unlistenError = listen<{ sessionId: string; error: string }>(
      'pipeline:error',
      (ev) => {
        setPipelineProgress(null);
        setErrorBanner(ev.payload.error);
        refreshSessions();
      }
    );

    return () => {
      unlistenProgress.then((f) => f());
      unlistenClipReady.then((f) => f());
      unlistenCompleted.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (diagnostics?.aiConfig) {
      setAiForm(diagnostics.aiConfig);
    }
  }, [diagnostics]);

  const handlePickLocalVideo = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: 'Video Files',
          extensions: ['mp4', 'mkv', 'mov', 'webm', 'avi'],
        },
      ],
    });
    if (typeof selected === 'string') {
      setLocalVideoPath(selected);
      setIsProbing(true);
      try {
        const meta = await invoke<VideoMetadata>('probe_video_source', {
          videoUrl: null,
          localPath: selected,
        });
        setProbedMeta(meta);
      } catch (e: any) {
        setErrorBanner(e?.message || String(e));
      } finally {
        setIsProbing(false);
      }
    }
  };

  const handleProbeUrl = async () => {
    if (!videoUrl.trim()) return;
    setIsProbing(true);
    setErrorBanner(null);
    try {
      const meta = await invoke<VideoMetadata>('probe_video_source', {
        videoUrl: videoUrl.trim(),
        localPath: null,
      });
      setProbedMeta(meta);
    } catch (e: any) {
      setErrorBanner(e?.message || String(e));
    } finally {
      setIsProbing(false);
    }
  };

  const handleStartPipeline = async () => {
    if (sourceMode === 'url' && !videoUrl.trim()) {
      setErrorBanner('Masukkan URL video YouTube / Podcast terlebih dahulu.');
      return;
    }
    if (sourceMode === 'local' && !localVideoPath.trim()) {
      setErrorBanner('Pilih file video lokal terlebih dahulu.');
      return;
    }

    try {
      await startNewPipeline({
        videoUrl: sourceMode === 'url' ? videoUrl.trim() : null,
        localVideoPath: sourceMode === 'local' ? localVideoPath.trim() : null,
        videoLanguage,
        contentType,
        defaultLayout,
        subtitlePreset,
        hookPreset,
        maxClips,
      });
    } catch (e: any) {
      setErrorBanner(e?.message || String(e));
    }
  };

  const handleQuickExportClip = async (clip: ClipRecord) => {
    const safeTitle = clip.title
      .replace(/[^a-zA-Z0-9-_ ]/g, '')
      .trim()
      .replace(/\s+/g, '_')
      .slice(0, 45);
    const targetPath = await save({
      defaultPath: `${safeTitle || 'oneklip_viral'}.mp4`,
      filters: [{ name: 'MP4 Video', extensions: ['mp4'] }],
    });
    if (targetPath) {
      await invoke('export_clip_to_path', {
        clipId: clip.id,
        destPath: targetPath,
      });
    }
  };

  const formatDuration = (secs?: number | null) => {
    if (!secs || secs <= 0) return '0:00';
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  return (
    <div className="h-screen w-screen flex bg-slate-950 text-slate-100 select-none overflow-hidden">
      {/* Left Sidebar Navigation */}
      <aside className="w-64 bg-slate-900/90 border-r border-slate-800/80 flex flex-col justify-between p-4">
        <div>
          {/* Brand Logo */}
          <div className="flex items-center gap-3 px-2 py-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-violet-600 to-fuchsia-600 flex items-center justify-center shadow-lg shadow-violet-600/30">
              <Scissors className="w-5 h-5 text-white" />
            </div>
            <div>
              <h1 className="text-base font-extrabold tracking-tight text-white flex items-center gap-1.5">
                OneKlip
                <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-violet-500/20 text-violet-300 border border-violet-500/30">
                  PRO
                </span>
              </h1>
              <p className="text-[11px] text-slate-400">
                AI Podcast & Stream Clipper
              </p>
            </div>
          </div>

          {/* Nav Links */}
          <nav className="space-y-1.5">
            {[
              {
                id: 'create',
                label: 'Buat Klip Baru',
                icon: Wand2,
                badge: null,
              },
              {
                id: 'studio',
                label: 'Studio Klip Viral',
                icon: Film,
                badge: currentClips.length > 0 ? currentClips.length : null,
              },
              {
                id: 'sessions',
                label: 'Riwayat Proyek',
                icon: History,
                badge: sessions.length > 0 ? sessions.length : null,
              },
              {
                id: 'settings',
                label: 'AI & Sistem',
                icon: Settings,
                badge: diagnostics?.binaries.allReady ? null : '!',
              },
            ].map((item) => {
              const Icon = item.icon;
              const active = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  onClick={() => setActiveTab(item.id as any)}
                  className={`w-full px-3.5 py-2.5 rounded-xl text-xs font-semibold flex items-center justify-between transition ${
                    active
                      ? 'bg-violet-600 text-white shadow-lg shadow-violet-600/25'
                      : 'text-slate-400 hover:text-slate-100 hover:bg-slate-800/60'
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <Icon className="w-4 h-4" />
                    <span>{item.label}</span>
                  </div>
                  {item.badge !== null && (
                    <span
                      className={`px-2 py-0.5 rounded-full text-[10px] font-bold ${
                        active
                          ? 'bg-white/20 text-white'
                          : 'bg-slate-800 text-slate-300'
                      }`}
                    >
                      {item.badge}
                    </span>
                  )}
                </button>
              );
            })}
          </nav>
        </div>

        {/* Bottom Hardware & Sidecar Status Card */}
        <div className="bg-slate-950/80 border border-slate-800 rounded-xl p-3 space-y-2">
          <div className="flex items-center justify-between text-[11px]">
            <span className="text-slate-400 flex items-center gap-1.5">
              <Cpu className="w-3.5 h-3.5 text-violet-400" /> Encoder
            </span>
            <span className="font-semibold text-emerald-400 truncate max-w-[115px]">
              {diagnostics?.encoder.codec || 'Mendeteksi...'}
            </span>
          </div>
          <div className="flex items-center justify-between text-[11px]">
            <span className="text-slate-400">Engine AI & Binary</span>
            {diagnostics?.binaries.allReady ? (
              <span className="text-emerald-400 font-semibold flex items-center gap-1">
                <CheckCircle2 className="w-3.5 h-3.5" /> Siap 100%
              </span>
            ) : (
              <button
                onClick={() => setActiveTab('settings')}
                className="text-amber-400 underline font-semibold"
              >
                Cek Komponen
              </button>
            )}
          </div>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Global Error Banner */}
        {errorBanner && (
          <div className="bg-rose-500/15 border-b border-rose-500/30 px-6 py-3 flex items-center justify-between text-xs text-rose-200">
            <div className="flex items-center gap-2">
              <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
              <span>{errorBanner}</span>
            </div>
            <button
              onClick={() => setErrorBanner(null)}
              className="text-rose-300 hover:text-white font-semibold"
            >
              Tutup
            </button>
          </div>
        )}

        {/* Active Pipeline Progress Bar */}
        {pipelineProgress && (
          <div className="bg-violet-950/40 border-b border-violet-500/30 px-6 py-3.5 flex items-center justify-between">
            <div className="flex items-center gap-3 flex-1 mr-6">
              <Loader2 className="w-5 h-5 text-violet-400 animate-spin shrink-0" />
              <div className="flex-1">
                <div className="flex items-center justify-between text-xs mb-1">
                  <span className="font-semibold text-violet-200">
                    {pipelineProgress.message}
                  </span>
                  <span className="font-mono text-violet-300">
                    {pipelineProgress.percent.toFixed(0)}%
                  </span>
                </div>
                <div className="w-full h-2 bg-slate-900 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-gradient-to-r from-violet-500 to-fuchsia-500 transition-all duration-300"
                    style={{
                      width: `${Math.min(100, Math.max(3, pipelineProgress.percent))}%`,
                    }}
                  />
                </div>
              </div>
            </div>
            {currentSession && (
              <button
                onClick={() => cancelActivePipeline(currentSession.id)}
                className="px-3 py-1.5 rounded-lg bg-rose-500/20 hover:bg-rose-500/30 text-rose-300 text-xs font-semibold border border-rose-500/30 transition"
              >
                Batalkan
              </button>
            )}
          </div>
        )}

        {/* Tab 1: CREATE NEW AUTO-CLIP SESSION */}
        {activeTab === 'create' && (
          <div className="flex-1 overflow-y-auto p-8">
            <div className="max-w-4xl mx-auto space-y-6">
              <div>
                <h2 className="text-2xl font-extrabold text-white">
                  Buat Klip Viral Otomatis dari Podcast & Livestream
                </h2>
                <p className="text-sm text-slate-400 mt-1">
                  OneKlip mendeteksi momen terbaik, memotong di batas kalimat,
                  melacak wajah pembicara 9:16 (atau Split-Screen 2 pembicara),
                  dan membakar subtitle animasi secara otomatis.
                </p>
              </div>

              {/* Source Selector Card */}
              <div className="bg-slate-900/90 border border-slate-800 rounded-2xl p-6 space-y-5">
                <div className="flex gap-2">
                  <button
                    onClick={() => setSourceMode('url')}
                    className={`px-4 py-2 rounded-xl text-xs font-bold flex items-center gap-2 transition ${
                      sourceMode === 'url'
                        ? 'bg-violet-600 text-white'
                        : 'bg-slate-800 text-slate-400 hover:text-white'
                    }`}
                  >
                    <Youtube className="w-4 h-4" /> Link YouTube / Livestream
                  </button>
                  <button
                    onClick={() => setSourceMode('local')}
                    className={`px-4 py-2 rounded-xl text-xs font-bold flex items-center gap-2 transition ${
                      sourceMode === 'local'
                        ? 'bg-violet-600 text-white'
                        : 'bg-slate-800 text-slate-400 hover:text-white'
                    }`}
                  >
                    <Upload className="w-4 h-4" /> File Video Lokal (MP4/MKV)
                  </button>
                </div>

                {sourceMode === 'url' ? (
                  <div className="flex gap-3">
                    <input
                      type="text"
                      value={videoUrl}
                      onChange={(e) => setVideoUrl(e.target.value)}
                      placeholder="Tempel link YouTube Podcast, Shorts, atau Livestream (contoh: https://www.youtube.com/watch?v=...)"
                      className="flex-1 bg-slate-950 border border-slate-800 focus:border-violet-500 rounded-xl px-4 py-3 text-sm text-white focus:outline-none"
                    />
                    <button
                      onClick={handleProbeUrl}
                      disabled={isProbing || !videoUrl.trim()}
                      className="px-5 py-3 rounded-xl bg-slate-800 hover:bg-slate-700 disabled:opacity-50 text-xs font-bold text-white flex items-center gap-2 transition"
                    >
                      {isProbing ? (
                        <Loader2 className="w-4 h-4 animate-spin" />
                      ) : (
                        <Sparkles className="w-4 h-4 text-violet-400" />
                      )}
                      Cek Info Video
                    </button>
                  </div>
                ) : (
                  <div className="flex items-center gap-3">
                    <div className="flex-1 bg-slate-950 border border-slate-800 rounded-xl px-4 py-3 text-sm text-slate-300 truncate">
                      {localVideoPath ||
                        'Belum ada file video lokal yang dipilih...'}
                    </div>
                    <button
                      onClick={handlePickLocalVideo}
                      className="px-5 py-3 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-bold text-white flex items-center gap-2 transition"
                    >
                      <FolderOpen className="w-4 h-4 text-violet-400" /> Pilih
                      File Video
                    </button>
                  </div>
                )}

                {/* Probed Metadata Preview */}
                {probedMeta && (
                  <div className="flex items-center gap-4 bg-slate-950/90 border border-slate-800 rounded-xl p-3.5">
                    {probedMeta.thumbnailUrl && (
                      <img
                        src={probedMeta.thumbnailUrl}
                        alt={probedMeta.title}
                        className="w-28 h-16 object-cover rounded-lg border border-slate-800"
                      />
                    )}
                    <div className="flex-1 min-w-0">
                      <h4 className="text-sm font-bold text-white truncate">
                        {probedMeta.title}
                      </h4>
                      <p className="text-xs text-slate-400 mt-0.5">
                        Channel: {probedMeta.channel} • Durasi:{' '}
                        {formatDuration(probedMeta.durationSeconds)}
                      </p>
                    </div>
                  </div>
                )}
              </div>

              {/* Pipeline Configuration Grid */}
              <div className="grid grid-cols-2 gap-4 bg-slate-900/90 border border-slate-800 rounded-2xl p-6">
                <div>
                  <label className="text-xs font-semibold text-slate-300 block mb-1.5">
                    Bahasa Percakapan Video
                  </label>
                  <select
                    value={videoLanguage}
                    onChange={(e) => setVideoLanguage(e.target.value)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-xs text-white"
                  >
                    <option value="id">Bahasa Indonesia (id)</option>
                    <option value="en">English (en)</option>
                    <option value="auto">Auto-Detect (Whisper)</option>
                  </select>
                </div>

                <div>
                  <label className="text-xs font-semibold text-slate-300 block mb-1.5">
                    Jenis Konten (Optimasi Prompt AI)
                  </label>
                  <select
                    value={contentType}
                    onChange={(e) => setContentType(e.target.value)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-xs text-white"
                  >
                    <option value="podcast">
                      Podcast / Talkshow / Wawancara
                    </option>
                    <option value="gaming">
                      Livestream Gaming / Reaksi Streamer
                    </option>
                    <option value="education">
                      Edukasi / Bisnis / Finansial
                    </option>
                    <option value="general">Umum / Entertainment</option>
                  </select>
                </div>

                <div>
                  <label className="text-xs font-semibold text-slate-300 block mb-1.5">
                    Mode Reframing Vertikal 9:16
                  </label>
                  <select
                    value={defaultLayout}
                    onChange={(e) => setDefaultLayout(e.target.value)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-xs text-white"
                  >
                    <option value="auto_banter">
                      Otomatis Pintar (Speaker Track + Split-Screen jika 2
                      Pembicara)
                    </option>
                    <option value="smart_speaker">
                      AI Smart Speaker Tracking (ONNX YuNet)
                    </option>
                    <option value="split_screen">
                      Split-Screen 2 Pembicara (Atas & Bawah)
                    </option>
                    <option value="fit_blur">
                      Fit 16:9 Utuh + Background Blur
                    </option>
                    <option value="fill">Center Crop 9:16 Statis</option>
                  </select>
                </div>

                <div>
                  <label className="text-xs font-semibold text-slate-300 block mb-1.5">
                    Preset Animasi Subtitle (.ASS)
                  </label>
                  <select
                    value={subtitlePreset}
                    onChange={(e) => setSubtitlePreset(e.target.value)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-xs text-white"
                  >
                    <option value="hormozi_pop">
                      Hormozi Pop (Kuning-Putih Tebal)
                    </option>
                    <option value="mrbeast_slam">
                      MrBeast Slam (Per Kata Besar)
                    </option>
                    <option value="karaoke_box">
                      Karaoke Word Box (Kotak Ungu)
                    </option>
                    <option value="neon_cyber">Neon Cyber (Cyan Glow)</option>
                    <option value="podcast_clean">
                      Podcast Clean (Inter Modern)
                    </option>
                    <option value="comic_punch">
                      Comic Punch (Lilita Orange)
                    </option>
                    <option value="glow_viral">Glow Viral (Rose Sweep)</option>
                    <option value="retro_gold">Retro Gold</option>
                  </select>
                </div>

                <div>
                  <label className="text-xs font-semibold text-slate-300 block mb-1.5">
                    Gaya Intro Hook Banner (3 Detik Pertama)
                  </label>
                  <select
                    value={hookPreset}
                    onChange={(e) => setHookPreset(e.target.value)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-xs text-white"
                  >
                    <option value="punchline">
                      Punchline Card (Putih + Badge Rose)
                    </option>
                    <option value="comic">
                      Comic Yellow (Kuning + Badge Pink)
                    </option>
                    <option value="sticker">
                      Dark Sticker (Navy + Badge Cyan)
                    </option>
                    <option value="classic">Classic News Banner</option>
                  </select>
                </div>

                <div>
                  <label className="text-xs font-semibold text-slate-300 block mb-1.5">
                    Jumlah Klip Viral Maksimal ({maxClips} Klip)
                  </label>
                  <input
                    type="range"
                    min={1}
                    max={15}
                    value={maxClips}
                    onChange={(e) => setMaxClips(parseInt(e.target.value, 10))}
                    className="w-full accent-violet-500 mt-2"
                  />
                </div>
              </div>

              {/* Launch Button */}
              <button
                onClick={handleStartPipeline}
                disabled={isBusy}
                className="w-full py-4 rounded-2xl bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500 disabled:opacity-50 text-white font-extrabold text-sm shadow-xl shadow-violet-600/30 flex items-center justify-center gap-2.5 transition"
              >
                <Wand2 className="w-5 h-5" />
                {isBusy
                  ? 'Sedang Memproses Video...'
                  : ' Analisis Momen Viral & Buat Klip Otomatis Sekarang'}
              </button>
            </div>
          </div>
        )}

        {/* Tab 2: STUDIO KLIP VIRAL */}
        {activeTab === 'studio' && (
          <div className="flex-1 overflow-y-auto p-6 space-y-6">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-xl font-extrabold text-white">
                  {currentSession?.videoTitle || 'Studio Klip Viral'}
                </h2>
                <p className="text-xs text-slate-400 mt-0.5">
                  {currentSession
                    ? `Status: ${currentSession.status.toUpperCase()} • ${currentClips.length} Klip Selesai Dirender`
                    : 'Pilih sesi dari Riwayat Proyek atau buat klip baru'}
                </p>
              </div>
              {currentSession && (
                <button
                  onClick={() =>
                    invoke('open_path_in_explorer', {
                      path: currentSession.folderPath,
                    })
                  }
                  className="px-3.5 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-semibold text-slate-200 flex items-center gap-2 transition"
                >
                  <FolderOpen className="w-4 h-4 text-violet-400" /> Buka Folder
                  Sesi
                </button>
              )}
            </div>

            {currentClips.length === 0 ? (
              <div className="h-96 border border-dashed border-slate-800 rounded-2xl flex flex-col items-center justify-center text-center p-8">
                <Film className="w-12 h-12 text-slate-600 mb-3" />
                <h3 className="text-sm font-bold text-slate-300">
                  Belum Ada Klip di Sesi Ini
                </h3>
                <p className="text-xs text-slate-500 max-w-md mt-1">
                  Jika proses sedang berjalan, klip akan otomatis muncul di sini
                  satu per satu begitu selesai dirender oleh FFmpeg.
                </p>
              </div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-5">
                {currentClips.map((clip) => {
                  const videoUrlSrc = convertFileSrc(clip.filePath);
                  return (
                    <div
                      key={clip.id}
                      className="bg-slate-900/90 border border-slate-800 hover:border-violet-500/50 rounded-2xl overflow-hidden flex flex-col justify-between transition shadow-lg"
                    >
                      <div>
                        {/* 9:16 Video Player Card Top */}
                        <div className="relative aspect-[9/16] bg-black overflow-hidden">
                          <video
                            src={videoUrlSrc}
                            controls
                            preload="metadata"
                            className="w-full h-full object-contain"
                          />
                          <div className="absolute top-3 left-3 flex items-center gap-1.5 pointer-events-none">
                            <span className="px-2.5 py-1 rounded-full text-[11px] font-extrabold bg-emerald-500 text-slate-950 shadow">
                              ⚡ {clip.viralityScore}
                            </span>
                            <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-slate-900/90 text-slate-200 border border-slate-700 uppercase">
                              {clip.category}
                            </span>
                          </div>
                          <div className="absolute top-3 right-3 pointer-events-none">
                            <span className="px-2 py-0.5 rounded-md text-[10px] font-mono bg-black/80 text-white">
                              {clip.durationSeconds.toFixed(0)}s
                            </span>
                          </div>
                        </div>

                        {/* Metadata & Description */}
                        <div className="p-4">
                          <h4 className="text-sm font-bold text-white line-clamp-2">
                            {clip.title}
                          </h4>
                          {clip.hookText && (
                            <p className="text-[11px] font-semibold text-violet-400 mt-1 truncate">
                              Hook: &ldquo;{clip.hookText}&rdquo;
                            </p>
                          )}
                          <p className="text-[11px] text-slate-400 mt-1.5 line-clamp-2">
                            {clip.description}
                          </p>
                        </div>
                      </div>

                      {/* Card Footer Actions */}
                      <div className="px-4 pb-4 pt-2 border-t border-slate-800/70 flex items-center gap-2">
                        <button
                          onClick={() => openClipEditor(clip.id)}
                          className="flex-1 py-2 rounded-xl bg-violet-600 hover:bg-violet-500 text-white text-xs font-bold flex items-center justify-center gap-1.5 transition"
                        >
                          <Edit3 className="w-3.5 h-3.5" /> Edit Subtitle &
                          Layout
                        </button>
                        <button
                          onClick={() => handleQuickExportClip(clip)}
                          title="Simpan / Ekspor MP4"
                          className="p-2 rounded-xl bg-slate-800 hover:bg-emerald-600 text-slate-200 hover:text-white transition"
                        >
                          <Download className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}

        {/* Tab 3: RIWAYAT PROYEK */}
        {activeTab === 'sessions' && (
          <div className="flex-1 overflow-y-auto p-8">
            <div className="max-w-5xl mx-auto space-y-4">
              <h2 className="text-xl font-extrabold text-white">
                Riwayat Sesi & Proyek Auto-Clip
              </h2>

              {sessions.length === 0 ? (
                <div className="p-12 bg-slate-900/60 border border-slate-800 rounded-2xl text-center text-sm text-slate-400">
                  Belum ada riwayat proyek. Mulai dari menu &ldquo;Buat Klip
                  Baru&rdquo;.
                </div>
              ) : (
                <div className="space-y-3">
                  {sessions.map((s) => (
                    <div
                      key={s.id}
                      className="bg-slate-900/90 border border-slate-800 rounded-2xl p-4 flex items-center justify-between hover:border-slate-700 transition"
                    >
                      <div className="min-w-0 pr-4">
                        <div className="flex items-center gap-2.5">
                          <span
                            className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase ${
                              s.status === 'completed'
                                ? 'bg-emerald-500/20 text-emerald-400'
                                : s.status === 'processing'
                                ? 'bg-amber-500/20 text-amber-400'
                                : 'bg-rose-500/20 text-rose-400'
                            }`}
                          >
                            {s.status}
                          </span>
                          <h4 className="text-sm font-bold text-white truncate">
                            {s.videoTitle || s.videoUrl || s.localVideoPath}
                          </h4>
                        </div>
                        <p className="text-xs text-slate-400 mt-1">
                          {s.clipsProcessed}/{s.highlightsCount} Klip • Bahasa:{' '}
                          {s.videoLanguage.toUpperCase()} • Kategori:{' '}
                          {s.contentType}
                        </p>
                        {s.lastError && (
                          <p className="text-xs text-rose-400 mt-1">
                            Error: {s.lastError}
                          </p>
                        )}
                      </div>

                      <div className="flex items-center gap-2 shrink-0">
                        <button
                          onClick={() => openSessionInStudio(s.id)}
                          className="px-4 py-2 rounded-xl bg-violet-600 hover:bg-violet-500 text-white text-xs font-bold flex items-center gap-1.5 transition"
                        >
                          <Play className="w-3.5 h-3.5" /> Buka di Studio
                        </button>
                        <button
                          onClick={() => deleteSessionById(s.id)}
                          className="p-2 rounded-xl bg-slate-800 hover:bg-rose-500/20 text-slate-400 hover:text-rose-400 transition"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Tab 4: PENGATURAN AI & DIAGNOSTIK SISTEM */}
        {activeTab === 'settings' && (
          <div className="flex-1 overflow-y-auto p-8">
            <div className="max-w-4xl mx-auto space-y-6">
              <div>
                <h2 className="text-xl font-extrabold text-white">
                  Pengaturan AI Viral Engine & Diagnostik Sistem
                </h2>
                <p className="text-xs text-slate-400 mt-1">
                  OneKlip berjalan 100% mandiri di komputer Anda. Gunakan API
                  LLM pilihan Anda (Gemini / Groq / OpenAI / Ollama) atau mode
                  Smart Heuristic Offline tanpa API key.
                </p>
              </div>

              {/* AI Provider Card */}
              <div className="bg-slate-900/90 border border-slate-800 rounded-2xl p-6 space-y-4">
                <h3 className="text-sm font-bold text-white">
                  Konfigurasi Otak Pencari Momen Viral (LLM / Heuristik)
                </h3>

                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="text-xs text-slate-400 block mb-1">
                      Provider AI
                    </label>
                    <select
                      value={aiForm.provider}
                      onChange={(e) => {
                        const prov = e.target.value as AiConfig['provider'];
                        const defaultModel =
                          prov === 'gemini'
                            ? 'gemini-2.5-flash'
                            : prov === 'groq'
                            ? 'llama-3.3-70b-versatile'
                            : prov === 'ollama'
                            ? 'llama3.1:8b'
                            : 'gpt-4o-mini';
                        setAiForm({
                          ...aiForm,
                          provider: prov,
                          model: defaultModel,
                        });
                      }}
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-white"
                    >
                      <option value="gemini">
                        Google Gemini API (Direkomendasikan - Cepat & Gratis
                        Tier)
                      </option>
                      <option value="groq">
                        Groq Cloud (Llama 3.3 70B Ultra-Fast)
                      </option>
                      <option value="openai">OpenAI (GPT-4o-mini / GPT-4o)</option>
                      <option value="openrouter">OpenRouter API</option>
                      <option value="ollama">
                        Ollama Lokal (100% Offline Tanpa API Key)
                      </option>
                      <option value="heuristic">
                        Smart Audio & Discourse Heuristic (Tanpa API Key)
                      </option>
                    </select>
                  </div>

                  <div>
                    <label className="text-xs text-slate-400 block mb-1">
                      Nama Model
                    </label>
                    <input
                      type="text"
                      value={aiForm.model}
                      onChange={(e) =>
                        setAiForm({ ...aiForm, model: e.target.value })
                      }
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-white"
                    />
                  </div>

                  {aiForm.provider !== 'heuristic' &&
                    aiForm.provider !== 'ollama' && (
                      <div className="col-span-2">
                        <label className="text-xs text-slate-400 block mb-1">
                          API Key ({aiForm.provider.toUpperCase()}) — Opsional
                          (otomatis fallback ke Smart Heuristic jika dikosongkan)
                        </label>
                        <input
                          type="password"
                          value={aiForm.apiKey}
                          onChange={(e) =>
                            setAiForm({ ...aiForm, apiKey: e.target.value })
                          }
                          placeholder="Masukkan API Key..."
                          className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-white"
                        />
                      </div>
                    )}

                  <div>
                    <label className="text-xs text-slate-400 block mb-1">
                      Durasi Klip Minimum (Detik)
                    </label>
                    <input
                      type="number"
                      value={aiForm.targetClipMinSecs}
                      onChange={(e) =>
                        setAiForm({
                          ...aiForm,
                          targetClipMinSecs: parseFloat(e.target.value) || 25,
                        })
                      }
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-white"
                    />
                  </div>

                  <div>
                    <label className="text-xs text-slate-400 block mb-1">
                      Durasi Klip Maksimum (Detik)
                    </label>
                    <input
                      type="number"
                      value={aiForm.targetClipMaxSecs}
                      onChange={(e) =>
                        setAiForm({
                          ...aiForm,
                          targetClipMaxSecs: parseFloat(e.target.value) || 75,
                        })
                      }
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-white"
                    />
                  </div>
                </div>

                <div className="flex items-center justify-between pt-2">
                  {settingsSavedMsg ? (
                    <span className="text-xs text-emerald-400 font-semibold">
                      ✓ {settingsSavedMsg}
                    </span>
                  ) : (
                    <span />
                  )}
                  <button
                    onClick={async () => {
                      await saveAiConfiguration(aiForm);
                      setSettingsSavedMsg('Pengaturan AI berhasil disimpan!');
                      setTimeout(() => setSettingsSavedMsg(null), 3000);
                    }}
                    className="px-5 py-2.5 rounded-xl bg-violet-600 hover:bg-violet-500 text-white text-xs font-bold transition"
                  >
                    Simpan Konfigurasi AI
                  </button>
                </div>
              </div>

              {/* Sidecar Binaries & AI Models Status */}
              {diagnostics && (
                <div className="bg-slate-900/90 border border-slate-800 rounded-2xl p-6 space-y-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-sm font-bold text-white">
                        Status Binary Sidecar & Model AI Lokal
                      </h3>
                      <p className="text-xs text-slate-400">
                        CPU: {diagnostics.cpuBrand} ({diagnostics.cpuCores}{' '}
                        Threads) • RAM: {diagnostics.totalRamGb.toFixed(1)} GB •
                        GPU Encoder: {diagnostics.encoder.label}
                      </p>
                    </div>
                    <button
                      onClick={installMissingBinaries}
                      disabled={isBusy}
                      className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-bold text-white flex items-center gap-2 transition"
                    >
                      <Download className="w-3.5 h-3.5 text-violet-400" />
                      Sinkronkan / Unduh Komponen
                    </button>
                  </div>

                  <div className="grid grid-cols-3 gap-2.5">
                    {[
                      diagnostics.binaries.ffmpeg,
                      diagnostics.binaries.ffprobe,
                      diagnostics.binaries.ytdlp,
                      diagnostics.binaries.deno,
                      diagnostics.binaries.whisperCli,
                      diagnostics.binaries.onnxruntime,
                      diagnostics.binaries.whisperModel,
                      diagnostics.binaries.yunetModel,
                      diagnostics.binaries.sileroVadModel,
                    ].map((comp) => (
                      <div
                        key={comp.name}
                        className="bg-slate-950/70 border border-slate-800 rounded-xl p-3 flex items-center justify-between"
                      >
                        <div className="min-w-0">
                          <div className="text-xs font-bold text-white truncate">
                            {comp.name}
                          </div>
                          <div className="text-[10px] text-slate-400">
                            {(comp.sizeBytes / (1024 * 1024)).toFixed(1)} MB
                          </div>
                        </div>
                        <span
                          className={`px-2 py-0.5 rounded text-[10px] font-bold ${
                            comp.installed
                              ? 'bg-emerald-500/20 text-emerald-400'
                              : 'bg-rose-500/20 text-rose-400'
                          }`}
                        >
                          {comp.installed ? 'READY' : 'MISSING'}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* YouTube Cookies.txt Card */}
              <div className="bg-slate-900/90 border border-slate-800 rounded-2xl p-6 space-y-3">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-sm font-bold text-white">
                      YouTube Cookies (cookies.txt Netscape Format)
                    </h3>
                    <p className="text-xs text-slate-400">
                      Digunakan pada Tier 3 jika video YouTube membatasi usia
                      atau membutuhkan login
                    </p>
                  </div>
                  <span
                    className={`text-xs font-bold ${
                      diagnostics?.binaries.cookiesPresent
                        ? 'text-emerald-400'
                        : 'text-slate-500'
                    }`}
                  >
                    {diagnostics?.binaries.cookiesPresent
                      ? '✓ cookies.txt Aktif'
                      : 'Belum diatur'}
                  </span>
                </div>
                <textarea
                  rows={3}
                  value={cookiesText}
                  onChange={(e) => setCookiesText(e.target.value)}
                  placeholder="# Netscape HTTP Cookie File..."
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl p-3 text-xs font-mono text-slate-300"
                />
                <div className="flex justify-end">
                  <button
                    onClick={async () => {
                      if (!cookiesText.trim()) return;
                      await invoke('save_youtube_cookies', {
                        cookiesText: cookiesText.trim(),
                      });
                      await fetchInitialData();
                      setCookiesText('');
                      setSettingsSavedMsg('cookies.txt berhasil disimpan!');
                    }}
                    className="px-4 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-xs font-bold text-white transition"
                  >
                    Simpan cookies.txt
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </main>

      {/* Interactive Clip & Subtitle Editor Modal */}
      <ClipEditorModal />
    </div>
  );
};

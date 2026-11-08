import React, { useEffect, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import {
  Check,
  Download,
  FolderOpen,
  Layers,
  Play,
  RefreshCw,
  Sparkles,
  Subtitles,
  Type,
  Video,
  X,
} from 'lucide-react';
import { useStudioStore } from '../store/useStudioStore';
import { ClipRenderSpec, SubtitleStyle, TranscriptWord } from '../types';

export const ClipEditorModal: React.FC = () => {
  const {
    editorData,
    subtitlePresets,
    isBusy,
    closeClipEditor,
    rerenderEditedClip,
  } = useStudioStore();

  const videoRef = useRef<HTMLVideoElement | null>(null);
  const [spec, setSpec] = useState<ClipRenderSpec | null>(null);
  const [startSec, setStartSec] = useState<number>(0);
  const [endSec, setEndSec] = useState<number>(30);
  const [activeSubTab, setActiveSubTab] = useState<
    'subtitles' | 'words' | 'hook' | 'layout'
  >('subtitles');
  const [renderVersion, setRenderVersion] = useState<number>(Date.now());
  const [currentTime, setCurrentTime] = useState<number>(0);
  const [exportSuccess, setExportSuccess] = useState<string | null>(null);

  useEffect(() => {
    if (editorData) {
      setSpec(JSON.parse(JSON.stringify(editorData.renderSpec)));
      setStartSec(editorData.clip.startSeconds);
      setEndSec(editorData.clip.endSeconds);
      setRenderVersion(Date.now());
    }
  }, [editorData]);

  if (!editorData || !spec) return null;

  const { clip, waveformPeaks } = editorData;
  const videoSrc = `${convertFileSrc(clip.filePath)}?v=${renderVersion}`;

  const handlePresetSelect = (preset: SubtitleStyle) => {
    setSpec({
      ...spec,
      subtitleStyle: { ...preset },
    });
  };

  const handleWordTextChange = (idx: number, newWord: string) => {
    const nextWords: TranscriptWord[] = spec.words.map((w, i) =>
      i === idx ? { ...w, word: newWord } : w
    );
    setSpec({ ...spec, words: nextWords });
  };

  const handleRerender = async () => {
    await rerenderEditedClip(clip.id, spec, startSec, endSec);
    setRenderVersion(Date.now());
  };

  const handleExportMp4 = async () => {
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
      setExportSuccess(targetPath);
      setTimeout(() => setExportSuccess(null), 4000);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-slate-950/90 backdrop-blur-md flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-6xl h-[90vh] flex flex-col overflow-hidden shadow-2xl">
        {/* Top Bar */}
        <div className="px-6 py-4 border-b border-slate-800 flex items-center justify-between bg-slate-900/80">
          <div className="flex items-center gap-3">
            <span className="px-2.5 py-1 rounded-full text-xs font-bold bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
              Skor Viral {clip.viralityScore}
            </span>
            <h2 className="text-lg font-bold text-white truncate max-w-xl">
              {clip.title}
            </h2>
          </div>

          <div className="flex items-center gap-2.5">
            {exportSuccess && (
              <span className="text-xs text-emerald-400 flex items-center gap-1 bg-emerald-500/10 px-3 py-1.5 rounded-lg border border-emerald-500/30">
                <Check className="w-3.5 h-3.5" /> Tersimpan!
              </span>
            )}
            <button
              onClick={() =>
                invoke('open_path_in_explorer', { path: clip.filePath })
              }
              className="px-3 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium flex items-center gap-1.5 transition"
            >
              <FolderOpen className="w-4 h-4" /> Buka Folder
            </button>
            <button
              onClick={handleExportMp4}
              className="px-3.5 py-2 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold flex items-center gap-1.5 transition"
            >
              <Download className="w-4 h-4" /> Ekspor MP4
            </button>
            <button
              onClick={handleRerender}
              disabled={isBusy}
              className="px-4 py-2 rounded-xl bg-violet-600 hover:bg-violet-500 disabled:opacity-50 text-white text-xs font-semibold flex items-center gap-1.5 shadow-lg shadow-violet-600/25 transition"
            >
              <RefreshCw
                className={`w-4 h-4 ${isBusy ? 'animate-spin' : ''}`}
              />
              {isBusy ? 'Merender Ulang...' : 'Terapkan & Render Ulang'}
            </button>
            <button
              onClick={closeClipEditor}
              className="p-2 rounded-xl bg-slate-800 hover:bg-rose-500/20 hover:text-rose-400 text-slate-400 transition"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Main Studio Body */}
        <div className="flex-1 flex overflow-hidden">
          {/* Left Column: 9:16 Video Preview & Waveform Timeline */}
          <div className="w-[380px] border-r border-slate-800 bg-slate-950/60 p-5 flex flex-col items-center justify-between">
            <div className="relative w-[260px] aspect-[9/16] rounded-2xl overflow-hidden bg-black border border-slate-800 shadow-xl">
              <video
                ref={videoRef}
                key={videoSrc}
                src={videoSrc}
                controls
                onTimeUpdate={(e) =>
                  setCurrentTime(e.currentTarget.currentTime)
                }
                className="w-full h-full object-contain"
              />
            </div>

            {/* Waveform & Trim Bar */}
            <div className="w-full mt-4 bg-slate-900/90 border border-slate-800 rounded-xl p-3">
              <div className="flex items-center justify-between text-[11px] text-slate-400 mb-1.5">
                <span>Playhead: {currentTime.toFixed(1)}s</span>
                <span>Durasi: {(endSec - startSec).toFixed(1)}s</span>
              </div>

              <div
                className="h-12 w-full bg-slate-950 rounded-lg p-1.5 flex items-end gap-[2px] cursor-pointer overflow-hidden"
                onClick={(e) => {
                  if (!videoRef.current) return;
                  const rect = e.currentTarget.getBoundingClientRect();
                  const ratio = (e.clientX - rect.left) / rect.width;
                  videoRef.current.currentTime =
                    ratio * (videoRef.current.duration || clip.durationSeconds);
                }}
              >
                {waveformPeaks.slice(0, 80).map((pk, idx) => {
                  const pct = idx / 80;
                  const active =
                    pct <= currentTime / Math.max(clip.durationSeconds, 1);
                  return (
                    <div
                      key={idx}
                      style={{ height: `${Math.max(12, pk * 100)}%` }}
                      className={`flex-1 rounded-full transition-colors ${
                        active ? 'bg-violet-500' : 'bg-slate-700'
                      }`}
                    />
                  );
                })}
              </div>

              <div className="grid grid-cols-2 gap-2 mt-2.5 text-xs">
                <div>
                  <label className="text-[10px] text-slate-400 block">
                    Mulai (detik sumber)
                  </label>
                  <input
                    type="number"
                    step="0.5"
                    value={startSec}
                    onChange={(e) =>
                      setStartSec(parseFloat(e.target.value) || 0)
                    }
                    className="w-full mt-0.5 bg-slate-950 border border-slate-800 rounded-lg px-2 py-1 text-slate-200"
                  />
                </div>
                <div>
                  <label className="text-[10px] text-slate-400 block">
                    Selesai (detik sumber)
                  </label>
                  <input
                    type="number"
                    step="0.5"
                    value={endSec}
                    onChange={(e) => setEndSec(parseFloat(e.target.value) || 0)}
                    className="w-full mt-0.5 bg-slate-950 border border-slate-800 rounded-lg px-2 py-1 text-slate-200"
                  />
                </div>
              </div>
            </div>
          </div>

          {/* Right Column: Controls */}
          <div className="flex-1 flex flex-col overflow-hidden">
            {/* Sub-navigation tabs */}
            <div className="flex border-b border-slate-800 bg-slate-900/40 px-6 pt-3 gap-2">
              {[
                {
                  id: 'subtitles',
                  label: 'Gaya & Animasi Subtitle',
                  icon: Sparkles,
                },
                {
                  id: 'words',
                  label: `Edit Kata Transkrip (${spec.words.length})`,
                  icon: Subtitles,
                },
                { id: 'hook', label: 'Banner Intro Hook', icon: Type },
                { id: 'layout', label: 'Reframing 9:16 & Kamera', icon: Layers },
              ].map((t) => {
                const Icon = t.icon;
                const active = activeSubTab === t.id;
                return (
                  <button
                    key={t.id}
                    onClick={() => setActiveSubTab(t.id as any)}
                    className={`px-4 py-2.5 text-xs font-semibold rounded-t-xl flex items-center gap-2 border-b-2 transition ${
                      active
                        ? 'border-violet-500 text-violet-400 bg-violet-500/10'
                        : 'border-transparent text-slate-400 hover:text-slate-200'
                    }`}
                  >
                    <Icon className="w-4 h-4" />
                    {t.label}
                  </button>
                );
              })}
            </div>

            {/* Tab Content Area */}
            <div className="flex-1 overflow-y-auto p-6 space-y-6">
              {activeSubTab === 'subtitles' && (
                <div className="space-y-6">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-sm font-bold text-white">
                        Preset Subtitle Animasi (.ASS Burn-In)
                      </h3>
                      <p className="text-xs text-slate-400">
                        Pilih preset ala kreator ternama atau sesuaikan warna &
                        animasi per kata
                      </p>
                    </div>
                    <label className="flex items-center gap-2 text-xs text-slate-300 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={spec.hasCaptions}
                        onChange={(e) =>
                          setSpec({ ...spec, hasCaptions: e.target.checked })
                        }
                        className="rounded accent-violet-500"
                      />
                      Aktifkan Subtitle
                    </label>
                  </div>

                  {/* Presets Grid */}
                  <div className="grid grid-cols-3 gap-3">
                    {subtitlePresets.map((p) => {
                      const isSelected =
                        spec.subtitleStyle.presetId === p.presetId;
                      return (
                        <button
                          key={p.presetId}
                          onClick={() => handlePresetSelect(p)}
                          className={`p-3.5 rounded-xl border text-left transition ${
                            isSelected
                              ? 'border-violet-500 bg-violet-500/15'
                              : 'border-slate-800 bg-slate-950/60 hover:border-slate-700'
                          }`}
                        >
                          <div className="flex items-center justify-between mb-1.5">
                            <span className="text-xs font-bold text-white uppercase tracking-wider">
                              {p.presetId.replace('_', ' ')}
                            </span>
                            <span className="text-[10px] px-2 py-0.5 rounded bg-slate-800 text-slate-300">
                              {p.animation}
                            </span>
                          </div>
                          <div
                            className="text-sm font-extrabold truncate py-1 px-2 rounded bg-slate-900 text-center"
                            style={{
                              color: p.highlightColor,
                              textShadow: `1px 1px 0 ${p.outlineColor}`,
                            }}
                          >
                            CONTOH KATA VIRAL
                          </div>
                        </button>
                      );
                    })}
                  </div>

                  {/* Fine-grained Subtitle Controls */}
                  <div className="grid grid-cols-2 gap-4 bg-slate-950/60 p-4 rounded-xl border border-slate-800">
                    <div>
                      <label className="text-xs text-slate-400 block mb-1">
                        Mode Kemunculan Kata (Reveal Mode)
                      </label>
                      <select
                        value={spec.subtitleStyle.revealMode}
                        onChange={(e) =>
                          setSpec({
                            ...spec,
                            subtitleStyle: {
                              ...spec.subtitleStyle,
                              revealMode: e.target.value as any,
                            },
                          })
                        }
                        className="w-full bg-slate-900 border border-slate-800 rounded-lg px-3 py-2 text-xs text-white"
                      >
                        <option value="chunk_karaoke">
                          Chunk Karaoke (Sorot Kata Aktif)
                        </option>
                        <option value="per_word">
                          1 Kata Penuh (MrBeast / Punchy)
                        </option>
                        <option value="karaoke_sweep">
                          Karaoke Sweep (\kf Halus)
                        </option>
                        <option value="cumulative">
                          Kumulatif Kata demi Kata
                        </option>
                        <option value="segment_static">Baris Statis</option>
                      </select>
                    </div>

                    <div>
                      <label className="text-xs text-slate-400 block mb-1">
                        Gaya Animasi Masuk
                      </label>
                      <select
                        value={spec.subtitleStyle.animation}
                        onChange={(e) =>
                          setSpec({
                            ...spec,
                            subtitleStyle: {
                              ...spec.subtitleStyle,
                              animation: e.target.value as any,
                            },
                          })
                        }
                        className="w-full bg-slate-900 border border-slate-800 rounded-lg px-3 py-2 text-xs text-white"
                      >
                        <option value="pop">Pop Elastic (Hormozi)</option>
                        <option value="slam">Slam Zoom (MrBeast)</option>
                        <option value="word_box">Kotak Warna Kata Aktif</option>
                        <option value="bump">Bump Halus</option>
                        <option value="neon">Neon Cyber Blur</option>
                        <option value="glow">Glow Viral</option>
                        <option value="comic">Comic Tilt</option>
                        <option value="fade">Smooth Fade</option>
                      </select>
                    </div>

                    <div>
                      <label className="text-xs text-slate-400 block mb-1">
                        Jenis Font Bundled
                      </label>
                      <select
                        value={spec.subtitleStyle.fontFamily}
                        onChange={(e) =>
                          setSpec({
                            ...spec,
                            subtitleStyle: {
                              ...spec.subtitleStyle,
                              fontFamily: e.target.value,
                            },
                          })
                        }
                        className="w-full bg-slate-900 border border-slate-800 rounded-lg px-3 py-2 text-xs text-white"
                      >
                        <option value="The Bold Font">The Bold Font</option>
                        <option value="Poppins">Poppins Bold</option>
                        <option value="Inter">Inter Bold</option>
                        <option value="Lilita One">Lilita One</option>
                        <option value="Lato">Lato Bold</option>
                      </select>
                    </div>

                    <div>
                      <label className="text-xs text-slate-400 block mb-1">
                        Ukuran Font ({spec.subtitleStyle.fontSize}px) & Kata per
                        Baris ({spec.subtitleStyle.wordsPerChunk})
                      </label>
                      <div className="flex items-center gap-3">
                        <input
                          type="range"
                          min={46}
                          max={105}
                          value={spec.subtitleStyle.fontSize}
                          onChange={(e) =>
                            setSpec({
                              ...spec,
                              subtitleStyle: {
                                ...spec.subtitleStyle,
                                fontSize: parseInt(e.target.value, 10),
                              },
                            })
                          }
                          className="flex-1 accent-violet-500"
                        />
                        <input
                          type="number"
                          min={1}
                          max={6}
                          value={spec.subtitleStyle.wordsPerChunk}
                          onChange={(e) =>
                            setSpec({
                              ...spec,
                              subtitleStyle: {
                                ...spec.subtitleStyle,
                                wordsPerChunk: parseInt(e.target.value, 10) || 3,
                              },
                            })
                          }
                          className="w-16 bg-slate-900 border border-slate-800 rounded-lg px-2 py-1 text-xs text-center"
                        />
                      </div>
                    </div>

                    <div className="col-span-2 grid grid-cols-4 gap-3 pt-2">
                      <div>
                        <label className="text-[11px] text-slate-400 block mb-1">
                          Warna Teks Utama
                        </label>
                        <input
                          type="color"
                          value={spec.subtitleStyle.primaryColor}
                          onChange={(e) =>
                            setSpec({
                              ...spec,
                              subtitleStyle: {
                                ...spec.subtitleStyle,
                                primaryColor: e.target.value,
                              },
                            })
                          }
                          className="w-full h-9 rounded cursor-pointer bg-slate-900 border border-slate-700"
                        />
                      </div>
                      <div>
                        <label className="text-[11px] text-slate-400 block mb-1">
                          Warna Sorot Aktif
                        </label>
                        <input
                          type="color"
                          value={spec.subtitleStyle.highlightColor}
                          onChange={(e) =>
                            setSpec({
                              ...spec,
                              subtitleStyle: {
                                ...spec.subtitleStyle,
                                highlightColor: e.target.value,
                              },
                            })
                          }
                          className="w-full h-9 rounded cursor-pointer bg-slate-900 border border-slate-700"
                        />
                      </div>
                      <div>
                        <label className="text-[11px] text-slate-400 block mb-1">
                          Warna Garis Tepi
                        </label>
                        <input
                          type="color"
                          value={spec.subtitleStyle.outlineColor}
                          onChange={(e) =>
                            setSpec({
                              ...spec,
                              subtitleStyle: {
                                ...spec.subtitleStyle,
                                outlineColor: e.target.value,
                              },
                            })
                          }
                          className="w-full h-9 rounded cursor-pointer bg-slate-900 border border-slate-700"
                        />
                      </div>
                      <div>
                        <label className="text-[11px] text-slate-400 block mb-1">
                          Posisi Vertikal (
                          {Math.round(spec.subtitleStyle.verticalPosition * 100)}
                          %)
                        </label>
                        <input
                          type="range"
                          min={0.25}
                          max={0.85}
                          step={0.02}
                          value={spec.subtitleStyle.verticalPosition}
                          onChange={(e) =>
                            setSpec({
                              ...spec,
                              subtitleStyle: {
                                ...spec.subtitleStyle,
                                verticalPosition: parseFloat(e.target.value),
                              },
                            })
                          }
                          className="w-full mt-2 accent-violet-500"
                        />
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {activeSubTab === 'words' && (
                <div className="space-y-4">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-sm font-bold text-white">
                        koreksi Kata demi Kata (Word-Level Timestamp)
                      </h3>
                      <p className="text-xs text-slate-400">
                        Klik tombol putar untuk melompat ke detik kata tersebut,
                        atau ubah ejaan kata secara langsung
                      </p>
                    </div>
                  </div>

                  <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5 max-h-[55vh] overflow-y-auto pr-1">
                    {spec.words.map((w, idx) => (
                      <div
                        key={idx}
                        className="flex items-center gap-1.5 bg-slate-950/70 border border-slate-800 rounded-xl px-2.5 py-1.5"
                      >
                        <button
                          onClick={() => {
                            if (videoRef.current) {
                              videoRef.current.currentTime = w.start;
                              videoRef.current.play();
                            }
                          }}
                          title={`Lompat ke ${w.start.toFixed(2)}s`}
                          className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-slate-800 hover:bg-violet-600 text-slate-300 hover:text-white transition flex items-center gap-0.5"
                        >
                          <Play className="w-2.5 h-2.5" />
                          {w.start.toFixed(1)}s
                        </button>
                        <input
                          type="text"
                          value={w.word}
                          onChange={(e) =>
                            handleWordTextChange(idx, e.target.value)
                          }
                          className="flex-1 bg-transparent text-xs text-white focus:outline-none border-b border-transparent focus:border-violet-500 px-1"
                        />
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {activeSubTab === 'hook' && (
                <div className="space-y-5">
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="text-sm font-bold text-white">
                        Intro Hook Banner (3 Detik Pertama)
                      </h3>
                      <p className="text-xs text-slate-400">
                        Banner visual otomatis untuk menghentikan scroll penonton
                        di detik-detik awal
                      </p>
                    </div>
                    <label className="flex items-center gap-2 text-xs text-slate-300 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={spec.hasHook}
                        onChange={(e) =>
                          setSpec({
                            ...spec,
                            hasHook: e.target.checked,
                            hookStyle: {
                              ...spec.hookStyle,
                              enabled: e.target.checked,
                            },
                          })
                        }
                        className="rounded accent-violet-500"
                      />
                      Tampilkan Intro Hook
                    </label>
                  </div>

                  <div className="space-y-4 bg-slate-950/60 p-4 rounded-xl border border-slate-800">
                    <div>
                      <label className="text-xs text-slate-400 block mb-1">
                        Headline Hook Utama
                      </label>
                      <input
                        type="text"
                        value={spec.hookText || ''}
                        onChange={(e) =>
                          setSpec({ ...spec, hookText: e.target.value })
                        }
                        placeholder="RAHASIA YANG JARANG DIBAHAS!"
                        className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3.5 py-2 text-sm font-bold text-white"
                      />
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <label className="text-xs text-slate-400 block mb-1">
                          Badge Kicker Atas
                        </label>
                        <input
                          type="text"
                          value={spec.hookKicker || ''}
                          onChange={(e) =>
                            setSpec({ ...spec, hookKicker: e.target.value })
                          }
                          placeholder="FAKTA MENGEJUTKAN"
                          className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3 py-2 text-xs text-white"
                        />
                      </div>
                      <div>
                        <label className="text-xs text-slate-400 block mb-1">
                          Tag Kategori Bawah
                        </label>
                        <input
                          type="text"
                          value={spec.hookTag || ''}
                          onChange={(e) =>
                            setSpec({ ...spec, hookTag: e.target.value })
                          }
                          placeholder="PODCAST HIGHLIGHT"
                          className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3 py-2 text-xs text-white"
                        />
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <label className="text-xs text-slate-400 block mb-1">
                          Durasi Tampil ({spec.hookStyle.durationSeconds} detik)
                        </label>
                        <input
                          type="range"
                          min={1.5}
                          max={6.0}
                          step={0.5}
                          value={spec.hookStyle.durationSeconds}
                          onChange={(e) =>
                            setSpec({
                              ...spec,
                              hookStyle: {
                                ...spec.hookStyle,
                                durationSeconds: parseFloat(e.target.value),
                              },
                            })
                          }
                          className="w-full accent-violet-500 mt-2"
                        />
                      </div>
                      <div className="grid grid-cols-2 gap-2">
                        <div>
                          <label className="text-[11px] text-slate-400 block mb-1">
                            Warna Kartu Hook
                          </label>
                          <input
                            type="color"
                            value={spec.hookStyle.cardBgColor}
                            onChange={(e) =>
                              setSpec({
                                ...spec,
                                hookStyle: {
                                  ...spec.hookStyle,
                                  cardBgColor: e.target.value,
                                },
                              })
                            }
                            className="w-full h-9 rounded cursor-pointer bg-slate-900 border border-slate-700"
                          />
                        </div>
                        <div>
                          <label className="text-[11px] text-slate-400 block mb-1">
                            Warna Badge Kicker
                          </label>
                          <input
                            type="color"
                            value={spec.hookStyle.kickerBgColor}
                            onChange={(e) =>
                              setSpec({
                                ...spec,
                                hookStyle: {
                                  ...spec.hookStyle,
                                  kickerBgColor: e.target.value,
                                },
                              })
                            }
                            className="w-full h-9 rounded cursor-pointer bg-slate-900 border border-slate-700"
                          />
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {activeSubTab === 'layout' && (
                <div className="space-y-4">
                  <h3 className="text-sm font-bold text-white">
                    Mode AI Smart Reframing 9:16 (Portrait 1080x1920)
                  </h3>
                  <div className="grid grid-cols-2 gap-3">
                    {[
                      {
                        id: 'smart_speaker',
                        title: 'AI Smart Speaker Tracking (ONNX YuNet)',
                        desc: 'Kamera virtual 9:16 otomatis mengikuti wajah pembicara aktif dengan deadzone halus.',
                      },
                      {
                        id: 'split_screen',
                        title: 'Split-Screen 2 Pembicara (Atas-Bawah)',
                        desc: 'Membagi layar 1080x960 atas (pembicara kiri) dan 1080x960 bawah (pembicara kanan).',
                      },
                      {
                        id: 'fill',
                        title: 'Center Crop 9:16 (Full Layar)',
                        desc: 'Potong bagian tengah video secara statis memenuhi layar vertikal.',
                      },
                      {
                        id: 'fit_blur',
                        title: 'Fit 16:9 + Background Blur',
                        desc: 'Tampilkan frame 16:9 utuh di tengah dengan latar belakang blur estetik.',
                      },
                      {
                        id: 'fit_black',
                        title: 'Fit 16:9 + Latar Hitam Cinema',
                        desc: 'Tampilkan frame 16:9 utuh di tengah dengan latar hitam bersih.',
                      },
                    ].map((lm) => {
                      const active = spec.videoLayout === lm.id;
                      return (
                        <button
                          key={lm.id}
                          onClick={() =>
                            setSpec({ ...spec, videoLayout: lm.id })
                          }
                          className={`p-4 rounded-xl border text-left transition ${
                            active
                              ? 'border-violet-500 bg-violet-500/15'
                              : 'border-slate-800 bg-slate-950/60 hover:border-slate-700'
                          }`}
                        >
                          <div className="flex items-center gap-2 text-xs font-bold text-white mb-1">
                            <Video className="w-4 h-4 text-violet-400" />
                            {lm.title}
                          </div>
                          <p className="text-[11px] text-slate-400 leading-relaxed">
                            {lm.desc}
                          </p>
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

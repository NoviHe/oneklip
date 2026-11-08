import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import {
  AiConfig,
  ClipEditorData,
  ClipRecord,
  ClipRenderSpec,
  PipelineProgressPayload,
  SessionRecord,
  StartPipelineRequest,
  SubtitleStyle,
  SystemDiagnostics,
  Transcript,
} from '../types';

interface StudioState {
  activeTab: 'create' | 'studio' | 'sessions' | 'settings';
  diagnostics: SystemDiagnostics | null;
  sessions: SessionRecord[];
  currentSession: SessionRecord | null;
  currentClips: ClipRecord[];
  currentTranscript: Transcript | null;
  selectedClipId: string | null;
  editorData: ClipEditorData | null;
  subtitlePresets: SubtitleStyle[];
  pipelineProgress: PipelineProgressPayload | null;
  isBusy: boolean;
  errorBanner: string | null;

  setActiveTab: (tab: 'create' | 'studio' | 'sessions' | 'settings') => void;
  setErrorBanner: (msg: string | null) => void;
  setPipelineProgress: (p: PipelineProgressPayload | null) => void;
  appendOrUpdateClip: (clip: ClipRecord) => void;

  fetchInitialData: () => Promise<void>;
  refreshSessions: () => Promise<void>;
  openSessionInStudio: (sessionId: string) => Promise<void>;
  deleteSessionById: (sessionId: string) => Promise<void>;
  startNewPipeline: (req: StartPipelineRequest) => Promise<string>;
  cancelActivePipeline: (sessionId: string) => Promise<void>;

  openClipEditor: (clipId: string) => Promise<void>;
  closeClipEditor: () => void;
  rerenderEditedClip: (
    clipId: string,
    spec: ClipRenderSpec,
    newStartSeconds?: number,
    newEndSeconds?: number
  ) => Promise<ClipRecord>;

  saveAiConfiguration: (config: AiConfig) => Promise<void>;
  installMissingBinaries: () => Promise<void>;
}

export const useStudioStore = create<StudioState>((set, get) => ({
  activeTab: 'create',
  diagnostics: null,
  sessions: [],
  currentSession: null,
  currentClips: [],
  currentTranscript: null,
  selectedClipId: null,
  editorData: null,
  subtitlePresets: [],
  pipelineProgress: null,
  isBusy: false,
  errorBanner: null,

  setActiveTab: (tab) => set({ activeTab: tab }),
  setErrorBanner: (msg) => set({ errorBanner: msg }),
  setPipelineProgress: (p) => set({ pipelineProgress: p }),

  appendOrUpdateClip: (clip) =>
    set((state) => {
      const exists = state.currentClips.some((c) => c.id === clip.id);
      const updated = exists
        ? state.currentClips.map((c) => (c.id === clip.id ? clip : c))
        : [...state.currentClips, clip];
      return { currentClips: updated };
    }),

  fetchInitialData: async () => {
    try {
      const [diag, sessions, presets] = await Promise.all([
        invoke<SystemDiagnostics>('get_system_diagnostics'),
        invoke<SessionRecord[]>('list_sessions'),
        invoke<SubtitleStyle[]>('get_subtitle_presets'),
      ]);
      set({
        diagnostics: diag,
        sessions,
        subtitlePresets: presets,
      });
      if (sessions.length > 0 && !get().currentSession) {
        const first = sessions[0];
        const detail = await invoke<{
          session: SessionRecord;
          clips: ClipRecord[];
          transcript: Transcript | null;
        }>('get_session_detail', { sessionId: first.id });
        set({
          currentSession: detail.session,
          currentClips: detail.clips,
          currentTranscript: detail.transcript,
        });
      }
    } catch (e: any) {
      set({ errorBanner: e?.message || String(e) });
    }
  },

  refreshSessions: async () => {
    try {
      const sessions = await invoke<SessionRecord[]>('list_sessions');
      set({ sessions });
    } catch (e: any) {
      set({ errorBanner: e?.message || String(e) });
    }
  },

  openSessionInStudio: async (sessionId) => {
    set({ isBusy: true, errorBanner: null });
    try {
      const detail = await invoke<{
        session: SessionRecord;
        clips: ClipRecord[];
        transcript: Transcript | null;
      }>('get_session_detail', { sessionId });
      set({
        currentSession: detail.session,
        currentClips: detail.clips,
        currentTranscript: detail.transcript,
        selectedClipId: null,
        editorData: null,
        activeTab: 'studio',
        isBusy: false,
      });
    } catch (e: any) {
      set({ isBusy: false, errorBanner: e?.message || String(e) });
    }
  },

  deleteSessionById: async (sessionId) => {
    try {
      await invoke('delete_session', { sessionId });
      const sessions = await invoke<SessionRecord[]>('list_sessions');
      const isCurrent = get().currentSession?.id === sessionId;
      set({
        sessions,
        currentSession: isCurrent ? null : get().currentSession,
        currentClips: isCurrent ? [] : get().currentClips,
        currentTranscript: isCurrent ? null : get().currentTranscript,
      });
    } catch (e: any) {
      set({ errorBanner: e?.message || String(e) });
    }
  },

  startNewPipeline: async (req) => {
    set({
      isBusy: true,
      errorBanner: null,
      currentClips: [],
      selectedClipId: null,
      editorData: null,
      pipelineProgress: {
        sessionId: 'pending',
        stage: 'starting',
        message: 'Memulai sesi pemotongan otomatis...',
        percent: 2,
      },
    });
    const sessionId = await invoke<string>('start_clipper_session', {
      request: req,
    });
    const detail = await invoke<{
      session: SessionRecord;
      clips: ClipRecord[];
      transcript: Transcript | null;
    }>('get_session_detail', { sessionId }).catch(() => null);
    if (detail) {
      set({
        currentSession: detail.session,
        currentClips: detail.clips,
        activeTab: 'studio',
      });
    } else {
      set({ activeTab: 'studio' });
    }
    await get().refreshSessions();
    return sessionId;
  },

  cancelActivePipeline: async (sessionId) => {
    await invoke('cancel_clipper_session', { sessionId });
    set({ isBusy: false, pipelineProgress: null });
    await get().refreshSessions();
  },

  openClipEditor: async (clipId) => {
    set({ isBusy: true, errorBanner: null, selectedClipId: clipId });
    try {
      const data = await invoke<ClipEditorData>('get_clip_editor_data', {
        clipId,
      });
      set({ editorData: data, isBusy: false });
    } catch (e: any) {
      set({ isBusy: false, errorBanner: e?.message || String(e) });
    }
  },

  closeClipEditor: () => set({ selectedClipId: null, editorData: null }),

  rerenderEditedClip: async (clipId, spec, newStartSeconds, newEndSeconds) => {
    set({ isBusy: true, errorBanner: null });
    try {
      const updated = await invoke<ClipRecord>('rerender_clip', {
        clipId,
        spec,
        newStartSeconds: newStartSeconds ?? null,
        newEndSeconds: newEndSeconds ?? null,
      });
      get().appendOrUpdateClip(updated);
      const refreshedEditor = await invoke<ClipEditorData>(
        'get_clip_editor_data',
        { clipId }
      );
      set({ editorData: refreshedEditor, isBusy: false });
      return updated;
    } catch (e: any) {
      set({ isBusy: false, errorBanner: e?.message || String(e) });
      throw e;
    }
  },

  saveAiConfiguration: async (config) => {
    const saved = await invoke<AiConfig>('save_ai_config', { config });
    const diag = get().diagnostics;
    if (diag) {
      set({ diagnostics: { ...diag, aiConfig: saved } });
    }
  },

  installMissingBinaries: async () => {
    set({ isBusy: true, errorBanner: null });
    try {
      await invoke('install_missing_binaries');
      const diag = await invoke<SystemDiagnostics>('get_system_diagnostics');
      set({ diagnostics: diag, isBusy: false });
    } catch (e: any) {
      set({ isBusy: false, errorBanner: e?.message || String(e) });
    }
  },
}));

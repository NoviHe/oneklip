export interface ComponentStatus {
  name: string;
  installed: boolean;
  path: string;
  sizeBytes: number;
}

export interface BinariesStatus {
  allReady: boolean;
  ffmpeg: ComponentStatus;
  ffprobe: ComponentStatus;
  ytdlp: ComponentStatus;
  deno: ComponentStatus;
  whisperCli: ComponentStatus;
  onnxruntime: ComponentStatus;
  whisperModel: ComponentStatus;
  yunetModel: ComponentStatus;
  sileroVadModel: ComponentStatus;
  cookiesPresent: boolean;
}

export interface EncoderInfo {
  codec: string;
  label: string;
  isHardware: boolean;
}

export interface AiConfig {
  provider: 'gemini' | 'groq' | 'openai' | 'openrouter' | 'ollama' | 'heuristic';
  apiKey: string;
  model: string;
  baseUrl?: string | null;
  targetClipMinSecs: number;
  targetClipMaxSecs: number;
  maxClips: number;
}

export interface SystemDiagnostics {
  binaries: BinariesStatus;
  encoder: EncoderInfo;
  aiConfig: AiConfig;
  cpuBrand: string;
  cpuCores: number;
  totalRamGb: number;
  appDataDir: string;
}

export interface VideoMetadata {
  videoId?: string | null;
  title: string;
  channel: string;
  durationSeconds: number;
  thumbnailUrl?: string | null;
  sourceUrl?: string | null;
  localPath?: string | null;
}

export interface TranscriptWord {
  word: string;
  start: number;
  end: number;
  probability?: number;
}

export interface TranscriptSegment {
  id: number;
  start: number;
  end: number;
  text: string;
  words: TranscriptWord[];
}

export interface Transcript {
  language: string;
  sourceTier: string;
  durationSeconds: number;
  segments: TranscriptSegment[];
}

export interface SessionRecord {
  id: string;
  folderPath: string;
  videoUrl?: string | null;
  videoId?: string | null;
  localVideoPath?: string | null;
  videoTitle?: string | null;
  videoChannel?: string | null;
  videoDuration?: number | null;
  videoLanguage: string;
  contentType: string;
  status: string;
  highlightsCount: number;
  clipsProcessed: number;
  totalSizeBytes: number;
  lastError?: string | null;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export interface ClipRecord {
  id: string;
  sessionId: string;
  position: number;
  startSeconds: number;
  endSeconds: number;
  durationSeconds: number;
  title: string;
  hookText?: string | null;
  hookKicker?: string | null;
  hookTag?: string | null;
  description?: string | null;
  category: string;
  viralityScore: number;
  videoLayout: string;
  previewPath?: string | null;
  filePath: string;
  fileSizeBytes?: number | null;
  resolution?: string | null;
  hasHook: boolean;
  hasCaptions: boolean;
  hasWatermark: boolean;
  editorStateJson?: string | null;
  exportedTo?: string | null;
  createdAt: string;
}

export interface SubtitleStyle {
  presetId: string;
  fontFamily: string;
  fontSize: number;
  primaryColor: string;
  highlightColor: string;
  outlineColor: string;
  boxColor: string;
  outlineWidth: number;
  shadowDepth: number;
  revealMode: 'chunk_karaoke' | 'per_word' | 'karaoke_sweep' | 'cumulative' | 'segment_static';
  animation: 'pop' | 'bump' | 'slam' | 'glow' | 'neon' | 'comic' | 'word_box' | 'fade';
  wordsPerChunk: number;
  verticalPosition: number;
  uppercase: boolean;
}

export interface HookStyle {
  enabled: boolean;
  preset: 'punchline' | 'classic' | 'comic' | 'sticker';
  durationSeconds: number;
  cardBgColor: string;
  textColor: string;
  kickerBgColor: string;
  kickerTextColor: string;
  yPositionRatio: number;
}

export interface ClipRenderSpec {
  clipId: string;
  videoLayout: string;
  subtitleStyle: SubtitleStyle;
  hookStyle: HookStyle;
  hookText?: string | null;
  hookKicker?: string | null;
  hookTag?: string | null;
  hasCaptions: boolean;
  hasHook: boolean;
  hasWatermark: boolean;
  watermarkText?: string | null;
  words: TranscriptWord[];
}

export interface ClipEditorData {
  clip: ClipRecord;
  renderSpec: ClipRenderSpec;
  waveformPeaks: number[];
  rawVideoPath: string;
}

export interface StartPipelineRequest {
  videoUrl?: string | null;
  localVideoPath?: string | null;
  videoLanguage: string;
  contentType: string;
  defaultLayout: string;
  subtitlePreset: string;
  hookPreset: string;
  maxClips?: number;
}

export interface PipelineProgressPayload {
  sessionId: string;
  stage: string;
  message: string;
  percent: number;
}

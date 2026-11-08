PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS sessions (
    id                 TEXT PRIMARY KEY,
    folder_path        TEXT NOT NULL,
    video_url          TEXT,
    video_id           TEXT,
    local_video_path   TEXT,
    video_title        TEXT,
    video_channel      TEXT,
    video_duration     REAL,
    video_language     TEXT DEFAULT 'id',
    content_type       TEXT NOT NULL DEFAULT 'podcast',
    status             TEXT NOT NULL DEFAULT 'created',
    highlights_count   INTEGER NOT NULL DEFAULT 0,
    clips_processed    INTEGER NOT NULL DEFAULT 0,
    total_size_bytes   INTEGER NOT NULL DEFAULT 0,
    last_error         TEXT,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL,
    completed_at       TEXT,
    CHECK (status IN (
        'created', 'downloading', 'transcribing', 'analyzing',
        'highlights_found', 'processing', 'completed', 'failed', 'canceled'
    ))
);

CREATE TABLE IF NOT EXISTS clips (
    id                  TEXT PRIMARY KEY,
    session_id          TEXT NOT NULL,
    position            INTEGER NOT NULL,
    start_seconds       REAL NOT NULL,
    end_seconds         REAL NOT NULL,
    duration_seconds    REAL NOT NULL,
    title               TEXT NOT NULL,
    hook_text           TEXT,
    hook_kicker         TEXT,
    hook_tag            TEXT,
    description         TEXT,
    category            TEXT DEFAULT 'general',
    virality_score      INTEGER NOT NULL DEFAULT 7,
    video_layout        TEXT NOT NULL DEFAULT 'smart_speaker',
    preview_path        TEXT,
    file_path           TEXT NOT NULL,
    file_size_bytes     INTEGER,
    resolution          TEXT DEFAULT '1080x1920',
    has_hook            INTEGER NOT NULL DEFAULT 1,
    has_captions        INTEGER NOT NULL DEFAULT 1,
    has_watermark       INTEGER NOT NULL DEFAULT 0,
    editor_state_json   TEXT,
    exported_to         TEXT,
    created_at          TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_clips_session ON clips(session_id);

CREATE TABLE IF NOT EXISTS local_jobs (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL,
    clip_id         TEXT,
    type            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    progress        INTEGER NOT NULL DEFAULT 0,
    sub_progress    REAL DEFAULT 0.0,
    current_step    TEXT,
    input_path      TEXT,
    output_path     TEXT,
    parameters      TEXT,
    error_message   TEXT,
    error_code      TEXT,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    max_retries     INTEGER NOT NULL DEFAULT 2,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    started_at      TEXT,
    completed_at    TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    CHECK (type IN (
        'DOWNLOAD_VIDEO', 'EXTRACT_AUDIO', 'TRANSCRIBE_LOCAL',
        'HIGHLIGHTS_AI', 'CUT_VIDEO', 'SMART_CROP_ANALYZE',
        'RENDER_MASTER', 'EXPORT'
    )),
    CHECK (status IN (
        'pending', 'running', 'completed', 'failed',
        'canceled', 'paused', 'interrupted'
    ))
);

CREATE INDEX IF NOT EXISTS idx_local_jobs_session ON local_jobs(session_id);
CREATE INDEX IF NOT EXISTS idx_local_jobs_status  ON local_jobs(status);

CREATE TABLE IF NOT EXISTS cache_kv (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    expires_at  TEXT,
    created_at  TEXT NOT NULL
);

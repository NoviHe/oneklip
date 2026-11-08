use crate::error::AppResult;
use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

const SCHEMA_SQL: &str = include_str!("../../migrations/V1__initial_schema.sql");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub folder_path: String,
    pub video_url: Option<String>,
    pub video_id: Option<String>,
    pub local_video_path: Option<String>,
    pub video_title: Option<String>,
    pub video_channel: Option<String>,
    pub video_duration: Option<f64>,
    pub video_language: String,
    pub content_type: String,
    pub status: String,
    pub highlights_count: i64,
    pub clips_processed: i64,
    pub total_size_bytes: i64,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipRecord {
    pub id: String,
    pub session_id: String,
    pub position: i64,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub duration_seconds: f64,
    pub title: String,
    pub hook_text: Option<String>,
    pub hook_kicker: Option<String>,
    pub hook_tag: Option<String>,
    pub description: Option<String>,
    pub category: String,
    pub virality_score: i64,
    pub video_layout: String,
    pub preview_path: Option<String>,
    pub file_path: String,
    pub file_size_bytes: Option<i64>,
    pub resolution: Option<String>,
    pub has_hook: bool,
    pub has_captions: bool,
    pub has_watermark: bool,
    pub editor_state_json: Option<String>,
    pub exported_to: Option<String>,
    pub created_at: String,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn upsert_session(&self, s: &SessionRecord) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO sessions (
                id, folder_path, video_url, video_id, local_video_path,
                video_title, video_channel, video_duration, video_language,
                content_type, status, highlights_count, clips_processed,
                total_size_bytes, last_error, created_at, updated_at, completed_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18
            )
            ON CONFLICT(id) DO UPDATE SET
                video_url = excluded.video_url,
                video_id = excluded.video_id,
                local_video_path = excluded.local_video_path,
                video_title = excluded.video_title,
                video_channel = excluded.video_channel,
                video_duration = excluded.video_duration,
                video_language = excluded.video_language,
                content_type = excluded.content_type,
                status = excluded.status,
                highlights_count = excluded.highlights_count,
                clips_processed = excluded.clips_processed,
                total_size_bytes = excluded.total_size_bytes,
                last_error = excluded.last_error,
                updated_at = excluded.updated_at,
                completed_at = excluded.completed_at",
            params![
                s.id,
                s.folder_path,
                s.video_url,
                s.video_id,
                s.local_video_path,
                s.video_title,
                s.video_channel,
                s.video_duration,
                s.video_language,
                s.content_type,
                s.status,
                s.highlights_count,
                s.clips_processed,
                s.total_size_bytes,
                s.last_error,
                s.created_at,
                s.updated_at,
                s.completed_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_session_status(
        &self,
        session_id: &str,
        status: &str,
        last_error: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let completed_at = if status == "completed" {
            Some(now.clone())
        } else {
            None
        };
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sessions SET status = ?2, last_error = ?3, updated_at = ?4, completed_at = COALESCE(?5, completed_at) WHERE id = ?1",
            params![session_id, status, last_error, now, completed_at],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self) -> AppResult<Vec<SessionRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, folder_path, video_url, video_id, local_video_path,
                    video_title, video_channel, video_duration, video_language,
                    content_type, status, highlights_count, clips_processed,
                    total_size_bytes, last_error, created_at, updated_at, completed_at
             FROM sessions ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SessionRecord {
                    id: row.get(0)?,
                    folder_path: row.get(1)?,
                    video_url: row.get(2)?,
                    video_id: row.get(3)?,
                    local_video_path: row.get(4)?,
                    video_title: row.get(5)?,
                    video_channel: row.get(6)?,
                    video_duration: row.get(7)?,
                    video_language: row.get::<_, Option<String>>(8)?.unwrap_or_else(|| "id".into()),
                    content_type: row.get::<_, Option<String>>(9)?.unwrap_or_else(|| "podcast".into()),
                    status: row.get(10)?,
                    highlights_count: row.get(11)?,
                    clips_processed: row.get(12)?,
                    total_size_bytes: row.get(13)?,
                    last_error: row.get(14)?,
                    created_at: row.get(15)?,
                    updated_at: row.get(16)?,
                    completed_at: row.get(17)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<Option<SessionRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, folder_path, video_url, video_id, local_video_path,
                    video_title, video_channel, video_duration, video_language,
                    content_type, status, highlights_count, clips_processed,
                    total_size_bytes, last_error, created_at, updated_at, completed_at
             FROM sessions WHERE id = ?1",
        )?;
        let res = stmt
            .query_row(params![session_id], |row| {
                Ok(SessionRecord {
                    id: row.get(0)?,
                    folder_path: row.get(1)?,
                    video_url: row.get(2)?,
                    video_id: row.get(3)?,
                    local_video_path: row.get(4)?,
                    video_title: row.get(5)?,
                    video_channel: row.get(6)?,
                    video_duration: row.get(7)?,
                    video_language: row.get::<_, Option<String>>(8)?.unwrap_or_else(|| "id".into()),
                    content_type: row.get::<_, Option<String>>(9)?.unwrap_or_else(|| "podcast".into()),
                    status: row.get(10)?,
                    highlights_count: row.get(11)?,
                    clips_processed: row.get(12)?,
                    total_size_bytes: row.get(13)?,
                    last_error: row.get(14)?,
                    created_at: row.get(15)?,
                    updated_at: row.get(16)?,
                    completed_at: row.get(17)?,
                })
            })
            .optional()?;
        Ok(res)
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<Option<String>> {
        let folder = self.get_session(session_id)?.map(|s| s.folder_path);
        let conn = self.conn.lock();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;
        Ok(folder)
    }

    pub fn upsert_clip(&self, c: &ClipRecord) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO clips (
                id, session_id, position, start_seconds, end_seconds, duration_seconds,
                title, hook_text, hook_kicker, hook_tag, description, category,
                virality_score, video_layout, preview_path, file_path, file_size_bytes,
                resolution, has_hook, has_captions, has_watermark, editor_state_json,
                exported_to, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17,
                ?18, ?19, ?20, ?21, ?22,
                ?23, ?24
            )
            ON CONFLICT(id) DO UPDATE SET
                position = excluded.position,
                start_seconds = excluded.start_seconds,
                end_seconds = excluded.end_seconds,
                duration_seconds = excluded.duration_seconds,
                title = excluded.title,
                hook_text = excluded.hook_text,
                hook_kicker = excluded.hook_kicker,
                hook_tag = excluded.hook_tag,
                description = excluded.description,
                category = excluded.category,
                virality_score = excluded.virality_score,
                video_layout = excluded.video_layout,
                preview_path = COALESCE(excluded.preview_path, clips.preview_path),
                file_path = excluded.file_path,
                file_size_bytes = COALESCE(excluded.file_size_bytes, clips.file_size_bytes),
                resolution = excluded.resolution,
                has_hook = excluded.has_hook,
                has_captions = excluded.has_captions,
                has_watermark = excluded.has_watermark,
                editor_state_json = COALESCE(excluded.editor_state_json, clips.editor_state_json),
                exported_to = COALESCE(excluded.exported_to, clips.exported_to)",
            params![
                c.id,
                c.session_id,
                c.position,
                c.start_seconds,
                c.end_seconds,
                c.duration_seconds,
                c.title,
                c.hook_text,
                c.hook_kicker,
                c.hook_tag,
                c.description,
                c.category,
                c.virality_score,
                c.video_layout,
                c.preview_path,
                c.file_path,
                c.file_size_bytes,
                c.resolution,
                if c.has_hook { 1 } else { 0 },
                if c.has_captions { 1 } else { 0 },
                if c.has_watermark { 1 } else { 0 },
                c.editor_state_json,
                c.exported_to,
                c.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_clips_for_session(&self, session_id: &str) -> AppResult<Vec<ClipRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, position, start_seconds, end_seconds, duration_seconds,
                    title, hook_text, hook_kicker, hook_tag, description, category,
                    virality_score, video_layout, preview_path, file_path, file_size_bytes,
                    resolution, has_hook, has_captions, has_watermark, editor_state_json,
                    exported_to, created_at
             FROM clips WHERE session_id = ?1 ORDER BY position ASC",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(ClipRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    position: row.get(2)?,
                    start_seconds: row.get(3)?,
                    end_seconds: row.get(4)?,
                    duration_seconds: row.get(5)?,
                    title: row.get(6)?,
                    hook_text: row.get(7)?,
                    hook_kicker: row.get(8)?,
                    hook_tag: row.get(9)?,
                    description: row.get(10)?,
                    category: row.get::<_, Option<String>>(11)?.unwrap_or_else(|| "general".into()),
                    virality_score: row.get(12)?,
                    video_layout: row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "smart_speaker".into()),
                    preview_path: row.get(14)?,
                    file_path: row.get(15)?,
                    file_size_bytes: row.get(16)?,
                    resolution: row.get(17)?,
                    has_hook: row.get::<_, i64>(18)? != 0,
                    has_captions: row.get::<_, i64>(19)? != 0,
                    has_watermark: row.get::<_, i64>(20)? != 0,
                    editor_state_json: row.get(21)?,
                    exported_to: row.get(22)?,
                    created_at: row.get(23)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_clip(&self, clip_id: &str) -> AppResult<Option<ClipRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, position, start_seconds, end_seconds, duration_seconds,
                    title, hook_text, hook_kicker, hook_tag, description, category,
                    virality_score, video_layout, preview_path, file_path, file_size_bytes,
                    resolution, has_hook, has_captions, has_watermark, editor_state_json,
                    exported_to, created_at
             FROM clips WHERE id = ?1",
        )?;
        let res = stmt
            .query_row(params![clip_id], |row| {
                Ok(ClipRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    position: row.get(2)?,
                    start_seconds: row.get(3)?,
                    end_seconds: row.get(4)?,
                    duration_seconds: row.get(5)?,
                    title: row.get(6)?,
                    hook_text: row.get(7)?,
                    hook_kicker: row.get(8)?,
                    hook_tag: row.get(9)?,
                    description: row.get(10)?,
                    category: row.get::<_, Option<String>>(11)?.unwrap_or_else(|| "general".into()),
                    virality_score: row.get(12)?,
                    video_layout: row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "smart_speaker".into()),
                    preview_path: row.get(14)?,
                    file_path: row.get(15)?,
                    file_size_bytes: row.get(16)?,
                    resolution: row.get(17)?,
                    has_hook: row.get::<_, i64>(18)? != 0,
                    has_captions: row.get::<_, i64>(19)? != 0,
                    has_watermark: row.get::<_, i64>(20)? != 0,
                    editor_state_json: row.get(21)?,
                    exported_to: row.get(22)?,
                    created_at: row.get(23)?,
                })
            })
            .optional()?;
        Ok(res)
    }

    pub fn set_kv(&self, key: &str, value: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO cache_kv (key, value, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value, now],
        )?;
        Ok(())
    }

    pub fn get_kv(&self, key: &str) -> AppResult<Option<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT value FROM cache_kv WHERE key = ?1")?;
        let val = stmt.query_row(params![key], |row| row.get(0)).optional()?;
        Ok(val)
    }
}

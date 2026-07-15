// This file is part of utoaudio, licensed under AGPL-3.0.
//
// `audio-ffi::library` — SQLite-backed library index.
//
// The library index persists across `pnpm tauri dev` restarts in a single
// SQLite database at `<app_data_dir>/utoaudio/library.sqlite`. The `bundled`
// feature on `rusqlite` avoids a system libsqlite dependency on Linux and
// Android.
//
// Schema (version 1):
//   tracks(id, path UNIQUE, title, artist, album, duration_secs, mtime, indexed_at)
//   scan_roots(path PRIMARY KEY, added_at)
//   schema_meta(key PRIMARY KEY, value)
//
// All multi-step writes are wrapped in a transaction. User input is always
// bound via prepared statements — never string-concatenated into SQL.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Current schema version. Bumped on any migration.
const SCHEMA_VERSION: &str = "1";

/// A single indexed track row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub mtime: i64,
    pub indexed_at: i64,
}

/// The full library index returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIndex {
    pub tracks: Vec<Track>,
    pub scan_roots: Vec<String>,
}

/// Thread-safe handle to the SQLite-backed library index.
///
/// The `Mutex<Connection>` is held inside `Tauri` managed state so every
/// `#[tauri::command]` handler can lock it briefly, run a prepared
/// statement, and release. SQLite is happy with many short-lived
/// transactions on a single connection.
pub struct LibraryDb {
    conn: Mutex<Connection>,
}

impl LibraryDb {
    /// Open (or create) the library database at `<app_data_dir>/utoaudio/library.sqlite`,
    /// run the `CREATE TABLE IF NOT EXISTS` migrations, and stamp the schema
    /// version on first creation.
    pub fn open(app_data_dir: &PathBuf) -> Result<Self, String> {
        let dir = app_data_dir.join("utoaudio");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let db_path = dir.join("library.sqlite");
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

        // Pragmas: WAL for concurrent reads, NORMAL sync for durability.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;\n\
             PRAGMA synchronous = NORMAL;\n\
             PRAGMA foreign_keys = ON;",
        )
        .map_err(|e| e.to_string())?;

        // Migrations.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tracks (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                path         TEXT NOT NULL UNIQUE,
                title        TEXT NOT NULL,
                artist       TEXT NOT NULL,
                album        TEXT NOT NULL,
                duration_secs REAL NOT NULL,
                mtime        INTEGER NOT NULL,
                indexed_at   INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
             CREATE INDEX IF NOT EXISTS idx_tracks_album  ON tracks(album);
             CREATE INDEX IF NOT EXISTS idx_tracks_title  ON tracks(title);
             CREATE TABLE IF NOT EXISTS scan_roots (
                path      TEXT PRIMARY KEY,
                added_at  INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS schema_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );",
        )
        .map_err(|e| e.to_string())?;

        // Stamp the schema version on first creation.
        let existing: Option<String> = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if existing.is_none() {
            conn.execute(
                "INSERT INTO schema_meta (key, value) VALUES ('schema_version', ?1)",
                params![SCHEMA_VERSION],
            )
            .map_err(|e| e.to_string())?;
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Return every track + every scan root.
    pub fn get_library_index(&self) -> Result<LibraryIndex, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tracks = select_all_tracks(&conn)?;
        let scan_roots = select_scan_roots(&conn)?;
        Ok(LibraryIndex { tracks, scan_roots })
    }

    /// Re-scan a single root directory: walk it (bounded depth), upsert every
    /// audio file into `tracks`, and return the full index.
    ///
    /// The actual filesystem walk is delegated to the existing
    /// `commands::scan_library` helper — this method only handles the DB
    /// upsert + return.
    pub fn rescan_library(&self, root: &str) -> Result<LibraryIndex, String> {
        let now = now_secs();
        let entries = super::commands::scan_library(vec![root.to_string()], audio_ext_list())
            .map_err(|e| e.to_string())?;

        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        for entry in &entries {
            if entry.is_directory {
                continue;
            }
            let title = entry.name.clone();
            let artist = String::new();
            let album = String::new();
            let duration_secs = 0.0_f64;
            let mtime = entry.modified.unwrap_or(now);
            // INSERT OR REPLACE on the UNIQUE path column.
            tx.execute(
                "INSERT OR REPLACE INTO tracks
                    (path, title, artist, album, duration_secs, mtime, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    entry.path,
                    title,
                    artist,
                    album,
                    duration_secs,
                    mtime,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;

        drop(conn);
        self.get_library_index()
    }

    /// Case-insensitive substring search across title / artist / album.
    pub fn search_library(&self, query: &str, limit: u32) -> Result<Vec<Track>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let q = format!("%{}%", query.to_lowercase());
        let mut stmt = conn
            .prepare(
                "SELECT id, path, title, artist, album, duration_secs, mtime, indexed_at
                 FROM tracks
                 WHERE LOWER(title) LIKE ?1
                    OR LOWER(artist) LIKE ?1
                    OR LOWER(album) LIKE ?1
                 ORDER BY artist, album, title
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![q, limit as i64], row_to_track)
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Add a scan root (idempotent — duplicate paths are ignored).
    pub fn add_scan_root(&self, path: &str) -> Result<(), String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("scan root path is empty".to_string());
        }
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO scan_roots (path, added_at) VALUES (?1, ?2)",
            params![trimmed, now_secs()],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Remove a scan root.
    pub fn remove_scan_root(&self, path: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM scan_roots WHERE path = ?1",
            params![path],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Return every configured scan root, sorted alphabetically.
    pub fn get_scan_roots(&self) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        select_scan_roots(&conn)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn select_all_tracks(conn: &Connection) -> Result<Vec<Track>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, path, title, artist, album, duration_secs, mtime, indexed_at
             FROM tracks
             ORDER BY artist, album, title",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_track)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn select_scan_roots(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM scan_roots ORDER BY path")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        artist: row.get(3)?,
        album: row.get(4)?,
        duration_secs: row.get(5)?,
        mtime: row.get(6)?,
        indexed_at: row.get(7)?,
    })
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The audio extension list passed to `scan_library` during a rescan.
/// Kept in sync with `apps/desktop/src/lib/file-browser.ts → AUDIO_EXTENSIONS`.
fn audio_ext_list() -> Vec<String> {
    vec![
        ".flac".into(),
        ".wav".into(),
        ".mp3".into(),
        ".opus".into(),
        ".ogg".into(),
        ".aac".into(),
        ".m4a".into(),
        ".wv".into(),
        ".dsf".into(),
        ".dff".into(),
        ".aiff".into(),
        ".ape".into(),
        ".wma".into(),
    ]
}

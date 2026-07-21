// This file is part of utoaudio, licensed under AGPL-3.0.
//
// `audio-ffi::library` — SQLite-backed library index.
//
// The library index persists across `pnpm tauri dev` restarts in a single
// SQLite database at `<app_data_dir>/utoaudio/library.sqlite`. The `bundled`
// feature on `rusqlite` avoids a system libsqlite dependency on Linux and
// Android.
//
// Schema (version 2):
//   tracks(id, path UNIQUE, title, artist, album, duration_secs, mtime, indexed_at, album_art_path)
//   scan_roots(path PRIMARY KEY, added_at)
//   schema_meta(key PRIMARY KEY, value)
//
// All multi-step writes are wrapped in a transaction. User input is always
// bound via prepared statements — never string-concatenated into SQL.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::playlist::migrate_playlist_tables;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Current schema version. Bumped on any migration.
const SCHEMA_VERSION: &str = "4";

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
    #[serde(default)]
    pub album_art_path: Option<String>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub bits_per_sample: Option<u32>,
}

/// The full library index returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIndex {
    pub tracks: Vec<Track>,
    pub scan_roots: Vec<String>,
}

/// A single entry in the playback queue, persisted to SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueTrack {
    pub path: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub album_art_path: Option<String>,
}

/// Thread-safe handle to the SQLite-backed library index.
///
/// The `Mutex<Connection>` is held inside `Tauri` managed state so every
/// `#[tauri::command]` handler can lock it briefly, run a prepared
/// statement, and release. SQLite is happy with many short-lived
/// transactions on a single connection.
pub struct LibraryDb {
    conn: Mutex<Connection>,
    art_dir: PathBuf,
    music_root: Option<String>,
}

impl LibraryDb {
    /// Acquire a short-lived lock on the inner connection.
    pub(crate) fn conn_lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|e| e.to_string())
    }
    pub(crate) fn art_dir(&self) -> &PathBuf {
        &self.art_dir
    }
    pub(crate) fn music_root(&self) -> &Option<String> {
        &self.music_root
    }
    /// Open (or create) the library database at `<app_data_dir>/utoaudio/library.sqlite`,
    /// run the `CREATE TABLE IF NOT EXISTS` migrations, and stamp the schema
    /// version on first creation.
    pub fn open(app_data_dir: &PathBuf, music_root: Option<String>) -> Result<Self, String> {
        let dir = app_data_dir.join("utoaudio");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let art_dir = dir.join("art");
        std::fs::create_dir_all(&art_dir).map_err(|e| e.to_string())?;
        let db_path = dir.join("library.sqlite");
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

        // Pragmas: WAL for concurrent reads, NORMAL sync for durability.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;\n\
             PRAGMA synchronous = NORMAL;\n\
             PRAGMA foreign_keys = ON;",
        )
        .map_err(|e| e.to_string())?;

        // Base schema (version 2 includes album_art_path).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tracks (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                path             TEXT NOT NULL UNIQUE,
                title            TEXT NOT NULL,
                artist           TEXT NOT NULL,
                album            TEXT NOT NULL,
                duration_secs    REAL NOT NULL,
                mtime            INTEGER NOT NULL,
                indexed_at       INTEGER NOT NULL,
                album_art_path   TEXT,
                sample_rate      INTEGER,
                bits_per_sample  INTEGER
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
             );
             CREATE TABLE IF NOT EXISTS playback_queue (
                position        INTEGER PRIMARY KEY,
                path            TEXT NOT NULL,
                title           TEXT,
                artist          TEXT,
                album           TEXT,
                duration_secs   REAL,
                album_art_path  TEXT
             );",
        )
        .map_err(|e| e.to_string())?;

        // Migration: v1 → v2 (add album_art_path column).
        let existing: Option<String> = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match existing.as_deref() {
            None => {
                conn.execute(
                    "INSERT INTO schema_meta (key, value) VALUES ('schema_version', ?1)",
                    params![SCHEMA_VERSION],
                )
                .map_err(|e| e.to_string())?;
            }
            Some("1") => {
                conn.execute_batch(
                    "ALTER TABLE tracks ADD COLUMN album_art_path TEXT;\n\
                     UPDATE schema_meta SET value = '2' WHERE key = 'schema_version';",
                )
                .map_err(|e| e.to_string())?;
            }
            Some("2") => {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS playback_queue (
                        position        INTEGER PRIMARY KEY,
                        path            TEXT NOT NULL,
                        title           TEXT,
                        artist          TEXT,
                        album           TEXT,
                        duration_secs   REAL,
                        album_art_path  TEXT
                     );
                     UPDATE schema_meta SET value = '3' WHERE key = 'schema_version';",
                )
                .map_err(|e| e.to_string())?;
            }
            Some("3") => {
                conn.execute_batch(
                    "ALTER TABLE tracks ADD COLUMN sample_rate INTEGER;\n\
                     ALTER TABLE tracks ADD COLUMN bits_per_sample INTEGER;\n\
                     UPDATE schema_meta SET value = '4' WHERE key = 'schema_version';",
                )
                .map_err(|e| e.to_string())?;
            }
            _ => {}
        }
        migrate_playlist_tables(&conn).ok();

        Ok(Self {
            conn: Mutex::new(conn),
            art_dir,
            music_root,
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
    /// upsert + return. Album art is discovered from parent-directory cover
    /// files and embedded tags (via lofty).
    pub fn rescan_library(&self, root: &str, app_handle: Option<&tauri::AppHandle>) -> Result<LibraryIndex, String> {
        let now = now_secs();
        let art_dir = self.art_dir.clone();
        let entries = super::commands::scan_library_inner(vec![root.to_string()], audio_ext_list(), &art_dir, app_handle)
            .map_err(|e| e.to_string())?;

        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        for entry in &entries {
            if entry.is_directory {
                continue;
            }
            let title = entry.title.clone().unwrap_or_else(|| entry.name.clone());
            let artist = entry.artist.clone().unwrap_or_default();
            let album = entry.album.clone().unwrap_or_default();
            let duration_secs = 0.0_f64;
            let mtime = entry.modified.unwrap_or(now);

            let album_art_path = discover_album_art(&entry.path, &self.art_dir);
            let sample_rate = entry.sample_rate;
            let bits_per_sample = entry.bits_per_sample;

            tx.execute(
                "INSERT OR REPLACE INTO tracks
                    (path, title, artist, album, duration_secs, mtime, indexed_at, album_art_path, sample_rate, bits_per_sample)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    entry.path,
                    title,
                    artist,
                    album,
                    duration_secs,
                    mtime,
                    now,
                    album_art_path,
                    sample_rate,
                    bits_per_sample,
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
                "SELECT id, path, title, artist, album, duration_secs, mtime, indexed_at, album_art_path, sample_rate, bits_per_sample
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

    /// Look up tracks by their exact path. Returns a map from path → Track
    /// for every path that exists in the library index. Used by the playlist
    /// import flow to avoid re-reading tags for files that were already
    /// scanned.
    pub fn lookup_tracks(&self, paths: &[String]) -> Result<std::collections::HashMap<String, Track>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let placeholders = paths.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, path, title, artist, album, duration_secs, mtime, indexed_at, album_art_path, sample_rate, bits_per_sample
             FROM tracks WHERE path IN ({})",
            placeholders
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let params = rusqlite::params_from_iter(paths);
        let rows = stmt
            .query_map(params, row_to_track)
            .map_err(|e| e.to_string())?;
        let mut out = std::collections::HashMap::new();
        for row in rows {
            let track = row.map_err(|e| e.to_string())?;
            out.insert(track.path.clone(), track);
        }
        Ok(out)
    }

    // ------------------------------------------------------------------
    // Playback queue persistence
    // ------------------------------------------------------------------

    /// Replace the entire playback queue with the given ordered list.
    pub fn set_queue(&self, tracks: &[QueueTrack]) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM playback_queue", [])
            .map_err(|e| e.to_string())?;
        for (i, t) in tracks.iter().enumerate() {
            tx.execute(
                "INSERT INTO playback_queue (position, path, title, artist, album, duration_secs, album_art_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    i as i64,
                    t.path,
                    t.title,
                    t.artist,
                    t.album,
                    t.duration_secs,
                    t.album_art_path,
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Return the full playback queue in position order.
    pub fn get_queue(&self) -> Result<Vec<QueueTrack>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT path, title, artist, album, duration_secs, album_art_path
                 FROM playback_queue ORDER BY position",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(QueueTrack {
                    path: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    duration_secs: row.get(4)?,
                    album_art_path: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Clear the playback queue.
    pub fn clear_queue(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM playback_queue", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn select_all_tracks(conn: &Connection) -> Result<Vec<Track>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, path, title, artist, album, duration_secs, mtime, indexed_at, album_art_path, sample_rate, bits_per_sample
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
        album_art_path: row.get(8)?,
        sample_rate: row.get(9)?,
        bits_per_sample: row.get(10)?,
    })
}

/// Discover album art for an audio file by extracting it from the file's
/// embedded metadata tags (via lofty). Extracted art is cached to `art_dir`
/// so subsequent scans skip re-extraction — the cached file is keyed by a
/// stable hash of the audio path and reused on every call after the first.
///
/// Returns the absolute path to the cached cover image, or `None` if the
/// file has no embedded artwork.
pub(crate) fn discover_album_art(audio_path: &str, art_dir: &PathBuf) -> Option<String> {
    // Stable cache key: hash of the canonical audio path. The same file
    // always maps to the same cache filename, so a second scan finds the
    // cached jpg and skips the expensive tag extraction entirely.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    audio_path.hash(&mut hasher);
    let hash = hasher.finish();
    let cache_path = art_dir.join(format!("{:016x}.jpg", hash));

    // Cache hit — return the existing cached file without re-extracting.
    if cache_path.is_file() {
        return Some(cache_path.to_string_lossy().into_owned());
    }

    // Cache miss — extract embedded artwork from the audio file's tags.
    if let Some(data) = audio_core::tauri_api::extract_embedded_artwork(audio_path) {
        if std::fs::write(&cache_path, &data).is_ok() {
            return Some(cache_path.to_string_lossy().into_owned());
        }
    }

    None
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

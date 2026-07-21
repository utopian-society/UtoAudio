// This file is part of utoaudio, licensed under AGPL-3.0.
//
// `audio-ffi::playlist` — SQLite-backed multi-playlist manager.
//
// Schema (appended to the existing `library.sqlite`):
//   playlists(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, created_at INTEGER, updated_at INTEGER)
//   playlist_tracks(id INTEGER PRIMARY KEY AUTOINCREMENT, playlist_id INTEGER NOT NULL, position INTEGER NOT NULL,
//                   path TEXT NOT NULL, title TEXT, artist TEXT, album TEXT, duration_secs REAL,
//                   album_art_path TEXT, sample_rate INTEGER, bits_per_sample INTEGER,
//                   FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE)
//   CREATE INDEX idx_pt_playlist ON playlist_tracks(playlist_id, position)

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::config::ParseOptions;
use lofty::tag::ItemKey;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::library::LibraryDb;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Summary row for a playlist (no tracks included).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInfo {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A track row inside a playlist, enriched with tag-based metadata and
/// album art so the frontend can render Library-style cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistTrackRow {
    pub id: i64,
    pub playlist_id: i64,
    pub position: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    #[serde(default)]
    pub album_art_path: Option<String>,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub bits_per_sample: Option<u32>,
    #[serde(default)]
    pub file_size: Option<u64>,
}

/// Response from `import_m3u8` containing only counts — the frontend
/// re-fetches the full track list via `get_playlist_tracks` to avoid
/// large IPC payloads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct M3u8ImportResult {
    pub imported_count: usize,
    pub missing_paths: Vec<String>,
}

/// A lightweight track descriptor used when importing from m3u8 before
/// metadata lookup.
#[derive(Debug, Clone)]
pub struct RawTrack {
    position: i64,
    path: String,
    title: Option<String>,
    artist: Option<String>,
    duration_secs: Option<f64>,
}

// ---------------------------------------------------------------------------
// Internal helpers shared with the Tauri commands module
// ---------------------------------------------------------------------------

/// Open (or create) the playlist tables in an existing connection.
pub fn migrate_playlist_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS playlists (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS playlist_tracks (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            playlist_id      INTEGER NOT NULL,
            position         INTEGER NOT NULL,
            path             TEXT NOT NULL,
            title            TEXT,
            artist           TEXT,
            album            TEXT,
            duration_secs    REAL,
            album_art_path   TEXT,
            sample_rate      INTEGER,
            bits_per_sample  INTEGER,
            FOREIGN KEY(playlist_id) REFERENCES playlists(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_pt_playlist
            ON playlist_tracks(playlist_id, position);",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Parse an m3u8 document and return raw track entries.
///
/// `base_dir` is used to resolve relative paths. When `None`, relative
/// paths are preserved verbatim.
pub fn parse_m3u8_content(content: &str, base_dir: Option<&str>) -> (Vec<RawTrack>, Vec<String>) {
    let lines: Vec<&str> = content.lines().collect();
    let mut tracks: Vec<RawTrack> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut pending_duration: Option<f64> = None;
    let mut pending_title: Option<String> = None;
    let mut position: i64 = 0;

    for raw_line in lines {
        let line = raw_line.trim();
        if line.is_empty() {
            pending_duration = None;
            pending_title = None;
            continue;
        }

        if line.starts_with("#EXTINF") {
            let rest = line.strip_prefix("#EXTINF:").unwrap_or(line);
            let comma_idx = rest.find(',').unwrap_or(rest.len());
            let dur_str = rest[..comma_idx].trim();
            let title_str = rest[comma_idx + 1..].trim();
            if let Ok(d) = dur_str.parse::<f64>() {
                pending_duration = Some(d);
            }
            if !title_str.is_empty() {
                pending_title = Some(title_str.to_string());
            }
            continue;
        }

        if line.starts_with('#') {
            continue;
        }

        // Path line
        let resolved = resolve_path(line, base_dir);
        let (artist, title) = split_artist_title(pending_title.as_deref());

        // Check file existence (best-effort)
        let path_buf = PathBuf::from(&resolved);
        let exists = path_buf.is_file();

        tracks.push(RawTrack {
            position,
            path: resolved.clone(),
            title: title.or_else(|| path_buf.file_stem().and_then(|s| s.to_str()).map(String::from)),
            artist: artist,
            duration_secs: pending_duration,
        });

        if !exists {
            missing.push(resolved);
        }

        position += 1;
        pending_duration = None;
        pending_title = None;
    }

    (tracks, missing)
}

/// Enrich a raw track with tag metadata and album art discovery.
pub fn enrich_track(
    track: &RawTrack,
    art_dir: &PathBuf,
) -> PlaylistTrackRow {
    let (title, artist, album, sample_rate, bits_per_sample) =
        read_track_metadata(&track.path);
    let display_title = track.title.as_deref().unwrap_or(title.as_deref().unwrap_or("")).to_string();
    let display_artist = track.artist.as_deref().or(artist.as_deref()).unwrap_or("").to_string();
    let album_art = discover_album_art(&track.path, art_dir);
    let file_size = std::fs::metadata(&track.path).ok().map(|m| m.len());

    PlaylistTrackRow {
        id: 0,
        playlist_id: 0,
        position: track.position,
        path: track.path.clone(),
        title: display_title,
        artist: display_artist,
        album: album.unwrap_or_default(),
        duration_secs: track.duration_secs.unwrap_or(0.0),
        album_art_path: album_art,
        sample_rate,
        bits_per_sample,
        file_size,
    }
}

/// Serialize playlist tracks to m3u8 content.
pub fn serialize_tracks_to_m3u8(tracks: &[PlaylistTrackRow], playlist_name: &str) -> String {
    let mut lines: Vec<String> = vec!["#EXTM3U".to_string()];
    if !playlist_name.is_empty() {
        lines.push(format!("#PLAYLIST:{}", playlist_name));
    }
    for t in tracks {
        let dur = if t.duration_secs > 0.0 {
            format!("{:.0}", t.duration_secs)
        } else {
            "-1".to_string()
        };
        let name = Path::new(&t.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown");
        let title = if !t.title.is_empty() {
            t.title.as_str()
        } else {
            name
        };
        lines.push(format!("#EXTINF:{},{}", dur, title));
        lines.push(t.path.clone());
    }
    lines.push(String::new());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// SQLite operations — all take &mut Connection (MutexGuard derefs mut)
// ---------------------------------------------------------------------------

pub fn db_list_playlists(conn: &mut Connection) -> Result<Vec<PlaylistInfo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, p.created_at, p.updated_at, COUNT(pt.id)
             FROM playlists p
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
             GROUP BY p.id
             ORDER BY p.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PlaylistInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: row.get(4)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn db_create_playlist(conn: &mut Connection, name: &str) -> Result<i64, String> {
    let now = now_secs();
    conn.execute(
        "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?3)",
        params![name, now, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn db_rename_playlist(conn: &mut Connection, id: i64, name: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now_secs(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn db_delete_playlist(conn: &mut Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn db_get_playlist_tracks(conn: &mut Connection, id: i64) -> Result<Vec<PlaylistTrackRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, playlist_id, position, path, title, artist, album, duration_secs,
                    album_art_path, sample_rate, bits_per_sample
             FROM playlist_tracks
             WHERE playlist_id = ?1
             ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![id], row_to_track)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Enrich raw tracks using ONLY the library cache (DB metadata).
/// Tracks not found in the library index are skipped entirely.
/// No file I/O — purely DB lookup. Fast m3u8 import.
pub fn enrich_tracks_from_cache(
    tracks: &[RawTrack],
    library_cache: &std::collections::HashMap<String, crate::library::Track>,
) -> Result<Vec<PlaylistTrackRow>, String> {
    let mut result = Vec::with_capacity(tracks.len());
    for t in tracks {
        if let Some(lib_track) = library_cache.get(&t.path) {
            result.push(PlaylistTrackRow {
                id: 0,
                playlist_id: 0,
                position: t.position,
                path: t.path.clone(),
                title: t.title.clone().unwrap_or_else(|| lib_track.title.clone()),
                artist: t.artist.clone().unwrap_or_else(|| lib_track.artist.clone()),
                album: lib_track.album.clone(),
                duration_secs: t.duration_secs.unwrap_or(lib_track.duration_secs),
                album_art_path: lib_track.album_art_path.clone(),
                sample_rate: lib_track.sample_rate,
                bits_per_sample: lib_track.bits_per_sample,
                file_size: None,
            });
        }
        // Tracks not in library index are silently skipped
    }
    Ok(result)
}

/// Insert pre-enriched tracks into the DB (fast — no file I/O).
/// Returns a summary with imported count and any missing paths.
pub fn db_add_enriched_tracks(
    conn: &mut Connection,
    playlist_id: i64,
    tracks: Vec<PlaylistTrackRow>,
) -> Result<M3u8ImportResult, String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let max_pos: Option<i64> = tx
        .query_row(
            "SELECT MAX(position) FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |r| r.get(0),
        )
        .ok();

    let mut start_pos = max_pos.unwrap_or(-1) + 1;
    let mut seen_paths: HashSet<String> = HashSet::new();
    let mut tracks_out = Vec::with_capacity(tracks.len());

    for mut row in tracks {
        if seen_paths.contains(&row.path) {
            continue;
        }
        seen_paths.insert(row.path.clone());

        row.position = start_pos;
        tx.execute(
            "INSERT INTO playlist_tracks
                (playlist_id, position, path, title, artist, album, duration_secs,
                 album_art_path, sample_rate, bits_per_sample)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                playlist_id,
                row.position,
                row.path,
                row.title,
                row.artist,
                row.album,
                row.duration_secs,
                row.album_art_path,
                row.sample_rate,
                row.bits_per_sample,
            ],
        )
        .map_err(|e| e.to_string())?;

        // Get the auto-assigned ID from the database
        row.id = tx.last_insert_rowid();
        row.playlist_id = playlist_id;
        tracks_out.push(row);
        start_pos += 1;
    }

    tx.execute(
        "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
        params![now_secs(), playlist_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(M3u8ImportResult {
        imported_count: tracks_out.len(),
        missing_paths: Vec::new(),
    })
}

pub fn db_remove_track(conn: &mut Connection, track_id: i64) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Fetch position + playlist_id before deletion.
    let (pl_id, pos): (i64, i64) = tx
        .query_row(
            "SELECT playlist_id, position FROM playlist_tracks WHERE id = ?1",
            params![track_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM playlist_tracks WHERE id = ?1", params![track_id])
        .map_err(|e| e.to_string())?;

    // Compact positions after the deleted track.
    tx.execute(
        "UPDATE playlist_tracks SET position = position - 1
         WHERE playlist_id = ?1 AND position > ?2",
        params![pl_id, pos],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
        params![now_secs(), pl_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())
}

pub fn db_move_track(conn: &mut Connection, track_id: i64, direction: &str) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let (pl_id, pos): (i64, i64) = tx
        .query_row(
            "SELECT playlist_id, position FROM playlist_tracks WHERE id = ?1",
            params![track_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let swap_pos = match direction {
        "up" => pos - 1,
        "down" => pos + 1,
        _ => return Ok(()),
    };
    if swap_pos < 0 {
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Find the track at the swap position.
    let swap_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
            params![pl_id, swap_pos],
            |r| r.get(0),
        )
        .ok();

    if let Some(sid) = swap_id {
        tx.execute(
            "UPDATE playlist_tracks SET position = ?1 WHERE id = ?2",
            params![pos, sid],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.execute(
        "UPDATE playlist_tracks SET position = ?1 WHERE id = ?2",
        params![swap_pos, track_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE playlists SET updated_at = ?1 WHERE id = ?2",
        params![now_secs(), pl_id],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())
}

pub fn db_export_playlist(conn: &mut Connection, id: i64) -> Result<(String, String), String> {
    let name: String = conn
        .query_row("SELECT name FROM playlists WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    let tracks = db_get_playlist_tracks(conn, id)?;
    let track_rows: Vec<_> = tracks
        .into_iter()
        .map(|t| PlaylistTrackRow {
            id: 0,
            playlist_id: 0,
            position: 0,
            path: t.path,
            title: t.title,
            artist: t.artist,
            album: t.album,
            duration_secs: t.duration_secs,
            album_art_path: t.album_art_path,
            sample_rate: t.sample_rate,
            bits_per_sample: t.bits_per_sample,
            file_size: t.file_size,
        })
        .collect();
    let content = serialize_tracks_to_m3u8(&track_rows, &name);
    Ok((name, content))
}

// ---------------------------------------------------------------------------
// Tauri commands — use Arc<LibraryDb> managed state (same as library commands)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_playlists(db: tauri::State<'_, Arc<LibraryDb>>) -> Result<Vec<PlaylistInfo>, String> {
    let mut conn = db.conn_lock()?;
    db_list_playlists(&mut conn)
}

#[tauri::command]
pub fn create_playlist(db: tauri::State<'_, Arc<LibraryDb>>, name: String) -> Result<i64, String> {
    let mut conn = db.conn_lock()?;
    db_create_playlist(&mut conn, &name)
}

#[tauri::command]
pub fn rename_playlist(
    db: tauri::State<'_, Arc<LibraryDb>>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let mut conn = db.conn_lock()?;
    db_rename_playlist(&mut conn, id, &name)
}

#[tauri::command]
pub fn delete_playlist(db: tauri::State<'_, Arc<LibraryDb>>, id: i64) -> Result<(), String> {
    let mut conn = db.conn_lock()?;
    db_delete_playlist(&mut conn, id)
}

#[tauri::command]
pub fn get_playlist_tracks(
    db: tauri::State<'_, Arc<LibraryDb>>,
    id: i64,
) -> Result<Vec<PlaylistTrackRow>, String> {
    let mut conn = db.conn_lock()?;
    db_get_playlist_tracks(&mut conn, id)
}

#[tauri::command]
pub async fn add_tracks_to_playlist(
    db: tauri::State<'_, Arc<LibraryDb>>,
    playlist_id: i64,
    paths: Vec<String>,
) -> Result<Vec<PlaylistTrackRow>, String> {
    let raw_tracks: Vec<RawTrack> = paths
        .into_iter()
        .enumerate()
        .map(|(i, p)| RawTrack {
            position: i as i64,
            path: p,
            title: None,
            artist: None,
            duration_secs: None,
        })
        .collect();

    if raw_tracks.is_empty() {
        return Ok(vec![]);
    }

    let all_paths: Vec<String> = raw_tracks.iter().map(|t| t.path.clone()).collect();
    let library_cache = db.lookup_tracks(&all_paths)?;

    let enriched = enrich_tracks_from_cache(&raw_tracks, &library_cache)?;

    let mut conn = db.conn_lock()?;
    let result = db_add_enriched_tracks(&mut conn, playlist_id, enriched)?;
    // Get the full track list for the playlist
    db_get_playlist_tracks(&mut conn, playlist_id)
}

#[tauri::command]
pub async fn import_m3u8_to_playlist(
    db: tauri::State<'_, Arc<LibraryDb>>,
    playlist_id: i64,
    content: String,
    base_dir: Option<String>,
) -> Result<M3u8ImportResult, String> {
    let (raw_tracks, _) = parse_m3u8_content(&content, base_dir.as_deref());
    if raw_tracks.is_empty() {
        return Ok(M3u8ImportResult::default());
    }

    let all_paths: Vec<String> = raw_tracks.iter().map(|t| t.path.clone()).collect();
    let library_cache = db.lookup_tracks(&all_paths)?;

    let enriched = enrich_tracks_from_cache(&raw_tracks, &library_cache)?;

    let mut conn = db.conn_lock()?;
    let result = db_add_enriched_tracks(&mut conn, playlist_id, enriched)?;
    Ok(M3u8ImportResult {
        imported_count: result.imported_count,
        missing_paths: Vec::new(),
    })
}

#[tauri::command]
pub async fn import_m3u8_to_playlist_with_root(
    db: tauri::State<'_, Arc<LibraryDb>>,
    playlist_id: i64,
    content: String,
    m3u8_path: String,
) -> Result<M3u8ImportResult, String> {
    let db = db.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        import_m3u8_to_playlist_inner(&db, playlist_id, &content, &m3u8_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn import_m3u8_to_playlist_inner(
    db: &LibraryDb,
    playlist_id: i64,
    content: &str,
    m3u8_path: &str,
) -> Result<M3u8ImportResult, String> {
    let scan_roots = db.get_scan_roots()?;
    if scan_roots.is_empty() {
        return Err("No scan roots configured. Add scan directories in Settings → Library.".to_string());
    }

    let base_dir = std::path::Path::new(m3u8_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string());
    let (mut raw_tracks, missing) = parse_m3u8_content(content, base_dir.as_deref());

    for track in &mut raw_tracks {
        let resolved = if track.path.starts_with('/') || track.path.starts_with("\\\\") || (track.path.len() >= 2 && track.path.as_bytes()[1] == b':') {
            track.path.clone()
        } else if let Some(ref base) = base_dir {
            let base_path = std::path::Path::new(base);
            base_path.join(&track.path).to_string_lossy().into_owned()
        } else {
            track.path.clone()
        };
        track.path = resolved;
    }

    let all_paths: Vec<String> = raw_tracks.iter().map(|t| t.path.clone()).collect();
    let library_cache = db.lookup_tracks(&all_paths)?;
    eprintln!("[import] total_raw={} cache_hits={} cache_keys={}", raw_tracks.len(), library_cache.len(), library_cache.len());

    let mut enriched = Vec::new();
    let mut final_missing = Vec::new();
    let mut cache_hits = 0;
    let mut enrich_calls = 0;

    for track in raw_tracks {
        if let Some(lib_track) = library_cache.get(&track.path) {
            cache_hits += 1;
            enriched.push(PlaylistTrackRow {
                id: 0,
                playlist_id: 0,
                position: track.position,
                path: track.path.clone(),
                title: track.title.clone().unwrap_or_else(|| lib_track.title.clone()),
                artist: track.artist.clone().unwrap_or_else(|| lib_track.artist.clone()),
                album: lib_track.album.clone(),
                duration_secs: track.duration_secs.unwrap_or(lib_track.duration_secs),
                album_art_path: lib_track.album_art_path.clone(),
                sample_rate: lib_track.sample_rate,
                bits_per_sample: lib_track.bits_per_sample,
                file_size: None,
            });
            continue;
        }

        let abs_path = std::path::Path::new(&track.path);
        let mut under_scan_root = false;
        for root in &scan_roots {
            if abs_path.strip_prefix(root).is_ok() {
                under_scan_root = true;
                break;
            }
        }

        if under_scan_root && abs_path.is_file() {
            enrich_calls += 1;
            let raw_for_enrich = RawTrack {
                position: track.position,
                path: track.path.clone(),
                title: track.title.clone(),
                artist: None,
                duration_secs: track.duration_secs,
            };
            let mut row = enrich_track(&raw_for_enrich, db.art_dir());
            if row.title.is_empty() {
                row.title = abs_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
            }
            enriched.push(row);
        } else {
            final_missing.push(track.path);
        }
    }

    let mut conn = db.conn_lock()?;
    let result = db_add_enriched_tracks(&mut conn, playlist_id, enriched)?;
    eprintln!("[import] result: cache_hits={} enrich_calls={} missing={} inserted={}", cache_hits, enrich_calls, final_missing.len(), result.imported_count);
    Ok(M3u8ImportResult {
        imported_count: result.imported_count,
        missing_paths: [missing, final_missing].concat(),
    })
}

#[tauri::command]
pub fn remove_playlist_track(
    db: tauri::State<'_, Arc<LibraryDb>>,
    track_id: i64,
) -> Result<(), String> {
    let mut conn = db.conn_lock()?;
    db_remove_track(&mut conn, track_id)
}

#[tauri::command]
pub fn move_playlist_track(
    db: tauri::State<'_, Arc<LibraryDb>>,
    track_id: i64,
    direction: String,
) -> Result<(), String> {
    let mut conn = db.conn_lock()?;
    db_move_track(&mut conn, track_id, &direction)
}

#[tauri::command]
pub fn export_playlist(
    db: tauri::State<'_, Arc<LibraryDb>>,
    id: i64,
) -> Result<String, String> {
    let mut conn = db.conn_lock()?;
    let (_, content) = db_export_playlist(&mut conn, id)?;
    Ok(content)
}

#[tauri::command]
pub fn export_playlist_name(
    db: tauri::State<'_, Arc<LibraryDb>>,
    id: i64,
) -> Result<String, String> {
    let mut conn = db.conn_lock()?;
    let (name, _) = db_export_playlist(&mut conn, id)?;
    Ok(name)
}

// ---------------------------------------------------------------------------
// Column mapper
// ---------------------------------------------------------------------------

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlaylistTrackRow> {
    Ok(PlaylistTrackRow {
        id: row.get(0)?,
        playlist_id: row.get(1)?,
        position: row.get(2)?,
        path: row.get(3)?,
        title: row.get(4)?,
        artist: row.get(5)?,
        album: row.get(6)?,
        duration_secs: row.get(7)?,
        album_art_path: row.get(8)?,
        sample_rate: row.get(9)?,
        bits_per_sample: row.get(10)?,
        file_size: None,
    })
}

// ---------------------------------------------------------------------------
// Shared metadata helpers
// ---------------------------------------------------------------------------

fn read_track_metadata(path: &str) -> (Option<String>, Option<String>, Option<String>, Option<u32>, Option<u32>) {
    let parse_options = ParseOptions::new().read_properties(true);
    let mut probe = match Probe::open(path) {
        Ok(p) => p.options(parse_options),
        Err(_) => return (None, None, None, None, None),
    };
    let mut probe = match probe.guess_file_type() {
        Ok(p) => p,
        Err(_) => return (None, None, None, None, None),
    };
    let tagged_file = match probe.read() {
        Ok(f) => f,
        Err(_) => return (None, None, None, None, None),
    };
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
    let title = tag.and_then(|t| t.get_string(&ItemKey::TrackTitle).map(|s| s.to_string()));
    let artist = tag.and_then(|t| t.get_string(&ItemKey::TrackArtist).map(|s| s.to_string()));
    let album = tag.and_then(|t| t.get_string(&ItemKey::AlbumTitle).map(|s| s.to_string()));
    let props = tagged_file.properties();
    let sample_rate = props.sample_rate();
    let bits_per_sample = props.bit_depth().map(|b| b as u32);
    (title, artist, album, sample_rate, bits_per_sample)
}

pub(crate) fn discover_album_art(audio_path: &str, art_dir: &PathBuf) -> Option<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    audio_path.hash(&mut hasher);
    let hash = hasher.finish();
    let cache_path = art_dir.join(format!("{:016x}.jpg", hash));

    if cache_path.is_file() {
        return Some(cache_path.to_string_lossy().into_owned());
    }

    if let Some(data) = audio_core::tauri_api::extract_embedded_artwork(audio_path) {
        if std::fs::write(&cache_path, &data).is_ok() {
            return Some(cache_path.to_string_lossy().into_owned());
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Resolve relative paths against base_dir and canonicalize to match library paths.
fn resolve_path(path: &str, base_dir: Option<&str>) -> String {
    let trimmed = path.trim();
    let is_absolute = trimmed.starts_with('/')
        || trimmed.starts_with("\\\\")
        || (trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':');
    let resolved = if is_absolute {
        trimmed.to_string()
    } else {
        match base_dir {
            Some(base) if !base.is_empty() => {
                let trimmed_base = base.trim_end_matches(['/', '\\']);
                format!("{}/{}", trimmed_base, trimmed)
            }
            _ => trimmed.to_string(),
        }
    };
    // Canonicalize to match library paths (handles symlinks, case differences)
    // Skipped for performance on slow external drives — resolved path is used as-is.
    resolved.to_string()
}

fn split_artist_title(input: Option<&str>) -> (Option<String>, Option<String>) {
    let input = match input {
        Some(s) if !s.is_empty() => s,
        _ => return (None, None),
    };
    if let Some(m) = input.match_indices(" - ").next() {
        let artist = Some(input[..m.0].trim().to_string());
        let title = Some(input[m.0 + 3..].trim().to_string());
        return (artist, title);
    }
    (None, Some(input.to_string()))
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

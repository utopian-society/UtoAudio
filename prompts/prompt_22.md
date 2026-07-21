## Context (carry forward)
utoaudio is an audiophile music player (AGPL-3.0). Stack: Svelte 5 + Tauri 2 + Rust workspace (crates/audio-core wraps vendor/flick submodule, audio-ffi has Tauri commands + SQLite library). Frontend at apps/desktop/src. Palette: pure white base, translucent glass, pale green #bef264 + yellow #fef08a accents. LiquidGlass component from liquid-glass-svelte submodule. Library page is apps/desktop/src/pages/Library.svelte (898 lines, Svelte 5 runes). SQLite schema in crates/audio-ffi/src/library.rs has tracks table (id, path, title, artist, album, duration_secs, mtime, indexed_at) — no album art column. The lofty crate (metadata extraction) is already a transitive dependency through vendor/flick. SongInfo serde type in crates/audio-core/src/tauri_api.rs has no album_art_path field.

## Task
Redesign the Library file browser: vertical rows (one per file/folder, not a CSS grid of cards) with an album art column. Add album art discovery + storage pipeline.

## Requirements

### Layout
MUST replace the current CSS grid (`grid-template-columns: repeat(auto-fill, minmax(220px, 1fr))`) with a vertical row layout. Each row spans full width, contains columns: album art thumbnail (left), name + metadata (centre), actions (right). Use the existing `LiquidGlass` wrapper on each row.

### Album art pipeline
MUST add an `album_art_path TEXT` column to the SQLite `tracks` table (with migration — bump schema_meta version to 2). MUST add `album_art_path: Option<String>` to `SongInfo` in `crates/audio-core/src/tauri_api.rs`. MUST add it to the frontend `SongInfo` mirror interface in Library.svelte.

During `rescan_library` in `crates/audio-ffi/src/library.rs`, for each audio file entry, scan its parent directory for cover art files matching: `cover.jpg`, `cover.jpeg`, `cover.png`, `folder.jpg`, `folder.jpeg`, `folder.png`, `albumart.jpg`, `albumart.jpeg`, `albumart.png`, `front.jpg`, `front.jpeg`, `front.png` (case-insensitive). Store the first match's absolute path in `album_art_path`. Also attempt to extract embedded cover art from the audio file using `lofty` — if found, write it to a temp cache file under `<app_data_dir>/utoaudio/art/<track_id>.jpg` and store that path.

MUST add a Tauri command `get_album_art_data(path: String) -> Vec<u8>` in `crates/audio-ffi/src/lib.rs` (inside `commands` module) that reads a file and returns its bytes, so the frontend can load cover images via IPC. Register it in `apps/desktop/src-tauri/src/lib.rs`'s `generate_handler!`.

### Frontend
In Library.svelte, each audio file row MUST show a 48×48 album art thumbnail. Use `invoke('get_album_art_data', { path })` to fetch bytes, convert to a `blob:` URL via `URL.createObjectURL(new Blob([data]))`. Show a music icon fallback when `album_art_path` is null/empty. The row layout MUST show: thumbnail | name (bold) + artist + album (grey, smaller) | duration | "+" queue button. Folder rows show folder icon | name | "Folder" label | entry count (if available).

## Scope lock
TOUCH ONLY:
- apps/desktop/src/pages/Library.svelte
- crates/audio-core/src/tauri_api.rs
- crates/audio-ffi/src/library.rs
- crates/audio-ffi/src/lib.rs
- apps/desktop/src-tauri/src/lib.rs

NEVER touch: vendor/flick, apps/desktop/src/components/lyrics/, apps/desktop/src/App.svelte, apps/desktop/src/app.css, any other page, Cargo.toml files.

## Stop conditions
MUST stop and ask before: adding any npm or Cargo dependency, modifying the SQLite schema beyond the one column + version bump, changing the LiquidGlass component, touching any file outside the scope lock.

## Verification
After changes, MUST pass: `cargo build --workspace`, `cargo test --workspace --exclude rust_lib_flick_player`, `cd apps/desktop && pnpm run check`, `cd apps/desktop && pnpm run build`.

🎯 Target: glm5.2, 💡 Reasoning model — kept short and declarative. Agentic controls added for filesystem access safety.

This prompt is for an agentic tool with real system access. Review the scope locks, forbidden actions, and stop conditions before pasting. Confirm file paths, directories, and permissions match the actual project.

Setup note for both prompts: The vendor/flick submodule has uncommitted local changes (1 commit ahead — crate-type addition at 510576e). Do NOT commit or push any submodule changes unless explicitly instructed. Work on the submodule's working tree directly.

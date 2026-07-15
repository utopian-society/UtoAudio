You are a senior Svelte 5 + Tauri engineer working on `utoaudio` (an AGPL-3.0 audiophile music player, Linux desktop + Android target, Svelte 5 runes + Rust workspace at `/home/bibichan/Programming/utoaudio`). First, read `/home/bibichan/Programming/utoaudio/AGENTS.md` and `/home/bibichan/Programming/utoaudio/progress.md` fully before doing anything — they are the authoritative state record. Append a new section to `progress.md` when finished.

## Architectural decisions (locked, do not relitigate)

1. **Dark mode is gone — finish the job.** Prompt 8 attempted to remove dark mode but the work was incomplete. The app still defaults to dark mode on startup, and the Settings page cannot be scrolled, so users cannot reach the theme controls. Complete the removal: delete every remaining dark-mode branch, dark-mode CSS variable, dark-mode toggle, and dark-mode default. The app must launch in light mode (or follow system preference) with no UI to switch. Audit `apps/desktop/src/app.css`, `apps/desktop/src/lib/store.svelte.ts`, `apps/desktop/src/App.svelte`, `apps/desktop/src/pages/Settings.svelte`, `apps/desktop/src/pages/NowPlaying.svelte`, and the `LyricPlayer` `theme.light` wiring — strip every dark path. All `--uto-*` tokens converge to the current light values. The Settings page must be fully scrollable end-to-end (likely a missing `overflow-y: auto` on the scroll container, or a fixed-height parent clipping the content).

2. **Persistent config: settings stay in JSON, library index moves to SQLite via `rusqlite`.** No TOML, no YAML, no other DB engine. Split the persistence layer cleanly:
   - **Settings** stay in the existing JSON file from prompt 8 at `tauri::api::path::app_config_dir()?.join("utoaudio/settings.json")`, schema `{"version": 2, "settings": {...}}`. No change here.
   - **Library index** moves to a SQLite database at `tauri::api::path::app_data_dir()?.join("utoaudio/library.sqlite")` (use `app_data_dir`, not `app_config_dir` — DBs are data, not config). Add `rusqlite = { version = "0.31", features = ["bundled"] }` to `crates/audio-ffi/Cargo.toml` (the `bundled` feature avoids a system libsqlite dependency on Linux/Android). Schema:
     ```sql
     CREATE TABLE IF NOT EXISTS tracks (
       id          INTEGER PRIMARY KEY AUTOINCREMENT,
       path        TEXT NOT NULL UNIQUE,
       title       TEXT NOT NULL,
       artist      TEXT NOT NULL,
       album       TEXT NOT NULL,
       duration_secs REAL NOT NULL,
       mtime       INTEGER NOT NULL,
       indexed_at  INTEGER NOT NULL
     );
     CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
     CREATE INDEX IF NOT EXISTS idx_tracks_album  ON tracks(album);
     CREATE INDEX IF NOT EXISTS idx_tracks_title  ON tracks(title);
     CREATE TABLE IF NOT EXISTS scan_roots (
       path        TEXT PRIMARY KEY,
       added_at    INTEGER NOT NULL
     );
     CREATE TABLE IF NOT EXISTS schema_meta (
       key   TEXT PRIMARY KEY,
       value TEXT NOT NULL
     );
     ```
   - On startup, open the DB, run the `CREATE TABLE IF NOT EXISTS` migrations, and read `schema_meta` for the version. Write `key='schema_version', value='1'` on first creation.
   - Add Tauri commands in `crates/audio-ffi/src/library.rs`: `get_library_index() -> LibraryIndex`, `rescan_library(path: String) -> LibraryIndex`, `search_library(query: String, limit: u32) -> Vec<Track>`, `add_scan_root(path: String)`, `remove_scan_root(path: String)`, `get_scan_roots() -> Vec<String>`. Use prepared statements, parameter binding (never string-concatenate user input), and wrap multi-step writes in a transaction.
   - The frontend `Library.svelte` must read from this SQLite-backed index instead of re-scanning on every mount. Debounced writes on mutation, same pattern as prompt 8's settings persistence.
   - The existing JSON `settings.json` is untouched by this change — settings and library index are now two separate stores with two separate files.

## Bugs to fix (real, not architectural)

3. **Library tab buttons are still unresponsive.** Prompt 8 attempted this and failed. Re-investigate `apps/desktop/src/pages/Library.svelte` from scratch. Read the file in full before patching. Likely root causes, in order of probability:
   - The click handler closes over the wrong `entry` reference (Svelte 5 runes — closures over `$state` snapshots in loops need careful identity handling; verify with `{#each entries as entry (entry.path)}` keyed iteration).
   - `enterDirectory` / `playEntry` / `queueNext` call sites were lost in a prior refactor and never restored.
   - The Tauri `invoke('scan_directory', { path })` call returns an error at runtime that is silently swallowed (no `try/catch`, no `.catch()`, no error UI).
   - The `onclick` is on a child element while a parent `pointer-events: none` style was applied during the glass migration.
   - The new `get_library_index` / `rescan_library` commands from decision 2 are not yet wired into the page.
   Fix at the root. Re-verify by reading `playEntry`, `enterDirectory`, `queueNext`, and every `onclick` binding on folder/file/root cards. Ensure roots-level clicks, folder navigation, audio file playback, and the queue `+` button all work. Add visible error feedback when an invoke fails.

4. **Liquid glass component shines too bright on mouse click, making text hard to read.** Find the glass primitive in use (likely `liquid-glass-svelte` from prompt 8, or the fallback `backdrop-filter` recipe in `apps/desktop/src/app.css`). The `:active` / `:hover` / `mousedown` state is over-brightening the surface. Reduce the active-state brightness/saturate/contrast boost so text contrast stays WCAG-AA readable while pressed. Likely fix: clamp the active-state `filter: brightness()` to ≤1.05 (or remove it entirely), and ensure the text color does not invert or lighten on press. Verify by reading the glass component's CSS and the `--uto-glass-*` token definitions.

5. **Program icon in the top-left titlebar is too small — enlarge it 2x.** Find the `<Logo size={N} />` (or equivalent) in `apps/desktop/src/App.svelte` titlebar. Double the current size. If the current size is `22`, set it to `44`. Adjust the `.title-group` spacing in the same file's styles so the larger logo does not overlap the window title or the minimise/close buttons. Verify the layout at the default window size and at the minimum window size.

## Output contract

- Read `AGENTS.md` + `progress.md` FIRST. Do not re-derive documented decisions.
- All edits stay within the existing architecture: Rust in `crates/`, Svelte in `apps/desktop/src/`, Tauri shell in `apps/desktop/src-tauri/`. No new top-level directories.
- Honour AGPL-3.0. Any third-party code (including `liquid-glass-svelte`) must be recorded in `THIRD_PARTY_LICENSES.md` with original license + commit hash.
- Run `cd apps/desktop && pnpm run check && pnpm run build` and `cargo build --workspace` at the end. All must pass with zero new errors. The pre-existing 163/164 audio-core test result, the inherited DSD test failure, and the clippy-component-missing limitation are out of scope — do not touch.
- Append a `## What prompt 9 did —` section to `progress.md` in the format specified by AGENTS.md rule 6, covering files, verification, architectural decisions, and hand-off notes.

## Stop conditions

- **Stop and ask before:** deleting any file outside the dark-mode audit list, the settings/library persistence files, or the glass CSS; adding any new dependency beyond `rusqlite` (with the `bundled` feature) in `crates/audio-ffi/Cargo.toml`; modifying `Cargo.lock` indirectly via a different crate; changing the Tauri `tauri.conf.json` schema; changing the on-disk JSON settings schema beyond the documented `version: 2`; changing the SQLite schema after first write without a migration.
- Do not refactor beyond what is asked. Do not redesign the page layouts. Do not rewrite NowPlaying, Playlist, or the AMLL lyric components — wire from them only.
- If the Library click handlers cannot be located after inspecting the relevant files, STOP and report what you found instead of guessing fixes.
- If the glass component's active-state CSS cannot be located (e.g. it lives inside a compiled vendored fork with no source map), STOP and ask before patching the vendored copy.

## Done when

1. App launches in light mode (or follows system preference) with no dark-mode code path remaining anywhere in the repo. Settings page scrolls smoothly end-to-end.
2. `settings.json` (version 2) persists user settings across `pnpm tauri dev` restarts. The audio file index persists in `library.sqlite` (rusqlite, `bundled` feature) across restarts. `Library.svelte` reads from the SQLite-backed index instead of re-scanning on every mount.
3. Clicking a root card, a folder card, and an audio file card in Library all produce visible behaviour: folder navigates, audio invokes the `play` command, queue `+` invokes `queue_next`. Failed invokes show a visible error.
4. The liquid glass surface stays readable when pressed — text contrast does not collapse on `:active`.
5. The titlebar logo is visibly 2x larger than before and the titlebar layout still aligns cleanly.
6. `pnpm run check`, `pnpm run build`, `cargo build --workspace` all exit 0.
7. `progress.md` has a new prompt-9 section.
🎯 Target: MiniMax-M3, 💡 Compressed five-issue bug-fix prompt with locked architectural decisions (dark-mode completion + split persistence: JSON settings, rusqlite library index), explicit stop conditions, and a verifiable done contract — adapted for MiniMax's strong instruction-following and long-context synthesis.

## Objective
Wire the Now Playing page into a fully functional immersive AMLL lyric display with
transport controls, album art, and persistent "last played" track state across app
restarts.

## Context
utoaudio is a Svelte 5 + Tauri 2.x + Rust audiophile music player. The "Now Playing"
page (`apps/desktop/src/pages/NowPlaying.svelte`) is the visual centrepiece — it
already has the `LyricPlayer` + `FluidBackground` Svelte 5 components rendering, the
engine event-stream listener (`start_event_stream` + `listen('audio-event')`),
transport controls (play/pause, seek bar), and a theme-extraction pipeline from album
art. However, **none of it is wired to live data**:

- `currentTrack` is `$state(null)` and never assigned — the page always shows
  "Nothing playing".
- `albumArtUrl` is `$state('')` and never set — `FluidBackground` has nothing to
  render, and no album art appears in the transport area.
- `lyricLines` is `$state([])` and never populated — `LyricPlayer` renders nothing.
- The `next_track_ready` event handler has a comment `// Future prompt wires
  currentTrack + lyric auto-load here`.
- There is no persistence: closing and reopening the app resets to "Nothing playing".

What the Playlist page already does (as a reference pattern):
- Periodically polls `invoke('current_path')` every 1500ms to detect the currently
  playing track path.
- Compares it against its own track list to highlight the active row.

What the engine emits as events:
- `state_changed` (state: idle/playing/paused/buffering/crossfading/stopped)
- `progress` (position_secs, duration_secs, buffer_level)
- `track_ended` (path)
- `crossfade_started` (from_path, to_path)
- `next_track_ready` (path)
- `error` (message)

## Target State
1. **On first launch with no music playing:** the Now Playing page shows a placeholder
   "Scan your collection and select one to play" with the `FluidBackground` in
   gradient mode and the `LyricPlayer` empty (hidden or showing the placeholder
   text).

2. **When a track is playing (from Library or Playlist):** the Now Playing page
   displays:
   - **Album art** — loaded from the track's `album_art_path` (already in
     `SongInfo.album_art_path`), displayed as a thumbnail in the transport area.
     The album art URL feeds `FluidBackground` for the dynamic blur backdrop.
   - **Song title + artist + album name** in the transport info area.
   - **Progress bar + time label** (already wired, just needs `duration` populated).
   - **Play/Pause button** (already wired via `togglePlay()`).
   - **Skip next + previous buttons** (new) — invoke `skip_to_next` and
     stop/play-previous-track (using the playlist index or just `stop` + replay).
   - **Lyric lines** — auto-load the `.lrc` file next to the audio file (same
     directory, same basename, `.lrc` extension), parse it with `parseLyrics()`,
     and feed it to `LyricPlayer`. If no `.lrc` exists, the lyric area stays empty
     (or shows the song title as a single centred line).

3. **When switching tracks (crossfade_started / next_track_ready):** the page updates
   `currentTrack` to the new track, re-loads album art, and re-loads lyrics.

4. **On app close and reopen:** the Now Playing page restores the last-playing
   track's `SongInfo` from persisted settings. If the file still exists on disk,
   the page shows its title/artist/album and album art (but does NOT auto-play —
   the user must press play). If the file no longer exists, show the placeholder.

5. **Playlist queue method button:** a button in the transport area that toggles
   between "Play Now" (replaces current track), "Play Next" (queues after current),
   and "Add to Queue" (appends to end). This button is contextual — it appears only
   when browsing the Library page and selecting a track, and sends the chosen
   queue method to the engine.

6. **The `LyricPlayer`'s `fontSize` is wired to `appState.lyricFontSize`** from
   `store.svelte.ts` (hand-off item 7 from progress.md).

## Scope
Work only in:
- `apps/desktop/src/pages/NowPlaying.svelte`
- `apps/desktop/src/lib/store.svelte.ts`
- `apps/desktop/src-tauri/src/lib.rs` (if persistence needs startup loading)
- `crates/audio-ffi/src/settings.rs` (if `last_played_track` field needed)
- `crates/audio-ffi/src/lib.rs` (if new Tauri commands needed)

Do NOT touch: `vendor/`, `crates/audio-core/`, any CSS files, `LiquidGlass.svelte`,
any submodule code, lyric components (`LyricPlayer`, `LyricLine`, `FluidBackground`).

## Constraints
- Svelte 5 runes mode (`$state`, `$derived`, `$effect`), TypeScript, Vite, Tailwind CSS.
- All engine interactions via `invoke()` (existing Tauri commands).
- The `SongInfo` type already has `path`, `title`, `artist`, `album`, `duration_secs`,
  `album_art_path`, `sample_rate`, `bits_per_sample`.
- Persist the last-playing track as JSON in `settings.json` via the existing
  `get_settings` / `set_settings` commands (add a `last_played_track` field).
- For lyric loading, read the `.lrc` file from disk. If `@tauri-apps/plugin-fs` is
  not installed, add a simple Tauri command `read_text_file(path: String) -> String`
  in `audio-ffi/src/lib.rs` and register it. Do NOT install the plugin-fs npm package
  unless it's already in the dependency tree.
- Keep the existing glass aesthetic — `LiquidGlass` transport bar at the bottom,
  `FluidBackground` full-screen behind the lyrics.
- `cargo build --workspace` must pass (0 new warnings).
- `pnpm run check` must pass (0 new errors).

## Acceptance Criteria
- [ ] First launch: Now Playing shows "Scan your collection and select one to play"
  with `FluidBackground` gradient backdrop.
- [ ] Playing a track from Library or Playlist: Now Playing shows the song title,
  artist, album, album art thumbnail, progress bar, and play/pause button.
- [ ] Album art feeds `FluidBackground` for dynamic blur.
- [ ] `.lrc` file next to the audio file is auto-loaded and parsed; `LyricPlayer`
  renders syllable-level karaoke lyrics.
- [ ] `lyricFontSize` from `appState` drives `LyricPlayer.fontSize`.
- [ ] Skip-next button invokes `skip_to_next` and updates `currentTrack`.
- [ ] Crossfade (`crossfade_started` event) updates `currentTrack` to the incoming
  track.
- [ ] `next_track_ready` event updates `currentTrack`.
- [ ] Closing and reopening the app: last-playing track's info (title, artist, album,
  album_art_path, path) is restored. If the file still exists, the page shows it
  (not auto-playing). If the file is gone, the placeholder appears.
- [ ] `cargo build --workspace` passes (9 pre-existing warnings allowed).
- [ ] `cargo test --workspace --exclude rust_lib_flick_player` passes.
- [ ] `pnpm run check` passes (0 errors, 5 pre-existing liquid-glass warnings).

## Stop Conditions
Stop and ask before:
- Installing any new npm package (including `@tauri-apps/plugin-fs`).
- Adding any new Rust crate dependency.
- Modifying any file outside the Scope section above.
- Changing the `LyricPlayer` / `FluidBackground` component APIs.
- Deleting any existing event handler or command registration.

## Progress
After each completed step: ✅ [what was done] — [file(s) affected]
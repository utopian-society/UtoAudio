## Context (carry forward)

utoaudio is an AGPL-3.0 audiophile music player targeting Linux desktop. The
architecture is Svelte 5 frontend ↔ Tauri IPC ↔ Rust workspace (crates/).

**Already built and verified:**
- `crates/audio-core/` — Flick audio engine forked, Flutter bridge stripped,
  exposes `AudioEngine` with ~30 serde methods in `src/tauri_api.rs`.
  163/164 tests pass. Builds clean.
- `apps/desktop/src/components/lyrics/` — AMLL lyric port (LyricPlayer,
  LyricLine, FluidBackground) in Svelte 5 + TypeScript. `pnpm check` clean,
  `pnpm build` passes.
- `apps/desktop/src/lib/lyric-parser/` — LRC, YRC, QRC, TTML parsers.
- `apps/desktop/src/lib/types/lyrics.ts` — canonical lyric types.
- `THIRD_PARTY_LICENSES.md` — Flick + AMLL attribution recorded.

**Current placeholder state (what you will replace):**
- `crates/audio-ffi/src/lib.rs` — only a `version()` stub, no Tauri commands.
- `crates/audio-ffi/Cargo.toml` — no dependencies declared.
- `apps/desktop/src-tauri/src/lib.rs` — bare `tauri::Builder::default().run()`.
- `apps/desktop/src-tauri/Cargo.toml` — only `tauri` dep, no `audio-ffi`.
- `apps/desktop/src/App.svelte` — splash screen showing "utoaudio — ready".
- No page components exist yet.

**Workspace config (do not change):**
- `Cargo.toml` workspace root already defines `audio-core` and `audio-ffi` as
  workspace dependencies.
- `apps/desktop/package.json` has `@tauri-apps/api ^2` already installed.
- `apps/desktop/src-tauri/tauri.conf.json` — configured for Linux (deb, AppImage,
  rpm), window 1280×800, decorations: false, theme: Dark, CSP: null.
- Frontend dev URL: `http://localhost:1420`, frontendDist: `../dist`.

## Task

Wire the Rust audio backend to the Svelte frontend through Tauri IPC, integrate
the LyricPlayer into a Now Playing page, and transform App.svelte into a
4-page navigation shell. The deliverable is a **compilable Linux executable**
where `pnpm tauri dev` launches a working app with audio playback driving the
lyric player.

## Files to edit (exhaustive list — touch ONLY these)

1. **`crates/audio-ffi/Cargo.toml`** — add dependencies: `audio-core` (workspace),
   `tauri` (workspace or `^2`), `serde` (workspace), `serde_json` (workspace),
   `tokio` (workspace, `sync` feature). Gate `uac2`-related commands behind
   `#[cfg(feature = "uac2")]` matching audio-core's feature.

2. **`crates/audio-ffi/src/lib.rs`** — replace the placeholder. Write
   `#[tauri::command]` handlers that wrap every public method on
   `audio_core::tauri_api::AudioEngine`. The engine is accessed as Tauri managed
   state: `engine: tauri::State<'_, Arc<AudioEngine>>`.

   Commands to implement (one per AudioEngine method):
   - `init`, `prepare(preferred_sample_rate: Option<u32>)`, `is_initialized`
   - `play(song: SongInfo)`, `queue_next(song: SongInfo)`
   - `pause`, `resume`, `stop`, `seek(position_secs: f64)`
   - `set_volume(volume: f32)`
   - `get_state` → returns `PlaybackState`
   - `get_progress` → returns `Option<PlaybackProgressInfo>`
   - `poll_event` → returns `Option<AudioEventInfo>`
   - `current_path` → returns `Option<String>`
   - `skip_to_next`, `set_playback_speed(speed: f32)`, `playback_speed`,
     `sample_rate`, `channels`
   - `set_equalizer(preset: EqualizerPreset)`, `set_fx(config: FxConfig)`
   - `set_convolver(config: ConvolverConfig)`, `set_convolver_ir(coeffs: Vec<Vec<f32>>)`,
     `clear_convolver_ir`
   - `set_crossfade(config: CrossfadeConfig)`
   - `set_high_res_mode(enabled: bool)`, `set_dap_bit_perfect_enabled(enabled: bool)`,
     `set_432hz_tuning_enabled(enabled: bool)`
   - `is_dac_available(preferred_sample_rate: Option<u32>)`
   - `shutdown`
   - `list_uac2_devices` (gated behind `#[cfg(feature = "uac2")]`)
   - `version` — keep as a simple command returning `"utoaudio audio-ffi 0.1.0"`.

   Also add an **event-forwarding command** `start_event_stream` that spawns a
   tokio task polling `engine.poll_event()` on a 100 ms interval and emits Tauri
   events (`app_handle.emit("audio-event", event_info)`) for the frontend to
   listen to. The task runs until a shutdown signal is received.

   Re-export all serde types from `audio_core::tauri_api` so the frontend
   TypeScript bindings can reference them (Tauri generates TS types from
   `#[tauri::command]` signatures).

   All commands return `Result<T, String>` (Tauri convention — map
   `AudioError` to its `Display` string).

3. **`apps/desktop/src-tauri/Cargo.toml`** — add dependencies:
   `audio-ffi = { workspace = true }`, `serde = { workspace = true }`,
   `serde_json = { workspace = true }`, `tokio = { workspace = true }`.

4. **`apps/desktop/src-tauri/src/lib.rs`** — replace the placeholder. On app
   setup (`tauri::Builder::default().setup(|app| { … })`):
   - Create `AudioEngine::new()`, call `engine.init()` and
     `engine.prepare(None)` (ignore errors — engine will work when a track is
     loaded).
   - Store as managed state: `app.manage(Arc::new(engine))`.
   - Register ALL commands from `audio-ffi` via `.invoke_handler(tauri::generate_handler![…])`.
   - On app exit/drop, call `engine.shutdown()` (best-effort, via a drop guard
     or the `on_event(|app, event| if let RunEvent::Exit = event { … })` hook).
   - Do NOT touch `main.rs` (it just calls `utoaudio_desktop_lib::run()`).

5. **`apps/desktop/src/App.svelte`** — replace the splash placeholder with a
   4-page navigation shell:
   - Four pages: **Now Playing** (default), **Playlist**, **Library**, **Settings**.
   - Use Svelte 5 runes: `$state` for `currentPage`.
   - Render the active page component via `{#if}` / `{:else if}` blocks.
   - Navigation: a bottom tab bar (mobile-friendly) or left sidebar with icons
     + labels. Use the liquid glass aesthetic: `backdrop-blur`, semi-transparent
     dark backgrounds, pale green (`#a3e635` / lime-400) and yellow
     (`#fde047` / yellow-300) accents.
   - Page components:
     - `NowPlaying` → the real component you will build (step 6).
     - `Playlist`, `Library`, `Settings` → minimal placeholder components with
       a centered title and "Coming soon" subtitle (styled consistently).
   - The window is undecorated (`decorations: false` in tauri.conf.json), so
     add a minimal custom titlebar (drag region + window controls are optional;
     at minimum add `data-tauri-drag-region` on the top bar area).

6. **`apps/desktop/src/pages/NowPlaying.svelte`** (NEW FILE) — the visual
   centrepiece. Create `apps/desktop/src/pages/` directory.
   - Import `LyricPlayer`, `FluidBackground` from `../components/lyrics`.
   - Import `extractTheme` from `../components/lyrics/color`.
   - Import `parseLyrics` from `../lib/lyric-parser`.
   - Import `invoke` from `@tauri-apps/api/core` and `listen` from
     `@tauri-apps/api/event`.
   - Layout: full-viewport. `FluidBackground` behind everything. `LyricPlayer`
     centered with padding.
   - State (Svelte 5 runes):
     - `currentTime: $state(0)`, `playing: $state(false)`, `isSeeking: $state(false)`.
     - `lyricLines` (parsed from loaded lyric file), `albumArtUrl` (for theme
       extraction), `theme` (from `extractTheme`).
     - `currentTrack: $state<SongInfo | null>(null)`.
   - On mount (`$effect`):
     - Start listening to `audio-event` Tauri events. On `StateChanged` →
       update `playing`. On `Progress` → update `currentTime` (and `duration`
       if provided). On `TrackEnded` → reset.
     - Call `invoke('start_event_stream')` to begin the event pipe.
     - Also poll `invoke('get_state')` on a 2-second interval as a fallback.
   - Transport controls (bottom overlay): play/pause button, seek bar
     (range input), track title + artist display. Style with liquid glass
     (backdrop-blur, rounded, semi-transparent dark).
   - The `LyricPlayer` receives: `lyrics={lyricLines}`, `currentTime`,
     `playing`, `onLineChange` (callback), `enableFluidBackground={false}`
     (FluidBackground is rendered separately behind), `enableBlur={true}`,
     `enableScale={true}`, `alignPosition={0.35}`.
   - For the MVP, lyric loading is manual: add a commented-out section showing
     how `parseLyrics` would be called when a lyric file path is available
     (the full lyric auto-load from metadata comes in a follow-up prompt).
   - Also create `apps/desktop/src/pages/Playlist.svelte`,
     `apps/desktop/src/pages/Library.svelte`, `apps/desktop/src/pages/Settings.svelte`
     as minimal placeholder stubs (centered title + "Coming soon" text).

7. **`apps/desktop/src/app.css`** — add custom properties for the liquid glass
   palette if not already present (the lyric styles.css already has `--amll-lp-*`
   vars). Add:
   ```css
   :root {
     --uto-accent-green: #a3e635;
     --uto-accent-yellow: #fde047;
     --uto-surface: rgb(15 23 42 / 0.6);  /* slate-900 @ 60% */
     --uto-glass-blur: 16px;
   }
   ```
   Ensure `@tailwind base/components/utilities` directives remain.

## Constraints (MUST)

- MUST use Svelte 5 runes mode (`$state`, `$derived`, `$effect`, `$props`) —
  no `let` reactivity, no `onMount`, no stores.
- MUST keep the existing lyric components unchanged — only import them.
- MUST keep `crates/audio-core/` untouched — it is already built and verified.
- MUST use `AudioEngine` via `tauri::State<'_, Arc<AudioEngine>>` — never
  create a second engine instance.
- MUST NOT add new npm dependencies beyond `@tauri-apps/api` (already present).
- MUST NOT add new Cargo dependencies beyond `audio-ffi`, `serde`, `serde_json`,
  `tokio` (all already workspace deps).
- MUST preserve the AGPL-3.0 license headers on all new Rust/TypeScript files.
- MUST append a section to `progress.md` at the end (format per AGENTS.md rule 6).

## Constraints (MUST NOT)

- MUST NOT touch any file in `crates/audio-core/`.
- MUST NOT touch any file in `apps/desktop/src/components/lyrics/`.
- MUST NOT touch any file in `apps/desktop/src/lib/lyric-parser/` or `lib/types/`.
- MUST NOT change `apps/desktop/src-tauri/tauri.conf.json`.
- MUST NOT change `apps/desktop/src-tauri/src/main.rs`.
- MUST NOT change workspace `Cargo.toml`.
- MUST NOT add new top-level directories or frameworks.
- MUST NOT add features or refactor beyond what is explicitly requested.
- Only make changes directly requested. Do not add extra files, abstractions, or
  features.

## Done when (verification checklist)

1. `cargo build --workspace` exits 0 with no errors.
2. `cargo test -p utoaudio-audio-core` — 163 passed, 1 failed (same as before;
   the pre-existing DSD test failure is NOT yours to fix).
3. `cd apps/desktop && pnpm run check` exits 0 (svelte-check + tsc clean).
4. `cd apps/desktop && pnpm run build` exits 0 (Vite build produces dist/).
5. `App.svelte` renders a 4-page navigation shell. Clicking tabs switches pages.
6. `NowPlaying.svelte` mounts the `LyricPlayer` and `FluidBackground` without
   errors. The transport controls (play/pause/seek) are visible.
7. Tauri commands `play`, `pause`, `resume`, `stop`, `seek`, `set_volume`,
   `get_state`, `get_progress`, `start_event_stream` are callable from the
   frontend via `invoke()` (test manually with `pnpm tauri dev` if possible,
   or verify that TypeScript types are generated correctly in
   `src-tauri/gen/schemas/`).
8. `progress.md` has a new section "## What prompt 4 did — …" appended.

## Stop conditions (MANDATORY — ask before proceeding)

- Stop and ask before adding ANY new npm package or Cargo crate not listed above.
- Stop and ask before modifying ANY file in `crates/audio-core/`.
- Stop and ask before deleting any existing file.
- Stop and ask if `cargo build --workspace` fails with an error you cannot
  resolve in 2 attempts.
```

🎯 **Target:** Zed coding agent (agentic — Claude Code pattern)
💡 **Optimized for:** Decomposed the monolith into two sequential prompts; this first prompt focuses exclusively on the Rust↔Svelte IPC bridge and Now Playing page integration — the critical path to a bootable Linux executable. File scope is exhaustively enumerated, stop conditions prevent scope creep into already-verified subsystems, and the "Done when" checklist gives binary pass/fail gates.

**Setup note:** This prompt assumes the workspace builds clean before you start. Verify with `cargo build --workspace` and `cd apps/desktop && pnpm run check` first.

---

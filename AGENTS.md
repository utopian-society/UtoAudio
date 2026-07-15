# AGENTS.md — utoaudio

> Instructions for AI coding agents working in this repository.
> Also read [`progress.md`](./progress.md) — it is the ground-truth log of what has been done and what remains.

## Project identity

**utoaudio** is an open-source (AGPL-3.0), audiophile-grade music player targeting **Linux desktop** and **Android**.

Philosophy: **charming but lightweight** — a beautiful, polished UI that stays fast and lean. Decorate the word usage.

### Visual identity

- **Palette:** pale green and yellow accents on a dark base.
- **Material language:** liquid glass — transparency, blur, fluid motion, soft edges.
- The "Now Playing" page is the visual centrepiece: full immersive lyric display with dynamic blur.

### Pages (only these four)

| Page | Purpose |
|---|---|
| Playlist | Manage and queue tracks (m3u8, absolute + relative paths). |
| Library | Browse / search the local music collection. |
| Now Playing | Full-screen AMLL lyric player — syllable-level lyrics, dynamic blur, fluid background. |
| Settings | General settings and file scanning |

## Architecture

```
Svelte 5 frontend  ←→  Tauri IPC  ←→  Rust workspace
apps/desktop/src        (serde)        crates/
```

### Git submodules (upstream forks)

| Submodule | Path | Upstream | Purpose |
|---|---|---|---|
| Flick | `vendor/flick` | [moss-apps/Flick](https://github.com/moss-apps/Flick) | Bit-perfect audio engine (decoder, EQ, FX, DSD, UAC2) |
| AMLL | `apps/desktop/src/lib/vendor/amll` | [amll-dev/applemusic-like-lyrics](https://github.com/amll-dev/applemusic-like-lyrics) | Lyric format parsers (LRC, YRC, QRC, TTML) + lyric player core |
| liquid-glass-svelte | `apps/desktop/src/lib/vendor/liquid-glass` | [danilofiumi/liquid-glass-svelte](https://github.com/danilofiumi/liquid-glass-svelte) | `LiquidGlass` Svelte 5 component (glassmorphism wrapper) |

All submodules point to forks under https://github.com/utopian-society/. See `.gitmodules`.

### Rust workspace (`crates/`)

| Crate | Role |
|---|---|
| `audio-core` | Thin adapter crate wrapping `vendor/flick` (`rust_lib_flick_player`). Preserves the `tauri_api` serde surface (`AudioEngine`, `SongInfo`, `PlaybackState`, …) so `audio-ffi` needs no changes. |
| `audio-ffi` | Fully wired: `#[tauri::command]` handlers wrapping `audio_core::AudioEngine`, plus SQLite-backed library index (`library.rs`) and JSON settings persistence (`settings.rs`). |

### Svelte frontend (`apps/desktop/`)

- **Framework:** Svelte 5 (runes mode), TypeScript, Vite, Tailwind CSS.
- **Lyric subsystem:** Lyric format parsers consumed from `apps/desktop/src/lib/vendor/amll` submodule via pre-built `.mjs` bundles. Svelte 5 lyric components (`LyricPlayer`, `LyricLine`, `FluidBackground`) are hand-written ports kept inline (no equivalent in upstream AMLL's React/Pixi code).
- **UI component library:** `LiquidGlass` wrapper consumed from `apps/desktop/src/lib/vendor/liquid-glass` submodule; re-exported via `src/lib/liquid-glass/index.ts` barrel.
- **Tauri shell:** `apps/desktop/src-tauri/` — Tauri 2.x Rust shell (`cdylib` crate, mobile entry point). Manages `AudioEngine` + `LibraryDb` as managed state, registers all `audio-ffi::commands` in `generate_handler!`.

## Feature requirements

- **m3u8 playlist support** — both absolute and relative paths.
- **Syllable-level (karaoke) lyrics** — LRC, YRC, QRC, TTML formats.
- **Dynamic blur + fluid background** — the AMLL "Now Playing" immersive view.
- **Liquid glass UI language** throughout all three pages.

## What is done (as of progress.md)

- ✅ `audio-core` — thin adapter crate wrapping `vendor/flick` submodule; preserves `tauri_api` serde surface. Builds & passes 6 tests.
- ✅ `audio-ffi` — fully wired Tauri `#[command]` handlers, SQLite library index, JSON settings persistence.
- ✅ `apps/desktop/src-tauri/` — fully wired shell managing `AudioEngine` + `LibraryDb` + event-stream `Notify`.
- ✅ All 4 pages built: Now Playing (AMLL lyric player), Playlist (m3u8 management), Library (directory browser + search), Settings (6-card collapsible hub).
- ✅ AMLL lyric port — Svelte 5 `LyricPlayer`, `LyricLine`, `FluidBackground` + parsers consumed from submodule. `pnpm check` clean, `pnpm build` passes.
- ✅ Liquid glass UI language applied uniformly across all pages (pure white base, translucent glass, pale green + yellow accents).
- ✅ AGPL-3.0 licensing + third-party attribution in `THIRD_PARTY_LICENSES.md`.
- ✅ Three upstream forks as git submodules: Flick, AMLL, liquid-glass-svelte.
- ✅ `audio-core` migrated from inline Flick copy to submodule dependency (thin adapter).
- ✅ Lyric parsers migrated from inline AMLL copy to submodule consumption (pre-built `.mjs` bundles).
- ✅ `LiquidGlass` migrated from vendored inline copy to submodule consumption (barrel re-export).
- ✅ Settings persistence: JSON (`settings.json`) + SQLite (`library.sqlite`).
- ✅ Tailwind exclusion for submodule files (fixes build-blocking `lightningcss` error).
- ✅ Tauri IPC permissions for window drag/close/minimize.

## What is NOT done (hand-off from progress.md)

1. Android cross-build wiring (`oboe-shared-stdcxx` feature).
2. Resolve the stale upstream DSD test and inherited clippy warnings (documented in progress.md).
3. Wire library-wide search to the SQLite `search_library` command (currently filters in-memory entries).
4. Wire `get_library_index` to the frontend for SQLite-backed browsing (currently uses live `scan_directory`).
5. Extract metadata (ID3/Vorbis/FLAC tags) during `rescan_library` via `lofty`.
6. Playlist open/save still uses browser File/Blob APIs — swap to Tauri `plugin-fs` / `plugin-dialog`.
7. `lyricFontSize` not wired from store to `LyricPlayer.fontSize`.
8. LiquidGlass performance optimisation: share single SVG filter across all instances.
9. Submodule fork pushes pending (no GitHub auth in this environment).
10. `pnpm tauri dev` end-to-end smoke test deferred (no display in this environment).

## Build & verify

### Rust

```sh
cargo build --workspace
cargo test --workspace --exclude rust_lib_flick_player   # submodule's own tests need a Flutter build env and don't compile
cargo clippy --workspace -- -D warnings   # if clippy is installed
```

### Frontend

```sh
cd apps/desktop
pnpm install
pnpm run check    # svelte-check + tsc (also runs build:submodule)
pnpm run build    # Vite build → dist/ (also runs build:submodule via prebuild hook)
```

### Full Tauri app (Linux)

```sh
cd apps/desktop
pnpm tauri dev
```

## Rules for agents

1. **Read [`progress.md`](./progress.md) first.** It is the authoritative record of every past prompt's work, architectural decisions, known issues, and the current hand-off state. Do not re-derive what is already documented there.

2. **Work in the existing architecture.** The Rust backend lives in `crates/`. The Svelte frontend lives in `apps/desktop/src/`. The Tauri shell lives in `apps/desktop/src-tauri/`. Do not introduce new top-level directories or frameworks without clear justification.

3. **Respect the license.** All new code in this repo is AGPL-3.0. Copied/derived code from upstream (Flick, AMLL) must retain attribution and be recorded in `THIRD_PARTY_LICENSES.md`.

4. **Honour the visual identity.** Use pale green + yellow. Liquid glass aesthetic (transparency, blur, soft edges). Keep the UI charming but lightweight — no heavy frameworks, no gratuitous dependencies.

5. **Keep it cross-platform.** Linux desktop is the primary dev target. Android is the secondary target — gate Android-specific code with `#[cfg(target_os = "android")]` and test that Linux builds still pass.

6. **At the end of every coding session, append a section to [`progress.md`](./progress.md).** Format it as:
   ```
   ## What prompt N did — <one-line summary>
   ### Files created / modified
   ### Verification
   ### Architectural decisions (if any)
   ### Known issues / hand-off notes
   ```
   Keep it factual and machine-readable. The next agent will rely on it.

7. **Before editing, check `progress.md` for "Not done yet."** That section is the explicit hand-off list — start there unless the user gives a different directive.

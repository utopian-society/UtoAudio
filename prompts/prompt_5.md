Context (carry forward)

utoaudio is an AGPL-3.0 audiophile music player targeting Linux desktop.
Architecture: Svelte 5 ↔ Tauri IPC ↔ Rust (audio-core + audio-ffi).

**Already built (Prompt 4 deliverables):**
- `crates/audio-ffi/` — full Tauri command surface wrapping AudioEngine.
- `apps/desktop/src-tauri/` — registers commands, manages engine state.
- `apps/desktop/src/App.svelte` — 4-page navigation shell (Now Playing, Playlist,
  Library, Settings) with liquid glass aesthetic.
- `apps/desktop/src/pages/NowPlaying.svelte` — integrated LyricPlayer +
  FluidBackground, wired to audio engine via Tauri IPC events.
- `apps/desktop/src/pages/Playlist.svelte` — placeholder stub ("Coming soon").
- `apps/desktop/src/pages/Library.svelte` — placeholder stub.
- `apps/desktop/src/pages/Settings.svelte` — placeholder stub.
- `pnpm tauri dev` produces a working Linux app.

**Also already built (Prompt 2 + 3):**
- `crates/audio-core/` — Flick engine with full serde API.
- `apps/desktop/src/components/lyrics/` — LyricPlayer, LyricLine, FluidBackground.
- `apps/desktop/src/lib/lyric-parser/` — LRC, YRC, QRC, TTML parsers.
- `apps/desktop/src/lib/types/lyrics.ts` — canonical lyric types.

**What remains:** The Playlist, Library, and Settings pages are empty stubs.
This prompt builds them out into fully functional pages with the liquid glass
aesthetic.

## Task

Build the three remaining pages — Playlist (m3u8 management), Library (file
browsing + search), and Settings (general config + scanning). Polish all pages
with the liquid glass visual language(liquid glass but not dark blue!!). Keep it charming but lightweight.

## Files to create/edit (exhaustive)

### New files

1. **`apps/desktop/src/lib/m3u8.ts`** (NEW) — m3u8 playlist parser/serializer.
   - Parse m3u8 format: `#EXTM3U`, `#EXTINF` (duration, title), file paths
     (absolute and relative). Handle `#PLAYLIST` name header.
   - `parseM3u8(content: string, baseDir?: string): M3u8Track[]` — returns
     parsed tracks with resolved absolute paths.
   - `stringifyM3u8(tracks: M3u8Track[], playlistName?: string): string` —
     writes back valid m3u8.
   - `M3u8Track` type: `{ path: string, title?: string, artist?: string, duration?: number }`.
   - Handle both `\n` and `\r\n` line endings.
   - This is a pure TypeScript module with no dependencies.

2. **`apps/desktop/src/lib/file-browser.ts`** (NEW) — file system browsing
   utilities using Tauri APIs.
   - Use `@tauri-apps/api/path` and `@tauri-apps/plugin-fs` if available, OR
     use Tauri's `invoke` with custom commands if plugins aren't set up.
     If neither is available: build a simulated browser that scans a configurable
     root directory by invoking a Rust command. For the MVP, use the Tauri
     `dialog` plugin to open directories, and `fs` plugin to read directory
     entries. If plugins are unavailable, create a minimal stub with a
     hardcoded demo directory tree that can be wired later.
   - `scanDirectory(path: string, extensions: string[]): Promise<FileEntry[]>`
   - `FileEntry` type: `{ name, path, isDirectory, size?, modified? }`.
   - Supported audio extensions filter: `['.flac','.wav','.mp3','.opus','.ogg',
     '.aac','.m4a','.wv','.dsf','.dff','.aiff','.ape','.wma']`.

3. **`apps/desktop/src/pages/Playlist.svelte`** (REPLACE stub) — full playlist
   management page.
   - Import `parseM3u8`, `stringifyM3u8`, `M3u8Track` from `../lib/m3u8`.
   - Import `invoke` from `@tauri-apps/api/core`.
   - State: `tracks: $state<M3u8Track[]>`, `currentIndex: $state(-1)`,
     `playlistName: $state('')`, `playlistPath: $state('')`.
   - Layout:
     - Header: playlist name (editable), track count, action buttons
       (New, Open, Save, Save As, Clear).
     - Track list: scrollable vertical list. Each row shows index, title,
       artist, duration. Current playing track is highlighted (pale green).
       Click a track to play it via `invoke('play', { song: { path } })`.
       Double-click or dedicated button to queue next.
     - Drag-to-reorder (optional for MVP; at minimum provide move-up/move-down
       buttons per track).
     - Bottom: "Add files…" button → opens native file dialog (use Tauri
       `dialog.open()` if available, or placeholder).
   - Operations:
     - Open: load `.m3u8` file, parse, populate track list.
     - Save: write back to the same path.
     - Save As: write to new path.
     - New: clear all tracks, start fresh.
     - Remove track: splice from array.
   - Style: liquid glass. List rows have `backdrop-blur` hover states.
     Active track has a pale green left-border accent. Scrollbar styled thin
     and semi-transparent.

4. **`apps/desktop/src/pages/Library.svelte`** (REPLACE stub) — music library
   browser + search.
   - Import `scanDirectory`, `FileEntry` from `../lib/file-browser`.
   - Import `invoke` from `@tauri-apps/api/core`.
   - State: `currentPath: $state('')`, `entries: $state<FileEntry[]>`,
     `searchQuery: $state('')`, `scanningRoots: $state<string[]>` (configurable
     directories to scan).
   - Layout:
     - Top: breadcrumb path bar (clickable segments to navigate up).
       "Add scan directory" button.
     - Search bar: filters visible entries by filename (case-insensitive
       substring match).
     - Content area: grid or list of entries.
       - Directories: folder icon, click to navigate in.
       - Audio files: music note icon, show name + metadata if available.
         Click to play (invoke `play`), right-click or ⋮ menu for "Add to
         playlist".
     - Sidebar or top section: list of configured scan roots with remove
       buttons.
   - Style: liquid glass. Grid cards with glassmorphism hover effect.
     Search bar with `backdrop-blur` background. Breadcrumb with subtle
     separators.

5. **`apps/desktop/src/pages/Settings.svelte`** (REPLACE stub) — settings page.
   - Sections (each a collapsible card):
     - **Audio output**: sample rate preference, bit-perfect toggle,
       high-res mode toggle, 432 Hz tuning toggle. Wire to
       `invoke('set_high_res_mode')`, `invoke('set_dap_bit_perfect_enabled')`,
       `invoke('set_432hz_tuning_enabled')`.
     - **Playback**: crossfade duration slider (0–30s), crossfade curve
       selector (EqualPower/Linear/SquareRoot/SCurve), default volume slider.
       Wire to `invoke('set_crossfade')`, `invoke('set_volume')`.
     - **Equalizer**: 10-band graphic EQ with sliders (−12 to +12 dB),
       enable/disable toggle. Wire to `invoke('set_equalizer')`.
       Band frequencies: 32, 64, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz.
     - **Library**: scan directory list, "Rescan now" button, file extension
       filter checkboxes.
     - **Appearance**: theme toggle (dark only for now, placeholder for light).
       Font size slider for lyrics.
     - **About**: version, license link, third-party licenses link.
   - Style: liquid glass cards with `backdrop-blur`, rounded corners,
     subtle borders. Toggle switches use pale green for active state.
     Sliders have pale green track with yellow thumb. Section headers
     use the accent colors.

### Files to modify

6. **`apps/desktop/src/App.svelte`** — update the page navigation to import
   the real page components instead of the placeholder stubs:
   - `import Playlist from './pages/Playlist.svelte'`
   - `import Library from './pages/Library.svelte'`
   - `import Settings from './pages/Settings.svelte'`
   - (NowPlaying is already imported from the real component.)
   - Remove the inline placeholder stubs.
   - No other changes to App.svelte's navigation shell structure.

7. **`apps/desktop/src/pages/index.ts`** (NEW, optional) — barrel export for
   all page components if you want cleaner imports in App.svelte.

8. **`progress.md`** — append a new section "## What prompt 5 did — …" per
   AGENTS.md rule 6.

## Visual identity (apply to ALL new UI)

- **Palette:** dark base (`slate-950` / `slate-900`), pale green accents
  (`#a3e635` lime-400), yellow accents (`#fde047` yellow-300).
- **Material language:** liquid glass — `backdrop-blur` (12–20px),
  semi-transparent surfaces (`bg-slate-900/60`), rounded corners (`rounded-2xl`
  or `rounded-3xl`), subtle borders (`border-white/5` or `border-white/10`).
- **Typography:** system font stack, `tracking-tight` for headings,
  `text-slate-100` for primary text, `text-slate-400` for secondary.
- **Transitions:** `transition-all duration-200` or `duration-300` on
  interactive elements. Hover: slight scale or brightness lift.
- **Scrollbars:** thin, semi-transparent, styled via Tailwind or CSS
  (`scrollbar-thin`, `scrollbar-thumb-white/10`).
- **Icons:** use inline SVGs or Unicode symbols (▶, ⏸, ⏭, ⏮, 🔀, 🔁, 📁, 🎵,
  ⚙️) — no icon library dependency.

## Constraints (MUST)

- MUST use Svelte 5 runes mode exclusively.
- MUST use Tailwind CSS for styling (leverage `@apply` in `<style>` blocks
  for complex glass effects).
- MUST keep all pages as single-file Svelte components (`.svelte` files with
  `<script lang="ts">`, `<template>`, `<style>` sections).
- MUST use `invoke()` from `@tauri-apps/api/core` for all backend calls.
- MUST preserve the liquid glass aesthetic consistently across all pages.
- MUST use the existing `--uto-accent-green`, `--uto-accent-yellow`,
  `--uto-surface`, `--uto-glass-blur` CSS custom properties (defined in
  `app.css` by Prompt 4).

## Constraints (MUST NOT)

- MUST NOT touch any file in `crates/` (all Rust code).
- MUST NOT touch `apps/desktop/src-tauri/`.
- MUST NOT touch `apps/desktop/src/components/lyrics/`.
- MUST NOT touch `apps/desktop/src/lib/lyric-parser/` or `lib/types/lyrics.ts`.
- MUST NOT touch `apps/desktop/src/pages/NowPlaying.svelte` except for
  trivial import adjustments if needed.
- MUST NOT add new npm packages. Use only `@tauri-apps/api` which is already
  installed.
- MUST NOT add heavy frameworks or icon libraries.
- MUST NOT change the navigation shell structure in `App.svelte` beyond
  swapping placeholder imports for real page imports.
- Only make changes directly requested. Do not add extra files, abstractions,
  or features beyond what is listed.

## Done when (verification checklist)

1. `pnpm run check` exits 0 (svelte-check 0 errors, tsc passes).
2. `pnpm run build` exits 0 (Vite build produces dist/, JS bundle ≤ 50 KB
   gzipped — keep it lightweight).
3. `cargo build --workspace` exits 0 (Rust side unchanged but must still build).
4. Playlist page: can parse a valid `.m3u8` file and display tracks. Save
   produces valid m3u8 output. "New" and "Clear" work. Clicking a track
   invokes `play`.
5. Library page: directory browsing works (or the stub is clearly marked as
   needing Tauri fs/dialog plugins). Search filters entries correctly.
   Clicking an audio file invokes `play`.
6. Settings page: all sections render. EQ sliders, crossfade controls,
   toggle switches are present and invoke the correct Tauri commands.
7. All pages share the liquid glass visual language — consistent colors,
   blur, rounded corners, transitions.
8. `progress.md` has a new section appended.

## Stop conditions (MANDATORY — ask before proceeding)

- Stop and ask before adding ANY new npm package.
- Stop and ask before modifying ANY file in `crates/` or `src-tauri/`.
- Stop and ask before modifying `NowPlaying.svelte` beyond import adjustments.
- Stop and ask before restructuring `App.svelte`'s navigation shell.
- Stop and ask if `pnpm run check` fails with an error you cannot resolve
  in 2 attempts.
```

🎯 **Target:** Zed coding agent (agentic — Claude Code pattern)
💡 **Optimized for:** Isolated the three missing pages into a single focused prompt after the backend wiring is complete. File scope is exhaustively enumerated so there is zero ambiguity about what to touch vs. what to leave alone. The liquid glass visual spec is concretely specified (colors, blur values, border opacities) rather than vague adjectives. Binary pass/fail verification gates keep the scope bounded.

**Setup note:** Run this only after Prompt 4 has completed successfully. Verify with `cargo build --workspace && cd apps/desktop && pnpm run check` before starting.

---

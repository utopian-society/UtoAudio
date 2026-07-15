You are an agentic coding assistant operating in the `utoaudio` repository (Linux desktop). This is a UI/UX polish + bugfix pass on top of the prompt-5 build. Read `utoaudio/AGENTS.md` and `utoaudio/progress.md` BEFORE editing — they are the authoritative project rules and log.

## Context (carry forward)
- utoaudio: AGPL-3.0 audiophile music player. Svelte 5 (runes mode) frontend ↔ Tauri IPC ↔ Rust workspace (`crates/audio-core` bit-perfect engine + `crates/audio-ffi` Tauri command surface + `apps/desktop/src-tauri` shell).
- Already shipped (progress.md prompts 2–5): audio-core forked & tested; lyric subsystem ported from AMLL; audio-ffi fully wired; `App.svelte` 4-page shell + `NowPlaying` / `Playlist` / `Library` / `Settings` pages all built. `cargo build --workspace`, `cargo test -p utoaudio-audio-core`, `pnpm run check`, `pnpm run build` all currently pass.
- Known hand-off items from progress.md prompt 5 that THIS prompt resolves:
  - "`Rescan now` is a `console.info` no-op — Rust-side `scan_library` command doesn't exist yet."
  - "`lib/file-browser.ts` returns an in-memory demo tree (no real filesystem scan)."
  - "EQ slider + scan + theme preference aren't persisted" (out of scope here).

## Project state — three issues to fix

1. **Theme reads as dark-green + dark-blue, not the brand's pale-green + pale-yellow liquid glass** (see AGENTS.md "Visual identity"). Root cause: every page's `<style>` hardcodes `#020617` slate-950 base + `rgba(15,23,42,0.55)` slate-900 surfaces — a *dark-blue* palette — although `--uto-accent-green` (`#a3e635`) and `--uto-accent-yellow` (`#fde047`) are nominally the accents. Result: blue dominates visually; the lime/yellow accents read as thin pinstripes.

2. **"Rescan now" in `Settings.svelte` does nothing** — see function `rescanNow()` at `apps/desktop/src/pages/Settings.svelte` lines ~213–218. The user cannot populate the library, so no audio file can be queued to test playback.

3. **UI uses Unicode emojis / glyphs** (🔊 🔊 ▶ ▶ 〰 ▤ ◐ 📁 ⚙ ♪ ☰ ⤴ ⤵ ✕ ＋ ⏭ ▾ ▸ × – ℹ). Replace every one with inline-SVG icons (zero dependency). Also: render the program logo from `utoaudio/icon.svg` to the LEFT of the title "UtoAudio" in the titlebar of `App.svelte`.

The user explicitly asked whether an existing third-party "Apple liquid glass" project should be imported for Svelte. You MUST address this question head-on in your final summary AND in the `progress.md` entry: **no, a third-party library is not needed** — Apple-style "Liquid Glass" (iOS 26 / Tahoe reference) is achievable with pure CSS `backdrop-filter` (blur + saturate + brightness), layered translucent gradients, inner-edge rim highlight, and inset/outer box-shadows. Document the reasoning concisely. Do NOT add a glassmorphism library.

## Task — three deliverables in one focused pass

### Deliverable A — Make scanning real + "Rescan now" functional + audio testable

**Rust side (permitted: `crates/audio-ffi/` and `apps/desktop/src-tauri/` for THIS prompt only):**
- Add to `crates/audio-ffi` a serde-friendly model and commands:
  - `#[derive(Serialize, Clone)]` `struct FileEntry { name: String, path: String, is_directory: bool, size: Option<u64>, modified: Option<i64> }` (use the existing package's serde re-exports if any).
  - `#[tauri::command] scan_directory(path: String) -> Result<Vec<FileEntry>, String>` — lists immediate entries of one directory using `std::fs::read_dir`; skips unreadable dirs; returns paths as absolute, normalised.
  - `#[tauri::command] scan_library(roots: Vec<String>, extensions: Vec<String>) -> Result<Vec<FileEntry>, String>` — walks each root up to a reasonable depth (e.g. 8), filters by `extensions` (case-insensitive, accepts with or without leading `.`), dedups, sorts directories-first then alphabetically.
- Register both commands in `apps/desktop/src-tauri/src/lib.rs` `tauri::generate_handler![…]` array alongside the existing audio commands. Do NOT modify the audio command handlers themselves or touch `audio-core`.
- Gate any `walkdir`-style feature behind `use std::fs` — **do NOT add new Cargo dependencies**. Plain `std::fs` recursion is sufficient.

**Frontend side (`apps/desktop/src/lib/file-browser.ts`):**
- KEEP the existing exported names/signatures (`scanDirectory`, `listAudioFiles`, `FileEntry`, `AudioEntry`, `AUDIO_EXTENSIONS`, `isAudioFile`) so call sites stay stable.
- Replace the in-memory demo tree internals with `invoke('scan_directory', { path })` and `invoke('scan_library', { roots, extensions })`. Wrap with thin TS helper functions that map camelCase → snake_case args (Tauri expects the keys it generated, e.g. `{ path }`, `{ roots, extensions }`).
- If a scan fails (permission, missing dir), surface the error to the caller — do not silently fall back to the demo tree.

**Library page (`apps/desktop/src/pages/Library.svelte`):**
- On mount, scan the first configured root (or all roots) and populate `entries`.
- Provide a refresh affordance that calls the scan again. Listen for a Tauri `library:rescanned` event emitted by Settings so the page refreshes after "Rescan now".
- Verify clicking an audio file invokes `invoke('play', { song: { path } })` (already wired); ensure audio actually produces sound during your manual check.

**Settings page (`apps/desktop/src/pages/Settings.svelte`):**
- Replace `rescanNow()` (currently `console.info`) with a real implementation:
  - Build the enabled-extensions array from the current `enabledExtensions: Set<string>` (prepend `.` if missing).
  - `await invoke('scan_library', { roots: scanRoots, extensions })` and capture results count.
  - `await emit('library:rescanned', { count })` — import `emit` from `@tauri-apps/api/event`.
  - Reflect transient errors via the existing `reportError()` path; show a brief "Scanned N files" confirmation.
- Make the "Add scan directory" input operational: the text field already adds to `scanRoots`; ensure pressing "Add" then "Rescan now" works end-to-end against a real directory.

**Deliverable A "done when":** clicking `Rescan now` runs a real filesystem scan, the Library reflects it, and the user can click an audio file to actually hear playback on Linux.

### Deliverable B — Liquid-glass theme overhaul (palette + glass recipe, no new dep)

Apply to: `apps/desktop/src/app.css`, `App.svelte`, `Playlist.svelte`, `Library.svelte`, `Settings.svelte`. Touches to `NowPlaying.svelte` MUST be limited to minor `<style>` consistency tweaks (selectors only) — its layout and lyric logic stay frozen.

**New CSS tokens in `app.css :root` (REPLACE the existing four with this richer set):**
```css
--uto-accent-green:  #a3e635;   /* lime-400  — pale green */
--uto-accent-yellow: #fde047;  /* yellow-300 — pale yellow */
--uto-bg:            #080b0a;   /* warm-neutral near-black, NOT slate-blue */
--uto-surface:       rgba(18, 26, 20, 0.34);
--uto-glass-blur:    24px;
--uto-glass-saturate: 180%;
--uto-glass-brightness: 1.08;
--uto-rim-light:    rgba(255, 255, 255, 0.16);
--uto-glass-border: rgba(255, 255, 255, 0.08);
--uto-glow-accent:  rgba(163, 230, 53, 0.18);
```

**Apple-"Liquid Glass" recipe — apply uniformly to every glass surface (`.card`, `.titlebar`, `.sidebar`, `.toggle`, `.root-row`, `.ext-chip`, library cards, playlist rows):**
```css
background: linear-gradient(135deg, rgba(255,255,255,0.06), rgba(255,255,255,0.015));
backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
-webkit-backdrop-filter: blur(var(--uto-glass-blur)) saturate(var(--uto-glass-saturate)) brightness(var(--uto-glass-brightness));
box-shadow:
  inset 0 1px 0 var(--uto-rim-light),      /* top-edge specular highlight */
  inset 0 -1px 0 rgba(0,0,0,0.35),         /* bottom inner shadow */
  0 8px 32px rgba(0,0,0,0.36);            /* soft outer drop */
border: 1px solid var(--uto-glass-border);
border-radius: 16px;                       /* 18–24px for the largest cards */
```

**Palette rules:**
- Body / `.app-shell` / page background: `var(--uto-bg)` overlaid with a faint ambient radial: `radial-gradient(circle at 20% -10%, rgba(163,230,53,0.05), transparent 55%)`. The base MUST no longer be slate-blue `#020617`.
- Glass surfaces use the warm-neutral translucent fill above — never the slate-900 dark-blue fill.
- `--uto-accent-green` becomes the visually dominant accent: active states use a lime-tinted glass (`background: linear-gradient(...) + rgba(163,230,53,0.18)`), with a lime border `rgba(163,230,53,0.35)` on hover/focus/active.
- `--uto-accent-yellow` is the secondary accent for slider thumbs, EQ gain values, directory cues.
- Hover lift: `transform: translateY(-1px)` + `box-shadow: 0 12px 40px var(--uto-glow-accent)`. Keep transitions at `0.18s cubic-bezier(0.22,1,0.36,1)` (soft, fluid).
- Scrollbars stay thin and semi-transparent (already implemented) — refresh thumb alpha via `rgba(255,255,255,0.16)`.

**Do NOT:** introduce a glassmorphism / shaders library; use Tailwind utilities plus a thin scoped `<style>` with `var(--uto-*)` tokens; keep the global variables the single source of truth so the lyric subsystem's `--amll-lp-*` namespace is untouched.

**Deliverable B "done when":** at first glance the empty app reads as "pale-green on luminous dark glass", not "dark blue with thin green lines". LIME + YELLOW accents carry the visual weight; blue recedes to neutral.

### Deliverable C — Inline-SVG icon system + program logo in the titlebar

**`Icon.svelte` (NEW, in `apps/desktop/src/components/Icon.svelte`):**
- Zero-dependency, single-file Svelte 5 component.
- Props: `name: IconName` (string union), `size?: number = 18`, optional `class?: string`, optional `strokeWidth?: number = 1.75`.
- Renders inline `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={strokeWidth} stroke-linecap="round" stroke-linejoin="round" width={size} height={size} aria-hidden="true">…</svg>`.
- An internal map of `name → SVG inner-path string`. Required set:
  `speaker`, `play`, `pause`, `skip-next`, `skip-prev`, `playlist`, `library`, `folder`, `gear`, `music`, `plus`, `chevron-down`, `chevron-right`, `close`, `minimize`, `search`, `rescan` (refresh-arrow), `queue-add`, `arrow-up`, `arrow-down`, `info`, `appearance` (sun/moon), `eq` (equalizer bars), `check`, `volume-low`.
- `fill="none"` + `stroke="currentColor"` so icons inherit the lime/yellow accent via `color: var(--uto-accent-*)`.

**`Logo.svelte` (NEW, in `apps/desktop/src/components/Logo.svelte`):**
- Renders the artwork from `utoaudio/icon.svg` as inline SVG (not an `<img>`). The cleanest robust approach: read the file at build time with Vite's `?raw` import and render with `{@html}` (sanitised by reading fixed content):
  ```ts
  import iconRaw from '../../../../icon.svg?raw';
  ```
  Strip the leading XML declaration if present; wrap in a `<span class="logo">` so the SVG inherits `currentColor`/lime. Alternatively, embed the path data directly as a string literal in the component (prefer this — statically verifiable).
- Default render at 22×22 px. Expose a `size` prop.

**Replace glyphs in:**
- `apps/desktop/src/App.svelte` — every `tab.icon` (♪ ☰ ▤ ⚙) → `<Icon name="…"/>`; the titlebar close (×) → `<Icon name="close"/>`, minimize (–) → `<Icon name="minimize"/>`. Add `<Logo size={22} />` immediately to the LEFT of the title text in `.titlebar`, then change that text from `utoaudio` to `UtoAudio` (title casing, per the user's wording).
- `apps/desktop/src/pages/Settings.svelte` — replace `card-icon` (🔊 ▶ 〰 ▤ ◐ ℹ), the chevron glyphs (`▾ ▸`), and the folder/remove glyphs (`📁 ✕`) with the matching `<Icon/>`. Keep the EQ vertical-slider numeric + frequency text.
- `apps/desktop/src/pages/Playlist.svelte` — replace glyph controls (`⏭ ⤴ ⤵ ✕ ＋`) with `<Icon/>`.
- `apps/desktop/src/pages/Library.svelte` — replace 📁 🎵 ＋ with `<Icon/>`; optionally replace traversal chevrons in the breadcrumb with `<Icon name="chevron-right"/>`.

**Deliverable C "done when":** `grep -nE '🔊|▶|〰|▤|◐|📁|⚙|♪|☰|⏭|⤴|⤵|✕|＋|▾|▸|ℹ' apps/desktop/src/pages apps/desktop/src/App.svelte` returns zero hits, and the Logo appears immediately left of "UtoAudio" in the titlebar.

## Files you MAY touch (exhaustive)
- `utoaudio/apps/desktop/src/app.css`
- `utoaudio/apps/desktop/src/App.svelte`
- `utoaudio/apps/desktop/src/pages/Playlist.svelte`
- `utoaudio/apps/desktop/src/pages/Library.svelte`
- `utoaudio/apps/desktop/src/pages/Settings.svelte`
- `utoaudio/apps/desktop/src/pages/NowPlaying.svelte` — style-only minor tweaks (selectors / palette alignment). NO layout/lyric-logic edits.
- `utoaudio/apps/desktop/src/lib/file-browser.ts`
- `utoaudio/apps/desktop/src/components/Icon.svelte`  (NEW)
- `utoaudio/apps/desktop/src/components/Logo.svelte` (NEW)
- `utoaudio/crates/audio-ffi/` (or whichever file in it exposes `#[tauri::command]`s) — to ADD the scan commands only. Do not alter existing audio commands.
- `utoaudio/apps/desktop/src-tauri/src/lib.rs` — to register the new commands in `generate_handler![…]` only.
- `utoaudio/progress.md` — append the standard section.

## Files you MUST NOT touch
- `utoaudio/crates/audio-core/` (bit-perfect engine — frozen).
- `utoaudio/apps/desktop/src/components/lyrics/` (lyric subsystem).
- `utoaudio/apps/desktop/src/lib/lyric-parser/` and `utoaudio/apps/desktop/src/lib/types/lyrics.ts`.
- `utoaudio/icon.svg` itself (only read it).
- No new top-level directories; no new Cargo or npm packages.

## Constraints (MUST)
- MUST use Svelte 5 runes exclusively (`$state`, `$derived`, `$effect`, snippets/{@render}).
- MUST keep total JS bundle ≤ 50 KB gzipped after `pnpm run build`.
- MUST keep Linux `cargo build --workspace`, `cargo test -p utoaudio-audio-core`, `pnpm run check`, `pnpm run build` all passing.
- MUST keep the no-new-npm-packages and no-new-Cargo-deps rules.
- MUST gate any platform-specific Rust code with `#[cfg(target_os = "linux")]`/etc. and ensure Linux still builds.
- MUST inline SVG paths (not `<img>` for icons); the Logo MAY use `<img>` only if the `?raw` approach fails Dare-import — try inlining first.
- Every changed `.svelte` / `.ts` must produce 0 errors / 0 warnings under `pnpm run check`.

## Constraints (MUST NOT)
- MUST NOT add an icon library, glassmorphism library, or animation library.
- MUST NOT modify `audio-core`'s engine, EQ, FX, convolver, crossfader, DSD, or UAC2 modules.
- MUST NOT change NowPlaying's lyric layout, parser, or WebGL fluid background logic (palette tweaks in its `<style>` are fine).
- MUST NOT reintroduce Unicode glyph icons anywhere new; replacing existing ones is the only allowed glyph deletion.
- MUST NOT persist scan roots / EQ / theme to disk in this pass (that is a separate follow-up).
- Only make changes directly requested. Do not add extra files, abstractions, or features beyond what is listed.

## Done when (binary verification — run all before yielding)
1. `cargo build --workspace` exits 0 with no new warnings vs. the inherited baseline.
2. `cargo test -p utoaudio-audio-core` exits 0 (the upstream-known failing test must remain the ONLY non-pass and must be marked as inherited in `progress.md`).
3. `cd apps/desktop && pnpm run check` exits 0 errors / 0 warnings.
4. `cd apps/desktop && pnpm run build` exits 0; JS bundle ≤ 50 KB gzipped.
5. Manual: run `pnpm tauri dev`, open Settings → Library → click "Rescan now" against a real directory containing `.flac`/`.mp3`, confirm the Library lists files; click one → confirmation that `invoke('play', …)` was reached and audio plays.
6. Manual: the visible dominant accent palette is pale-green (lime-400) + pale-yellow (yellow-300) on a luminous liquid-glass surface; the dark-blue slate-950 surface is gone.
7. `grep -nE '🔊|▶|〰|▤|◐|📁|⚙|♪|☰|⏭|⤴|⤵|✕|＋|▾|▸|ℹ' apps/desktop/src pages apps/desktop/src/App.svelte` returns no hits.
8. The Logo SVG renders to the immediate left of "UtoAudio" in the titlebar.
9. `progress.md` has a new section appended (see format below) including an explicit one-paragraph answer to the "do we need a third-party liquid-glass library" question.

## progress.md entry format
Append a section titled `## What prompt 6 did — Liquid-glass theme overhaul, real library scan, inline-SVG icons + app logo` with subsections: `### Files created / modified`, `### Verification`, `### Architectural decisions` (include the "no third-party glass library needed" reasoning), `### Known issues / hand-off notes`.

## Stop conditions (ask before proceeding — MANDATORY)
- Stop and ask before adding ANY new npm package or Cargo dependency.
- Stop and ask before editing `crates/audio-core/` or `apps/desktop/src/components/lyrics/`.
- Stop and ask before deleting any existing file.
- Stop and ask if `pnpm run check` fails after 2 focused fix attempts.
- Stop and ask before applying layout/structure changes to NowPlaying beyond style selector tweaks.

After each completed deliverable output: ✅ [deliverable name — concrete result]. Be concise; do not narrate exploratory steps.
```

🎯 **Target:** minimax-m3 (agentic — Claude Code pattern, OpenAI-compatible API)
💡 **Optimized for:** A single bounded polish/bugfix pass — three deliverables (functional scan + real audio test path, liquid-glass palette overhaul, inline-SVG icon system + app logo) with concrete CSS values, exhaustive file scope locks, and binary "done when" gates that include manual audio-playback verification. Hard scope locks forbid the bit-perfect engine and lyric subsystem; experimentation on the Apple-liquid-glass question is explicitly resolved ("no third-party library needed") so the model doesn't drift into dependency hunting.

**Setup note:** Run from `utoaudio/` after prompting — first verify the baseline with `cargo build --workspace && cd apps/desktop && pnpm run check && pnpm run build`, then paste the block above into the minimax-m3 session as the user turn. The model has long-context strength, so all three deliverables can ship in one pass without splitting.

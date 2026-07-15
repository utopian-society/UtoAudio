You are a senior Svelte 5 + Tauri engineer working on `utoaudio` (an AGPL-3.0 audiophile music player, Linux desktop + Android target, Svelte 5 runes + Rust workspace at `/home/bibichan/Programming/utoaudio`). First, read `/home/bibichan/Programming/utoaudio/AGENTS.md` and `/home/bibichan/Programming/utoaudio/progress.md` fully before doing anything — they are the authoritative state record. Append a new section to `progress.md` when finished.

## Architectural decisions (locked, do not relitigate)

1. **Adopt `liquid-glass-svelte`** (https://github.com/danilofiumi/liquid-glass-svelte) into `apps/desktop/src`. The prompt explicitly overrides your previous "built-in CSS only" recommendation — built-in CSS was deemed insufficient. Add as a normal dependency: `pnpm --filter @utoaudio/desktop add liquid-glass-svelte` (or whatever the package name on npm is — verify with `pnpm view liquid-glass-svelte` first; if it lives only on GitHub, clone or vendor a minimal compatible implementation into `apps/desktop/src/lib/liquid-glass/` and document the fork in `THIRD_PARTY_LICENSES.md`). Wire it so that titlebar, sidebar tabs, cards, transport bar, and now-playing surfaces all use the library's glass primitive instead of the existing `backdrop-filter` recipe in `apps/desktop/src/app.css`. Keep the existing `--uto-glass-*` CSS custom properties as the design tokens the library reads from where possible, so the lime/yellow palette survives the migration.
2. **Remove dark mode entirely.** Delete the `:root[data-theme="light"]` block and the `dark` tokens from `apps/desktop/src/app.css`. Simplify `apps/desktop/src/lib/store.svelte.ts` so `theme` is either dropped or fixed to `'light'` with no UI to switch it. Remove the theme dropdown from `apps/desktop/src/pages/Settings.svelte`, remove the `$effect` in `apps/desktop/src/App.svelte` that toggles `<html data-theme>`, and remove the `light` flag wiring in `apps/desktop/src/pages/NowPlaying.svelte` (lyrics always render against a single light backdrop now). All `--uto-*` token values should converge to the current light-mode values.

## Bugs to fix (real, not architectural)

3. **The "X" close button in the titlebar does not work.** Find it in `apps/desktop/src/App.svelte` (the titlebar `_glass` block). Determine why the click handler is silently failing — most likely it is `<button>`-less (a `<div>` with `onclick` swallowed by backdrop-filter pointer-events, or overlapping the drag region), or the helper it calls no longer exists, or `getCurrentWindow().close()` is not imported. Inspect the existing minimise/close button markup and how they differ; restore both minimise and close to actually invoke `getCurrentWindow().minimize()` / `.close()` from `@tauri-apps/api/window`. Verify by reading the file before patching.

4. **No persistent settings across app restarts.** Add a Rust Tauri command surface (e.g. `get_settings` / `set_settings` in `crates/audio-ffi/`) backed by a single JSON file at `tauri::api::path::app_config_dir()?.join("utoaudio/settings.json")`. On startup, the frontend should read this file and rehydrate `appState` (scan roots, enabled extensions, EQ state). On any mutation in `apps/desktop/src/lib/store.svelte.ts`, debounce-write back. Persist at minimum: `scanRoots`, `enabledExtensions`, `equalizer` (bands + preset), `crossfade`, `convolver` config, `lyricFontSize`, theme (which becomes fixed after decision 2). Use `serde_json` (already a transitive dep of `audio-core`). Add a `crates/audio-ffi/src/settings.rs` module + wire it from `lib.rs` + register the commands in `apps/desktop/src-tauri/src/lib.rs`. Schema-version the JSON file (`{"version": 1, "settings": {...}}`) so future migrations are possible.

5. **Library page buttons are unresponsive** (clicks on audio files and folders do nothing, including the roots grid). Investigate `apps/desktop/src/pages/Library.svelte`. Read it in full before patching. Likely root causes, in order of probability:
   - The click handler closes over the wrong `entry` reference (Svelte 5 runes — closures over `$state` snapshots in loops need careful identity handling).
   - `enterDirectory` / `playEntry` call sites were lost in a prior refactor and never restored.
   - The Tauri `invoke('scan_directory', { path })` call returns an error at runtime that is silently swallowed.
   - The `onclick` is on a child element while a parent `pointer-events: none` style was applied during the glass migration.
   Once found, fix at the root. Re-verify by reading `playEntry`, `enterDirectory`, `queueNext`, and every `onclick` binding on folder/file cards. Ensure roots-level clicks also work.

## Output contract

- Read `AGENTS.md` + `progress.md` FIRST. Do not re-derive documented decisions.
- All edits stay within the existing architecture: Rust in `crates/`, Svelte in `apps/desktop/src/`, Tauri shell in `apps/desktop/src-tauri/`. No new top-level directories.
- Honour AGPL-3.0. Any third-party code (including liquid-glass-svelte) must be recorded in `THIRD_PARTY_LICENSES.md` with original license + commit hash.
- Run `cd apps/desktop && pnpm run check && pnpm run build` and `cargo build --workspace` at the end. All must pass with zero new errors. The pre-existing 163/164 audio-core test result, the inherited DSD test failure, and the clippy-component-missing limitation are out of scope — do not touch.
- Append a `## What prompt N did —` section to `progress.md` in the format specified by AGENTS.md rule 6, covering files, verification, architectural decisions, and hand-off notes.

## Stop conditions

- **Stop and ask before:** deleting any file outside `apps/desktop/src/lib/store.svelte.ts` or `apps/desktop/src/app.css`'s dark block; adding any dependency beyond `liquid-glass-svelte`; modifying `Cargo.lock` indirectly via a different crate; changing the Tauri `tauri.conf.json` schema.
- Do not refactor beyond what is asked. Do not redesign the page layouts. Do not rewrite NowPlaying, Playlist, or the AMLL lyric components — wire from them only.
- If the liquid-glass-svelte npm package is unreachable or unmaintained, STOP and ask before forking/vendoring.
- If any of the three bugs (X button, persistence, library clicks) cannot be located after inspecting the relevant files, STOP and report what you found instead of guessing fixes.

## Done when

1. Titlebar X (and minimise) actually close/minimise the window.
2. Settings (scan roots, extensions, theme state) survive `pnpm tauri dev` restarts.
3. Clicking a folder card, a root card, and an audio file card in Library all produce visible behaviour consistent with prior progress.md notes (folder opens, audio invokes the `play` command).
4. Dark-mode code is fully removed; `liquid-glass-svelte` primitives are visibly in use on at least the titlebar, sidebar, and one card surface.
5. `pnpm run check`, `pnpm run build`, `cargo build --workspace` all exit 0.
6. `progress.md` has a new prompt-N section.
🎯 Target: MiniMax-M3, 💡 Compressed three-issue bug-fix prompt with locked architectural decisions, explicit stop conditions, and a verifiable done contract — adapted for MiniMax's strong instruction-following and long-context synthesis.

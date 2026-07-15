Prompt 1 — UI harmony (light-yellow text, white background, liquid glass)
You are working in the utoaudio repository (AGPL-3.0, Svelte 5 + Tauri 2.x, Linux desktop + Android). The visual identity is "charming but lightweight": pale green + pale yellow accents on a dark base, liquid-glass material language (transparency, blur, fluid motion, soft edges).

## Context (carry forward)
- Stack: Svelte 5 (runes mode), TypeScript, Vite, Tailwind CSS, Tauri 2.x.
- Design tokens live in `apps/desktop/src/app.css` under the `--uto-*` namespace. The lyric subsystem has its own `--amll-lp-*` namespace in `apps/desktop/src/components/lyrics/styles.css` — do NOT touch that.
- Current palette (from app.css):
  - `--uto-bg: #ffffff` (pure white background)
  - `--uto-accent-green: #bef264` (lime-300)
  - `--uto-accent-yellow: #fef08a` (yellow-200)
  - `--uto-text: #334155`, `--uto-text-strong: #1e293b`
  - Glass: `--uto-glass-blur: 32px`, `--uto-glass-saturate: 120%`, `--uto-glass-brightness: 1.05`
  - Glass surfaces: `--uto-glass-gradient-start: rgba(255,255,255,0.8)`, `--uto-glass-gradient-end: rgba(255,255,255,0.5)`
  - Shadows: `--uto-glass-outer-shadow: rgba(0,0,0,0.08)`, `--uto-glass-inset-bottom: rgba(0,0,0,0.04)`
- The user reports the current look is dissonant: light-yellow text + pure white background feel "too shiny", and the liquid-glass surfaces feel "too dull" — they don't harmonize.

## Task
Rebalance the palette and glass recipe so the three layers (background, glass surfaces, accent text) feel like one coherent material instead of three competing ones. The result must still read as "pale green + yellow on dark" per AGENTS.md — but the current implementation is light-mode. You may shift the base toward a darker neutral (charcoal/near-black with a faint warm tint) so the pale green/yellow accents and the glass actually have something to glow against.

## Constraints
- MUST keep the pale-green (#bef264 family) and pale-yellow (#fef08a family) accents as the primary brand colors.
- MUST preserve the liquid-glass material language (transparency, blur, soft edges, rim light, inset shadow).
- MUST NOT touch `apps/desktop/src/components/lyrics/styles.css` or any `--amll-lp-*` variable.
- MUST NOT introduce new dependencies or new frameworks.
- MUST NOT change component structure, layout, or copy — only color/opacity/blur/shadow tokens and any directly-coupled CSS rules.
- MUST keep WCAG AA contrast for body text against the new background.
- Only make changes directly requested. Do not refactor unrelated CSS.

## Files in scope
- `apps/desktop/src/app.css` (primary — token definitions)
- Any `.svelte` file under `apps/desktop/src/` that hardcodes a color/opacity that should now reference a token (e.g. `Library.svelte`, `NowPlaying.svelte`, `Playlist.svelte`, `Settings.svelte`, `App.svelte`, `LiquidGlass.svelte`). Prefer adding/updating tokens over scattering new literals.

## Deliverable
1. The updated `app.css` token block.
2. A short list of any `.svelte` files you touched and why (one line each).
3. A 3–5 line "before → after" summary describing the new harmony (e.g. "background shifted from #ffffff to #0f172a at 4% green tint; glass surfaces now use rgba(15,23,42,0.55) with 24px blur; yellow accent reserved for active states only").

## Success criteria
- Background is no longer pure white — it has a dark or near-dark neutral base.
- Glass surfaces are visibly translucent against that base (you can see the base through them).
- Pale-green and pale-yellow accents are the brightest things on screen and feel intentional, not noisy.
- `pnpm run check` and `pnpm run build` still pass.
- No new files created unless strictly required.
🎯 Target: minimax-m3, 💡 Optimized for M3's strong instruction-following and code-generation: front-loaded context, explicit token-level scope, binary success criteria, and a hard "do not touch the lyric namespace" guard to prevent collateral damage.
Prompt 2 — Library tab: folders show 4 KB and are unclickable, audio files unclickable
You are working in the utoaudio repository (AGPL-3.0, Svelte 5 + Tauri 2.x). The Library page lets the user browse their local music collection.

## Context (carry forward)
- Frontend: `apps/desktop/src/pages/Library.svelte` calls `scanDirectory(path)` from `apps/desktop/src/lib/file-browser.ts`, which invokes the Tauri command `scan_directory`.
- Backend: `crates/audio-ffi/src/lib.rs` defines `scan_directory` → `read_dir_entries` → `build_file_entry(path, is_directory)`.
- `FileEntry` shape (from `file-browser.ts`): `{ name, path, isDirectory, size?, modified? }`.
- In `Library.svelte`, `playEntry(entry)` branches on `entry.isDirectory`: directories call `enterDirectory(entry.path)`, audio files call `invoke('play', { song })`. The card template shows "Folder" for directories and `formatSize(entry.size)` for files.

## Bug report (from the user)
- File navigation OUTSIDE the scanned directory works.
- INSIDE a scanned directory:
  1. Every folder is displayed with a size of "4 KB" (instead of the word "Folder").
  2. Folders cannot be clicked — clicking does nothing.
  3. Audio files also cannot be clicked.

## Root cause (already identified — verify before fixing)
In `crates/audio-ffi/src/lib.rs`, `read_dir_entries` calls `build_file_entry(&entry.path(), false)` — it hardcodes `is_directory = false` for every entry, including directories. As a result:
- Directories get `isDirectory: false` and a real `size` (the directory inode's metadata size, typically 4096 bytes on ext4 → "4 KB").
- `playEntry` sees `isDirectory === false`, falls through to `isAudioFile(entry.name)` which is false for folder names, and returns early — so folder clicks do nothing.
- Audio files are also affected because the same broken `read_dir_entries` path is what `scan_directory` uses; the `scan_library` path (`walk_dir`) passes the correct `is_dir` flag, which is why "Show all files" works.

## Task
Fix the bug so that inside a scanned directory, folders display as "Folder", are clickable to descend into, and audio files are clickable to play.

## Constraints
- MUST fix the root cause in `read_dir_entries` (or `build_file_entry`'s call site) — do NOT paper over it in the frontend by re-deriving `isDirectory` from `size` or extension.
- MUST preserve the existing `FileEntry` shape and serde contract — no breaking changes to the Tauri command signature.
- MUST keep `scan_library` / `walk_dir` behavior unchanged (it already passes the correct flag).
- MUST NOT introduce new dependencies.
- Only make changes directly requested. Do not refactor unrelated code.

## Files in scope
- `crates/audio-ffi/src/lib.rs` — the `read_dir_entries` function and its call to `build_file_entry`.
- `apps/desktop/src/pages/Library.svelte` — only if a frontend change is genuinely needed after the backend fix (e.g. a defensive check). Prefer fixing it in Rust.

## Verification
1. `cargo build --workspace` passes.
2. `cargo test -p utoaudio-audio-core` still passes (163/164 — the one known failure is the stale DSD test, unrelated).
3. Add or extend a unit test in `crates/audio-ffi` that calls `read_dir_entries` on a temp dir containing both a subdirectory and a file, and asserts the returned `FileEntry` has the correct `is_directory` flag for each.
4. `pnpm run check` and `pnpm run build` still pass.

## Deliverable
1. The diff (or full updated function) for `read_dir_entries`.
2. The new/updated test.
3. A 2–3 line confirmation that the three symptoms (4 KB label, unclickable folders, unclickable audio files) are all resolved by the same one-line fix.

## Success criteria
- `read_dir_entries` returns directories with `is_directory: true` and `size: None`.
- `read_dir_entries` returns files with `is_directory: false` and a real `size`.
- The new test passes.
- No other behavior changes.
🎯 Target: minimax-m3, 💡 Pre-diagnosed the root cause from the codebase so M3 spends its tokens on the fix + test, not on re-deriving the bug — and locked the scope to one function plus one test to prevent over-engineering.
Prompt 3 — Change Tauri identifier to com.github.utopian-society.UtoAudio for future Flatpak
You are working in the utoaudio repository (AGPL-3.0, Svelte 5 + Tauri 2.x, Linux desktop + Android). The project will be published as a Flatpak in the future, which requires a reverse-DNS identifier that matches the Flatpak naming convention.

## Context (carry forward)
- The Tauri app identifier is currently `"org.utopia.utoaudio"` in `apps/desktop/src-tauri/tauri.conf.json` (field: `identifier`).
- The product name is `"utoaudio"` and the window title is `"utoaudio"` — these are display strings and should stay as-is unless explicitly asked.
- The bundle targets are `["deb", "appimage", "rpm"]` — Flatpak is NOT yet a target, but the identifier must be Flatpak-compatible now so we don't have to migrate later.
- The Flatpak convention is reverse-DNS, lowercase segments separated by dots, matching the project's web/GitHub home. The new identifier is `com.github.utopian-society.UtoAudio` (note the capital U, A in "UtoAudio" — preserve it exactly as given).

## Task
Change the Tauri application identifier from `org.utopia.utoaudio` to `com.github.utopian-society.UtoAudio` everywhere it appears in the repository, so the app is ready for Flatpak publishing without a future identifier migration.

## Constraints
- MUST update the `identifier` field in `apps/desktop/src-tauri/tauri.conf.json` to exactly `com.github.utopian-society.UtoAudio`.
- MUST update every other place the old identifier appears: Rust crate names, Cargo.toml `[package].name` if it embeds the id, Android `applicationId` / `namespace` in `apps/desktop/src-tauri/src-tauri.conf.json` or Gradle files if present, any `.desktop` file template, any Flatpak manifest stub, any CI workflow that references the id, any docs/README that mention it.
- MUST NOT change the `productName`, window `title`, or any user-facing display string.
- MUST NOT change the bundle `targets` array (Flatpak is not being added in this prompt).
- MUST NOT introduce new dependencies.
- Only make changes directly requested. Do not refactor unrelated config.

## Files in scope (search the repo to find every occurrence)
- `apps/desktop/src-tauri/tauri.conf.json` (primary)
- `apps/desktop/src-tauri/Cargo.toml` if it embeds the id
- `apps/desktop/src-tauri/src/lib.rs` / `main.rs` if they reference the id
- `crates/*/Cargo.toml` if any crate name embeds the id
- Any Android manifest under `apps/desktop/src-tauri/gen/android/**` or similar
- Any `.desktop` file, Flatpak manifest (`*.flatpak.xml`, `*.yaml`, `yml.json`), or CI workflow under `.github/workflows/**`
- `README.md`, `AGENTS.md`, `progress.md` if they mention the old id
- `THIRD_PARTY_LICENSES.md` — do NOT touch (third-party attribution is unrelated)

## Verification
1. `rg -n "org\.utopia\.utoaudio" .` returns zero matches outside of `THIRD_PARTY_LICENSES.md` and `target/` / `node_modules/` / `Cargo.lock`.
2. `rg -n "com\.github\.utopian-society\.UtoAudio" .` returns matches in every file that previously held the old id.
3. `cargo build --workspace` passes.
4. `pnpm run check` and `pnpm run build` pass.
5. `pnpm tauri dev` launches without an "identifier mismatch" warning.

## Deliverable
1. A list of every file changed, with the old → new value for each occurrence.
2. The output of the two `rg` commands above (proving the migration is complete).
3. Confirmation that `cargo build` and `pnpm run build` both pass.

## Success criteria
- Zero remaining references to `org.utopia.utoaudio` in source/config (excluding lockfiles, build artifacts, and third-party license docs).
- The new identifier `com.github.utopian-society.UtoAudio` is present wherever the old one was.
- The app still builds and launches.
🎯 Target: minimax-m3, 💡 Scoped as a mechanical identifier migration with explicit verification commands (rg + cargo build + pnpm build) so M3 can prove completeness instead of guessing — and explicitly excluded THIRD_PARTY_LICENSES.md and lockfiles to prevent collateral edits.

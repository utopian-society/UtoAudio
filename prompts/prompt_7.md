Fix the following 6 issues in the utoaudio Tauri + Svelte 5 music player. Read progress.md first to understand the architecture and what has been done. Work only in the existing codebase structure (apps/desktop/src/ for frontend, crates/ and apps/desktop/src-tauri/ for backend).
Issues to fix
1. Add light mode theme (liquid glass best represented in light mode)
The app currently has dark mode only (color-scheme: dark in app.css:6). The liquid glass aesthetic should be fully represented in light mode.
What to do:
- Add a --uto-theme CSS variable system in app.css that supports both light and dark modes
- Define a complete light theme palette:
- Light background: warm off-white base (e.g., #f8faf8 or similar)
- Light surface: translucent white with blur (e.g., rgba(255, 255, 255, 0.65))
- Light text: dark slate/neutral (e.g., #1e2925)
- Keep the same --uto-accent-green (#a3e635) and --uto-accent-yellow (#fde047) accents
- Adjust glass recipe for light mode: darker rim light, adjusted brightness/saturate
- Wire the theme choice in Settings.svelte (currently line 272: let themeChoice = $state<'dark'>('dark')) to actually apply the theme:
- Change the dropdown to allow selecting light or dark
- When theme changes, update a CSS class on html or #app element, or set color-scheme dynamically
- Ensure all pages (App.svelte, NowPlaying.svelte, Playlist.svelte, Library.svelte, Settings.svelte) use the theme-aware CSS variables so they switch correctly
- The lyric player (LyricPlayer.svelte) already has light/dark support via theme.light flag — wire the app theme to pass the correct value
Files to modify: app.css, Settings.svelte, possibly App.svelte (to apply theme class), other pages if they have hardcoded dark colors
2. Player state disappears after switching tabs
Files scanned in Settings page (scan roots) disappear after switching to Library page. This is because state is component-local ($state arrays) and not persisted or shared.
What to do:
- Create a shared store for global app state (Svelte 5 runes store pattern or a simple writable store)
- Move scanRoots, enabledExtensions, and other persistent settings into the store
- Import the store in both Settings.svelte and Library.svelte so they read/write the same state
- Optionally: persist to disk using Tauri's store plugin or a simple JSON file in the app config directory (requires backend command)
Minimum fix: At minimum, create a src/lib/store.ts with a Svelte 5 runes-based store and wire scanRoots so Library sees what Settings sets.
Files to modify: Create apps/desktop/src/lib/store.ts, modify Settings.svelte, Library.svelte
3. Remove project presentation text from UI
The "About" card in Settings.svelte (lines 573-589) displays:
- "Version: utoaudio 0.1.0"
- "Backend: {backendVersion}"
- "License: AGPL-3.0"
- "Third-party: View third-party licenses" button
What to do:
- Remove the entire "About" section card from Settings.svelte (lines ~565-592)
- Remove the aboutOpen state variable and related code
- Remove the card icon 'info' from the imports if no longer used
- Check other pages for similar "coming soon" or project description text and remove placeholders
Files to modify: Settings.svelte
4. Library page folders are not responsive (likely placeholder)
The Library page folder cards may not be responding to clicks or the directory tree may still be using demo data.
What to do:
- Verify Library.svelte is using the real scanDirectory and scanLibrary Tauri commands from file-browser.ts (wired in prompt 6)
- Ensure clicking a folder card navigates into that directory (updates currentPath and re-scans)
- Ensure clicking an audio file plays it via invoke('play', { song: {...} })
- Ensure the "＋" queue button works (invoke('queue_next', { song: {...} }))
- If any demo/mock data remains, replace with real invoke() calls
- Add loading states and error handling for scan operations
Files to verify/modify: Library.svelte, lib/file-browser.ts
5. Program icon in titlebar is too small
The <Logo size={22} /> in App.svelte (line 53) renders at 22px, which is "barely visible".
What to do:
- Increase the Logo size in the titlebar from size={22} to size={28} or size={32}
- Adjust the .title-group spacing in App.svelte styles if needed to accommodate the larger logo
- Ensure the logo doesn't overlap or cause layout issues at the larger size
Files to modify: App.svelte (line ~53 and possibly CSS around lines 122-128)
6. Fix pnpm tauri build failure: "invalid category"
The build error is:
failed to build bundler settings: invalid category
Error failed to build bundler settings: invalid category
This is in tauri.conf.json line 35: "category": "AudioVideo".
What to do:
- Tauri 2 requires specific category IDs from the XDG desktop file spec (https://specifications.freedesktop.org/menu-spec/latest/apa.html) or macOS categories
- For Linux, change "category": "AudioVideo" to "category": "Audio" (valid XDG category)
- Alternatively, use the full XDG category path: "category": "AudioVideo;Player;Audio"
- Verify the fix by running pnpm tauri build and confirming it produces .deb, .AppImage, or .rpm bundles without the "invalid category" error
Files to modify: apps/desktop/src-tauri/tauri.conf.json (line 35)
Verification
After completing the fixes, verify:
1. Light mode:
cd apps/desktop
pnpm tauri dev
- Open Settings → Appearance → switch to "Light" theme
- Confirm all pages switch to light theme with proper liquid glass appearance
- Confirm dark mode still works
2. State persistence:
- Add a scan root in Settings → Library
- Switch to Library page → confirm the scan root is still there
- Navigate in the library tree → confirm folders respond to clicks
3. No project text:
- Open Settings → confirm the "About" card is removed
4. Library responsiveness:
- Click folders in Library → should navigate into them
- Click audio files → should play (verify audio works)
5. Logo size:
- Confirm the logo in the titlebar is clearly visible
6. Build succeeds:
cd apps/desktop
pnpm tauri build
- Should complete without "invalid category" error
- Bundles should be created in apps/desktop/src-tauri/target/release/bundle/
Rules
- Follow the existing code style (Svelte 5 runes, TypeScript, liquid-glass CSS patterns)
- Do not add new npm or Cargo dependencies unless absolutely necessary
- Update progress.md after completing the work with a "What prompt 7 did" section
- Keep the AGPL-3.0 license and visual identity (pale green + yellow, liquid glass)

You are a senior frontend architect specializing in React-to-Svelte migrations, TypeScript, and music player UIs. You will port the AMLL (Apple Music Like Lyrics) React binding to Svelte 5 for the utoaudio Tauri frontend.

## Context

**Project**: `utoaudio` at `/home/bibichan/Programming/utoaudio/`. Svelte 5 frontend exists at `apps/desktop/src/` with empty `App.svelte`.

**Upstream**: https://github.com/amll-dev/applemusic-like-lyrics (AGPL-3.0). Key packages:
- `packages/core` — DOM-based lyric display (language-agnostic)
- `packages/react` — React bindings (use as API reference, NOT to copy)
- `packages/lyric` — lyric parsing (LyRiC, YRC, QRC, TTML)
- License: AGPL-3.0 (entire derivative work must be AGPL-3.0)

**Target**: Svelte 5 with runes (`$state`, `$derived`, `$effect`), TypeScript, integrated into Tauri frontend at `apps/desktop/src/components/lyrics/`.

**Licensing**: All code in this project is AGPL-3.0. AMLL is already AGPL-3.0, so the entire derivative work complies.

## Task

Fork AMLL, analyze the React binding API, and port the lyric display components to Svelte 5. Do NOT copy React code verbatim — use the API design and visual intent from the React package, but implement natively in Svelte.

## Required deliverables

1. **Fork AMLL into scratch directory**:
   ```bash
   cd /tmp
   git clone https://github.com/amll-dev/applemusic-like-lyrics.git amll-upstream
   cd amll-upstream
   git log --oneline -1  # record commit hash
   ```

2. **Analyze AMLL packages**:
   - Read `packages/react/src/LyricPlayer.tsx` to understand the API surface
   - Read `packages/core/src/LyricPlayer.ts` to understand the DOM logic
   - Identify props: `lyrics`, `currentTime`, `onLineChange`, `theme`, `animationMode`, `backgroundEffect`, `height`, `width`
   - Note TTML/LyRiC/YRC parsing in `packages/lyric/`

3. **Create Svelte port** at `apps/desktop/src/components/lyrics/`:
   - `LyricPlayer.svelte` — main component with props matching AMLL's API
   - `LyricLine.svelte` — individual lyric line with morph effects
   - `FluidBackground.svelte` — dynamic fluid background (port from AMLL core)
   - `types.ts` — TypeScript types: `LyricLine`, `LyricSource`, `LyricPlayerProps`, `AnimationMode`
   - `index.ts` — public exports
   - `styles.css` — Tailwind utility classes for Apple Music–like styling

4. **Port lyric parsing logic** from `packages/lyric/`:
   - Copy TTML, LyRiC, YRC, QRC parsers into `apps/desktop/src/lib/lyric-parser/`
   - Rewrite from TypeScript to Svelte-friendly modules (no Node-specific APIs)
   - Ensure async parsing for large lyric files
   - Export `parseLyrics(content: string, format: 'ttml' | 'lrc' | 'yrc' | 'qrc') => Promise<LyricLine[]>`

5. **Rewrite React patterns to Svelte**:
   - React `useState` → Svelte `$state` runes
   - React `useEffect` → Svelte `$effect` runes
   - React `useMemo` → Svelte `$derived` runes
   - React `forwardRef` → Svelte `bind:this` or component exports
   - React Context → Svelte `setContext`/`getContext` or simple prop drilling
   - No `createRoot`, no `ReactDOM.render` — Svelte uses native module imports

6. **Port fluid background**:
   - Extract GLSL shaders from AMLL's core package
   - Create `FluidBackground.svelte` with WebGL canvas
   - Use `$effect` for animation loop instead of React `requestAnimationFrame` in `useEffect`
   - Support the same modes: `fluid`, `gradient`, `blur`, `solid`

7. **Implement Apple Music–like visual effects**:
   - Line highlighting with smooth blur/opacity transitions
   - Karaoke-style word-by-word highlight (for TTML/YRC)
   - Swipe-to-pause gesture
   - Tap-to-toggle full-screen
   - Background color extraction from album art (use Lumina or simple平均色 algorithm)

8. **Create TypeScript types** in `apps/desktop/src/lib/types/lyrics.ts`:
   ```typescript
   export interface LyricLine {
     startTime: number;      // ms
     endTime: number | null; // ms or null for last line
     text: string;
     translations?: string[];
     karaokeWords?: KaraokeWord[]; // for TTML/YRC
   }
   export interface KaraokeWord {
     startTime: number;
     endTime: number;
     text: string;
   }
   export type AnimationMode = 'default' | 'karaoke' | 'scrolling' | 'static';
   export interface LyricPlayerProps {
     lyrics: LyricLine[];
     currentTime: number;
     onLineChange?: (lineIndex: number) => void;
     animationMode?: AnimationMode;
     height?: number;
     width?: number;
     enableFluidBackground?: boolean;
   }
   ```

9. **Add Svelte-specific styling**:
   - Tailwind utility classes for layout (flex, grid, absolute positioning)
   - Custom CSS for blur effects, transitions, gradients
   - CSS variables for dynamic theming from album art
   - Ensure dark mode default, light mode optional

10. **Verify the components integrate**:
    ```bash
    cd /home/bibichan/Programming/utoaudio/apps/desktop
    pnpm run check    # TypeScript type check
    pnpm run build    # frontend builds without errors
    ```

11. **Add license header** to every file:
    ```typescript
    // This file is part of utoaudio, licensed under AGPL-3.0.
    // Derivative work based on AMLL (https://github.com/amll-dev/applemusic-like-lyrics),
    // which is also licensed under AGPL-3.0. See LICENSE for full license text.
    ```

12. **Update `THIRD_PARTY_LICENSES.md`** with AMLL's commit hash and note that the entire lyric component is AGPL-3.0 derivative work.

## Hard constraints

- DO NOT touch `/home/bibichan/Programming/utoaudio/crates/` — Rust backend is complete from prior prompt.
- DO NOT copy React code verbatim — rewrite in native Svelte.
- DO NOT add features beyond what AMLL provides (no new lyric formats, no new animation modes).
- DO NOT use a license other than AGPL-3.0 for this code.
- DO NOT add authentication, telemetry, or external services.
- DO NOT modify `apps/desktop/src-tauri/` — Tauri command wiring is a separate task.

## Verification (must pass before stopping)

```bash
cd /home/bibichan/Programming/utoaudio/apps/desktop
pnpm run check                              # TypeScript exits 0
pnpm run build                              # frontend builds without errors
grep -r "react" src/components/lyrics/      # should find no import statements from 'react'
```

Report each command's exit code and any findings.

## Stop conditions

Stop after:
1. All Svelte lyric components created and functional
2. TypeScript checks pass
3. Frontend builds without errors
4. No React imports remain in lyric components
5. `THIRD_PARTY_LICENSES.md` updated with AMLL commit hash

Do NOT wire up Tauri commands or connect to the audio backend — that is a separate integration task.

## Output format

End with:
1. Bullet list of every file created/modified (path only)
2. Verification results (commands + exit codes)
3. List of any React imports still found (if verification failed)
4. The exact AMLL commit hash used
5. Notes on any AMLL features that could not be ported (with justification)

---

*This prompt targets minimax-m3. Agentic tool warning: Review file paths and build verification before pasting.

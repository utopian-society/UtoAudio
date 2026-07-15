Fix the vendored LiquidGlass.svelte at `apps/desktop/src/lib/vendor/liquid-glass/LiquidGlass.svelte` so its glass layers no longer visually override icons and text inside it.

The `.lg-tint` layer (line 53) covers the full area with `position:absolute;inset:0` and `background-color:rgba(255,255,255,0.15)` at `opacity:0.30` — it has no explicit `z-index` and no `pointer-events:none`. The `.lg-glass-filter` (line 66) uses an SVG `feDisplacementMap` at `scale="230"` (line 82) which aggressively displaces pixels on the glass surface, making content underneath illegible.

MUST:
- Add `z-index: 5` and `pointer-events: none` to `.lg-tint`.
- Reduce `scale` from `230` to `80` on the `feDisplacementMap`.
- Verify `.lg-content` stays at `z-index: 10` above all layers.

Do NOT change the component's props API. Do NOT add dependencies. Verify with `pnpm run check && pnpm run build`.
🎯 Target: deepseek-v4-pro, 💡 Front-loaded file path + exact line numbers + specific CSS properties to change — no ambiguity, single-pass fix.
Setup note: Run from the repo root. cd apps/desktop && pnpm run check after the edit.

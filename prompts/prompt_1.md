You are a senior systems architect specializing in cross-platform audio applications using Tauri, Rust, and modern web technologies. You will bootstrap a new open-source audiophile-grade music player project from scratch.

## Context

**Project**: `utoaudio` — an open-source, cross-platform (Linux desktop + Android), audiophile-grade music player with bit-perfect audio output and a beautiful UI.

**Working directory**: `/home/bibichan/Programming/utoaudio` (currently contains only a `skills/` folder at the root that you MUST NOT touch).

**Stack decisions (locked, do not change)**:
- Shell: Tauri 2.x (Rust core + webview frontend)
- Frontend framework: Svelte 5 (with runes) + TypeScript + Vite
- Audio backend: Rust (forked from Flick music player, MIT; modifications AGPL-3.0)
- Lyric UI: Svelte port of AMLL (AGPL-3.0)
- Package manager: pnpm (frontend), cargo (Rust)
- **Project license: AGPL-3.0** (all code in this project is AGPL-3.0, including forked/modified portions from upstream projects)

**Two upstream forks will be integrated in follow-up prompts**:
- Flick (https://github.com/moss-apps/Flick) — Rust audio engine; MIT license (original code retains MIT with attribution; modifications AGPL-3.0)
- AMLL (https://github.com/amll-dev/applemusic-like-lyrics) — lyric component library; AGPL-3.0 license

## Task

Bootstrap the project structure and tooling. Produce a working monorepo where the Tauri shell, Rust workspace, and Svelte frontend are all initialized and verified to build empty.

## Required deliverables

1. **Monorepo layout** at `/home/bibichan/Programming/utoaudio/`:
   ```
   utoaudio/
   ├── apps/
   │   └── desktop/              # Tauri app (Linux + Android targets)
   │       ├── src/              # Svelte frontend (entry: main.ts, App.svelte)
   │       ├── src-tauri/        # Tauri Rust shell (Cargo.toml, tauri.conf.json)
   │       ├── package.json
   │       ├── vite.config.ts
   │       ├── svelte.config.js
   │       └── tsconfig.json
   ├── crates/
   │   ├── audio-core/           # placeholder for Flick audio engine (filled by next prompt)
   │   │   ├── Cargo.toml
   │   │   └── src/lib.rs        # empty pub fn version() -> &'static str
   │   └── audio-ffi/            # placeholder for Tauri command bindings
   │       ├── Cargo.toml
   │       └── src/lib.rs
   ├── Cargo.toml                # workspace root
   ├── pnpm-workspace.yaml
   ├── README.md
   ├── LICENSE                   # AGPL-3.0 (entire project)
   ├── THIRD_PARTY_LICENSES.md   # Flick (MIT original), AMLL (AGPL-3.0)
   ├── .gitignore                # node_modules, target, dist, .DS_Store, etc.
   └── .gitattributes            # eol=lf
   ```

2. **Tauri 2.x configuration** in `apps/desktop/src-tauri/`:
   - `tauri.conf.json` with `productName: "utoaudio"`, `version: "0.1.0"`, `identifier: "org.utopia.utoaudio"`
   - Bundle targets: `["deb", "appimage", "rpm"]` for Linux, `"android"` for Android
   - Window default: 1280×800, dark theme by default, frameless with custom titlebar
   - Allowlist: enable audio-related permissions (later restricted)

3. **Rust workspace** in root `Cargo.toml`:
   - Members: `crates/*`, `apps/desktop/src-tauri`
   - resolver = "2", edition = "2024"
   - Shared dependencies: `serde = { version = "1", features = ["derive"] }`, `thiserror = "1"`, `anyhow = "1"`, `tokio = { version = "1", features = ["full"] }`, `tracing = "0.1"`, `tracing-subscriber = "0.3"`

4. **Svelte 5 frontend** in `apps/desktop/`:
   - Svelte 5 with runes (`$state`, `$derived`, `$effect`)
   - Vite 5+ with `@sveltejs/vite-plugin-svelte`
   - TypeScript strict mode
   - Tailwind CSS (dark mode default, audiophile-clean aesthetic)
   - Empty `App.svelte` showing "utoaudio — ready" centered on dark background

5. **Cross-platform audio scaffolding** in `crates/audio-core/src/lib.rs`:
   - Stub `pub enum AudioBackend { Alsa, PulseAudio, PipeWire, AndroidOboe }` with platform-conditional compilation
   - Empty `pub trait AudioSink` with `fn bitperfect_supported(&self) -> bool;`
   - Document that real implementation arrives in the next prompt

6. **THIRD_PARTY_LICENSES.md** with:
   - Full MIT text for Flick (original code only)
   - Full AGPL-3.0 text for AMLL
   - Clear attribution: "Flick (MIT): https://github.com/moss-apps/Flick — original Rust audio engine code retains MIT license"
   - Clear attribution: "AMLL (AGPL-3.0): https://github.com/amll-dev/applemusic-like-lyrics"
   - Statement: "All modifications and derivative works in this repository are licensed under AGPL-3.0"

7. **README.md** with: project description, ASCII architecture diagram, build instructions for Linux and Android, license, attribution.

8. **`.gitignore`** covering: `node_modules/`, `dist/`, `target/`, `.svelte-kit/`, `*.log`, `.DS_Store`, `Thumbs.db`, `.env`, `.env.local`, `gen/`.

## Hard constraints

- DO NOT touch `/home/bibichan/Programming/utoaudio/skills/` — external prompt-master skill directory.
- DO NOT clone Flick or AMLL in this prompt — those arrive in separate prompts.
- DO NOT add any feature, page, or component beyond the empty App.svelte placeholder.
- DO NOT add authentication, telemetry, analytics, or external services.
- DO NOT use a license other than AGPL-3.0 for this project.
- DO NOT initialize git history — user will commit manually after review.
- All filenames: kebab-case. Rust modules: snake_case. Svelte components: PascalCase.

## Verification (must pass before you stop)

Run each command and confirm exit code 0:

```bash
cd /home/bibichan/Programming/utoaudio
cargo build --workspace                          # exits 0
cd apps/desktop && pnpm install && pnpm run check  # type check exits 0
cd apps/desktop && pnpm run build                # produces dist/ without errors
```

Report verification results in your final message.

## Stop conditions

Stop after all deliverables complete AND verification passes. Do not start Flick fork or AMLL port — those are separate prompts. If verification fails, fix before stopping.

## Output format

End with:
1. Bullet list of every file created (path only)
2. Exact verification commands run and their exit codes
3. Any deviations from spec with justification

---

*This prompt targets minimax-m3. Agentic tool warning: Review file paths, directory permissions, and build verification before pasting.

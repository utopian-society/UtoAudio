# utoaudio

> *Charming but lightweight* — an open-source, cross-platform, audiophile-grade
> music player with bit-perfect audio output and a beautiful liquid-glass UI.

`utoaudio` is a Tauri 2 application: a Rust core driving a bit-perfect audio
engine, wrapped by a Svelte 5 (runes) + TypeScript + Vite frontend. It targets
**Linux desktop** and **Android**.

---

## Visual identity

utoaudio speaks the language of **liquid glass** — pure white base,
highly transparent surfaces, `backdrop-filter` blur, soft rounded edges,
and subtle depth shadows.

The palette is **pale green** (`#bef264`, lime-300) and **pale yellow**
(`#fef08a`, yellow-200) — accents that feel fresh, musical, and gentle
on the eyes. Four pages carry this aesthetic uniformly:

| Page | Purpose |
|---|---|
| **Playlist** | Manage and queue tracks (m3u8, absolute + relative paths). |
| **Library** | Browse / search the local music collection. |
| **Now Playing** | Full-screen AMLL lyric player — syllable-level lyrics, dynamic blur, fluid background. |
| **Settings** | Six collapsible cards: audio output, playback, equalizer, library scan, appearance, and the sync workflow. |

---

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

All submodules point to forks under https://github.com/utopian-society/. See
[`.gitmodules`](./.gitmodules).

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

---

## Repository layout

```
utoaudio/
├── apps/
│   └── desktop/              # Tauri app (Linux + Android targets)
│       ├── src/              # Svelte 5 frontend (App.svelte, pages/, lib/)
│       │   └── lib/vendor/   # git submodules (amll, liquid-glass)
│       └── src-tauri/        # Tauri Rust shell
├── crates/
│   ├── audio-core/           # thin adapter over vendor/flick submodule
│   └── audio-ffi/            # Tauri command bindings + library/settings persistence
├── vendor/
│   └── flick/                # git submodule (Flick audio engine)
├── scripts/
│   └── sync-submodules.sh    # submodule sync (read-only default)
├── Cargo.toml                # workspace root
├── LICENSE                   # AGPL-3.0
├── CONTRIBUTING.md           # fork → upstream contribution workflow
├── THIRD_PARTY_LICENSES.md   # Flick (MIT) + AMLL (AGPL-3.0) + liquid-glass (MIT)
├── AGENTS.md                 # instructions for AI coding agents
└── progress.md               # ground-truth log of every prompt's work
```

---

## Build

### Prerequisites

- Rust 1.85+ (edition 2024), `cargo` (Tauri 2.x).
- Node.js 20+ and `pnpm` 10 (`corepack enable pnpm && corepack prepare pnpm@10 --activate`).
- Linux system libraries (Debian/Ubuntu):
  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev \
                   librsvg2-dev libsoup-3.0-dev pkg-config build-essential
  ```

### Quick start (Linux desktop)

From the repository root:

```sh
# Rust workspace
cargo build --workspace
cargo test --workspace --exclude rust_lib_flick_player

# Frontend
cd apps/desktop
pnpm install
pnpm run check    # svelte-check + tsc (also runs build:submodule)
pnpm run build    # Vite build → dist/

# Run the app
pnpm tauri dev

# Bundle (deb, AppImage, rpm)
pnpm tauri build
```

### Android

The Rust shell is Android-ready (`cdylib` crate + `tauri::mobile_entry_point`).
Setting up Android needs the Android Studio SDK + NDK, then:

```sh
cd apps/desktop
pnpm tauri android init         # generates gen/android/
pnpm tauri android dev          # run on a device/emulator
pnpm tauri android build        # produces APK/AAB
```

> Android builds use the `tauri android` CLI subcommands. `bundle.targets:
> "android"` is not a valid Tauri 2 bundle target on Linux, so desktop bundle
> targets are `["deb", "appimage", "rpm"]` and Android is driven separately.

---

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the fork → upstream contribution
workflow and the submodule sync script (`./scripts/sync-submodules.sh`).

Quick summary:

1. Make changes inside the relevant submodule (`vendor/flick`,
   `apps/desktop/src/lib/vendor/amll`, or `apps/desktop/src/lib/vendor/liquid-glass`).
2. Push the branch to the utopian-society fork (`origin`).
3. Open a pull request from the fork to the original upstream repo.
4. After upstream merges, pull the merged state back into the fork and bump the
   submodule reference in utoaudio.

```sh
./scripts/sync-submodules.sh          # check status
./scripts/sync-submodules.sh --pull   # pull origin/main into each submodule
./scripts/sync-submodules.sh --push   # push HEAD to origin/main for each submodule
```

---

## License

This project is licensed under **AGPL-3.0**. See [`LICENSE`](./LICENSE).

All modifications and derivative works in this repository are licensed under
AGPL-3.0.

## Attribution / third-party

See [`THIRD_PARTY_LICENSES.md`](./THIRD_PARTY_LICENSES.md) for full upstream
license texts and attribution, including:

- **Flick** (MIT) — https://github.com/moss-apps/Flick — Rust audio engine.
  Original code retains MIT; modifications are AGPL-3.0.
- **AMLL** (AGPL-3.0) — https://github.com/amll-dev/applemusic-like-lyrics —
  Apple Music-like lyric component library.
- **liquid-glass-svelte** (MIT) — https://github.com/danilofiumi/liquid-glass-svelte —
  Liquid glass Svelte component.
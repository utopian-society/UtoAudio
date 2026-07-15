You are a senior Rust systems engineer specializing in audio DSP, USB audio class implementations, and cross-platform audio I/O. You will fork the Flick music player's Rust audio engine into the existing `utoaudio` Tauri project.

## Context

**Project**: `utoaudio` — bootstrapped at `/home/bibichan/Programming/utoaudio/` (see prior prompt). Layout exists:
- `crates/audio-core/` — placeholder for Flick audio engine (stub only)
- `crates/audio-ffi/` — placeholder for Tauri command bindings
- `apps/desktop/src-tauri/` — Tauri shell

**Upstream**: https://github.com/moss-apps/Flick (MIT license for original code). Rust backend at `rust/src/`:
- `audio/engine.rs`, `decoder.rs`, `dsd_engine/`, `resampler.rs`, `equalizer.rs`, `fx.rs`, `dynamics.rs`, `crossfader.rs`, `source.rs`, `strategy.rs`
- `uac2/` — USB Audio Class 2.0
- `api/` — flutter_rust_bridge FFI bindings (we will REPLACE with Tauri commands)

**Target platforms**: Linux (ALSA/PipeWire via cpal), Android (Oboe/AAudio via cpal-oboe). Bit-perfect output mandatory — no resampling/dithering/format conversion unless explicitly requested.

**Licensing**: 
- Flick original code: MIT (retain with attribution)
- Modifications and derivations in `utoaudio/`: AGPL-3.0
- Note: When distributing this combined work, AGPL-3.0 applies to the derivative portions; original Flick code remains MIT with attribution

## Task

Lift Flick's Rust audio engine into `utoaudio/crates/audio-core/` as a standalone library, strip Flutter bindings, expose clean Rust API for Tauri commands.

## Required deliverables

1. **Fork Flick into scratch directory**:
   ```bash
   cd /tmp
   git clone https://github.com/moss-apps/Flick.git flick-upstream
   cd flick-upstream
   git log --oneline -1  # record commit hash
   ```

2. **Copy Rust source tree** into `utoaudio/crates/audio-core/src/`:
   - Copy `rust/src/audio/` → `crates/audio-core/src/audio/`
   - Copy `rust/src/uac2/` → `crates/audio-core/src/uac2/`
   - Discard `rust/src/api/` (flutter_rust_bridge FFI)
   - Update `Cargo.toml`:
     - `name = "utoaudio-audio-core"`
     - `version = "0.1.0"`
     - `edition = "2021"`
     - `license = "MIT"` (original Flick code)
     - `description = "Audiophile-grade audio engine forked from Flick (MIT, moss-apps/Flick); modifications AGPL-3.0"`

3. **Create `crates/audio-core/src/tauri_api.rs`** with:
   - `#[derive(Serialize, Deserialize)]` structs: `SongInfo`, `PlaybackState`, `EqualizerPreset`, `EQBand`, `FxConfig`, `ConvolverConfig`, `CrossfadeConfig`, `Uac2DeviceInfo`
   - Strip all `flutter_rust_bridge` derive macros
   - Function signatures: `pub fn play(song: SongInfo) -> Result<(), AudioError>`, etc.
   - Top-level `pub struct AudioEngine` owning decoder, EQ, FX, output sink

4. **Replace flutter_rust_bridge with serde everywhere**:
   - Remove all `flutter_rust_bridge` references from source and Cargo.toml
   - Replace with `serde`-serializable types
   - Remove `pub extern "C"` blocks

5. **Make engine work without Flutter**:
   - Add `tokio = { version = "1", features = ["rt-multi-thread", "sync", "macros"] }`
   - Add `pub async fn run(engine: Arc<AudioEngine>) -> Result<(), AudioError>` entry point

6. **Platform-conditional compilation** in `crates/audio-core/src/audio/mod.rs`:
   - `#[cfg(target_os = "android")]` — cpal with oboe backend
   - `#[cfg(target_os = "linux")]` — cpal with alsa backend, `bitperfect_supported()` check
   - Other platforms: `compile_error!("utoaudio currently supports Linux and Android only")`

7. **Add license header comment** to every modified/derived file:
   ```rust
   // Portions of this file are derived from Flick (https://github.com/moss-apps/Flick),
   // which is licensed under the MIT License. Original copyright © 2024-2026 moss-apps.
   // Modifications and derivative works are licensed under AGPL-3.0. See LICENSE and
   // THIRD_PARTY_LICENSES.md for full license texts.
   ```

8. **Verify compilation**:
   ```bash
   cd /home/bibichan/Programming/utoaudio
   cargo build -p utoaudio-audio-core
   ```

9. **Update `THIRD_PARTY_LICENSES.md`** with Flick's commit hash and clear statement of MIT for original code, AGPL-3.0 for modifications.

## Hard constraints

- DO NOT touch files outside `utoaudio/crates/audio-core/`, `utoaudio/Cargo.toml`, `utoaudio/THIRD_PARTY_LICENSES.md`.
- DO NOT add features/improvements — copy as-is, only make listed changes.
- DO NOT use flutter_rust_bridge or Flutter dependencies.
- DO NOT relicense Flick's original code — it stays MIT with attribution; your modifications are AGPL-3.0.
- DO NOT modify `crates/audio-ffi/` or `apps/desktop/src-tauri/` (next prompt).

## Verification (must pass before stopping)

```bash
cd /home/bibichan/Programming/utoaudio
cargo build -p utoaudio-audio-core --release   # exits 0
cargo test -p utoaudio-audio-core --no-run     # tests compile
cargo clippy -p utoaudio-audio-core -- -D warnings  # no warnings
```

Report exit codes and warnings.

## Stop conditions

Stop after crate compiles cleanly, verification passes, and `THIRD_PARTY_LICENSES.md` updated. Do not wire up Tauri commands — that's the next prompt. Debug and fix any build errors.

## Output format

End with:
1. Bullet list of modified files
2. Verification results (commands + exit codes)
3. List of clippy warnings (if any) with file:line references
4. The exact Flick commit hash used

---

*This prompt targets minimax-m3. Agentic tool warning: Review file paths and build verification before pasting.

## Context (carry forward)
utoaudio audiophile music player. Audio engine lives in vendor/flick submodule (rust_lib_flick_player). crates/audio-core is a thin adapter wrapping it. On Linux, the engine creates output via cpal::default_host() → ALSA backend → default_output_device() using "default" device name (PipeWire ALSA compatibility layer). cpal uses BufferSize::Default (shared mode, not exclusive). There are NO #[cfg(target_os = "linux")] gates — all non-Android platforms share one cpal code path. The decoder works (logs confirm [DECODER] codec=0x2003 sr=44100 ch=2), but no sound is produced — likely because cpal's default device opens through PipeWire's ALSA plugin which may mute or fail silently.

Pre-existing warnings in vendor/flick (inherited from upstream, documented in progress.md): 13 clippy warnings (unused imports, dead code, unused variables, unused mut) plus 1 unused `Path` import in crates/audio-ffi/src/settings.rs.

## Task
Add Linux audio output support: ALSA exclusive (hw-device direct access) and native PipeWire streaming. Fix all pre-existing warnings in code we touch.

## Requirements

### ALSA exclusive path
In vendor/flick/rust/src/audio/engine.rs, MUST add a new code path gated on `#[cfg(target_os = "linux")]` within `create_audio_engine()`. Enumerate hardware ALSA PCM devices via cpal's `host.devices()` instead of `host.default_output_device()`. Filter to devices whose name starts with `"hw:"` (hardware devices, bypassing PipeWire/PulseAudio plugins). Select the first available hw device. Request `BufferSize::Fixed(512)` for low-latency exclusive access. Set `StreamConfig::buffer_size` explicitly. If no hw device is found, fall back to the existing `default_output_device()` path.

### PipeWire native path
Add `pipewire = "0.8"` as an optional dependency to vendor/flick/rust/Cargo.toml behind a `pipewire` feature (not default-enabled — ALSA exclusive is the primary path). Gate the import with `#[cfg(all(target_os = "linux", feature = "pipewire"))]`. The PipeWire stream MUST use `pw::stream::Stream` with `SPA_PARAM_EnumFormat` for f32 interleaved audio, match the target sample rate and channel count, and feed the same `audio_callback` buffer. This is a secondary path — the ALSA exclusive path is the default.

### Fix inherited warnings
In vendor/flick/rust/src/audio/engine.rs: fix unused import `crate::dev_eprintln` (remove it if unused, or annotate with `#[allow(unused_imports)]` if it's used in cfg-gated code). Fix unused variable `supervisor` and `mut` not needed on it (line ~2544) — prefix with `_` or wrap in cfg.

In vendor/flick/rust/src/audio/decoder.rs: fix unused import `crate::dev_eprintln`.

In vendor/flick/rust/src/audio/ir_loader.rs: fix unused imports `symphonia::core::codecs::Decoder` and `symphonia::core::formats::FormatReader`.

In crates/audio-ffi/src/settings.rs: remove the unused `use std::path::Path;` import.

For the remaining 9 inherited warnings in files NOT touched by this prompt (dsd_engine, uac2, api/audio_api.rs), do NOT fix them — they are in upstream code outside scope.

### Volume passthrough
Ensure volume changes from the frontend (`invoke('set_volume', { volume })`) reach the output stream. The existing `take_pending_volume()` mechanism in the submodule's `api/audio_api.rs` MUST be wired into the ALSA exclusive and PipeWire paths — multiply output samples by the pending volume before writing.

## Scope lock
TOUCH ONLY:
- vendor/flick/rust/src/audio/engine.rs
- vendor/flick/rust/src/audio/decoder.rs
- vendor/flick/rust/src/audio/ir_loader.rs
- vendor/flick/rust/Cargo.toml
- crates/audio-ffi/src/settings.rs

NEVER touch: crates/audio-core (adapter crate — zero changes needed), crates/audio-ffi/src/lib.rs (commands), crates/audio-ffi/src/library.rs, apps/desktop/src-tauri/, any frontend files, any other vendor/flick files (dsd_engine, uac2, api/audio_api.rs, manager.rs, device.rs).

## Stop conditions
MUST stop and ask before: adding any dependency beyond `pipewire`, modifying any file outside the scope lock, changing the `crate-type` in vendor/flick/rust/Cargo.toml, modifying vendor/flick/rust/src/audio/mod.rs, touching any uac2 or dsd_engine file.

## Verification
After changes, MUST pass: `cargo build --workspace` (zero new warnings in touched files), `cargo test --workspace --exclude rust_lib_flick_player`, `cd apps/desktop && pnpm run check`.

🎯 Target: glm5.2, 💡 Reasoning model — kept declarative with no scaffolding. Audio subsystem context front-loaded so the model doesn't re-derive the cpal/ALSA/PipeWire architecture.

Setup note for both prompts: The vendor/flick submodule has uncommitted local changes (1 commit ahead — crate-type addition at 510576e). Do NOT commit or push any submodule changes unless explicitly instructed. Work on the submodule's working tree directly.

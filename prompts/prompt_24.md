## Objective
Fix bit-perfect ALSA exclusive playback on the HiBy FC4 USB DAC so it opens
at the file's native sample rate on first launch without requiring output
toggling, and restore the DAC to PipeWire on app exit.

## Context
utoaudio is a Svelte 5 + Tauri 2.x + Rust audiophile music player targeting
Linux desktop. The audio engine lives in the vendored `vendor/flick` submodule
(`rust_lib_flick_player`), adapted by `crates/audio-core` (thin wrapper) →
`crates/audio-ffi` (Tauri commands) → `apps/desktop/src-tauri` (shell).

What has been built so far:
- D-Bus Device Reservation (`org.freedesktop.ReserveDevice1`) via `zbus` v4
  (blocking) — successfully tells PipeWire to release the HiBy FC4 (card 1).
  Implemented in `vendor/flick/rust/src/audio/engine.rs`: `alsa_card_index_map()`
  and `reserve_alsa_device()`.
- Retry loop after D-Bus release: 20 attempts × 50ms waiting for PipeWire to
  actually drop the ALSA fd before cpal enumeration.
- S32_LE format fallback: when F32 stream build fails (FC4 doesn't support
  float format), retries with `cpal::SampleFormat::I32` and f32→s32 conversion
  in the audio callback. In `create_audio_engine()` at engine.rs ~line 1070.
- Oneshot channel: `create_audio_engine()` waits for the audio thread to signal
  stream build success/failure before returning the handle — prevents "sending
  on a disconnected channel" from dead threads.
- `/proc/asound/cards` parser in `alsa_hw_devices_from_proc()` now strips the
  `pcm` prefix from device numbers (e.g. `pcm0p` → `0`), matching cpal's naming.
- ALAC magic cookie sample rate override in `decoder.rs` for files with sample
  rates >65535 Hz that overflow the MP4 container's 16.16 fixed-point field.
- `SongInfo` now has optional `sample_rate: Option<u32>` and
  `bits_per_sample: Option<u16>` fields in `crates/audio-core/src/tauri_api.rs`.
- `probe_audio_file(path)` Tauri command added to `audio-ffi`, registered in
  `src-tauri`. Returns `ProbeInfo { sample_rate, channels, duration_secs }`.
- `play`/`queue_next` commands pass `song.sample_rate` to `engine.prepare()`
  for bit-perfect rate matching.
- `appState.outputDevice` in `store.svelte.ts` persists output backend/device
  choice across page navigations (rehydrated from `settings.json`).
- Startup in `src-tauri/src/lib.rs` restores saved ALSA device preference
  before the first `prepare()` call.

What is still broken (verified by logs):
1. The ALSA stream build for FC4 still fails on startup even after D-Bus
   release + retry. The log shows "F32 stream failed, retrying with S32_LE
   format..." then silence. The S32 fallback either also fails or succeeds
   but the stream is silent. After manually toggling PipeWire→ALSA in
   Settings, playback eventually works (sometimes).
2. The DAC opens at 44100 Hz, not 96kHz. The frontend never calls
   `probe_audio_file` before `play`, so `song.sample_rate` is always `None`.
3. On app exit, PipeWire does not reclaim the DAC — system audio stays
   broken until the user manually restarts pipewire/wireplumber. We never
   call the D-Bus release/restore on shutdown.
4. The S32_LE fallback may not be the right format — the FC4 might need
   S24_LE or S16_LE. The format negotiation needs to try multiple integer
   formats, not just S32.

Key files:
- `vendor/flick/rust/src/audio/engine.rs` — `create_audio_engine()`,
  `resolve_output_device()`, `reserve_alsa_device()`, retry loop, format
  fallback, oneshot channel, `desired_output_signature()`
- `vendor/flick/rust/Cargo.toml` — `zbus` dep, `cpal` vendored path
- `crates/audio-ffi/src/lib.rs` — `play`, `queue_next`, `probe_audio_file`
- `crates/audio-core/src/tauri_api.rs` — `SongInfo`, `ProbeInfo`,
  `probe_audio_file()`
- `apps/desktop/src-tauri/src/lib.rs` — startup prepare, command registration
- `apps/desktop/src/lib/store.svelte.ts` — `appState`, `rehydrateSettings()`
- `apps/desktop/src/pages/Settings.svelte` — output device UI
- `apps/desktop/src/pages/Library.svelte` — track selection and playback
- `apps/desktop/src/pages/Playlist.svelte` — track selection and playback
- `vendor/flick/rust/vendor/cpal/` — vendored cpal 0.15.3 ALSA backend

## Target State
1. On first launch with ALSA+FC4 selected in settings, the DAC opens at the
   correct format (negotiated: try F32 → S32 → S24 → S16) and playback works
   immediately without any output toggling.
2. When a 96kHz ALAC file is played, the DAC opens at 96000 Hz (bit-perfect).
3. When a 44.1kHz file is played next, the engine recreates at 44100 Hz.
4. On app exit (or when switching back to PipeWire mode), the DAC is properly
   released back to PipeWire via D-Bus (`RequestRelease` with priority 0 or
   dropping the reservation).
5. The frontend calls `probe_audio_file` before `play` and passes
   `sample_rate` in the `SongInfo` object.
6. `cargo build --workspace` passes, `pnpm run check` passes.

## Scope
- Work only in:
  `vendor/flick/rust/src/audio/engine.rs`
  `crates/audio-ffi/src/lib.rs`
  `crates/audio-core/src/tauri_api.rs`
  `apps/desktop/src-tauri/src/lib.rs`
  `apps/desktop/src/pages/Library.svelte`
  `apps/desktop/src/pages/Playlist.svelte`
  `apps/desktop/src/lib/store.svelte.ts`
- Do NOT touch: `vendor/flick/rust/vendor/cpal/` (vendored, don't modify
  upstream cpal), `Cargo.lock`, `.gitmodules`, any submodule config, any CSS
  files, `LiquidGlass.svelte`

## Constraints
- Rust edition 2021, tokio 1.x, cpal 0.15.3 (vendored), symphonia 0.5
- Svelte 5 runes mode, TypeScript, Vite, Tailwind CSS
- zbus v4 blocking API for D-Bus calls
- Only make changes directly requested. Do not add features, abstractions,
  or files beyond what was asked.
- Follow existing code conventions: `dev_eprintln!` for debug logging,
  `#[cfg(target_os = "linux")]` for Linux-only code
- Preserve existing Tauri command signatures
- `cargo clippy` must not produce new warnings

## Acceptance Criteria
- [ ] `pnpm tauri dev` — on first launch with ALSA+FC4 saved in settings,
  the DAC opens at the negotiated integer format and produces audible output
  without toggling the output backend
- [ ] Playing a 96kHz ALAC file opens the DAC at 96000 Hz (verify via
  `[DECODER]` log showing sr=96000 AND no "at 44100 Hz" in ALSA exclusive log)
- [ ] Switching between 96kHz and 44.1kHz tracks recreates the engine at the
  correct rate each time
- [ ] Closing the app (Ctrl+C or window close) restores system audio through
  the DAC within 3 seconds
- [ ] `cargo build --workspace` passes (9 pre-existing upstream warnings
  allowed, 0 new)
- [ ] `cargo test --workspace --exclude rust_lib_flick_player` passes
- [ ] `pnpm run check` passes (0 errors, 5 pre-existing liquid-glass
  warnings allowed)

## Stop Conditions
Stop and ask before:
- Adding any new crate dependency beyond what's already in Cargo.toml
- Modifying any file in `vendor/flick/rust/vendor/`
- Deleting any existing function or command handler
- Changing the Tauri IPC permission model

## Progress
After each completed step: ✅ [what was done] — [file(s) affected]

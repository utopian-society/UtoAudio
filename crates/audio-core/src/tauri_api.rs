// Portions of this file are derived from Flick (https://github.com/moss-apps/Flick),
// which is licensed under the MIT License. Original copyright © 2024-2026 moss-apps.
// Modifications and derivative works are licensed under AGPL-3.0. See LICENSE and
// THIRD_PARTY_LICENSES.md for full license texts.

//! Clean, serde-serializable Rust API for the utoaudio audio engine, designed to
//! be driven by Tauri commands (see the `audio-ffi` crate).
//!
//! This module replaces Flick's `flutter_rust_bridge` FFI surface (`rust/src/api/`)
//! with plain Rust types that implement [`Serialize`]/[`Deserialize`]. The engine
//! itself — decoder, equalizer, FX, convolver, crossfader and the cpal/Oboe output
//! sink — is owned by [`AudioEngine`], a thin wrapper around Flick's
//! [`EngineManager`](crate::audio::manager::EngineManager) /
//! [`AudioEngineHandle`](rust_lib_flick_player::audio::engine::AudioEngineHandle).
//!
//! The async entry point [`run`] keeps a shared engine alive until shutdown is
//! requested, at which point it drains the underlying runtime and returns.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Notify;

use rust_lib_flick_player::audio::commands::{
    AudioEvent, PlaybackProgress, PlaybackState as EnginePlaybackState,
};
use rust_lib_flick_player::audio::crossfader::CrossfadeCurve;
use rust_lib_flick_player::audio::manager::EngineManager;
use rust_lib_flick_player::audio::strategy::OutputStrategy;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by the [`AudioEngine`] API.
#[derive(Debug, Error)]
pub enum AudioError {
    /// The Rust audio engine is not initialized; call [`AudioEngine::prepare`] first.
    #[error("audio engine not initialized: {0}")]
    NotInitialized(String),
    /// A command rejected by the running engine (e.g. decode/open failure).
    #[error("audio engine error: {0}")]
    Engine(String),
    /// Invalid argument supplied by the caller.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<String> for AudioError {
    /// `EngineManager` / `AudioEngineHandle` report failures as `Result<_, String>`.
    fn from(value: String) -> Self {
        if value.contains("not initialized") || value.contains("not initialize") {
            AudioError::NotInitialized(value)
        } else {
            AudioError::Engine(value)
        }
    }
}

// ---------------------------------------------------------------------------
// serde-serializable data types (the Tauri command surface)
// ---------------------------------------------------------------------------

/// A song to be played, addressing it by filesystem path.
///
/// The heavy metadata / tagging work is performed by the engine's decoder; this
/// struct only carries what the Tauri command layer needs to hand a track to the
/// engine (it intentionally matches Flick's `audio_play(path)` entry point).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongInfo {
    /// Absolute filesystem path to the track (PCM, DSD/DSF/DFF, WavPack, Opus …).
    pub path: String,
    /// Optional display title (filled from tags by the frontend / scanner).
    #[serde(default)]
    pub title: Option<String>,
    /// Optional display artist.
    #[serde(default)]
    pub artist: Option<String>,
    /// Optional album name.
    #[serde(default)]
    pub album: Option<String>,
    /// Optional duration in seconds (used for UI; the engine re-probes if absent).
    #[serde(default)]
    pub duration_secs: Option<f64>,
}

impl From<SongInfo> for PathBuf {
    fn from(song: SongInfo) -> Self {
        PathBuf::from(song.path)
    }
}

impl From<&SongInfo> for PathBuf {
    fn from(song: &SongInfo) -> Self {
        PathBuf::from(&song.path)
    }
}

/// Playback state reported back to the frontend.
///
/// Mirrors [`crate::audio::commands::PlaybackState`] but is `Serialize`/
/// `Deserialize` so it can cross the Tauri IPC boundary. Active during
/// crossfade the engine reports [`PlaybackState::Crossfading`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    /// Engine idle, no track loaded.
    Idle,
    /// Track loaded and playing.
    Playing,
    /// Playback paused (position retained).
    Paused,
    /// Loading / buffering.
    Buffering,
    /// Crossfading between two tracks.
    Crossfading,
    /// Playback stopped (track ended or stop requested).
    Stopped,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::Idle
    }
}

impl From<EnginePlaybackState> for PlaybackState {
    fn from(state: EnginePlaybackState) -> Self {
        match state {
            EnginePlaybackState::Idle => PlaybackState::Idle,
            EnginePlaybackState::Playing => PlaybackState::Playing,
            EnginePlaybackState::Paused => PlaybackState::Paused,
            EnginePlaybackState::Buffering => PlaybackState::Buffering,
            EnginePlaybackState::Crossfading => PlaybackState::Crossfading,
            EnginePlaybackState::Stopped => PlaybackState::Stopped,
        }
    }
}

/// A single graphic-EQ band (center frequency + gain).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EQBand {
    /// Center frequency in Hz.
    pub freq_hz: f32,
    /// Band gain in dB.
    pub gain_db: f32,
}

/// A 10-band graphic equalizer preset.
///
/// Matches Flick's fixed 10-band EQ (see [`rust_lib_flick_player::audio::equalizer::BAND_FREQS_HZ`]).
/// When `enabled` is `false` the engine bypasses the EQ regardless of `bands`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqualizerPreset {
    pub enabled: bool,
    /// Exactly 10 band gains in dB, ordered low → high (32 Hz … 16 kHz).
    pub bands: Vec<EQBand>,
}

impl EqualizerPreset {
    /// Build a `disabled` preset with flat (0 dB) bands at the fixed frequencies.
    pub fn flat() -> Self {
        Self {
            enabled: false,
            bands: rust_lib_flick_player::audio::equalizer::BAND_FREQS_HZ
                .iter()
                .map(|&hz| EQBand {
                    freq_hz: hz,
                    gain_db: 0.0,
                })
                .collect(),
        }
    }

    /// Collapse the preset to the `[f32; 10]` gains the engine expects.
    ///
    /// Returns `Err` if `bands` does not have exactly 10 entries.
    pub fn gains(&self) -> Result<[f32; 10], AudioError> {
        if self.bands.len() != 10 {
            return Err(AudioError::InvalidArgument(format!(
                "equalizer preset must have 10 bands, got {}",
                self.bands.len()
            )));
        }
        let mut out = [0.0f32; 10];
        for (slot, band) in out.iter_mut().zip(self.bands.iter()) {
            *slot = band.gain_db;
        }
        Ok(out)
    }
}

/// Spatial / time FX configuration — the 10 knobs of Flick's `FxSettings`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FxConfig {
    pub enabled: bool,
    pub balance: f32,
    pub tempo: f32,
    pub damp: f32,
    pub filter_hz: f32,
    pub delay_ms: f32,
    pub size: f32,
    pub mix: f32,
    pub feedback: f32,
    pub width: f32,
}

impl Default for FxConfig {
    fn default() -> Self {
        let d = rust_lib_flick_player::audio::fx::FxSettings::disabled();
        Self {
            enabled: d.enabled,
            balance: d.balance,
            tempo: d.tempo,
            damp: d.damp,
            filter_hz: d.filter_hz,
            delay_ms: d.delay_ms,
            size: d.size,
            mix: d.mix,
            feedback: d.feedback,
            width: d.width,
        }
    }
}

/// Impulse-response convolver configuration (enable + wet/dry mix).
///
/// The IR coefficients themselves are loaded separately via
/// [`AudioEngine::load_ir`] (mirroring Flick's `audio_load_ir`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConvolverConfig {
    pub enabled: bool,
    /// Wet/dry mix in `[0.0, 1.0]`.
    pub mix: f32,
}

/// Crossfade configuration: enable, duration and curve.
///
/// Maps onto Flick's `set_crossfade` + `set_crossfade_curve` commands.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CrossfadeConfig {
    pub enabled: bool,
    /// Crossfade duration in seconds.
    pub duration_secs: f32,
    /// Crossfade curve.
    pub curve: CrossfadeCurveSerde,
}

/// serde-friendly mirror of [`CrossfadeCurve`] (which has no serde derives in
/// upstream Flick).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossfadeCurveSerde {
    /// Equal power using sin/cos (recommended; constant perceived loudness).
    EqualPower,
    /// Linear fade (may dip in the middle).
    Linear,
    /// Square-root curve (alternative equal power).
    SquareRoot,
    /// S-curve for smoother transitions.
    SCurve,
}

impl Default for CrossfadeCurveSerde {
    fn default() -> Self {
        Self::EqualPower
    }
}

impl From<CrossfadeCurveSerde> for CrossfadeCurve {
    fn from(curve: CrossfadeCurveSerde) -> Self {
        match curve {
            CrossfadeCurveSerde::EqualPower => CrossfadeCurve::EqualPower,
            CrossfadeCurveSerde::Linear => CrossfadeCurve::Linear,
            CrossfadeCurveSerde::SquareRoot => CrossfadeCurve::SquareRoot,
            CrossfadeCurveSerde::SCurve => CrossfadeCurve::SCurve,
        }
    }
}

/// A discovered USB Audio Class 2.0 device (DAC/AMP).
///
/// Mirrors the relevant fields of Flick's `uac2::device::{DeviceIdentification,
/// DeviceMetadata}`. Returned by [`AudioEngine::list_uac2_devices`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Uac2DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    #[serde(default)]
    pub serial: Option<String>,
    pub product_name: String,
    pub manufacturer: String,
    /// USB vendor/product string identifier `"vid:pid"` (lowercase hex).
    pub id: String,
}

#[cfg(feature = "uac2")]
impl From<&rust_lib_flick_player::uac2::Uac2Device<rusb::Context>> for Uac2DeviceInfo {
    fn from(device: &rust_lib_flick_player::uac2::Uac2Device<rusb::Context>) -> Self {
        Self {
            vendor_id: device.identification.vendor_id,
            product_id: device.identification.product_id,
            serial: device.identification.serial.clone(),
            product_name: device.metadata.product_name.clone(),
            manufacturer: device.metadata.manufacturer.clone(),
            id: format!(
                "{:04x}:{:04x}",
                device.identification.vendor_id, device.identification.product_id
            ),
        }
    }
}

/// Periodic progress update emitted to the frontend.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlaybackProgressInfo {
    pub position_secs: f64,
    #[serde(default)]
    pub duration_secs: Option<f64>,
    pub buffer_level: f32,
}

impl From<PlaybackProgress> for PlaybackProgressInfo {
    fn from(p: PlaybackProgress) -> Self {
        Self {
            position_secs: p.position_secs,
            duration_secs: p.duration_secs,
            buffer_level: p.buffer_level,
        }
    }
}

/// Events emitted by the engine for the frontend to react to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioEventInfo {
    StateChanged { state: PlaybackState },
    Progress { progress: PlaybackProgressInfo },
    TrackEnded { path: String },
    CrossfadeStarted { from_path: String, to_path: String },
    Error { message: String },
    NextTrackReady { path: String },
}

impl From<AudioEvent> for AudioEventInfo {
    fn from(event: AudioEvent) -> Self {
        match event {
            AudioEvent::StateChanged(state) => AudioEventInfo::StateChanged {
                state: state.into(),
            },
            AudioEvent::Progress(progress) => AudioEventInfo::Progress {
                progress: progress.into(),
            },
            AudioEvent::TrackEnded { path } => AudioEventInfo::TrackEnded { path },
            AudioEvent::CrossfadeStarted { from_path, to_path } => {
                AudioEventInfo::CrossfadeStarted { from_path, to_path }
            }
            AudioEvent::Error { message } => AudioEventInfo::Error { message },
            AudioEvent::NextTrackReady { path } => AudioEventInfo::NextTrackReady { path },
        }
    }
}

// ---------------------------------------------------------------------------
// AudioEngine — owns the decoder, EQ, FX, convolver and output sink
// ---------------------------------------------------------------------------

/// Top-level handle owning the Flick audio engine: the background decoder, the
/// graphic EQ, spatial/time FX, the impulse-response convolver, the crossfader
/// and the cpal (Linux) / Oboe (Android) output sink.
///
/// Internally these live behind Flick's [`EngineManager`] /
/// [`AudioEngineHandle`](rust_lib_flick_player::audio::engine::AudioEngineHandle); this struct
/// exposes them as a clean, serde-friendly API for Tauri commands. Clone the
/// handle (it is cheap — an [`Arc`] internally) to share it across command
/// handlers and an async [`run`] task.
#[derive(Clone)]
pub struct AudioEngine {
    manager: Arc<EngineManager>,
    /// Signaled by [`AudioEngine::shutdown`] to stop the [`run`] entry point.
    shutdown: Arc<Notify>,
}

impl AudioEngine {
    /// Create a new, uninitialized engine handle.
    ///
    /// The underlying capability detection and Rust engine spawn happen lazily
    /// on the first call to [`AudioEngine::prepare`].
    pub fn new() -> Self {
        Self {
            manager: Arc::new(EngineManager::new()),
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Initialize engine bookkeeping (selects the default engine without
    /// spawning the Rust engine yet). Mirrors Flick's `audio_init`.
    pub fn init(&self) {
        self.manager.init();
    }

    /// Whether the Rust engine has been spawned and is ready for commands.
    pub fn is_initialized(&self) -> bool {
        self.manager.is_rust_initialized()
    }

    /// Detect & spawn the native Rust engine for the requested output rate.
    ///
    /// Call this before [`AudioEngine::play`] (mirrors Flick's
    /// `audio_prepare_engine`). Pass `None` to let the engine pick a rate.
    pub fn prepare(&self, preferred_sample_rate: Option<u32>) -> Result<(), AudioError> {
        self.manager
            .ensure_rust_engine(preferred_sample_rate, Vec::<OutputStrategy>::new())?;
        Ok(())
    }

    /// Whether a USB DAC is currently available.
    pub fn is_dac_available(&self, preferred_sample_rate: Option<u32>) -> Result<bool, AudioError> {
        Ok(self.manager.is_dac_available(preferred_sample_rate)?)
    }

    /// Load and play a track immediately. Replaces any current track.
    pub fn play(&self, song: SongInfo) -> Result<(), AudioError> {
        self.with_handle(|h| h.play(song.into()))
    }

    /// Queue a track for gapless playback once the current track ends.
    pub fn queue_next(&self, song: SongInfo) -> Result<(), AudioError> {
        self.with_handle(|h| h.queue_next(song.into()))
    }

    /// Pause playback (position is retained).
    pub fn pause(&self) -> Result<(), AudioError> {
        self.with_handle(|h| h.pause())
    }

    /// Resume playback from the paused position.
    pub fn resume(&self) -> Result<(), AudioError> {
        self.with_handle(|h| h.resume())
    }

    /// Stop playback completely and clear buffers.
    pub fn stop(&self) -> Result<(), AudioError> {
        self.with_handle(|h| h.stop())
    }

    /// Seek to `position_secs` within the current track.
    pub fn seek(&self, position_secs: f64) -> Result<(), AudioError> {
        self.with_handle(|h| h.seek(position_secs))
    }

    /// Set the main volume in `[0.0, 1.0]`.
    pub fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        self.with_handle(|h| h.set_volume(volume))
    }

    /// Apply a graphic-EQ preset (10 bands). See [`EqualizerPreset`].
    pub fn set_equalizer(&self, preset: &EqualizerPreset) -> Result<(), AudioError> {
        let gains = preset.gains()?;
        self.with_handle(move |h| h.set_equalizer(preset.enabled, gains))
    }

    /// Apply spatial / time FX settings. See [`FxConfig`].
    pub fn set_fx(&self, config: FxConfig) -> Result<(), AudioError> {
        self.with_handle(move |h| {
            h.set_fx(
                config.enabled,
                config.balance,
                config.tempo,
                config.damp,
                config.filter_hz,
                config.delay_ms,
                config.size,
                config.mix,
                config.feedback,
                config.width,
            )
        })
    }

    /// Enable/disable the impulse-response convolver and set its wet/dry mix.
    pub fn set_convolver(&self, config: ConvolverConfig) -> Result<(), AudioError> {
        self.with_handle(move |h| h.set_convolver(config.enabled, config.mix))
    }

    /// Load impulse-response coefficients (1 mono or 2 stereo L/R tap vectors).
    pub fn set_convolver_ir(&self, coeffs: Vec<Vec<f32>>) -> Result<(), AudioError> {
        self.with_handle(move |h| h.set_convolver_ir(coeffs))
    }

    /// Remove the current IR (convolver becomes a no-op).
    pub fn clear_convolver_ir(&self) -> Result<(), AudioError> {
        self.with_handle(|h| h.clear_convolver_ir())
    }

    /// Configure crossfade (enable, duration, curve).
    pub fn set_crossfade(&self, config: CrossfadeConfig) -> Result<(), AudioError> {
        self.with_handle(move |h| {
            h.set_crossfade(config.enabled, config.duration_secs)?;
            h.set_crossfade_curve(config.curve.into())?;
            Ok(())
        })
    }

    /// Skip to the next queued track (crossfading if enabled).
    pub fn skip_to_next(&self) -> Result<(), AudioError> {
        self.with_handle(|h| h.skip_to_next())
    }

    /// Set playback speed multiplier (typically `[0.5, 2.0]`).
    pub fn set_playback_speed(&self, speed: f32) -> Result<(), AudioError> {
        self.with_handle(|h| h.set_playback_speed(speed))
    }

    /// Current playback speed, if a track is loaded.
    pub fn playback_speed(&self) -> Option<f32> {
        self.manager
            .read_rust_handle(|h| h.get_playback_speed())
    }

    /// Current output sample rate (Hz), once the engine is running.
    pub fn sample_rate(&self) -> Option<u32> {
        self.manager.read_rust_handle(|h| h.sample_rate())
    }

    /// Channel count of the active output stream.
    pub fn channels(&self) -> Option<usize> {
        self.manager.read_rust_handle(|h| h.channels())
    }

    /// Current playback state.
    pub fn get_state(&self) -> PlaybackState {
        self.manager
            .read_rust_handle(|h| h.state().into())
            .unwrap_or_default()
    }

    /// Latest progress update (position / duration / buffer fill), if any.
    pub fn get_progress(&self) -> Option<PlaybackProgressInfo> {
        self.manager
            .read_rust_handle(|h| h.get_progress())
            .flatten()
            .map(Into::into)
    }

    /// Filesystem path of the currently loaded track, if any.
    pub fn current_path(&self) -> Option<String> {
        self.manager
            .read_rust_handle(|h| h.get_current_path())
            .flatten()
            .map(|p| p.to_string_lossy().into_owned())
    }

    /// Poll for one pending engine event (state change, progress, track end …).
    pub fn poll_event(&self) -> Option<AudioEventInfo> {
        self.manager
            .read_rust_handle(|h| h.try_recv_event())
            .flatten()
            .map(Into::into)
    }

    /// Enumerate currently-connected UAC2 DAC/AMP devices.
    #[cfg(feature = "uac2")]
    pub fn list_uac2_devices(&self) -> Vec<Uac2DeviceInfo> {
        rust_lib_flick_player::uac2::enumerate_uac2_devices()
            .map(|devices| devices.iter().map(Into::into).collect())
            .unwrap_or_default()
    }

    // --- high-res / bit-perfect preferences (engine selection hints) ---------

    /// Allow the Rust engine to initialize even without a detected DAC.
    pub fn set_high_res_mode(&self, enabled: bool) {
        self.manager.set_high_res_mode(enabled);
    }

    pub fn is_high_res_mode_enabled(&self) -> bool {
        self.manager.is_high_res_mode_enabled()
    }

    /// Sync the "Bit-perfect (DAP Internal)" toggling from the frontend.
    pub fn set_dap_bit_perfect_enabled(&self, enabled: bool) {
        self.manager.set_dap_bit_perfect_enabled(enabled);
    }

    pub fn get_dap_bit_perfect_enabled(&self) -> bool {
        self.manager.get_dap_bit_perfect_enabled()
    }

    /// Toggle experimental 432 Hz tuning (leaves bit-perfect, runs the DSP path).
    pub fn set_432hz_tuning_enabled(&self, enabled: bool) {
        self.manager.set_432hz_tuning_enabled(enabled);
    }

    /// Request shutdown of the running engine (drained by [`run`]).
    pub fn shutdown(&self) -> Result<(), AudioError> {
        let manager = Arc::clone(&self.manager);
        // Best-effort: stop the live stream, then wake any `run` task.
        if manager.is_rust_initialized() {
            manager.shutdown()?;
        }
        self.shutdown.notify_waiters();
        Ok(())
    }

    // --- internals ----------------------------------------------------------

    /// Run `f` against the live Rust engine handle, mapping `String` errors.
    fn with_handle<T>(&self, f: impl FnOnce(&rust_lib_flick_player::audio::engine::AudioEngineHandle) -> Result<T, String>) -> Result<T, AudioError> {
        self.manager.with_rust_handle(f).map_err(AudioError::from)
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Async entry point
// ---------------------------------------------------------------------------

/// Keep a shared [`AudioEngine`] alive until shutdown is requested.
///
/// This is the async entry point referenced by the fork brief: it parks the
/// calling task until [`AudioEngine::shutdown`] (or dropping the last handle)
/// signals, then returns. It does not itself drive the audio callback — that
/// lives on Flick's internal decoder/output threads; this only owns the
/// lifetime of the shared engine for Tauri's async command runtime.
pub async fn run(engine: Arc<AudioEngine>) -> Result<(), AudioError> {
    // Await shutdown. `Notify` is edge-triggered: if shutdown already happened
    // (or no engine was ever prepared) we still return cleanly.
    engine.shutdown.notified().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_preset_has_ten_bands_and_is_disabled() {
        let preset = EqualizerPreset::flat();
        assert!(!preset.enabled);
        assert_eq!(preset.bands.len(), 10);
        assert!(preset.gains().is_ok());
    }

    #[test]
    fn serde_roundtrip_playback_state() {
        let json = serde_json::to_string(&PlaybackState::Playing).unwrap();
        assert_eq!(json, "\"playing\"");
        let back: PlaybackState = serde_json::from_str("\"crossfading\"").unwrap();
        assert_eq!(back, PlaybackState::Crossfading);
    }

    #[test]
    fn serde_roundtrip_crossfade_curve() {
        let json = serde_json::to_string(&CrossfadeCurveSerde::SCurve).unwrap();
        assert_eq!(json, "\"s_curve\"");
        let back: CrossfadeCurveSerde = serde_json::from_str("\"equal_power\"").unwrap();
        assert_eq!(back, CrossfadeCurveSerde::EqualPower);
    }

    #[test]
    fn fx_default_matches_disabled_settings() {
        let cfg = FxConfig::default();
        let d = rust_lib_flick_player::audio::fx::FxSettings::disabled();
        assert!(!cfg.enabled);
        assert_eq!(cfg.delay_ms, d.delay_ms);
        assert_eq!(cfg.mix, d.mix);
    }

    #[test]
    fn string_error_maps_to_engine_variant() {
        let err = AudioError::from("Rust audio engine is not initialized".to_string());
        assert!(matches!(err, AudioError::NotInitialized(_)));
        let err = AudioError::from("decoder boom".to_string());
        assert!(matches!(err, AudioError::Engine(_)));
    }

    #[test]
    fn engine_from_playback_state() {
        assert_eq!(
            PlaybackState::from(EnginePlaybackState::Buffering),
            PlaybackState::Buffering
        );
    }
}

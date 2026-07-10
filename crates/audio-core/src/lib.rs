// This file is part of utoaudio, licensed under AGPL-3.0.

//! `utoaudio-audio-core` — thin adapter wrapping the Flick audio engine submodule.
//!
//! The actual engine code lives in the `vendor/flick` git submodule
//! (`rust_lib_flick_player`). This crate re-exports the engine types and
//! exposes a clean, serde-serializable Rust API via [`tauri_api`] for the
//! `audio-ffi` Tauri command layer.

pub mod tauri_api;

// Re-export engine types from the submodule so audio-ffi can reach them
// through this crate's namespace (preserving the existing import paths).
pub use rust_lib_flick_player::audio::backend::AudioBackend;
pub use rust_lib_flick_player::audio::engine::{create_audio_engine, AudioEngineHandle};
pub use rust_lib_flick_player::audio::manager::{
    AudioCapability, AudioCapabilitySnapshot, EngineManager,
};
pub use rust_lib_flick_player::audio::strategy::{
    select_strategy, BackendCandidate, DeviceCaps, TrackInfo,
};

// Re-export the adapter's serde surface.
pub use tauri_api::{
    AudioEngine, AudioError, AudioEventInfo, ConvolverConfig, CrossfadeConfig,
    CrossfadeCurveSerde, EqualizerPreset, EQBand, FxConfig, PlaybackProgressInfo,
    PlaybackState, SongInfo, Uac2DeviceInfo,
};

// Re-export the submodule's machine-side playback state under a suffixed name
// so it stays reachable without clashing with the serde `PlaybackState` above.
pub use rust_lib_flick_player::audio::commands::PlaybackState as EnginePlaybackState;

/// Returns the crate's version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
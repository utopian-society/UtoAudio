// This file is part of utoaudio, licensed under AGPL-3.0.

//! `utoaudio-desktop` Tauri shell.
//!
//! Builds the Tauri application, owns the shared [`AudioEngine`] state and an
//! event-stream shutdown [`Notify`], registers every `#[tauri::command]`
//! handler from `audio-ffi`, and performs best-effort engine shutdown on app
//! exit.
//!
//! On app setup a single [`AudioEngine`] is created, initialised and (best
//! effort) prepared, then wrapped in [`Arc`] and stored as Tauri managed
//! state (`app.manage`). The Tauri shell also manages an `Arc<Notify>` used
//! by the `start_event_stream` command to gracefully stop its polling task on
//! `RunEvent::Exit`. The SQLite-backed library index is opened at
//! `<app_data_dir>/utoaudio/library.sqlite` and stored as managed state too.
//!
//! The mobile (Android) entry point is the same `run()` function — gated by
//! `#[cfg_attr(mobile, tauri::mobile_entry_point)]`. Android-specific wiring
//! (cpal `oboe-shared-stdcxx` feature, Android plugin set) is deferred to a
//! later prompt.

use std::sync::Arc;

use audio_ffi::{AudioEngine, LibraryDb};
use tauri::{Manager, RunEvent};
use tokio::sync::Notify;

/// Builds and runs the utoaudio Tauri application.
///
/// On mobile (Android) targets this is annotated as the platform entry point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Construct the engine up-front and best-effort prepare it. Both calls
    // are deliberately tolerant — `prepare` may legitimately fail when no
    // output device is ready, and the engine re-prepares lazily once a track
    // is actually loaded via `play`.
    let engine = AudioEngine::new();
    engine.init();
    let _ = engine.prepare(None);

    let engine = Arc::new(engine);
    let event_stream_shutdown = Arc::new(Notify::new());

    // Clone for the `setup` closure (FnOnce). The originals are moved into
    // the `run` closure below for use on `RunEvent::Exit`.
    let setup_engine = Arc::clone(&engine);
    let setup_shutdown = Arc::clone(&event_stream_shutdown);

    tauri::Builder::default()
        .setup(move |app| {
            app.manage(setup_engine);
            app.manage(setup_shutdown);

            // Open the SQLite-backed library index. The DB lives at
            // `<app_data_dir>/utoaudio/library.sqlite` — `app_data_dir` is
            // the correct location for data (vs. `app_config_dir` for
            // config). If the path can't be resolved (e.g. on a platform
            // without a standard data dir), we log and skip — the
            // library commands will then return errors at runtime.
            if let Ok(data_dir) = app.path().app_data_dir() {
                match LibraryDb::open(&data_dir) {
                    Ok(db) => {
                        app.manage(Arc::new(db));
                    }
                    Err(e) => {
                        eprintln!("[utoaudio] failed to open library DB: {e}");
                    }
                }
            }

            Ok(())
        })
.invoke_handler(tauri::generate_handler![
  // Lifecycle / capability
  audio_ffi::commands::init,
  audio_ffi::commands::prepare,
  audio_ffi::commands::is_initialized,
  // Playback
  audio_ffi::commands::play,
  audio_ffi::commands::queue_next,
  audio_ffi::commands::pause,
  audio_ffi::commands::resume,
  audio_ffi::commands::stop,
  audio_ffi::commands::seek,
  audio_ffi::commands::set_volume,
  audio_ffi::commands::get_state,
  audio_ffi::commands::get_progress,
  audio_ffi::commands::poll_event,
  audio_ffi::commands::current_path,
  audio_ffi::commands::skip_to_next,
  audio_ffi::commands::set_playback_speed,
  audio_ffi::commands::playback_speed,
  audio_ffi::commands::sample_rate,
  audio_ffi::commands::channels,
  // DSP / FX
  audio_ffi::commands::set_equalizer,
  audio_ffi::commands::set_fx,
  audio_ffi::commands::set_convolver,
  audio_ffi::commands::set_convolver_ir,
  audio_ffi::commands::clear_convolver_ir,
  audio_ffi::commands::set_crossfade,
  // High-res / bit-perfect
  audio_ffi::commands::set_high_res_mode,
  audio_ffi::commands::set_dap_bit_perfect_enabled,
  audio_ffi::commands::set_432hz_tuning_enabled,
  // Hardware / shutdown
  audio_ffi::commands::is_dac_available,
  audio_ffi::commands::shutdown,
  audio_ffi::commands::list_uac2_devices,
  // Misc / event pipe
  audio_ffi::commands::version,
  audio_ffi::commands::start_event_stream,
  // File system
  audio_ffi::commands::scan_directory,
  audio_ffi::commands::scan_library,
  // Persistent settings
  audio_ffi::commands::get_settings,
  audio_ffi::commands::set_settings,
  // Library index (SQLite-backed)
  audio_ffi::commands::get_library_index,
  audio_ffi::commands::rescan_library,
  audio_ffi::commands::search_library,
  audio_ffi::commands::add_scan_root,
  audio_ffi::commands::remove_scan_root,
  audio_ffi::commands::get_scan_roots,
])
        .build(tauri::generate_context!())
        .expect("error while building the utoaudio application")
        .run(move |_app_handle, event| {
            if let RunEvent::Exit = event {
                // Best-effort: stop the live stream, then wake the
                // event-stream polling task so it can exit cleanly.
                let _ = engine.shutdown();
                event_stream_shutdown.notify_waiters();
            }
        });
}

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
    let engine = AudioEngine::new();
    engine.init();

    // Restore saved output device preference before the first prepare so
    // the engine opens the correct device from the start.
    #[cfg(target_os = "linux")]
    {
        if let Ok(settings) = audio_ffi::settings::load_settings() {
            if let Some(od) = &settings.output_device {
                if od.backend == "alsa" {
                    audio_core::set_linux_alsa_device(od.alsa_device.clone());
                }
            }
        }
    }

    // Best-effort initial prepare. Try 48 kHz (the most common USB audio
    // default) first — this "primes" the ALSA device so subsequent
    // re-prepares at other rates (44.1 kHz, 96 kHz, …) succeed. If 48 kHz
    // fails, fall back to None (engine default) and finally 44.1 kHz.
    if engine.prepare(Some(48_000)).is_err() && engine.prepare(None).is_err() {
        let _ = engine.prepare(Some(44_100));
        eprintln!(
            "[utoaudio] startup prepare: all attempts failed (engine will lazy-init on first play)"
        );
    }

    let engine = Arc::new(engine);
    let event_stream_shutdown = Arc::new(Notify::new());

    // Clone for the `setup` closure (FnOnce). The originals are moved into
    // the `run` closure below for use on `RunEvent::Exit`.
    let setup_engine = Arc::clone(&engine);
    let setup_shutdown = Arc::clone(&event_stream_shutdown);

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
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
                // Load music root from settings if available
                let music_root = None; // TODO: load from settings
                match LibraryDb::open(&data_dir, music_root) {
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
            audio_ffi::commands::probe_audio_file,
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
            // Output device (Linux only)
            #[cfg(target_os = "linux")]
            audio_ffi::commands::list_alsa_devices,
            #[cfg(target_os = "linux")]
            audio_ffi::commands::set_output_device,
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
            // Playback queue (SQLite-backed)
            audio_ffi::commands::set_playback_queue,
            audio_ffi::commands::get_playback_queue,
            audio_ffi::commands::clear_playback_queue,
            // Playlist management (SQLite-backed)
            audio_ffi::playlist::list_playlists,
            audio_ffi::playlist::create_playlist,
            audio_ffi::playlist::rename_playlist,
            audio_ffi::playlist::delete_playlist,
            audio_ffi::playlist::get_playlist_tracks,
            audio_ffi::playlist::add_tracks_to_playlist,
            audio_ffi::playlist::import_m3u8_to_playlist,
            audio_ffi::playlist::import_m3u8_to_playlist_with_root,
            audio_ffi::playlist::remove_playlist_track,
            audio_ffi::playlist::move_playlist_track,
            audio_ffi::playlist::export_playlist,
            audio_ffi::playlist::export_playlist_name,
            // Album art
            audio_ffi::commands::get_album_art_data,
            audio_ffi::commands::get_album_art_path,
            // File I/O
            audio_ffi::commands::read_text_file,
            audio_ffi::commands::file_exists,
            audio_ffi::commands::get_embedded_lyrics,
            audio_ffi::commands::load_lyrics,
            audio_ffi::commands::get_song_metadata,
            // Now Playing state
            audio_ffi::commands::get_current_song_info,
            audio_ffi::commands::set_current_song,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the utoaudio application")
        .run(move |_app_handle, event| {
            if let RunEvent::Exit = event {
                // Best-effort: stop the live stream, then wake the
                // event-stream polling task so it can exit cleanly.
                let _ = engine.shutdown();
                #[cfg(target_os = "linux")]
                {
                    // Release the ALSA device reservation so PipeWire can
                    // reclaim the DAC for system audio.
                    audio_core::release_reserved_alsa_device();
                }
                event_stream_shutdown.notify_waiters();
            }
        });
}

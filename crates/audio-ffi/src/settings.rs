// This file is part of utoaudio, licensed under AGPL-3.0.
// audio-ffi::settings - persistent app settings backed by a single JSON
// file in ~/.config/utoaudio/settings.json.
// Schema: { "version": 1, "settings": { ... } }
// Future migrations can bump `version` and handle the transition here.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub enabled_extensions: Vec<String>,
    pub equalizer: Option<EqualizerSettings>,
    pub crossfade: Option<CrossfadeSettings>,
    pub convolver: Option<ConvolverSettings>,
    pub lyric_font_size: Option<u32>,
    pub output_device: Option<OutputDeviceSettings>,
    pub last_played_track: Option<LastPlayedTrack>,
    /// Playback queue repeat mode: `"sequential"`, `"repeat-one"`, `"shuffle"`.
    /// Persisted so the user's choice survives app restarts.
    #[serde(default)]
    pub repeat_mode: Option<String>,
    /// Current playback queue index. Persisted alongside the queue so the
    /// active position is restored on app restart.
    #[serde(default)]
    pub queue_index: Option<i64>,
    /// Global music root directory. All library paths are stored relative to this.
    /// If empty, defaults to the first scan root or a sensible default.
    #[serde(default)]
    pub music_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPlayedTrack {
    pub path: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub album_art_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDeviceSettings {
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alsa_device: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EqualizerSettings {
    pub enabled: bool,
    pub bands: Vec<BandSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BandSettings {
    pub freq_hz: u32,
    pub gain_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrossfadeSettings {
    pub enabled: bool,
    pub duration_secs: f32,
    pub curve: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConvolverSettings {
    pub enabled: bool,
    pub mix: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsFile {
    version: u32,
    settings: Settings,
}

impl SettingsFile {
    fn new(settings: Settings) -> Self {
        Self {
            version: SCHEMA_VERSION,
            settings,
        }
    }
}

fn settings_path() -> Result<PathBuf, String> {
    let app_name = "utoaudio";
    let home = std::env::var("HOME").map_err(|_| "HOME env var not set".to_string())?;
    let p = PathBuf::from(home).join(".config").join(app_name).join("settings.json");
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(p)
}

pub fn load_settings() -> Result<Settings, String> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let contents = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let file: SettingsFile =
        serde_json::from_str(&contents).map_err(|e| format!("parse error: {e}"))?;
    Ok(file.settings)
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    let file = SettingsFile::new(settings.clone());
    let contents = serde_json::to_vec_pretty(&file)
        .map_err(|e| format!("serialize error: {e}"))?;
    let mut f = fs::File::create(&path).map_err(|e| e.to_string())?;
    f.write_all(&contents).map_err(|e| e.to_string())?;
    Ok(())
}

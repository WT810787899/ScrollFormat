use std::collections::HashMap;
use std::path::PathBuf;

use scroll_format_core::AppError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub ui_scale: f32,
    pub default_tab: String,
    pub default_out_dir: String,
    pub max_parallel: usize,
    pub log_level: String,
    pub log_dir: String,
    pub default_preset_id: String,
    pub default_conflict: String,
    pub command_timeout_secs: u64,
    pub on_error: String,
    pub preview_thumb_size: u32,
    pub preview_enabled: bool,
    pub preview_cache_limit: usize,
    pub shortcuts: HashMap<String, String>,
    pub window_w: u32,
    pub window_h: u32,
    pub active_preset_id: String,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            ui_scale: 1.0,
            default_tab: "image".into(),
            default_out_dir: String::new(),
            max_parallel: 2,
            log_level: "info".into(),
            log_dir: String::new(),
            default_preset_id: String::new(),
            default_conflict: "auto_rename".into(),
            command_timeout_secs: 0,
            on_error: "continue".into(),
            preview_thumb_size: 256,
            preview_enabled: true,
            preview_cache_limit: 128,
            shortcuts: HashMap::new(),
            window_w: 1350,
            window_h: 870,
            active_preset_id: String::new(),
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
        }
    }
}

fn settings_path() -> PathBuf {
    crate::paths::app_data_dir().join("settings.json")
}

pub fn load() -> Settings {
    match std::fs::read_to_string(settings_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> Result<(), AppError> {
    let dir = crate::paths::app_data_dir();
    std::fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| AppError::invalid(e.to_string()))?;
    std::fs::write(settings_path(), text)?;
    Ok(())
}

pub fn reset() -> Result<Settings, AppError> {
    let s = Settings::default();
    save(&s)?;
    Ok(s)
}

use std::collections::HashMap;
use std::path::PathBuf;

use scroll_format_core::AppError;

fn path() -> PathBuf {
    crate::paths::app_data_dir().join("ui_state.json")
}

pub fn get(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(path()).ok()?;
    let map: HashMap<String, String> = serde_json::from_str(&text).ok()?;
    map.get(key).cloned()
}

pub fn set(key: &str, value: &str) -> Result<(), AppError> {
    let dir = crate::paths::app_data_dir();
    std::fs::create_dir_all(&dir)?;
    let mut map: HashMap<String, String> = std::fs::read_to_string(path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    map.insert(key.to_string(), value.to_string());
    let text = serde_json::to_string_pretty(&map).map_err(|e| AppError::invalid(e.to_string()))?;
    std::fs::write(path(), text)?;
    Ok(())
}

use std::path::PathBuf;

use scroll_format_core::{AppError, ConvertOptions, OutDirMode, Preset};

fn presets_dir() -> PathBuf {
    crate::paths::app_data_dir().join("presets")
}

fn builtin_presets() -> Vec<Preset> {
    let now = chrono::Utc::now();
    let mk = |id: &str, name: &str| Preset {
        id: id.into(),
        name: name.into(),
        builtin: true,
        kind: None,
        options: ConvertOptions {
            target_ext: String::new(),
            quality: match id {
                "high" => Some(95),
                "size" => Some(60),
                "fast" => Some(70),
                _ => Some(85),
            },
            preset: Some(id.into()),
            extra: serde_json::Value::Null,
        },
        out_dir_mode: OutDirMode::ProjectDefault,
        out_dir: None,
        naming: None,
        custom_args: vec![],
        updated_at: now,
    };
    vec![
        mk("high", "高质量"),
        mk("size", "体积优先"),
        mk("fast", "快速"),
        mk("compatible", "兼容优先"),
    ]
}

pub fn list() -> Vec<Preset> {
    let mut out = builtin_presets();
    let dir = presets_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(text) => match serde_json::from_str::<Preset>(&text) {
                    Ok(p) if !p.builtin => out.push(p),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("跳过损坏的预设 {}: {}", path.display(), e),
                },
                Err(e) => tracing::warn!("读取预设失败 {}: {}", path.display(), e),
            }
        }
    }
    out
}

pub fn save(preset: &Preset) -> Result<(), AppError> {
    let dir = presets_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", preset.id));
    let text = serde_json::to_string_pretty(preset).map_err(|e| AppError::invalid(e.to_string()))?;
    std::fs::write(path, text)?;
    Ok(())
}

pub fn delete(id: &str) -> Result<(), AppError> {
    let path = presets_dir().join(format!("{id}.json"));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn rename(id: &str, name: &str) -> Result<(), AppError> {
    let path = presets_dir().join(format!("{id}.json"));
    let text = std::fs::read_to_string(&path).map_err(|_| AppError::invalid("预设不存在"))?;
    let mut preset: Preset = serde_json::from_str(&text).map_err(|e| AppError::invalid(e.to_string()))?;
    preset.name = name.into();
    preset.updated_at = chrono::Utc::now();
    save(&preset)
}

use std::sync::Arc;

use scroll_format_app::AppService;
use scroll_format_core::{NewTask, Task};
use scroll_format_infra::EnvReport;
use tauri::State;

pub struct AppState {
    pub service: Arc<AppService>,
}
#[tauri::command]
pub async fn create_task(state: State<'_, AppState>, new: NewTask) -> Result<Task, String> {
    state.service.create_task(new).await.map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Clone)]
pub struct PerfStats {
    pub cpu_usage: f32,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    pub gpu_usage: Option<f32>,
    pub gpu_mem_used_mb: Option<u64>,
    pub gpu_mem_total_mb: Option<u64>,
    pub gpu_name: Option<String>,
    pub gpu_available: bool,
    pub running: usize,
    pub queued: usize,
    pub progress: f32,
}

#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    Ok(state.service.list_tasks().await)
}

#[tauri::command]
pub async fn perf_stats(state: State<'_, AppState>) -> Result<PerfStats, String> {
    // 两次刷新以获得有效 CPU 占用（sysinfo 需要采样间隔）
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_usage();
    tokio::time::sleep(std::time::Duration::from_millis(160)).await;
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    let tasks = state.service.list_tasks().await;
    let running = tasks.iter().filter(|t| t.status == scroll_format_core::TaskStatus::Running).count();
    let queued = tasks.iter().filter(|t| t.status == scroll_format_core::TaskStatus::Queued).count();
    let active: Vec<_> = tasks.iter().filter(|t| !t.status.is_terminal()).collect();
    let progress = if active.is_empty() {
        0.0
    } else {
        active.iter().map(|t| t.progress).sum::<f32>() / active.len() as f32
    };
    let gpu = scroll_format_infra::env::probe_gpu().await;
    Ok(PerfStats {
        cpu_usage: sys.global_cpu_usage().clamp(0.0, 100.0),
        mem_used_mb: sys.used_memory() / 1024 / 1024,
        mem_total_mb: sys.total_memory() / 1024 / 1024,
        gpu_usage: gpu.usage,
        gpu_mem_used_mb: gpu.mem_used_mb,
        gpu_mem_total_mb: gpu.mem_total_mb,
        gpu_name: gpu.name,
        gpu_available: gpu.available,
        running,
        queued,
        progress,
    })
}

#[tauri::command]
pub async fn cancel_task(state: State<'_, AppState>, id: uuid::Uuid) -> Result<(), String> {
    state.service.cancel_task(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn retry_task(state: State<'_, AppState>, id: uuid::Uuid) -> Result<(), String> {
    state.service.retry_task(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_file(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err("文件不存在或尚未生成".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(&p)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&p).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(&p).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    // 仅允许 http/https，避免被当作可执行参数注入
    let u = url.trim();
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        return Err("仅支持 http/https 链接".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe").arg(u).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(u).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(u).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    let dir = if p.is_dir() { p } else { p.parent().map(|p| p.to_path_buf()).unwrap_or(p) };
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&dir).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(&dir).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn task_action(state: State<'_, AppState>, action: String, ids: Vec<uuid::Uuid>) -> Result<(), String> {
    for id in ids {
        match action.as_str() {
            "pause" => { state.service.pause_task(id).await.map_err(|e| e.to_string())?; }
            "resume" => { state.service.resume_task(id).await.map_err(|e| e.to_string())?; }
            "cancel" => { state.service.cancel_task(id).await.map_err(|e| e.to_string())?; }
            "retry" => { state.service.retry_task(id).await.map_err(|e| e.to_string())?; }
            "delete" => { state.service.delete_task(id).await.map_err(|e| e.to_string())?; }
            other => return Err(format!("未知操作 {other}")),
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_tasks(state: State<'_, AppState>, statuses: Option<Vec<String>>) -> Result<usize, String> {
    let parsed = statuses.map(|v| {
        v.iter()
            .map(|s| scroll_format_core::TaskStatus::from_str(s))
            .collect::<Vec<_>>()
    }).unwrap_or_default();
    state.service.clear_tasks(parsed).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, id: uuid::Uuid) -> Result<(), String> {
    state.service.delete_task(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn probe_env() -> Result<EnvReport, String> {
    Ok(scroll_format_infra::probe_env().await)
}

#[derive(serde::Serialize)]
pub struct DetectResult {
    pub path: String,
    pub kind: Option<scroll_format_core::FormatKind>,
    pub size: u64,
}

#[tauri::command]
pub async fn detect_format(path: String) -> Result<DetectResult, String> {
    let p = std::path::PathBuf::from(&path);
    let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
    Ok(DetectResult { path, kind: scroll_format_core::detect_kind(&p), size })
}

#[tauri::command]
pub async fn get_preview(state: State<'_, AppState>, path: String) -> Result<serde_json::Value, String> {
    let p = std::path::PathBuf::from(&path);
    let converter = state.service.registry.pick(&p).ok_or_else(|| "不支持的格式".to_string())?;
    let preview = converter.preview(&p).await.map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "thumb_path": preview.thumb_path, "extra": preview.extra }))
}

#[tauri::command]
pub async fn get_logs(limit: Option<usize>) -> Result<Vec<(String, String, String)>, String> {
    let data_dir = scroll_format_infra::paths::app_data_dir();
    let db = scroll_format_infra::Db::open(&data_dir.join("scrollformat.db")).map_err(|e| e.to_string())?;
    db.recent_logs(limit.unwrap_or(200)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_presets(kind: Option<scroll_format_core::FormatKind>) -> Vec<scroll_format_core::Preset> {
    scroll_format_infra::presets::list()
        .into_iter()
        .filter(|p| kind.is_none() || p.kind.is_none() || p.kind == kind)
        .collect()
}

#[tauri::command]
pub fn save_preset(preset: scroll_format_core::Preset) -> Result<(), String> {
    if preset.builtin {
        return Err("内置预设不可修改".into());
    }
    scroll_format_infra::presets::save(&preset).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_preset(id: String) -> Result<(), String> {
    scroll_format_infra::presets::delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_preset(id: String, name: String) -> Result<(), String> {
    scroll_format_infra::presets::rename(&id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_active_preset() -> String {
    scroll_format_infra::settings::load().active_preset_id
}

#[tauri::command]
pub fn set_active_preset(id: String) -> Result<(), String> {
    let mut s = scroll_format_infra::settings::load();
    s.active_preset_id = id;
    scroll_format_infra::settings::save(&s).map_err(|e| e.to_string())
}

#[derive(Debug, serde::Deserialize)]
pub struct SettingsPatch {
    pub theme: Option<String>,
    pub default_out_dir: Option<String>,
    pub max_parallel: Option<usize>,
    pub log_level: Option<String>,
    pub default_conflict: Option<String>,
    pub command_timeout_secs: Option<u64>,
    pub on_error: Option<String>,
    pub preview_enabled: Option<bool>,
    pub preview_cache_limit: Option<usize>,
    pub window_w: Option<u32>,
    pub window_h: Option<u32>,
    pub ui_scale: Option<f32>,
    pub ffmpeg_path: Option<String>,
    pub ffprobe_path: Option<String>,
    pub shortcuts: Option<std::collections::HashMap<String, String>>,
}



#[tauri::command]
pub fn get_settings() -> scroll_format_infra::settings::Settings {
    scroll_format_infra::settings::load()
}

#[tauri::command]
pub fn set_settings(state: State<'_, AppState>, patch: SettingsPatch) -> Result<scroll_format_infra::settings::Settings, String> {
    let mut s = scroll_format_infra::settings::load();
    if let Some(v) = patch.theme { s.theme = v; }
    if let Some(v) = patch.default_out_dir { s.default_out_dir = v; }
    if let Some(v) = patch.max_parallel { s.max_parallel = v; state.service.set_max_parallel(v); }
    if let Some(v) = patch.log_level { s.log_level = v; }
    if let Some(v) = patch.default_conflict { s.default_conflict = v; }
    if let Some(v) = patch.command_timeout_secs { s.command_timeout_secs = v; }
    if let Some(v) = patch.on_error { s.on_error = v; }
    if let Some(v) = patch.preview_enabled { s.preview_enabled = v; }
    if let Some(v) = patch.preview_cache_limit { s.preview_cache_limit = v; }
    if let Some(v) = patch.window_w { s.window_w = v; }
    if let Some(v) = patch.window_h { s.window_h = v; }
    if let Some(v) = patch.ui_scale { s.ui_scale = v; }
    if let Some(v) = patch.ffmpeg_path { s.ffmpeg_path = v.trim().to_string(); }
    if let Some(v) = patch.ffprobe_path { s.ffprobe_path = v.trim().to_string(); }
    if let Some(v) = patch.shortcuts { s.shortcuts = v; }
    scroll_format_infra::settings::save(&s).map_err(|e| e.to_string())?;
    Ok(s)
}

#[tauri::command]
pub fn reset_settings() -> Result<scroll_format_infra::settings::Settings, String> {
    scroll_format_infra::settings::reset().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_task(state: State<'_, AppState>, id: uuid::Uuid) -> Result<(), String> {
    state.service.pause_task(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_task(state: State<'_, AppState>, id: uuid::Uuid) -> Result<(), String> {
    state.service.resume_task(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn kv_get(key: String) -> Option<String> {
    scroll_format_infra::uistate::get(&key)
}

#[tauri::command]
pub fn kv_set(key: String, value: String) -> Result<(), String> {
    scroll_format_infra::uistate::set(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn render_name(rule: scroll_format_core::NamingRule, sample: String) -> Result<String, String> {
    scroll_format_core::render_output_name(&rule, &std::path::PathBuf::from(&sample), rule.index_start)
        .map_err(|e| e.to_string())
}

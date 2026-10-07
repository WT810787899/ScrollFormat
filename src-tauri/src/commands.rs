use std::sync::Arc;

use scroll_format_app::AppService;
use scroll_format_core::{NewTask, Task};
use scroll_format_infra::EnvReport;
use tauri::{Manager, State};

pub struct AppState {
    pub service: Arc<AppService>,
}

/// 以「无窗口」方式启动外部程序（避免黑窗一闪而过）
fn spawn_silent(program: &str, arg: &std::path::Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new(program);
    cmd.arg(arg);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
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
    pub gpu_encoder_usage: Option<f32>,
    pub gpu_mem_used_mb: Option<u64>,
    pub gpu_mem_total_mb: Option<u64>,
    pub gpu_name: Option<String>,
    pub gpu_available: bool,
    pub running: usize,
    pub queued: usize,
    pub progress: f32,
}

/// 监控栏的任务计数与整体进度
///   running  = 转换中（含探测阶段）
///   queued   = 等待中（排队 + 已暂停，都还没开始跑）
///   progress = 仅对「排队/探测/转换中」的任务求平均——暂停中的任务进度恒为 0，
///              混进平均值会把监控栏的进度一直压低
pub fn task_counters(tasks: &[Task]) -> (usize, usize, f32) {
    use scroll_format_core::TaskStatus as S;
    let running = tasks.iter().filter(|t| matches!(t.status, S::Running | S::Probing)).count();
    let queued = tasks.iter().filter(|t| matches!(t.status, S::Queued | S::Paused)).count();
    let active: Vec<_> = tasks.iter().filter(|t| matches!(t.status, S::Queued | S::Probing | S::Running)).collect();
    let progress = if active.is_empty() {
        0.0
    } else {
        active.iter().map(|t| t.progress).sum::<f32>() / active.len() as f32
    };
    (running, queued, progress)
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
    let (running, queued, progress) = task_counters(&tasks);
    let gpu = scroll_format_infra::env::probe_gpu().await;
    Ok(PerfStats {
        cpu_usage: sys.global_cpu_usage().clamp(0.0, 100.0),
        mem_used_mb: sys.used_memory() / 1024 / 1024,
        mem_total_mb: sys.total_memory() / 1024 / 1024,
        gpu_usage: gpu.usage,
        gpu_encoder_usage: gpu.encoder_usage,
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
        spawn_silent("explorer.exe", &p)?;
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
        spawn_silent("explorer.exe", std::path::Path::new(u))?;
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
        spawn_silent("explorer.exe", &dir)?;
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

/// 修改任务参数：输出格式 / 质量 / 高级参数 / 输出目录 / 命名规则
#[tauri::command]
pub async fn update_task(
    state: State<'_, AppState>,
    id: uuid::Uuid,
    options: scroll_format_core::ConvertOptions,
    output_dir: Option<String>,
    output_dir_mode: Option<scroll_format_core::OutDirMode>,
    naming: Option<scroll_format_core::NamingRule>,
) -> Result<scroll_format_core::Task, String> {
    state
        .service
        .update_options(id, options, output_dir, output_dir_mode, naming)
        .await
        .map_err(|e| e.to_string())
}

/// 前端首帧绘制完成后调用：显示启动时隐藏的窗口（消除白屏）
#[tauri::command]
pub fn frontend_ready(app: tauri::AppHandle) {
    show_main_window(&app);
}

/// 找到主窗口并显示（幂等，可重复调用）
pub fn show_main_window(app: &tauri::AppHandle) {
    let win = app
        .get_webview_window("main")
        .or_else(|| app.webview_windows().into_values().next());
    if let Some(w) = win {
        let _ = w.show();
        let _ = w.set_focus();
    }
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

#[cfg(test)]
mod task_counter_tests {
    use super::*;
    use scroll_format_core::{FormatKind, TaskItem, TaskStatus};

    fn task(status: TaskStatus, progress: f32) -> Task {
        Task {
            id: uuid::Uuid::new_v4(),
            name: "t".into(),
            kind: FormatKind::Video,
            status,
            priority: 0,
            items: vec![TaskItem {
                input: "a.mp4".into(),
                size: 0,
                output: None,
                outputs: vec![],
                status,
                progress,
                error: None,
                log: vec![],
            }],
            output_dir: std::path::PathBuf::from("out"),
            options: scroll_format_core::ConvertOptions {
                target_ext: "mp4".into(),
                quality: None,
                preset: None,
                extra: serde_json::Value::Null,
            },
            output_dir_mode: scroll_format_core::OutDirMode::Custom,
            naming: None,
            progress,
            error: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            finished_at: None,
        }
    }

    #[test]
    fn paused_task_does_not_drag_progress_down() {
        // 1 个转换到 40% + 1 个暂停（进度恒为 0）：进度应为 40%，而不是 20%
        let tasks = vec![
            task(TaskStatus::Running, 0.4),
            task(TaskStatus::Paused, 0.0),
        ];
        let (running, queued, progress) = task_counters(&tasks);
        assert_eq!(running, 1);
        assert_eq!(queued, 1); // 暂停计入等待中
        assert!((progress - 0.4).abs() < 1e-5, "progress = {progress}");
    }

    #[test]
    fn probing_counts_as_running_and_terminal_is_ignored() {
        let tasks = vec![
            task(TaskStatus::Probing, 0.1),
            task(TaskStatus::Queued, 0.0),
            task(TaskStatus::Completed, 1.0),
            task(TaskStatus::Failed, 0.0),
            task(TaskStatus::Cancelled, 0.0),
        ];
        let (running, queued, progress) = task_counters(&tasks);
        assert_eq!(running, 1);
        assert_eq!(queued, 1);
        assert!((progress - 0.05).abs() < 1e-5, "progress = {progress}");
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(task_counters(&[]), (0, 0, 0.0));
    }
}
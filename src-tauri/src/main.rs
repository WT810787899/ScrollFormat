mod commands;

use std::sync::Arc;

use scroll_format_app::AppService;
use scroll_format_infra::Db;

use commands::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let data_dir = scroll_format_infra::paths::app_data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    let db = Arc::new(Db::open(&data_dir.join("scrollformat.db")).expect("open db"));
    db.recover().ok();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let service = Arc::new(rt.block_on(AppService::new(db)));
    {
        let s = service.clone();
        rt.block_on(async move {
            s.spawn_scheduler();
        });
    }
    // 保持 runtime 存活（调度器在后台线程上运行）
    std::mem::forget(rt);

    let service2 = service.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { service })
        .setup(move |app| {
            let handle = app.handle().clone();
            let mut rx = service2.events.subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(ev) = rx.recv().await {
                    use tauri::Emitter;
                    let (name, payload) = match &ev {
                        scroll_format_app::ServiceEvent::TaskCreated { id } => ("task://created", serde_json::json!({ "id": id })),
                        scroll_format_app::ServiceEvent::TaskUpdated { id } => ("task://updated", serde_json::json!({ "id": id })),
                        scroll_format_app::ServiceEvent::TaskProgress { id, progress } => ("task://progress", serde_json::json!({ "id": id, "progress": progress })),
                        scroll_format_app::ServiceEvent::TaskCompleted { id } => ("task://completed", serde_json::json!({ "id": id })),
                        scroll_format_app::ServiceEvent::TaskFailed { id, error } => ("task://failed", serde_json::json!({ "id": id, "error": error })),
                        scroll_format_app::ServiceEvent::TaskLog { id, line } => ("task://log", serde_json::json!({ "id": id, "line": line })),
                    };
                    let _ = handle.emit(name, payload);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_task,
            commands::list_tasks,
            commands::cancel_task,
            commands::retry_task,
            commands::delete_task,
            commands::clear_tasks,
            commands::task_action,
            commands::open_in_explorer,
            commands::open_url,
            commands::open_file,
            commands::perf_stats,
            commands::probe_env,
            commands::detect_format,
            commands::get_preview,
            commands::get_logs,
            commands::list_presets,
            commands::save_preset,
            commands::delete_preset,
            commands::rename_preset,
            commands::get_active_preset,
            commands::set_active_preset,
            commands::get_settings,
            commands::set_settings,
            commands::reset_settings,
            commands::pause_task,
            commands::resume_task,
            commands::render_name,
            commands::kv_get,
            commands::kv_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

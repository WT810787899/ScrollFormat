use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::Utc;
use scroll_format_core::{AppError, ConvertContext, ConverterRegistry, NewTask, Task, TaskStatus};
use scroll_format_infra::Db;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServiceEvent {
    #[serde(rename = "task_created")]
    TaskCreated { id: Uuid },
    #[serde(rename = "task_updated")]
    TaskUpdated { id: Uuid },
    #[serde(rename = "task_progress")]
    TaskProgress { id: Uuid, progress: f32 },
    #[serde(rename = "task_completed")]
    TaskCompleted { id: Uuid },
    #[serde(rename = "task_failed")]
    TaskFailed { id: Uuid, error: AppError },
    #[serde(rename = "task_log")]
    TaskLog { id: Uuid, line: String },
}

pub struct AppService {
    pub db: Arc<Db>,
    pub registry: Arc<ConverterRegistry>,
    tasks: Arc<Mutex<HashMap<Uuid, Task>>>,
    queue: Arc<Mutex<VecDeque<Uuid>>>,
    cancels: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
    notify: Arc<Notify>,
    pub events: tokio::sync::broadcast::Sender<ServiceEvent>,
    pub max_parallel: std::sync::atomic::AtomicUsize,
}

impl AppService {
    pub async fn new(db: Arc<Db>) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(256);
        let registry = Arc::new(scroll_format_converters::build_default_registry());
        let service = Self {
            db,
            registry,
            tasks: Default::default(),
            queue: Default::default(),
            cancels: Default::default(),
            notify: Arc::new(Notify::new()),
            events,
            max_parallel: std::sync::atomic::AtomicUsize::new(2),
        };
        service.reload_from_db().await;
        service
    }

    async fn reload_from_db(&self) {
        if let Ok(tasks) = self.db.list_tasks() {
            let mut map = self.tasks.lock().await;
            let mut queue = self.queue.lock().await;
            for t in tasks {
                let requeue = matches!(t.status, TaskStatus::Queued);
                if requeue {
                    queue.push_back(t.id);
                }
                map.insert(t.id, t);
            }
        }
    }

    pub async fn create_task(&self, new: NewTask) -> Result<Task, AppError> {
        if new.input_files.is_empty() {
            return Err(AppError::invalid("请至少选择一个文件"));
        }
        let task = Task::new(new);
        self.db.upsert_task(&task)?;
        self.tasks.lock().await.insert(task.id, task.clone());
        self.queue.lock().await.push_back(task.id);
        self.notify.notify_one();
        let _ = self.events.send(ServiceEvent::TaskCreated { id: task.id });
        Ok(task)
    }

    pub async fn list_tasks(&self) -> Vec<Task> {
        let mut v: Vec<Task> = self.tasks.lock().await.values().cloned().collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub async fn get_task(&self, id: Uuid) -> Option<Task> {
        self.tasks.lock().await.get(&id).cloned()
    }

    pub async fn cancel_task(&self, id: Uuid) -> Result<(), AppError> {
        if let Some(token) = self.cancels.lock().await.get(&id) {
            token.cancel();
        }
        let mut map = self.tasks.lock().await;
        if let Some(t) = map.get_mut(&id) {
            if !t.status.is_terminal() {
                t.status = TaskStatus::Cancelled;
                t.finished_at = Some(Utc::now());
                for it in &mut t.items {
                    if it.status == TaskStatus::Running || it.status == TaskStatus::Queued {
                        it.status = TaskStatus::Cancelled;
                    }
                }
                self.db.upsert_task(t)?;
                let _ = self.events.send(ServiceEvent::TaskUpdated { id });
            }
        }
        self.queue.lock().await.retain(|q| *q != id);
        Ok(())
    }

    pub async fn retry_task(&self, id: Uuid) -> Result<(), AppError> {
        let mut map = self.tasks.lock().await;
        let t = map.get_mut(&id).ok_or_else(|| AppError::invalid("任务不存在"))?;
        t.status = TaskStatus::Queued;
        t.error = None;
        t.progress = 0.0;
        t.finished_at = None;
        t.started_at = None;
        for it in &mut t.items {
            it.status = TaskStatus::Queued;
            it.progress = 0.0;
            it.error = None;
            it.output = None;
        }
        self.db.upsert_task(t)?;
        let _ = self.events.send(ServiceEvent::TaskUpdated { id });
        drop(map);
        self.queue.lock().await.push_back(id);
        self.notify.notify_one();
        Ok(())
    }

    pub async fn delete_task(&self, id: Uuid) -> Result<(), AppError> {
        self.tasks.lock().await.remove(&id);
        self.queue.lock().await.retain(|q| *q != id);
        self.db.delete_task(&id)?;
        Ok(())
    }

    pub fn set_max_parallel(&self, n: usize) {
        self.max_parallel.store(n.max(1), std::sync::atomic::Ordering::Relaxed);
    }

    pub async fn pause_task(&self, id: Uuid) -> Result<(), AppError> {
        let mut map = self.tasks.lock().await;
        let t = map.get_mut(&id).ok_or_else(|| AppError::invalid("任务不存在"))?;
        match t.status {
            TaskStatus::Queued | TaskStatus::Probing => {
                t.status = TaskStatus::Paused;
                for it in &mut t.items {
                    if it.status == TaskStatus::Queued {
                        it.status = TaskStatus::Paused;
                    }
                }
                self.db.upsert_task(t)?;
                let _ = self.events.send(ServiceEvent::TaskUpdated { id });
            }
            TaskStatus::Running => {
                // 暂停：取消当前转换但保持 Paused，等待「继续」再重新入队
                t.status = TaskStatus::Paused;
                for it in &mut t.items {
                    if it.status == TaskStatus::Running || it.status == TaskStatus::Queued {
                        it.status = TaskStatus::Paused;
                        it.progress = 0.0;
                    }
                }
                self.db.upsert_task(t)?;
                if let Some(token) = self.cancels.lock().await.get(&id) {
                    token.cancel();
                }
                let _ = self.events.send(ServiceEvent::TaskUpdated { id });
            }
            _ => {}
        }
        drop(map);
        self.queue.lock().await.retain(|q| *q != id);
        Ok(())
    }

    pub async fn resume_task(&self, id: Uuid) -> Result<(), AppError> {
        let mut map = self.tasks.lock().await;
        let t = map.get_mut(&id).ok_or_else(|| AppError::invalid("任务不存在"))?;
        if t.status == TaskStatus::Paused {
            t.status = TaskStatus::Queued;
            t.started_at = None;
            for it in &mut t.items {
                if it.status == TaskStatus::Paused || it.status == TaskStatus::Cancelled {
                    it.status = TaskStatus::Queued;
                    it.progress = 0.0;
                    it.error = None;
                }
            }
            self.db.upsert_task(t)?;
            let _ = self.events.send(ServiceEvent::TaskUpdated { id });
            drop(map);
            self.queue.lock().await.push_back(id);
            self.notify.notify_one();
        }
        Ok(())
    }

    pub async fn clear_tasks(&self, statuses: Vec<scroll_format_core::TaskStatus>) -> Result<usize, AppError> {
        let ids: Vec<Uuid> = {
            let map = self.tasks.lock().await;
            map.values()
                .filter(|t| statuses.is_empty() || statuses.contains(&t.status))
                .map(|t| t.id)
                .collect()
        };
        let mut count = 0;
        for id in ids {
            self.tasks.lock().await.remove(&id);
            self.queue.lock().await.retain(|q| *q != id);
            self.db.delete_task(&id)?;
            count += 1;
        }
        Ok(count)
    }

    /// 启动调度循环（在 tokio 任务中运行）
    pub fn spawn_scheduler(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let id = {
                    let running = {
                        let map = this.tasks.lock().await;
                        map.values().filter(|t| t.status == TaskStatus::Running).count()
                    };
                    if running >= this.max_parallel.load(std::sync::atomic::Ordering::Relaxed) {
                        this.notify.notified().await;
                        continue;
                    } else {
                        let mut q = this.queue.lock().await;
                        q.pop_front()
                    }
                };
                match id {
                    Some(id) => this.run_task(id).await,
                    None => {
                        this.notify.notified().await;
                    }
                }
            }
        });
    }

    async fn run_task(self: &Arc<Self>, id: Uuid) {
        let task = match self.get_task(id).await {
            Some(t) => t,
            None => return,
        };
        if task.status != TaskStatus::Queued {
            return;
        }
        let cancel = CancellationToken::new();
        self.cancels.lock().await.insert(id, cancel.clone());
        {
            let mut map = self.tasks.lock().await;
            if let Some(t) = map.get_mut(&id) {
                t.status = TaskStatus::Running;
                t.started_at = Some(Utc::now());
                self.db.upsert_task(t).ok();
            }
        }
        let _ = self.events.send(ServiceEvent::TaskUpdated { id });

        let mut failed: Option<AppError> = None;
        for (idx, item) in task.items.iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            let converter = match self.registry.for_kind(task.kind).into_iter().next() {
                Some(c) => c,
                None => {
                    failed = Some(AppError::unsupported("无可用转换器"));
                    break;
                }
            };
            let (ptx, mut prx) = mpsc::channel::<f32>(8);
            let (ltx, mut lrx) = mpsc::channel::<String>(64);
            let id_copy = id;
            let events = self.events.clone();
            let prog_task = tokio::spawn(async move {
                while let Some(p) = prx.recv().await {
                    let _ = events.send(ServiceEvent::TaskProgress { id: id_copy, progress: p });
                }
            });
            let log_events = self.events.clone();
            let db = self.db.clone();
            let log_task = tokio::spawn(async move {
                while let Some(line) = lrx.recv().await {
                    let _ = log_events.send(ServiceEvent::TaskLog { id: id_copy, line: line.clone() });
                    db.add_log("info", &line).ok();
                }
            });

            let input = std::path::PathBuf::from(&item.input);
            let stem = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "output".into());
            let out_dir = match task.output_dir_mode {
                scroll_format_core::OutDirMode::SourceDir => input.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| task.output_dir.clone()),
                _ => task.output_dir.clone(),
            };
            let (name, conflict) = match &task.naming {
                Some(rule) => (
                    scroll_format_core::render_output_name(rule, &input, idx as u32 + 1).unwrap_or_else(|_| stem.clone()),
                    rule.conflict.clone(),
                ),
                None => (stem.clone(), scroll_format_core::ConflictPolicy::AutoRename),
            };
            let skip = conflict == scroll_format_core::ConflictPolicy::Skip
                && out_dir.join(format!("{name}.{}", task.options.target_ext)).exists();
            let output = match conflict {
                scroll_format_core::ConflictPolicy::Overwrite => out_dir.join(format!("{name}.{}", task.options.target_ext)),
                scroll_format_core::ConflictPolicy::Skip => out_dir.join(format!("{name}.{}", task.options.target_ext)),
                scroll_format_core::ConflictPolicy::AutoRename => scroll_format_converters::unique_output_path(&out_dir, &name, &task.options.target_ext),
            };

            if skip {
                let mut map = self.tasks.lock().await;
                if let Some(t) = map.get_mut(&id) {
                    t.items[idx].status = TaskStatus::Completed;
                    t.items[idx].progress = 1.0;
                    t.items[idx].output = Some(output.to_string_lossy().to_string());
                    t.recompute_progress();
                    self.db.upsert_task(t).ok();
                }
                drop(ptx);
                drop(ltx);
                let _ = prog_task.await;
                let _ = log_task.await;
                continue;
            }

            {
                let mut map = self.tasks.lock().await;
                if let Some(t) = map.get_mut(&id) {
                    t.items[idx].status = TaskStatus::Running;
                    t.items[idx].output = Some(output.to_string_lossy().to_string());
                    self.db.upsert_task(t).ok();
                }
            }

            let ctx = ConvertContext {
                input: input.clone(),
                output: output.clone(),
                options: task.options.clone(),
                cancel: cancel.clone(),
                progress: ptx,
                log: ltx,
            };
            let settings = scroll_format_infra::settings::load();
            let result = if settings.command_timeout_secs > 0 {
                match tokio::time::timeout(std::time::Duration::from_secs(settings.command_timeout_secs), converter.convert(ctx)).await {
                    Ok(r) => r,
                    Err(_) => Err(AppError::new(scroll_format_core::ErrorCode::ProcessTimeout, format!("转换超过 {} 秒超时", settings.command_timeout_secs))),
                }
            } else {
                converter.convert(ctx).await
            };
            let _ = prog_task.await;
            let _ = log_task.await;
            match result {
                Ok(out) => {
                    let mut map = self.tasks.lock().await;
                    if let Some(t) = map.get_mut(&id) {
                        t.items[idx].status = TaskStatus::Completed;
                        t.items[idx].progress = 1.0;
                        t.items[idx].output = Some(out.output);
                        t.recompute_progress();
                        self.db.upsert_task(t).ok();
                    }
                }
                Err(e) => {
                    let mut map = self.tasks.lock().await;
                    if let Some(t) = map.get_mut(&id) {
                        let cancelled = matches!(e.code, scroll_format_core::ErrorCode::Cancelled);
                        t.items[idx].status = if cancelled && t.status == TaskStatus::Paused {
                            TaskStatus::Paused
                        } else if cancelled {
                            TaskStatus::Cancelled
                        } else {
                            TaskStatus::Failed
                        };
                        if !(cancelled && t.status == TaskStatus::Paused) {
                            t.items[idx].error = Some(e.clone());
                        }
                        t.recompute_progress();
                        self.db.upsert_task(t).ok();
                    }
                    failed = Some(e);
                }
            }
            let _ = self.events.send(ServiceEvent::TaskUpdated { id });
        }

        self.cancels.lock().await.remove(&id);
        let mut map = self.tasks.lock().await;
        if let Some(t) = map.get_mut(&id) {
            if cancel.is_cancelled() {
                if t.status == TaskStatus::Paused {
                    // 暂停：保持 Paused，不标记取消/失败，等待用户继续
                    t.finished_at = None;
                    let _ = self.events.send(ServiceEvent::TaskUpdated { id });
                } else {
                    t.status = TaskStatus::Cancelled;
                    t.finished_at = Some(Utc::now());
                }
            } else if let Some(e) = failed {
                t.status = if matches!(e.code, scroll_format_core::ErrorCode::Cancelled) { TaskStatus::Cancelled } else { TaskStatus::Failed };
                t.error = Some(e.clone());
                let _ = self.events.send(ServiceEvent::TaskFailed { id, error: e });
            } else {
                t.status = TaskStatus::Completed;
                t.progress = 1.0;
                let _ = self.events.send(ServiceEvent::TaskCompleted { id });
            }
            t.finished_at = Some(Utc::now());
            self.db.upsert_task(t).ok();
        }
        drop(map);
        self.notify.notify_one();
    }
}

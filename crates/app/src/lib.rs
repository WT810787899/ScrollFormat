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
    /// 已派发但尚未结束的任务数（并行度占用计数，由调度器增减）
    active: std::sync::atomic::AtomicUsize,
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
            active: std::sync::atomic::AtomicUsize::new(0),
        };
        service.reload_from_db().await;
        // 启动即应用设置里的并行任务数，否则改完设置重启会悄悄退回 2
        let settings = scroll_format_infra::settings::load();
        service.set_max_parallel(settings.max_parallel);
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
        // 补齐老任务的源文件大小（早期版本的 items_json 里没有 size 字段），
        // 补完落库，之后就不再 stat 了
        {
            let mut map = self.tasks.lock().await;
            let mut changed: Vec<Task> = Vec::new();
            for t in map.values_mut() {
                let mut dirty = false;
                for it in t.items.iter_mut() {
                    if it.size == 0 {
                        if let Ok(m) = std::fs::metadata(&it.input) {
                            it.size = m.len();
                            dirty = true;
                        }
                    }
                }
                if dirty {
                    changed.push(t.clone());
                }
            }
            for t in changed {
                self.db.upsert_task(&t).ok();
            }
        }
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
            it.outputs.clear();
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

    /// 修改任务参数（格式 / 质量 / 高级参数 / 输出目录 / 命名规则）。
    /// 运行中的任务不允许改：此时 ffmpeg 已经带着旧参数在跑，改了只会让界面与实际不一致。
    pub async fn update_options(
        &self,
        id: Uuid,
        options: scroll_format_core::ConvertOptions,
        output_dir: Option<String>,
        output_dir_mode: Option<scroll_format_core::OutDirMode>,
        naming: Option<scroll_format_core::NamingRule>,
    ) -> Result<Task, AppError> {
        let mut map = self.tasks.lock().await;
        let t = map.get_mut(&id).ok_or_else(|| AppError::invalid("任务不存在"))?;
        if matches!(t.status, TaskStatus::Running | TaskStatus::Probing) {
            return Err(AppError::invalid("任务进行中，请先暂停后再修改参数"));
        }
        if options.target_ext.trim().is_empty() {
            return Err(AppError::invalid("输出格式不能为空"));
        }
        t.options = options;
        if let Some(dir) = output_dir {
            if !dir.trim().is_empty() {
                t.output_dir = std::path::PathBuf::from(dir);
            }
        }
        if let Some(mode) = output_dir_mode {
            t.output_dir_mode = mode;
        }
        if naming.is_some() {
            t.naming = naming;
        }
        // 参数变了，旧的输出路径已失效（下次运行会按新命名/格式重新计算）
        for it in &mut t.items {
            it.output = None;
            it.outputs.clear();
        }
        self.db.upsert_task(t)?;
        let snapshot = t.clone();
        let _ = self.events.send(ServiceEvent::TaskUpdated { id });
        Ok(snapshot)
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
    ///
    /// 并发模型：`active` 原子计数 = 已派发但尚未结束的任务数。
    /// 关键点是**派发即占名额**（同步完成），执行则丢到独立 tokio 任务里跑，
    /// 否则循环会被 `run_task().await` 阻塞住，实际并行度永远是 1。
    pub fn spawn_scheduler(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            use std::sync::atomic::Ordering::Relaxed;
            loop {
                if this.active.load(Relaxed) >= this.max_parallel.load(Relaxed) {
                    this.notify.notified().await;
                    continue;
                }
                let next = { this.queue.lock().await.pop_front() };
                let Some(id) = next else {
                    this.notify.notified().await;
                    continue;
                };
                // 抢占：任务必须仍是 Queued（可能已被取消/暂停/删除），同时占名额
                let claimed = {
                    let mut map = this.tasks.lock().await;
                    match map.get_mut(&id) {
                        Some(t) if t.status == TaskStatus::Queued => {
                            t.status = TaskStatus::Running;
                            t.started_at = Some(Utc::now());
                            this.db.upsert_task(t).ok();
                            true
                        }
                        _ => false,
                    }
                };
                if !claimed {
                    continue;
                }
                this.active.fetch_add(1, Relaxed);
                let _ = this.events.send(ServiceEvent::TaskUpdated { id });
                let runner = Arc::clone(&this);
                tokio::spawn(async move {
                    runner.run_task(id).await;
                    runner.active.fetch_sub(1, Relaxed);
                    runner.notify.notify_one();
                });
            }
        });
    }

    /// 执行一个已被调度器抢占（状态已是 Running）的任务
    async fn run_task(self: &Arc<Self>, id: Uuid) {
        let task = match self.get_task(id).await {
            Some(t) => t,
            None => return,
        };
        let cancel = CancellationToken::new();
        self.cancels.lock().await.insert(id, cancel.clone());

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
            let this = self.clone();
            let prog_task = tokio::spawn(async move {
                while let Some(p) = prx.recv().await {
                    // 进度必须写回内存任务：监控栏与任务列表都读这里的值。
                    // 以前只广播事件，导致转换期间 list_tasks() 里的 progress 恒为 0
                    // （前端靠事件兜底，切页/重启就丢），监控栏进度整段显示 0%。
                    {
                        let mut map = this.tasks.lock().await;
                        if let Some(t) = map.get_mut(&id_copy) {
                            if let Some(it) = t.items.get_mut(idx) {
                                it.progress = p;
                            }
                            t.recompute_progress();
                        }
                    }
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
                        // 多产物（PDF 多页导图）：主输出是目录，全部产物记在 outputs
                        t.items[idx].outputs = out.all_outputs();
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

#[cfg(test)]
mod tests {
    use super::*;
    use scroll_format_core::{ConvertOptions, FormatKind, NewTask, OutDirMode};

    async fn svc() -> (AppService, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("sf_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(Db::open(&dir.join("t.db")).unwrap());
        (AppService::new(db).await, dir)
    }

    fn new_task() -> NewTask {
        NewTask {
            kind: FormatKind::Video,
            input_files: vec!["C:\\v\\a.mp4".into()],
            output_dir: "C:\\out".into(),
            options: ConvertOptions { target_ext: "mp4".into(), quality: Some(85), preset: None, extra: serde_json::Value::Null },
            priority: 0,
            output_dir_mode: OutDirMode::Custom,
            naming: None,
        }
    }

    #[tokio::test]
    async fn update_options_changes_params_and_clears_stale_output() {
        let (s, dir) = svc().await;
        let t = s.create_task(new_task()).await.unwrap();
        // 模拟一次已完成的任务：带一个旧输出路径
        s.cancel_task(t.id).await.unwrap();

        let updated = s
            .update_options(
                t.id,
                ConvertOptions { target_ext: "mkv".into(), quality: Some(60), preset: None, extra: serde_json::json!({"crf": 30}) },
                Some("D:\\newout".into()),
                Some(OutDirMode::Custom),
                None,
            )
            .await
            .unwrap();

        assert_eq!(updated.options.target_ext, "mkv");
        assert_eq!(updated.options.quality, Some(60));
        assert_eq!(updated.options.extra["crf"], 30);
        assert_eq!(updated.output_dir.to_string_lossy(), "D:\\newout");
        assert!(updated.items.iter().all(|i| i.output.is_none()), "旧输出路径应清空");
        // 落库校验
        let saved = s.db.list_tasks().unwrap().into_iter().find(|x| x.id == t.id).unwrap();
        assert_eq!(saved.options.target_ext, "mkv");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// 并行调度：max_parallel=2 时应同时跑 2 个任务，且绝不超过 2 个
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn scheduler_runs_tasks_in_parallel_up_to_max() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;

        let dir = std::env::temp_dir().join(format!("sf_par_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        // 3000x2000 的 PNG，转码耗时足够长，便于观察到并发窗口
        let img = image::RgbImage::from_pixel(3000, 2000, image::Rgb([40, 90, 160]));
        let mut paths = Vec::new();
        for i in 0..4 {
            let p = dir.join(format!("src{i}.png"));
            img.save(&p).unwrap();
            paths.push(p.to_string_lossy().to_string());
        }
        let db = Arc::new(Db::open(&dir.join("t.db")).unwrap());
        let service = Arc::new(AppService::new(db).await);
        service.set_max_parallel(2);
        service.spawn_scheduler();

        for p in &paths {
            service
                .create_task(NewTask {
                    kind: FormatKind::Image,
                    input_files: vec![p.clone()],
                    output_dir: dir.join("out").to_string_lossy().to_string(),
                    options: ConvertOptions {
                        target_ext: "jpg".into(),
                        quality: Some(80),
                        preset: None,
                        extra: serde_json::json!({ "scale_mode": "original" }),
                    },
                    priority: 0,
                    output_dir_mode: OutDirMode::Custom,
                    naming: None,
                })
                .await
                .unwrap();
        }

        let peak = Arc::new(AtomicUsize::new(0));
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
            let tasks = service.list_tasks().await;
            let running = tasks.iter().filter(|t| t.status == TaskStatus::Running).count();
            peak.fetch_max(running, Ordering::Relaxed);
            let done = tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
            if done == paths.len() {
                break;
            }
            assert!(tasks.iter().all(|t| t.status != TaskStatus::Failed), "有任务失败");
            assert!(running <= 2, "并发数 {running} 超过了 max_parallel=2");
            if std::time::Instant::now() > deadline {
                panic!("60 秒内未完成全部任务");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let peak = peak.load(Ordering::Relaxed);
        assert!(peak >= 2, "从未观察到并行执行（峰值并发 {peak}），max_parallel 未生效");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[tokio::test]
    async fn update_options_rejects_running_task_and_empty_target() {        let (s, dir) = svc().await;
        let t = s.create_task(new_task()).await.unwrap();
        let opts = ConvertOptions { target_ext: "mkv".into(), quality: None, preset: None, extra: serde_json::Value::Null };
        // 直接把状态改成 running，验证运行中不可改
        {
            let mut map = s.tasks.lock().await;
            map.get_mut(&t.id).unwrap().status = TaskStatus::Running;
        }
        let err = s.update_options(t.id, opts.clone(), None, None, None).await.unwrap_err();
        assert!(err.message.contains("进行中"), "{}", err.message);

        {
            let mut map = s.tasks.lock().await;
            map.get_mut(&t.id).unwrap().status = TaskStatus::Queued;
        }
        let bad = ConvertOptions { target_ext: " ".into(), quality: None, preset: None, extra: serde_json::Value::Null };
        assert!(s.update_options(t.id, bad, None, None, None).await.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
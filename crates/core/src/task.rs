use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::AppError;
use crate::format::FormatKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Probing,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Queued => "queued",
            TaskStatus::Probing => "probing",
            TaskStatus::Running => "running",
            TaskStatus::Paused => "paused",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "probing" => TaskStatus::Probing,
            "running" => TaskStatus::Running,
            "paused" => TaskStatus::Paused,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Queued,
        }
    }
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertOptions {
    pub target_ext: String,
    #[serde(default)]
    pub quality: Option<u8>,
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTask {
    pub kind: FormatKind,
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub options: ConvertOptions,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub output_dir_mode: crate::preset::OutDirMode,
    #[serde(default)]
    pub naming: Option<crate::naming::NamingRule>,
}

fn default_priority() -> i32 { 0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub input: String,
    /// 源文件大小（建任务时记录，任务卡片上展示；老数据为 0 表示未知）
    #[serde(default)]
    pub size: u64,
    pub output: Option<String>,
    /// 全部产物（PDF 多页导图等场景会有多个）；`output` 指向主产物/目录
    #[serde(default)]
    pub outputs: Vec<String>,
    pub status: TaskStatus,
    pub progress: f32,
    pub error: Option<AppError>,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub kind: FormatKind,
    pub status: TaskStatus,
    pub priority: i32,
    pub items: Vec<TaskItem>,
    pub output_dir: PathBuf,
    pub options: ConvertOptions,
    #[serde(default)]
    pub output_dir_mode: crate::preset::OutDirMode,
    #[serde(default)]
    pub naming: Option<crate::naming::NamingRule>,
    pub progress: f32,
    pub error: Option<AppError>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl Task {
    pub fn new(new: NewTask) -> Self {
        let id = Uuid::new_v4();
        let items = new
            .input_files
            .iter()
            .map(|f| TaskItem {
                input: f.clone(),
                size: std::fs::metadata(f).map(|m| m.len()).unwrap_or(0),
                output: None,
                outputs: Vec::new(),
                status: TaskStatus::Queued,
                progress: 0.0,
                error: None,
                log: vec![],
            })
            .collect::<Vec<_>>();
        let name = PathBuf::from(&new.input_files[0])
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "任务".into());
        Self {
            id,
            name,
            kind: new.kind,
            status: TaskStatus::Queued,
            priority: new.priority,
            items,
            output_dir: PathBuf::from(new.output_dir),
            options: new.options,
            output_dir_mode: new.output_dir_mode,
            naming: new.naming,
            progress: 0.0,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
        }
    }

    pub fn recompute_progress(&mut self) {
        if self.items.is_empty() {
            self.progress = 0.0;
            return;
        }
        let sum: f32 = self.items.iter().map(|i| i.progress).sum();
        self.progress = sum / self.items.len() as f32;
    }
}

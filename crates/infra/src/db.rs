use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use scroll_format_core::{AppError, ConvertOptions, FormatKind, Task, TaskStatus};

pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &std::path::Path) -> Result<Self, AppError> {
        let conn = Connection::open(path).map_err(|e| AppError::io(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                priority INTEGER NOT NULL,
                items_json TEXT NOT NULL,
                output_dir TEXT NOT NULL,
                options_json TEXT NOT NULL,
                progress REAL NOT NULL,
                error_json TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT,
                output_dir_mode TEXT DEFAULT 'project_default',
                naming_json TEXT
            );
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS logs (id INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL, level TEXT NOT NULL, message TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);",
        )
        .map_err(|e| AppError::io(e.to_string()))?;
        // 老库迁移：补齐新增列
        let mut columns: Vec<String> = vec![];
        {
            let mut stmt = conn
                .prepare("PRAGMA table_info(tasks)")
                .map_err(|e| AppError::io(e.to_string()))?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .map_err(|e| AppError::io(e.to_string()))?;
            for row in rows.flatten() {
                columns.push(row);
            }
        }
        if !columns.iter().any(|c| c == "output_dir_mode") {
            conn.execute_batch("ALTER TABLE tasks ADD COLUMN output_dir_mode TEXT DEFAULT 'project_default';")
                .map_err(|e| AppError::io(e.to_string()))?;
        }
        if !columns.iter().any(|c| c == "naming_json") {
            conn.execute_batch("ALTER TABLE tasks ADD COLUMN naming_json TEXT;")
                .map_err(|e| AppError::io(e.to_string()))?;
        }
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn upsert_task(&self, t: &Task) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks (id,name,kind,status,priority,items_json,output_dir,options_json,progress,error_json,created_at,started_at,finished_at,output_dir_mode,naming_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,kind=excluded.kind,status=excluded.status,priority=excluded.priority,items_json=excluded.items_json,output_dir=excluded.output_dir,options_json=excluded.options_json,progress=excluded.progress,error_json=excluded.error_json,started_at=excluded.started_at,finished_at=excluded.finished_at,output_dir_mode=excluded.output_dir_mode,naming_json=excluded.naming_json",
            rusqlite::params![
                t.id.to_string(),
                t.name,
                serde_json::to_string(&t.kind).unwrap_or_default().trim_matches('"'),
                t.status.as_str(),
                t.priority,
                serde_json::to_string(&t.items).unwrap_or_default(),
                t.output_dir.to_string_lossy(),
                serde_json::to_string(&t.options).unwrap_or_default(),
                t.progress,
                t.error.as_ref().and_then(|e| serde_json::to_string(e).ok()),
                t.created_at.to_rfc3339(),
                t.started_at.map(|d| d.to_rfc3339()),
                t.finished_at.map(|d| d.to_rfc3339()),
                serde_json::to_string(&t.output_dir_mode).unwrap_or_else(|_| "\"project_default\"".into()).trim_matches('"'),
                t.naming.as_ref().and_then(|n| serde_json::to_string(n).ok()),
            ],
        )
        .map_err(|e| AppError::io(e.to_string()))?;
        Ok(())
    }

    pub fn list_tasks(&self) -> Result<Vec<Task>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id,name,kind,status,priority,items_json,output_dir,options_json,progress,error_json,created_at,started_at,finished_at,output_dir_mode,naming_json FROM tasks ORDER BY created_at DESC")
            .map_err(|e| AppError::io(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i32>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, f32>(8)?,
                    r.get::<_, Option<String>>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<String>>(12)?,
                    r.get::<_, Option<String>>(13)?,
                    r.get::<_, Option<String>>(14)?,
                ))
            })
            .map_err(|e| AppError::io(e.to_string()))?;
        let mut out = vec![];
        for row in rows {
            let (id, name, kind_str, status, priority, items_json, output_dir, options_json, progress, error_json, created_at, started_at, finished_at, output_dir_mode, naming_json) =
                row.map_err(|e| AppError::io(e.to_string()))?;
            let kind = match kind_str.as_str() {
                "\"image\"" | "image" => FormatKind::Image,
                "\"document\"" | "document" => FormatKind::Document,
                "\"audio\"" | "audio" => FormatKind::Audio,
                "\"video\"" | "video" => FormatKind::Video,
                "\"ebook\"" | "ebook" => FormatKind::Ebook,
                "\"data\"" | "data" => FormatKind::Data,
                "\"archive\"" | "archive" => FormatKind::Archive,
                "\"font\"" | "font" => FormatKind::Font,
                _ => FormatKind::Custom,
            };
            out.push(Task {
                id: uuid::Uuid::parse_str(&id).unwrap_or_default(),
                name,
                kind,
                status: TaskStatus::from_str(&status),
                priority,
                items: serde_json::from_str(&items_json).unwrap_or_default(),
                output_dir: output_dir.into(),
                options: serde_json::from_str(&options_json).unwrap_or(ConvertOptions {
                    target_ext: String::new(),
                    quality: None,
                    preset: None,
                    extra: serde_json::Value::Null,
                }),
                output_dir_mode: output_dir_mode
                    .and_then(|s| serde_json::from_str::<scroll_format_core::OutDirMode>(&format!("\"{s}\"")).ok())
                    .unwrap_or_default(),
                naming: naming_json.and_then(|s| serde_json::from_str(&s).ok()),
                progress,
                error: error_json.and_then(|j| serde_json::from_str(&j).ok()),
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .unwrap_or_default(),
                started_at: started_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
                finished_at: finished_at.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
            });
        }
        Ok(out)
    }

    pub fn delete_task(&self, id: &uuid::Uuid) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM tasks WHERE id=?1", [id.to_string()])
            .map_err(|e| AppError::io(e.to_string()))?;
        Ok(())
    }

    pub fn recover(&self) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tasks SET status='failed', error_json='{\"code\":\"FMT-PROC-001\",\"message\":\"应用上次异常退出，任务中断\"}' WHERE status IN ('running','probing')",
            [],
        )
        .map_err(|e| AppError::io(e.to_string()))?;
        Ok(())
    }

    pub fn add_log(&self, level: &str, message: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO logs (ts,level,message) VALUES (?1,?2,?3)",
            rusqlite::params![chrono::Utc::now().to_rfc3339(), level, message],
        )
        .map_err(|e| AppError::io(e.to_string()))?;
        Ok(())
    }

    pub fn recent_logs(&self, limit: usize) -> Result<Vec<(String, String, String)>, AppError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT ts,level,message FROM logs ORDER BY id DESC LIMIT ?1")
            .map_err(|e| AppError::io(e.to_string()))?;
        let rows = stmt
            .query_map([limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| AppError::io(e.to_string()))?;
        let mut out = vec![];
        for r in rows {
            out.push(r.map_err(|e| AppError::io(e.to_string()))?);
        }
        Ok(out)
    }
}

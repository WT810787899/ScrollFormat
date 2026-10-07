use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use scroll_format_core::{AppError, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

/// 创建「静默」子进程命令：Windows 下加 CREATE_NO_WINDOW，杜绝黑窗闪烁
pub fn silent_command(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    cmd
}

pub struct RunSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub timeout: Option<Duration>,
    pub cancel: Option<CancellationToken>,
    /// 回调每一行 stdout
    pub on_stdout: Option<Box<dyn Fn(&str) + Send + Sync>>,
    pub on_stderr: Option<Box<dyn Fn(&str) + Send + Sync>>,
}

pub struct RunOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// 从 stderr 中挑出「最有信息量」的一行：ffmpeg 的有效报错常在最后几行，
/// 但末尾往往是统计/进度行，直接取 last 会丢出关键原因。
fn key_line(stderr: &str) -> String {
    const MARKS: [&str; 10] = [
        "error", "invalid", "unable", "cannot", "no such", "not found", "failed",
        "denied", "not supported", "unsupported",
    ];
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for l in lines.iter().rev() {
        let low = l.to_ascii_lowercase();
        if MARKS.iter().any(|m| low.contains(m)) {
            return (*l).to_string();
        }
    }
    lines.last().copied().unwrap_or("外部进程未输出错误信息").to_string()
}

pub struct ProcessRunner;

impl ProcessRunner {
    pub async fn run(spec: RunSpec) -> Result<RunOutput> {
        if !spec.program.exists() && which::which(&spec.program).is_err() {
            return Err(AppError::tool_missing(format!("外部工具不存在: {}", spec.program.display())));
        }
        let mut cmd = silent_command(&spec.program);
        cmd.args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| AppError::process(e.to_string()))?;

        let stdout_cb = spec.on_stdout;
        let stderr_cb = spec.on_stderr;

        let stdout_handle = {
            let pipe = child.stdout.take().unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(pipe).lines();
                let mut buf = String::new();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(cb) = &stdout_cb {
                        cb(&line);
                    }
                    buf.push_str(&line);
                    buf.push('\n');
                }
                buf
            })
        };
        let stderr_handle = {
            let pipe = child.stderr.take().unwrap();
            tokio::spawn(async move {
                let mut reader = BufReader::new(pipe).lines();
                let mut buf = String::new();
                while let Ok(Some(line)) = reader.next_line().await {
                    if let Some(cb) = &stderr_cb {
                        cb(&line);
                    }
                    buf.push_str(&line);
                    buf.push('\n');
                }
                buf
            })
        };

        let status = tokio::select! {
            s = child.wait() => s.map_err(|e| AppError::process(e.to_string()))?,
            _ = async { if let Some(t) = spec.timeout { tokio::time::sleep(t).await; } else { std::future::pending::<()>().await } } => {
                let _ = child.kill().await;
                return Err(AppError::new(scroll_format_core::ErrorCode::ProcessTimeout, "外部进程超时"));
            }
            _ = async { if let Some(c) = &spec.cancel { c.cancelled().await; } else { std::future::pending::<()>().await } } => {
                let _ = child.kill().await;
                return Err(AppError::cancelled());
            }
        };

        let stdout = stdout_handle.await.unwrap_or_default();
        let stderr = stderr_handle.await.unwrap_or_default();
        let code = status.code().unwrap_or(-1);
        if code != 0 {
            return Err(AppError::process(format!("退出码 {code}: {}", key_line(&stderr))));
        }
        Ok(RunOutput { code, stdout, stderr })
    }
}

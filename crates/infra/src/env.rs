use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::paths::find_tool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpuInfo {
    pub available: bool,
    pub name: Option<String>,
    pub usage: Option<f32>,
    pub mem_used_mb: Option<u64>,
    pub mem_total_mb: Option<u64>,
    pub source: String,
    pub suggestion: String,
}

/// 通过 nvidia-smi 查询 GPU（无则返回不可用）
pub async fn probe_gpu() -> GpuInfo {
    let smi = find_tool("nvidia-smi");
    let Some(smi) = smi else {
        return GpuInfo {
            available: false,
            source: "未检测到 nvidia-smi".into(),
            suggestion: "NVIDIA 显卡请安装驱动；其它显卡暂不支持占用读取".into(),
            ..Default::default()
        };
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_millis(2500),
        tokio::process::Command::new(&smi)
            .args([
                "--query-gpu=utilization.gpu,memory.used,memory.total,name",
                "--format=csv,noheader,nounits",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output(),
    )
    .await;
    let Ok(Ok(out)) = output else {
        return GpuInfo {
            available: false,
            source: "nvidia-smi 查询失败".into(),
            suggestion: "驱动异常或权限不足".into(),
            ..Default::default()
        };
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
    if line.is_empty() {
        return GpuInfo {
            available: false,
            source: "无 GPU 数据".into(),
            suggestion: "未检测到可用的 NVIDIA 设备".into(),
            ..Default::default()
        };
    }
    let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    GpuInfo {
        available: true,
        usage: cols.first().and_then(|s| s.parse().ok()),
        mem_used_mb: cols.get(1).and_then(|s| s.parse().ok()).map(|v: u64| v / 1024),
        mem_total_mb: cols.get(2).and_then(|s| s.parse().ok()).map(|v: u64| v / 1024),
        name: cols.get(3).map(|s| s.to_string()),
        source: "nvidia-smi".into(),
        suggestion: String::new(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvReport {
    pub os: String,
    pub arch: String,
    pub cpu_cores: usize,
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub cpu_usage: f32,
    pub tools: Vec<ToolStatus>,
    pub gpu: GpuInfo,
    pub temp_dir_writable: bool,
    pub warnings: Vec<String>,
}

const TOOLS: &[&str] = &["ffmpeg", "ffprobe", "pandoc", "libreoffice", "calibre", "7z", "imagemagick"];

pub async fn probe_env() -> EnvReport {
    // CPU 采样需要两次刷新才能得到有效占用率
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_usage();
    tokio::time::sleep(std::time::Duration::from_millis(220)).await;
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let total = sys.total_memory() / 1024 / 1024;
    let used = sys.used_memory() / 1024 / 1024;

    let settings = crate::settings::load();
    let mut tools = vec![];
    for name in TOOLS {
        let real = if *name == "libreoffice" { "soffice" } else { name };
        let manual = match *name {
            "ffmpeg" => Some(settings.ffmpeg_path.trim().to_string()),
            "ffprobe" => Some(settings.ffprobe_path.trim().to_string()),
            _ => None,
        }
        .filter(|s| !s.is_empty());
        let source = if manual.is_some() { "手动指定" } else { "自动检测" };
        let path = find_tool(real);
        let (available, version, path_str) = match path {
            Some(p) => {
                let version = probe_version(&p, name).await;
                (true, version, Some(p.to_string_lossy().to_string()))
            }
            None => (false, None, None),
        };
        let suggestion = if available {
            "可用".to_string()
        } else {
            match *name {
                "ffmpeg" | "ffprobe" => "建议安装 ffmpeg 或将 sidecar 随包分发".to_string(),
                "pandoc" => "文档转换需要 pandoc".to_string(),
                "libreoffice" => "办公文档转换需要 LibreOffice".to_string(),
                "calibre" => "电子书转换需要 calibre".to_string(),
                _ => "未安装".to_string(),
            }
        };
        tools.push(ToolStatus {
            name: name.to_string(),
            available,
            path: path_str,
            version,
            source: if available { source.to_string() } else { "未找到".to_string() },
            suggestion,
        });
    }

    let temp = std::env::temp_dir();
    let temp_ok = tempfile_probe(&temp);

    let mut warnings = vec![];
    if tools.iter().filter(|t| t.available).count() == 0 {
        warnings.push("未检测到任何外部工具，仅内置转换器可用".into());
    }
    if !temp_ok {
        warnings.push("临时目录不可写".into());
    }

    EnvReport {
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        arch: std::env::consts::ARCH.to_string(),
        cpu_cores: sys.cpus().len(),
        memory_total_mb: total,
        memory_used_mb: used,
        cpu_usage: sys.global_cpu_usage().clamp(0.0, 100.0),
        tools,
        gpu: probe_gpu().await,
        temp_dir_writable: temp_ok,
        warnings,
    }
}

fn tempfile_probe(dir: &std::path::Path) -> bool {
    let p = dir.join(format!("scrollfmt_probe_{}", std::process::id()));
    match std::fs::write(&p, b"ok") {
        Ok(_) => {
            let _ = std::fs::remove_file(&p);
            true
        }
        Err(_) => false,
    }
}

async fn probe_version(path: &PathBuf, tool: &str) -> Option<String> {
    let args: &[&str] = match tool {
        "ffmpeg" | "ffprobe" => &["-version"],
        "pandoc" => &["--version"],
        "libreoffice" => &["--version"],
        "calibre" => &["--version"],
        "7z" => &[],
        _ => &["-version"],
    };
    let out = tokio::process::Command::new(path)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().next().map(|s| s.chars().take(80).collect())
}

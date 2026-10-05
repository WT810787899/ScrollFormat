use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SCROLLFORMAT_DATA_DIR") {
        return PathBuf::from(p);
    }
    // 优先定位到项目根（含 Cargo.toml 且含 crates 目录的祖先目录）
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::env::temp_dir());
    for _ in 0..8 {
        if dir.join("Cargo.toml").exists() && dir.join("crates").is_dir() {
            return dir.join(".scrollformat");
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }
    // 回退：cwd
    std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(".scrollformat")
}

/// 探测外部工具：用户手动指定（settings.json） > sidecar > PATH > 常见安装目录
pub fn find_tool(name: &str) -> Option<PathBuf> {
    // 1) 用户手动指定路径优先
    let override_key = match name {
        "ffmpeg" => "ffmpeg_path",
        "ffprobe" => "ffprobe_path",
        _ => "",
    };
    if !override_key.is_empty() {
        let s = crate::settings::load();
        let custom = match override_key {
            "ffmpeg_path" => s.ffmpeg_path,
            _ => s.ffprobe_path,
        };
        let custom = custom.trim().to_string();
        if !custom.is_empty() {
            let p = PathBuf::from(&custom);
            if p.exists() {
                return Some(p);
            }
            // 也允许只填文件名，尝试在 PATH 中解析
            if let Ok(found) = which::which(&custom) {
                return Some(found);
            }
        }
    }
    // 2) sidecar next to exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for candidate in [dir.join(name), dir.join(format!("{name}.exe")), dir.join("sidecars").join(name), dir.join("sidecars").join(format!("{name}.exe"))] {
                if candidate.exists() { return Some(candidate); }
            }
        }
    }
    if let Ok(path) = which::which(name) {
        return Some(path);
    }
    let fixed = match std::env::consts::OS {
        "windows" => [
            format!(r"C:\Program Files\ffmpeg\bin\{name}.exe"),
            format!(r"C:\ffmpeg\bin\{name}.exe"),
        ],
        "macos" => [format!("/opt/homebrew/bin/{name}"), format!("/usr/local/bin/{name}")],
        _ => [format!("/usr/bin/{name}"), format!("/usr/local/bin/{name}")],
    };
    for f in fixed {
        let p = PathBuf::from(f);
        if p.exists() { return Some(p); }
    }
    None
}

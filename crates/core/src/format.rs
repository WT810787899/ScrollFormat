use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatKind {
    Image,
    Document,
    Audio,
    Video,
    Ebook,
    Data,
    Archive,
    Font,
    Custom,
}

impl FormatKind {
    pub fn label(&self) -> &'static str {
        match self {
            FormatKind::Image => "图片",
            FormatKind::Document => "文档",
            FormatKind::Audio => "音频",
            FormatKind::Video => "视频",
            FormatKind::Ebook => "电子书",
            FormatKind::Data => "数据",
            FormatKind::Archive => "压缩/归档",
            FormatKind::Font => "字体",
            FormatKind::Custom => "其他/自定义",
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            FormatKind::Image => &["jpg", "jpeg", "png", "webp", "avif", "gif", "bmp", "tiff", "tif", "svg", "ico", "heic"],
            FormatKind::Document => &["pdf", "docx", "doc", "odt", "rtf", "txt", "md", "html", "tex"],
            FormatKind::Audio => &["mp3", "wav", "flac", "aac", "ogg", "m4a", "opus", "wma"],
            FormatKind::Video => &["mp4", "mkv", "mov", "avi", "webm", "gif", "flv", "wmv"],
            FormatKind::Ebook => &["epub", "mobi", "azw3", "pdf", "txt", "html", "docx"],
            FormatKind::Data => &["json", "xml", "csv", "xlsx", "yaml", "yml", "toml", "sql", "parquet"],
            FormatKind::Archive => &["zip", "7z", "tar", "gz", "zst", "rar", "bz2", "xz"],
            FormatKind::Font => &["ttf", "otf", "woff", "woff2", "eot"],
            FormatKind::Custom => &[],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Format {
    pub ext: String,
    pub kind: FormatKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    pub path: String,
    pub ext: String,
    pub kind: FormatKind,
    pub size: u64,
    pub display_name: String,
    pub media: Option<MediaInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MediaInfo {
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub codec: Option<String>,
    pub bitrate: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}

pub fn detect_kind(path: &Path) -> Option<FormatKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    for kind in [
        FormatKind::Image, FormatKind::Audio, FormatKind::Video,
        FormatKind::Document, FormatKind::Data, FormatKind::Archive,
        FormatKind::Font, FormatKind::Ebook,
    ] {
        if kind.extensions().contains(&ext.as_str()) {
            // disambiguate overlapping exts by primary mapping
        }
    }
    // explicit priority mapping
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "avif" | "bmp" | "tiff" | "tif" | "svg" | "ico" | "heic" => Some(FormatKind::Image),
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "opus" | "wma" => Some(FormatKind::Audio),
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv" | "wmv" => Some(FormatKind::Video),
        "pdf" | "docx" | "doc" | "odt" | "rtf" | "tex" => Some(FormatKind::Document),
        "md" | "html" | "txt" => Some(FormatKind::Document),
        "epub" | "mobi" | "azw3" => Some(FormatKind::Ebook),
        "json" | "xml" | "csv" | "xlsx" | "yaml" | "yml" | "toml" | "sql" | "parquet" => Some(FormatKind::Data),
        "zip" | "7z" | "tar" | "gz" | "zst" | "rar" | "bz2" | "xz" => Some(FormatKind::Archive),
        "ttf" | "otf" | "woff" | "woff2" | "eot" => Some(FormatKind::Font),
        "gif" => Some(FormatKind::Image),
        _ => None,
    }
}

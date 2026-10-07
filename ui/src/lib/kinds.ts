export type Kind = "image" | "document" | "audio" | "video" | "ebook" | "data" | "archive" | "font" | "custom";

export const KIND_META: Record<Kind, { label: string; inputs: string[]; outputs: string[]; ready: boolean }> = {
  image: { label: "图片", inputs: ["jpg", "png", "webp", "gif", "bmp", "tiff", "ico"], outputs: ["jpg", "png", "webp", "gif", "bmp", "tiff", "ico"], ready: true },
  document: { label: "文档", inputs: ["pdf", "docx", "doc", "md", "html", "txt", "rtf", "odt"], outputs: ["pdf", "docx", "odt", "rtf", "md", "html", "txt", "png", "jpg"], ready: true },
  audio: { label: "音频", inputs: ["mp3", "wav", "flac", "aac", "ogg", "m4a", "opus", "wma"], outputs: ["mp3", "wav", "flac", "aac", "ogg", "m4a", "opus"], ready: true },
  video: { label: "视频", inputs: ["mp4", "mkv", "mov", "avi", "webm", "gif", "flv", "wmv"], outputs: ["mp4", "mkv", "mov", "webm", "avi", "gif", "wav", "mp3"], ready: true },
  ebook: { label: "电子书", inputs: ["epub", "mobi", "azw3", "pdf", "txt", "html", "docx"], outputs: ["epub", "mobi", "azw3", "pdf", "txt", "html"], ready: false },
  data: { label: "数据", inputs: ["json", "xml", "csv", "xlsx", "yaml", "toml", "sql"], outputs: ["json", "xml", "csv", "xlsx", "yaml", "toml", "sql"], ready: false },
  archive: { label: "压缩/归档", inputs: ["zip", "7z", "tar", "gz", "zst", "rar", "bz2", "xz"], outputs: ["zip", "7z", "tar", "gz", "zst"], ready: false },
  font: { label: "字体", inputs: ["ttf", "otf", "woff", "woff2", "eot"], outputs: ["ttf", "otf", "woff", "woff2"], ready: false },
  custom: { label: "其他/自定义", inputs: [], outputs: [], ready: false },
};

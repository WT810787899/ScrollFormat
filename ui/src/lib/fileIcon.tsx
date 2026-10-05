import {
  FileImage, FileAudio, FileVideo, FileText, FileArchive, Database, BookOpen, Type, File as FileIcon, Puzzle,
} from "lucide-react";

const MAP: Record<string, { icon: any; color: string; label: string }> = {
  // 图片
  jpg: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  jpeg: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  png: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  webp: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  gif: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  bmp: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  tiff: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  tif: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  ico: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  avif: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  heic: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  svg: { icon: FileImage, color: "text-emerald-400", label: "图片" },
  // 音频
  mp3: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  wav: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  flac: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  aac: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  ogg: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  m4a: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  opus: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  wma: { icon: FileAudio, color: "text-sky-400", label: "音频" },
  // 视频
  mp4: { icon: FileVideo, color: "text-violet-400", label: "视频" },
  mkv: { icon: FileVideo, color: "text-violet-400", label: "视频" },
  mov: { icon: FileVideo, color: "text-violet-400", label: "视频" },
  avi: { icon: FileVideo, color: "text-violet-400", label: "视频" },
  webm: { icon: FileVideo, color: "text-violet-400", label: "视频" },
  flv: { icon: FileVideo, color: "text-violet-400", label: "视频" },
  wmv: { icon: FileVideo, color: "text-violet-400", label: "视频" },
  // 文档
  pdf: { icon: FileText, color: "text-rose-400", label: "文档" },
  docx: { icon: FileText, color: "text-rose-400", label: "文档" },
  doc: { icon: FileText, color: "text-rose-400", label: "文档" },
  odt: { icon: FileText, color: "text-rose-400", label: "文档" },
  rtf: { icon: FileText, color: "text-rose-400", label: "文档" },
  txt: { icon: FileText, color: "text-rose-400", label: "文档" },
  md: { icon: FileText, color: "text-rose-400", label: "文档" },
  html: { icon: FileText, color: "text-rose-400", label: "文档" },
  tex: { icon: FileText, color: "text-rose-400", label: "文档" },
  // 电子书
  epub: { icon: BookOpen, color: "text-amber-400", label: "电子书" },
  mobi: { icon: BookOpen, color: "text-amber-400", label: "电子书" },
  azw3: { icon: BookOpen, color: "text-amber-400", label: "电子书" },
  // 数据
  json: { icon: Database, color: "text-cyan-400", label: "数据" },
  xml: { icon: Database, color: "text-cyan-400", label: "数据" },
  csv: { icon: Database, color: "text-cyan-400", label: "数据" },
  xlsx: { icon: Database, color: "text-cyan-400", label: "数据" },
  yaml: { icon: Database, color: "text-cyan-400", label: "数据" },
  yml: { icon: Database, color: "text-cyan-400", label: "数据" },
  toml: { icon: Database, color: "text-cyan-400", label: "数据" },
  sql: { icon: Database, color: "text-cyan-400", label: "数据" },
  parquet: { icon: Database, color: "text-cyan-400", label: "数据" },
  // 压缩
  zip: { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  "7z": { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  tar: { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  gz: { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  zst: { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  rar: { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  bz2: { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  xz: { icon: FileArchive, color: "text-yellow-400", label: "压缩包" },
  // 字体
  ttf: { icon: Type, color: "text-fuchsia-400", label: "字体" },
  otf: { icon: Type, color: "text-fuchsia-400", label: "字体" },
  woff: { icon: Type, color: "text-fuchsia-400", label: "字体" },
  woff2: { icon: Type, color: "text-fuchsia-400", label: "字体" },
  eot: { icon: Type, color: "text-fuchsia-400", label: "字体" },
};

export function fileKind(nameOrPath: string) {
  const ext = (nameOrPath.split(".").pop() || "").toLowerCase();
  return MAP[ext] || { icon: FileIcon, color: "t-3", label: "未知" };
}

export function FileKindIcon({ name, size = "w-4 h-4" }: { name: string; size?: string }) {
  const k = fileKind(name);
  const Icon = k.icon;
  return <Icon className={`${size} flex-none ${k.color}`} />;
}

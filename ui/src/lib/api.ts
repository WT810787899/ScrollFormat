import { invoke } from "@tauri-apps/api/core";

export interface TaskItem {
  input: string;
  /** 源文件大小（字节，0 = 未知/老数据） */
  size: number;
  output: string | null;
  /** 全部产物路径（PDF 多页导图等场景有多个）；老数据可能没有这个字段 */
  outputs?: string[];
  status: string;
  progress: number;
  error: { code: string; message: string } | null;
  log: string[];
}

export interface Task {
  id: string;
  name: string;
  kind: string;
  status: string;
  priority: number;
  items: TaskItem[];
  output_dir: string;
  options: { target_ext: string; quality?: number | null; preset?: string | null; extra: any };
  progress: number;
  error: { code: string; message: string } | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface EnvReport {
  os: string;
  arch: string;
  cpu_cores: number;
  memory_total_mb: number;
  memory_used_mb: number;
  cpu_usage: number;
  tools: { name: string; available: boolean; path: string | null; version: string | null; source: string; suggestion: string }[];
  gpu: { available: boolean; name: string | null; usage: number | null; mem_used_mb: number | null; mem_total_mb: number | null; source: string; suggestion: string };
  temp_dir_writable: boolean;
  warnings: string[];
}

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export interface Preset {
  id: string;
  name: string;
  builtin: boolean;
  kind: string | null;
  options: { target_ext: string; quality?: number | null; preset?: string | null; extra: any };
  out_dir_mode: "project_default" | "source_dir" | "custom";
  out_dir: string | null;
  naming: any | null;
  updated_at: string;
}

export interface Settings {
  theme: string;
  default_out_dir: string;
  max_parallel: number;
  log_level: string;
  default_conflict: string;
  command_timeout_secs: number;
  on_error: string;
  preview_enabled: boolean;
  preview_cache_limit: number;
  window_w: number;
  window_h: number;
  ui_scale: number;
  ffmpeg_path: string;
  ffprobe_path: string;
  shortcuts: Record<string, string>;
}

export interface Wallpaper {
  path: string;
  enabled: boolean;
  opacity: number;      // 0-100 壁纸不透明度
  saturation: number;   // 0-200 %
  brightness: number;   // 0-200 %
  blur: number;         // px
  maskColor: string;    // 遮罩颜色
  maskOpacity: number;  // 0-100 遮罩不透明度
}

export const api = {
  listTasks: () => invoke<Task[]>("list_tasks"),
  createTask: (newTask: any) => invoke<Task>("create_task", { new: newTask }),
  cancelTask: (id: string) => invoke<void>("cancel_task", { id }),
  retryTask: (id: string) => invoke<void>("retry_task", { id }),
  deleteTask: (id: string) => invoke<void>("delete_task", { id }),
  updateTask: (
    id: string,
    payload: { options: { target_ext: string; quality?: number | null; preset?: string | null; extra: any }; output_dir?: string; output_dir_mode?: string; naming?: any },
  ) => invoke<Task>("update_task", { id, ...payload }),
  clearTasks: (statuses: string[]) => invoke<number>("clear_tasks", { statuses }),
  taskAction: (action: string, ids: string[]) => invoke<void>("task_action", { action, ids }),
  openInExplorer: (path: string) => invoke<void>("open_in_explorer", { path }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openFile: (path: string) => invoke<void>("open_file", { path }),
  perfStats: () => invoke<{ cpu_usage: number; mem_used_mb: number; mem_total_mb: number; gpu_usage: number | null; gpu_encoder_usage: number | null; gpu_mem_used_mb: number | null; gpu_mem_total_mb: number | null; gpu_name: string | null; gpu_available: boolean; running: number; queued: number; progress: number }>("perf_stats"),
  pauseTask: (id: string) => invoke<void>("pause_task", { id }),
  resumeTask: (id: string) => invoke<void>("resume_task", { id }),
  probeEnv: () => invoke<EnvReport>("probe_env"),
  getLogs: (limit?: number) => invoke<[string, string, string][]>("get_logs", { limit }),
  getPreview: (path: string) => invoke<{ thumb_path: string | null; extra: any }>("get_preview", { path }),
  listPresets: (kind?: string) => invoke<Preset[]>("list_presets", { kind: kind ?? null }),
  savePreset: (preset: Preset) => invoke<void>("save_preset", { preset }),
  deletePreset: (id: string) => invoke<void>("delete_preset", { id }),
  renamePreset: (id: string, name: string) => invoke<void>("rename_preset", { id, name }),
  getActivePreset: () => invoke<string>("get_active_preset"),
  setActivePreset: (id: string) => invoke<void>("set_active_preset", { id }),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (patch: any) => invoke<Settings>("set_settings", { patch }),
  resetSettings: () => invoke<Settings>("reset_settings"),
  renderName: (rule: any, sample: string) => invoke<string>("render_name", { rule, sample }),
};

/** 字节数 → 人类可读大小（B / KB / MB / GB / TB） */
export function formatSize(bytes: number | null | undefined): string {
  const n = Number(bytes ?? 0);
  if (!n || n <= 0) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  // 字节取整，其余保留一位小数
  return `${i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

/** ISO 时间串 → 「MM-DD」（任务卡片用：分钟级精度没意义，只看是哪天加的） */
export function formatDateShort(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${mm}-${dd}`;
}

/** ISO 时间串 → 「YYYY-MM-DD HH:MM」（悬停提示用：补全年份与分钟） */
export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const mi = String(d.getMinutes()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}`;
}

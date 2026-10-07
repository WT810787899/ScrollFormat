import { useEffect, useState } from "react";
import { GlassSelect } from "../../components/GlassSelect";
import { useAppStore } from "../../store";
import { api, isTauri } from "../../lib/api";
import { ParamField } from "../../components/ParamField";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useToasts } from "../../components/Toast";
import { AboutPanel } from "../about/aboutInfo";
import { GlassCheckbox } from "../../components/GlassCheckbox";

const ACCENTS = [
  { id: "indigo", label: "靛蓝", rgb: "99 102 241" },
  { id: "violet", label: "紫罗兰", rgb: "139 92 246" },
  { id: "purple", label: "深紫", rgb: "168 85 247" },
  { id: "fuchsia", label: "洋红", rgb: "217 70 239" },
  { id: "sky", label: "天蓝", rgb: "14 165 233" },
  { id: "blue", label: "蔚蓝", rgb: "59 130 246" },
  { id: "cyan", label: "青色", rgb: "6 182 212" },
  { id: "teal", label: "蓝绿", rgb: "20 184 166" },
  { id: "emerald", label: "翠绿", rgb: "16 185 129" },
  { id: "green", label: "草绿", rgb: "34 197 94" },
  { id: "lime", label: "嫩绿", rgb: "132 204 22" },
  { id: "amber", label: "琥珀", rgb: "245 158 11" },
  { id: "yellow", label: "明黄", rgb: "234 179 8" },
  { id: "orange", label: "橙", rgb: "249 115 22" },
  { id: "brown", label: "赭石", rgb: "180 83 9" },
  { id: "rose", label: "玫红", rgb: "244 63 94" },
  { id: "red", label: "朱红", rgb: "239 68 68" },
  { id: "pink", label: "桃粉", rgb: "236 72 153" },
  { id: "slate", label: "石板", rgb: "100 116 139" },
  { id: "gray", label: "冷灰", rgb: "107 114 128" },
  { id: "stone", label: "暖灰", rgb: "120 113 108" },
];

const TABS = [
  { key: "general", label: "通用" },
  { key: "appearance", label: "外观" },
  { key: "convert", label: "转换" },
  { key: "preview", label: "预览" },
  { key: "about", label: "关于" },
];

function hexToRgbTriplet(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return "99 102 241";
  const n = parseInt(m[1], 16);
  return `${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255}`;
}

function ResetBtn({ title, onClick }: { title: string; onClick: () => void }) {
  return (
    <button
      title={title}
      onClick={onClick}
      className="w-6 h-7 flex-none rounded-lg bg-slate-500/15 hover:bg-accent-soft hover:text-accent text-xs transition"
    >
      ↺
    </button>
  );
}

function RangeInput({ value, min, max, onChange }: { value: number; min: number; max: number; onChange: (v: number) => void }) {
  return (
    <div className="flex items-center gap-2 w-full">
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1 min-w-0"
      />
      <span className="text-xs opacity-70 w-10 text-right tabular-nums">{value}</span>
    </div>
  );
}

export function SettingsTab() {
  const [tab, setTab] = useState("general");
  const [s, setS] = useState<any>(null);
  const { theme, setTheme, defaultOutDir, setDefaultOutDir, wallpaper, setWallpaper, glass, setGlass } = useAppStore();
  const { push } = useToasts();

  useEffect(() => {
    api.getSettings().then(setS).catch(() => {});
  }, []);

  /** 本地即时更新（输入框未失焦时不写盘） */
  const setLocal = (patch: any) => setS((prev: any) => (prev ? { ...prev, ...patch } : prev));

  const pickBinary = async (key: "ffmpeg_path" | "ffprobe_path") => {
    const r = await openDialog({ multiple: false, filters: [{ name: "可执行文件", extensions: ["exe", "bat", "cmd", ""] }] });
    if (typeof r === "string") save({ [key]: r });
  };

  const save = async (patch: any) => {
    try {
      setS(await api.setSettings(patch));
    } catch (e: any) {
      push(String(e), "error");
    }
  };

  const [customColor, setCustomColor] = useState(localStorage.getItem("sf-accent-rgb-hex") || "#6366f1");
  const [accentId, setAccentId] = useState<string>(localStorage.getItem("sf-accent") || "indigo");
  const currentAccent = accentId;

  const setAccent = (id: string, rgb: string) => {
    if (id === "custom") {
      document.body.style.setProperty("--accent", rgb);
      document.body.style.setProperty("--accent-soft", rgb);
      document.body.setAttribute("data-accent", "");
    } else {
      document.body.style.removeProperty("--accent");
      document.body.style.removeProperty("--accent-soft");
      document.body.setAttribute("data-accent", id);
    }
    localStorage.setItem("sf-accent", id);
    localStorage.setItem("sf-accent-rgb", rgb);
    if (id === "custom") localStorage.setItem("sf-accent-rgb-hex", customColor);
    setAccentId(id);
  };

  return (
    /* 面板尺寸：随窗口高度自适应（最高 600px），窗口不够高时也不会顶出屏幕 */
    <div className="w-[620px] max-w-[92vw] h-[min(70vh,600px)] min-h-[360px] overflow-hidden flex flex-col">
      <div className="flex gap-1 mb-3 flex-none">
        {TABS.map((t) => (
          <button key={t.key} onClick={() => setTab(t.key)} className={`px-3 py-1 rounded-xl text-sm ${tab === t.key ? "tab-active" : "hover:bg-slate-500/15 hover:bg-slate-500/25"}`}>
            {t.label}
          </button>
        ))}
      </div>
      <div className="flex-1 min-h-0 overflow-auto pr-1">
        {!s && <p className="opacity-60 text-sm">加载中…</p>}
        {s && tab === "general" && (
          <div className="flex flex-col gap-3">
            <ParamField label="主题">
              <GlassSelect value={theme} onChange={(v: string) => { setTheme(v as any); save({ theme: v }); }}>
                <option value="dark">深色</option>
                <option value="light">亮色</option>
              </GlassSelect>
            </ParamField>
            <ParamField label="默认输出目录">
              <button onClick={async () => { const r = await openDialog({ directory: true }); if (typeof r === "string") { setDefaultOutDir(r); save({ default_out_dir: r }); } }} className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 text-sm truncate max-w-[320px]">
                {defaultOutDir || "未设置"}
              </button>
            </ParamField>
          </div>
        )}
        {s && tab === "appearance" && (
          <div className="flex flex-col gap-3">
            <ParamField label="关键色" hint="影响按钮、进度条、选中高亮与背景光斑">
              {/* 注意两点：
                  1) 色块不能用 hover:scale —— 变换后的元素会计入「可滚动溢出区域」，
                     贴着边界的行一放大就冒出滚动条，滚动条一出现宽度又变化 → 换行重排 → 闪烁；
                  2) 选中态与悬浮态用完全相同的外圈样式（.sf-ring-on 与 .sf-hover-ring 同一条声明），
                     容器留了 py-1，外圈不会被 overflow 裁掉。 */}
              <div className="flex flex-wrap gap-1.5 max-h-[120px] overflow-y-auto overflow-x-hidden py-1 pr-1">
                {ACCENTS.map((a) => (
                  <button
                    key={a.id}
                    title={a.label}
                    onClick={() => setAccent(a.id, a.rgb)}
                    className={`w-6 h-6 rounded-full transition-[filter,box-shadow] duration-150 hover:brightness-110 ${
                      currentAccent === a.id ? "sf-ring-on" : "sf-hover-ring"
                    }`}
                    style={{ backgroundColor: `rgb(${a.rgb})` }}
                  />
                ))}
              </div>
            </ParamField>
            <ParamField label="自定义颜色" hint="任意取色，保存后随主题持久化">
              <div className="flex items-center gap-2">
                <input
                  type="color"
                  value={customColor}
                  onChange={(e) => setCustomColor(e.target.value)}
                  className="w-9 h-7 rounded-lg bg-transparent cursor-pointer"
                />
                <button
                  onClick={() => {
                    const rgb = hexToRgbTriplet(customColor);
                    setAccent("custom", rgb);
                  }}
                  className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 text-sm hover:bg-slate-500/25"
                >
                  应用
                </button>
                <span className="text-xs opacity-60 font-mono">{customColor}</span>
              </div>
            </ParamField>

            <div className="h-px bg-white/10 my-1" />

            <ParamField label="磨砂模糊" hint={`${glass.blur}px`} wide>
              <div className="flex items-center gap-2">
                <RangeInput value={glass.blur} min={0} max={48} onChange={(v) => setGlass({ blur: v })} />
                <ResetBtn title="恢复默认模糊 24px" onClick={() => setGlass({ blur: 24 })} />
              </div>
            </ParamField>
            <ParamField label="磨砂饱和度" hint={`${glass.sat}%`} wide>
              <div className="flex items-center gap-2">
                <RangeInput value={glass.sat} min={100} max={220} onChange={(v) => setGlass({ sat: v })} />
                <ResetBtn title="恢复默认饱和度 140%" onClick={() => setGlass({ sat: 140 })} />
              </div>
            </ParamField>
            <ParamField label="磨砂浓度" hint={`${Math.round(glass.alpha * 100)}%`} wide>
              <div className="flex items-center gap-2">
                <RangeInput value={Math.round(glass.alpha * 100)} min={0} max={95} onChange={(v) => setGlass({ alpha: v / 100 })} />
                <ResetBtn title="恢复默认浓度 8%" onClick={() => setGlass({ alpha: 0.08 })} />
              </div>
            </ParamField>
            <ParamField label="边框强度" hint={`${Math.round(glass.border * 100)}%`} wide>
              <div className="flex items-center gap-2">
                <RangeInput value={Math.round(glass.border * 100)} min={0} max={100} onChange={(v) => setGlass({ border: v / 100 })} />
                <ResetBtn title="恢复默认边框 16%" onClick={() => setGlass({ border: 0.16 })} />
              </div>
            </ParamField>
            <ParamField label="恢复默认磨砂">
              <button onClick={() => setGlass({ blur: 24, sat: 140, alpha: 0.08, border: 0.16 })} className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25">
                全部还原
              </button>
            </ParamField>

            <div className="h-px bg-slate-500/15 hover:bg-slate-500/25 my-1" />

            <ParamField label="壁纸">
              <div className="flex items-center gap-2">
                <button
                  onClick={async () => {
                    const r = await openDialog({ multiple: false, filters: [{ name: "图片", extensions: ["jpg", "jpeg", "png", "webp", "bmp"] }] });
                    if (typeof r === "string") setWallpaper({ path: r, enabled: true });
                  }}
                  className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25 whitespace-nowrap"
                >
                  选择图片
                </button>
                {wallpaper.path && (
                  <button onClick={() => setWallpaper({ path: "" })} className="text-[11px] t-3 hover:t-2 underline whitespace-nowrap">
                    清除
                  </button>
                )}
                <span className="text-[11px] t-3 truncate max-w-[150px]" title={wallpaper.path}>
                  {wallpaper.path ? wallpaper.path.split(/[\\/]/).pop() : "未设置"}
                </span>
                <span className="flex-1" />
                <span className="text-[11px] t-3 whitespace-nowrap">启用</span>
                <GlassCheckbox
                  checked={!!wallpaper.enabled}
                  onChange={(v) => setWallpaper({ enabled: v })}
                  title={wallpaper.enabled ? "关闭壁纸" : "开启壁纸"}
                />
              </div>
            </ParamField>
            <ParamField label="壁纸不透明度">
              <RangeInput value={wallpaper.opacity} min={0} max={100} onChange={(v) => setWallpaper({ opacity: v })} />
            </ParamField>
            <ParamField label="饱和度">
              <RangeInput value={wallpaper.saturation} min={0} max={200} onChange={(v) => setWallpaper({ saturation: v })} />
            </ParamField>
            <ParamField label="亮度">
              <RangeInput value={wallpaper.brightness} min={20} max={200} onChange={(v) => setWallpaper({ brightness: v })} />
            </ParamField>
            <ParamField label="模糊">
              <RangeInput value={wallpaper.blur} min={0} max={24} onChange={(v) => setWallpaper({ blur: v })} />
            </ParamField>
            <ParamField label="遮罩颜色">
              <div className="flex items-center gap-2">
                <input
                  type="color"
                  value={wallpaper.maskColor}
                  onChange={(e) => setWallpaper({ maskColor: e.target.value })}
                  className="w-9 h-7 rounded-lg bg-transparent cursor-pointer"
                />
                <span className="text-xs opacity-60 font-mono">{wallpaper.maskColor}</span>
                <div className="flex gap-1">
                  {["#0b1020", "#111827", "#000000", "#1e293b"].map((c) => (
                    <button key={c} onClick={() => setWallpaper({ maskColor: c })} className="w-5 h-5 rounded-md border border-white/20" style={{ backgroundColor: c }} />
                  ))}
                </div>
              </div>
            </ParamField>
            <ParamField label="遮罩不透明度">
              <RangeInput value={wallpaper.maskOpacity} min={0} max={100} onChange={(v) => setWallpaper({ maskOpacity: v })} />
            </ParamField>
          </div>
        )}
        {s && tab === "convert" && (
          <div className="flex flex-col gap-3">
            <ParamField label="ffmpeg 路径" hint="留空为自动检测（sidecar → PATH → 常见安装目录）">
              <div className="flex items-center gap-2">
                <input
                  value={s.ffmpeg_path || ""}
                  onChange={(e) => setLocal({ ffmpeg_path: e.target.value })}
                  onBlur={(e) => save({ ffmpeg_path: e.target.value })}
                  placeholder="自动检测"
                  className="flex-1 bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 text-sm min-w-0"
                />
                <button onClick={() => pickBinary("ffmpeg_path")} className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 text-sm hover:bg-slate-500/25 flex-none">浏览</button>
                <button onClick={() => save({ ffmpeg_path: "" })} className="text-xs opacity-70 hover:opacity-100 underline flex-none">自动</button>
              </div>
            </ParamField>
            <ParamField label="ffprobe 路径" hint="留空为自动检测">
              <div className="flex items-center gap-2">
                <input
                  value={s.ffprobe_path || ""}
                  onChange={(e) => setLocal({ ffprobe_path: e.target.value })}
                  onBlur={(e) => save({ ffprobe_path: e.target.value })}
                  placeholder="自动检测"
                  className="flex-1 bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 text-sm min-w-0"
                />
                <button onClick={() => pickBinary("ffprobe_path")} className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 text-sm hover:bg-slate-500/25 flex-none">浏览</button>
                <button onClick={() => save({ ffprobe_path: "" })} className="text-xs opacity-70 hover:opacity-100 underline flex-none">自动</button>
              </div>
            </ParamField>
            <ParamField label="检测结果" hint="保存后可在环境检测页查看来源">
              <span className="text-xs opacity-70">{s.ffmpeg_path ? "ffmpeg：手动指定" : "ffmpeg：自动检测"} · {s.ffprobe_path ? "ffprobe：手动指定" : "ffprobe：自动检测"}</span>
            </ParamField>
            <div className="h-px bg-slate-500/15 hover:bg-slate-500/25 my-1" />
            <ParamField label="最大并行任务" hint="运行时立即生效">
              <input type="number" min={1} max={8} value={s.max_parallel} onChange={(e) => save({ max_parallel: Number(e.target.value) })} className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 w-24" />
            </ParamField>
            <ParamField label="冲突策略">
              <GlassSelect value={s.default_conflict} onChange={(v: string) => save({ default_conflict: v })}>
                <option value="auto_rename">自动重命名</option>
                <option value="overwrite">覆盖</option>
                <option value="skip">跳过</option>
              </GlassSelect>
            </ParamField>
            <ParamField label="错误处理">
              <GlassSelect value={s.on_error} onChange={(v: string) => save({ on_error: v })}>
                <option value="continue">继续</option>
                <option value="abort">中止</option>
              </GlassSelect>
            </ParamField>
            <ParamField label="超时（秒，0 不限）">
              <input type="number" min={0} value={s.command_timeout_secs} onChange={(e) => save({ command_timeout_secs: Number(e.target.value) })} className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 w-24" />
            </ParamField>
            <ParamField label="日志级别">
              <GlassSelect value={s.log_level} onChange={(v: string) => save({ log_level: v })}>
                {["off", "error", "warn", "info", "debug", "trace"].map((l) => <option key={l} value={l}>{l}</option>)}
              </GlassSelect>
            </ParamField>
          </div>
        )}
        {s && tab === "preview" && (
          <div className="flex flex-col gap-3">
            <ParamField label="启用预览">
              <input type="checkbox" checked={s.preview_enabled} onChange={(e) => save({ preview_enabled: e.target.checked })} />
            </ParamField>
            <ParamField label="缓存上限">
              <input type="number" min={16} max={512} value={s.preview_cache_limit} onChange={(e) => save({ preview_cache_limit: Number(e.target.value) })} className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1.5 w-24" />
            </ParamField>
          </div>
        )}
        {s && tab === "about" && (
          <div className="flex flex-col gap-3">
            <AboutPanel />
            <button onClick={() => api.resetSettings().then(() => push("已恢复默认", "info"))} className="w-fit h-7 px-3 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25">
              恢复默认设置
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

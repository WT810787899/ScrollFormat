import { useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Image as ImageIcon, FileText, Music, Film, ListChecks, Settings as SettingsIcon, X, ScrollText, Play, Sun, Moon } from "lucide-react";
import { useAppStore } from "./store";
import { KindTab } from "./features/workspace/KindTab";
import { TasksTab } from "./features/tasks/TasksTab";
import { SettingsTab } from "./features/settings/SettingsTab";
import { StatusBar } from "./components/StatusBar";
import { WallpaperLayer } from "./components/Wallpaper";
import { Kind } from "./lib/kinds";

function WinBtn({ onClick, label, danger }: { onClick: () => void; label: string; danger?: boolean }) {
  return (
    <button onClick={onClick} className={`px-2.5 py-1 rounded-lg text-sm ${danger ? "hover:bg-red-500/60" : "hover:bg-slate-500/15 hover:bg-slate-500/25"}`} aria-label={label}>
      {label}
    </button>
  );
}

type Tab = "image" | "document" | "audio" | "video" | "tasks";

const TABS: { key: Tab; icon: any; label: string }[] = [
  { key: "image", icon: ImageIcon, label: "图片" },
  { key: "document", icon: FileText, label: "文档" },
  { key: "audio", icon: Music, label: "音频" },
  { key: "video", icon: Film, label: "视频" },
  { key: "tasks", icon: ListChecks, label: "任务列表" },
];

export default function App() {
  const { theme, toggleTheme } = useAppStore();
  const [tab, setTab] = useState<Tab>("image");
  const [showSettings, setShowSettings] = useState(false);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    document.body.classList.toggle("light", theme === "light");
    const accent = localStorage.getItem("sf-accent");
    const accentRgb = localStorage.getItem("sf-accent-rgb");
    if (accent) document.body.setAttribute("data-accent", accent);
    if (accent === "custom" && accentRgb) {
      document.body.style.setProperty("--accent", accentRgb);
      document.body.style.setProperty("--accent-soft", accentRgb);
    }
    // 磨砂玻璃参数
    const g = useAppStore.getState().glass;
    const rs = document.documentElement.style;
    rs.setProperty("--glass-blur", `${g.blur}px`);
    rs.setProperty("--glass-sat", `${g.sat}%`);
    rs.setProperty("--glass-alpha", String(g.alpha));
    rs.setProperty("--glass-border-alpha", String(g.border));
  }, [theme]);

  // 从项目本地 .scrollformat/ui_state.json 水合 UI 状态
  useEffect(() => {
    import("@tauri-apps/api/core").then(async ({ invoke }) => {
      const keys = ["sf-theme", "sf-outdir", "sf-outdir-by-kind", "sf-outdirmode-by-kind", "sf-preview", "sf-split", "sf-wallpaper", "sf-glass"];
      for (const k of keys) {
        const v = await invoke<string | null>("kv_get", { key: k }).catch(() => null);
        if (v != null) localStorage.setItem(k, v);
      }
      const theme = localStorage.getItem("sf-theme");
      if (theme === "light" || theme === "dark") useAppStore.getState().setTheme(theme);
      const outDir = localStorage.getItem("sf-outdir");
      if (outDir) useAppStore.getState().setDefaultOutDir(outDir);
      try {
        const byKind = localStorage.getItem("sf-outdir-by-kind");
        const modeByKind = localStorage.getItem("sf-outdirmode-by-kind");
        const wp = localStorage.getItem("sf-wallpaper");
        const glass = localStorage.getItem("sf-glass");
        let mergedWallpaper: any = null;
        try {
          const base = useAppStore.getState().wallpaper;
          mergedWallpaper = wp ? { ...base, ...JSON.parse(wp) } : null;
        } catch {
          mergedWallpaper = null;
        }
        useAppStore.setState({
          ...(byKind ? { outDirByKind: JSON.parse(byKind) } : {}),
          ...(modeByKind ? { outDirModeByKind: JSON.parse(modeByKind) } : {}),
          ...(mergedWallpaper ? { wallpaper: mergedWallpaper } : {}),
          ...(glass ? { glass: JSON.parse(glass) } : {}),
        });
        if (glass) useAppStore.getState().setGlass({});
      } catch {}
    });
  }, []);

  // 窗口尺寸记忆
  useEffect(() => {
    let dispose: (() => void) | undefined;
    import("./lib/api").then(({ api, isTauri }) => {
      if (!isTauri()) return;
      api.getSettings().then((s) => {
        import("@tauri-apps/api/window").then(({ getCurrentWindow, LogicalSize }) => {
          if (s.window_w && s.window_h) getCurrentWindow().setSize(new LogicalSize(s.window_w, s.window_h));
        });
      });
      let t: any = null;
      const onResize = () => {
        clearTimeout(t);
        t = setTimeout(() => {
          api.setSettings({ window_w: window.innerWidth, window_h: window.innerHeight }).catch(() => {});
        }, 500);
      };
      window.addEventListener("resize", onResize);
      dispose = () => {
        clearTimeout(t);
        window.removeEventListener("resize", onResize);
      };
    });
    return () => dispose?.();
  }, []);

  useEffect(() => {
    const onOpenSettings = () => setShowSettings(true);
    window.addEventListener("sf:open-settings", onOpenSettings);
    return () => window.removeEventListener("sf:open-settings", onOpenSettings);
  }, []);

  return (
    <div className="bg-scene h-full w-full relative overflow-hidden">
      <WallpaperLayer />
      <div className="relative z-10 h-full w-full flex flex-col">
      <header data-tauri-drag-region className="glass m-3 mb-0 rounded-2xl px-5 py-3 flex items-center gap-3 select-none">
        <div data-tauri-drag-region className="flex items-center gap-2">
          <ScrollText className="w-6 h-6 text-accent" />
          <h1 className="text-base font-bold tracking-wide whitespace-nowrap">格式卷轴<span className="text-xs opacity-60 font-normal ml-1">ScrollFormat</span></h1>
        </div>
        <nav data-tauri-drag-region className="flex items-center gap-1 ml-2">
          {TABS.map(({ key, icon: Icon, label }) => (
            <button
              key={key}
              onClick={() => setTab(key)}
              className={`flex items-center gap-1.5 px-4 py-1.5 rounded-xl text-sm transition ${tab === key ? "tab-active" : "hover:bg-slate-500/15 hover:bg-slate-500/25"}`}
            >
              <Icon className="w-4 h-4" />
              {label}
            </button>
          ))}
        </nav>
        {tab !== "tasks" && (
          <button
            onClick={() => useAppStore.getState().requestStart()}
            className="flex items-center gap-1.5 accent-grad hover:opacity-90 text-white rounded-xl px-5 py-1.5 text-sm"
          >
            <Play className="w-4 h-4" />开始转换
          </button>
        )}
        <div data-tauri-drag-region className="flex-1" />
        <button
          onClick={toggleTheme}
          aria-label={theme === "dark" ? "切换到亮色主题" : "切换到深色主题"}
          title={theme === "dark" ? "切换到亮色主题" : "切换到深色主题"}
          className="p-2 rounded-xl glow-btn"
        >
          {theme === "dark" ? <Sun className="w-5 h-5" /> : <Moon className="w-5 h-5" />}
        </button>
        <button onClick={() => setShowSettings(true)} aria-label="设置" className="p-2 rounded-xl hover:bg-slate-500/20">
          <SettingsIcon className="w-5 h-5" />
        </button>
        <WinBtn onClick={() => import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().minimize())} label="—" />
        <WinBtn onClick={() => import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().toggleMaximize())} label="▢" />
        <WinBtn onClick={() => import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().close())} label="✕" danger />
      </header>

      <div className="flex-1 p-3 pb-2 min-h-0">
        <main className="h-full min-w-0">
          {tab === "tasks" ? <TasksTab /> : <KindTab kind={tab as Kind} onGoTasks={() => setTab("tasks")} />}
        </main>
      </div>

      <StatusBar />

      <AnimatePresence>
        {showSettings && (
          <motion.div
            className="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setShowSettings(false)}
          >
            <motion.div
              className="glass glass-fixed rounded-3xl p-5 relative"
              initial={{ scale: 0.95, y: 12 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.95, y: 12 }}
              onClick={(e) => e.stopPropagation()}
            >
              <button onClick={() => setShowSettings(false)} className="absolute top-4 right-4 p-1.5 rounded-xl hover:bg-slate-500/15 hover:bg-slate-500/25" aria-label="关闭">
                <X className="w-4 h-4" />
              </button>
              <SettingsTab />
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
      </div>
    </div>
  );
}

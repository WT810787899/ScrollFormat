import { useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Image as ImageIcon, FileText, Music, Film, ListChecks, Settings as SettingsIcon, X, ScrollText, Sun, Moon } from "lucide-react";
import { useAppStore } from "./store";
import { KindTab } from "./features/workspace/KindTab";
import { TasksTab } from "./features/tasks/TasksTab";
import { SettingsTab } from "./features/settings/SettingsTab";
import { StatusBar } from "./components/StatusBar";
import { WallpaperLayer } from "./components/Wallpaper";
import { StartButton } from "./components/StartButton";
import { useToasts } from "./components/Toast";
import { api, isTauri } from "./lib/api";
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
  const { theme, toggleTheme, filesByKind } = useAppStore();
  const { push } = useToasts();
  const [tab, setTab] = useState<Tab>("image");
  const [showSettings, setShowSettings] = useState(false);
  /** 上次停留的工作台 TAB */
  const lastKindRef = useRef<Tab>("image");
  /** 待继续的任务数（已暂停 + 排队中；失败任务不计入） */
  const [pendingTasks, setPendingTasks] = useState(0);

  /** 文件区已添加但还没转化的文件数（当前工作台 TAB；在任务列表页看上一次的工作台） */
  const filesKind: Tab = tab === "tasks" ? lastKindRef.current : tab;
  const stagedCount = (filesByKind[filesKind] || []).length;

  const switchTab = (key: Tab) => {
    if (key !== "tasks") lastKindRef.current = key;
    setTab(key);
  };

  /** 「开始转换」：任务列表页 = 继续所有暂停/未开始的任务（跳过失败）；工作台 = 按参数建任务并开始 */
  const onStartClick = async () => {
    if (tab !== "tasks") {
      useAppStore.getState().requestStart();
      return;
    }
    // 任务列表页：文件区还堆着文件时，跳回工作台把它们一起建任务并开始
    if (stagedCount > 0) {
      setTab(filesKind);
      useAppStore.getState().requestStart();
      return;
    }
    try {
      const tasks = await api.listTasks();
      const paused = tasks.filter((t) => t.status === "paused");
      const queued = tasks.filter((t) => t.status === "queued");
      if (paused.length === 0) {
        push(
          queued.length > 0
            ? `${queued.length} 个任务已在队列中，无需再次开始`
            : "没有可开始的任务（失败任务需先改参数或点重试）",
          "info",
        );
        return;
      }
      await api.taskAction("resume", paused.map((t) => t.id));
      push(
        `已继续 ${paused.length} 个暂停任务${queued.length > 0 ? `，${queued.length} 个在队列中等待` : ""}${
          tasks.some((t) => t.status === "failed") ? "（失败任务已跳过）" : ""
        }`,
        "success",
      );
    } catch (e: any) {
      push(String(e), "error");
    }
  };

  // 顶栏按钮的待办角标 + 呼吸提示：订阅任务事件实时刷新
  useEffect(() => {
    const count = () =>
      api
        .listTasks()
        .then((ts) => setPendingTasks(ts.filter((t) => t.status === "paused" || t.status === "queued").length))
        .catch(() => {});
    count();
    if (!isTauri()) return;
    let un: (() => void)[] = [];
    import("@tauri-apps/api/event").then(async ({ listen }) => {
      for (const ev of ["task://created", "task://updated", "task://completed", "task://failed"]) {
        un.push(await listen(ev, count));
      }
    });
    return () => un.forEach((f) => f());
  }, []);

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
              onClick={() => switchTab(key)}
              className={`flex items-center gap-1.5 px-4 py-1.5 rounded-xl text-sm transition ${tab === key ? "tab-active" : "hover:bg-slate-500/15 hover:bg-slate-500/25"}`}
            >
              <Icon className="w-4 h-4" />
              {label}
            </button>
          ))}
        </nav>
        {/* 开始转换：常驻顶栏；工作台建任务并开始，任务列表页继续所有暂停/未开始的任务 */}
        {/* 开始转换：常驻顶栏；工作台建任务并开始，任务列表页继续所有暂停/未开始的任务 */}
        <StartButton
          queueMode={tab === "tasks"}
          pendingTasks={pendingTasks}
          stagedFiles={stagedCount}
          onStart={onStartClick}
          className="ml-10"
        />
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
              className="glass glass-fixed rounded-3xl p-5 relative max-h-[92vh] flex flex-col min-h-0"
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

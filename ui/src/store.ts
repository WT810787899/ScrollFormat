import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

function kv(key: string, value: string) {
  try {
    invoke("kv_set", { key, value }).catch(() => {});
  } catch {}
}

interface AppState {
  theme: "dark" | "light";
  setTheme: (t: "dark" | "light") => void;
  toggleTheme: () => void;
  defaultOutDir: string;
  setDefaultOutDir: (d: string) => void;
  filesByKind: Record<string, string[]>;
  addFiles: (kind: string, paths: string[]) => void;
  startTrigger: number;
  requestStart: () => void;
  removeFile: (kind: string, index: number) => void;
  clearFiles: (kind: string) => void;
  outDirByKind: Record<string, string>;
  outDirModeByKind: Record<string, string>;
  setOutDir: (kind: string, dir: string) => void;
  setOutDirMode: (kind: string, mode: string) => void;
  wallpaper: {
    path: string;
    enabled: boolean;
    opacity: number;
    saturation: number;
    brightness: number;
    blur: number;
    maskColor: string;
    maskOpacity: number;
  };
  setWallpaper: (patch: Partial<AppState["wallpaper"]>) => void;
  glass: { blur: number; sat: number; alpha: number; border: number };
  setGlass: (patch: Partial<AppState["glass"]>) => void;
}

export const useAppStore = create<AppState>((set) => ({
  theme: (localStorage.getItem("sf-theme") as "dark" | "light") || "dark",
  setTheme: (theme) => {
    localStorage.setItem("sf-theme", theme); kv("sf-theme", theme);
    set({ theme });
  },
  toggleTheme: () =>
    set((s) => {
      const theme = s.theme === "dark" ? "light" : "dark";
      localStorage.setItem("sf-theme", theme); kv("sf-theme", theme);
      return { theme };
    }),
  defaultOutDir: localStorage.getItem("sf-outdir") || "",
  setDefaultOutDir: (d) => {
    localStorage.setItem("sf-outdir", d); kv("sf-outdir", d);
    set({ defaultOutDir: d });
  },
  filesByKind: {},
  startTrigger: 0,
  requestStart: () => set((s) => ({ startTrigger: s.startTrigger + 1 })),
  addFiles: (kind, paths) =>
    set((s) => ({ filesByKind: { ...s.filesByKind, [kind]: [...(s.filesByKind[kind] || []), ...paths] } })),
  removeFile: (kind, index) =>
    set((s) => ({ filesByKind: { ...s.filesByKind, [kind]: (s.filesByKind[kind] || []).filter((_, i) => i !== index) } })),
  clearFiles: (kind) => set((s) => ({ filesByKind: { ...s.filesByKind, [kind]: [] } })),
  wallpaper: (() => {
    // 与默认值合并，保证旧版本存档（缺少 enabled 字段）也能正确工作
    try {
      const saved = JSON.parse(localStorage.getItem("sf-wallpaper") || "{}");
      return {
        path: "",
        enabled: true,
        opacity: 100,
        saturation: 100,
        brightness: 100,
        blur: 0,
        maskColor: "#0b1020",
        maskOpacity: 55,
        ...saved,
      };
    } catch {
      return { path: "", enabled: true, opacity: 100, saturation: 100, brightness: 100, blur: 0, maskColor: "#0b1020", maskOpacity: 55 };
    }
  })(),
  setWallpaper: (patch) =>
    set((s) => {
      const wallpaper = { ...s.wallpaper, ...patch };
      localStorage.setItem("sf-wallpaper", JSON.stringify(wallpaper));
      kv("sf-wallpaper", JSON.stringify(wallpaper));
      return { wallpaper };
    }),
  glass: JSON.parse(localStorage.getItem("sf-glass") || '{"blur":24,"sat":140,"alpha":0.08,"border":0.16}'),
  setGlass: (patch) =>
    set((s) => {
      const glass = { ...s.glass, ...patch };
      localStorage.setItem("sf-glass", JSON.stringify(glass));
      kv("sf-glass", JSON.stringify(glass));
      if (typeof document !== "undefined") {
        const r = document.documentElement.style;
        r.setProperty("--glass-blur", `${glass.blur}px`);
        r.setProperty("--glass-sat", `${glass.sat}%`);
        r.setProperty("--glass-alpha", String(glass.alpha));
        r.setProperty("--glass-border-alpha", String(glass.border));
      }
      return { glass };
    }),
  outDirByKind: JSON.parse(localStorage.getItem("sf-outdir-by-kind") || "{}"),
  outDirModeByKind: JSON.parse(localStorage.getItem("sf-outdirmode-by-kind") || "{}"),
  setOutDir: (kind, dir) =>
    set((s) => {
      const outDirByKind = { ...s.outDirByKind, [kind]: dir };
      localStorage.setItem("sf-outdir-by-kind", JSON.stringify(outDirByKind)); kv("sf-outdir-by-kind", JSON.stringify(outDirByKind));
      return { outDirByKind };
    }),
  setOutDirMode: (kind, mode) =>
    set((s) => {
      const outDirModeByKind = { ...s.outDirModeByKind, [kind]: mode };
      localStorage.setItem("sf-outdirmode-by-kind", JSON.stringify(outDirModeByKind)); kv("sf-outdirmode-by-kind", JSON.stringify(outDirModeByKind));
      return { outDirModeByKind };
    }),
}));

import { useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauri, Wallpaper } from "../lib/api";
import { useAppStore } from "../store";

export const DEFAULT_WALLPAPER: Wallpaper = {
  path: "",
  enabled: true,
  opacity: 100,
  saturation: 100,
  brightness: 100,
  blur: 0,
  maskColor: "#0b1020",
  maskOpacity: 55,
};

/** 壁纸层：位于场景渐变之下，随设置实时变化 */
export function WallpaperLayer() {
  const wp = useAppStore((s) => s.wallpaper);
  const path = wp?.enabled ? wp?.path || "" : "";

  useEffect(() => {
    if (!path || !isTauri()) return;
    // 预加载，失败则清空避免显示破图
    const img = new Image();
    img.onerror = () => useAppStore.getState().setWallpaper({ path: "" });
    img.src = convertFileSrc(path);
  }, [path]);

  if (!path) return null;
  return (
    <div className="absolute inset-0 z-0 pointer-events-none overflow-hidden">
      <img
        src={convertFileSrc(path)}
        alt=""
        className="w-full h-full object-cover transition-[filter,opacity] duration-200"
        style={{
          filter: `saturate(${wp.saturation}%) brightness(${wp.brightness}%) blur(${wp.blur}px)`,
          opacity: wp.opacity / 100,
        }}
      />
      <div className="absolute inset-0" style={{ backgroundColor: wp.maskColor, opacity: wp.maskOpacity / 100 }} />
    </div>
  );
}

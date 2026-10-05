import { useEffect } from "react";
import { isTauri } from "./api";

let handler: ((paths: string[]) => void) | null = null;
let registered = false;

/** 全局单例拖拽监听（避免 StrictMode/重复挂载导致重复注册） */
export function useDragDrop(onDrop: (paths: string[]) => void) {
  handler = onDrop;
  useEffect(() => {
    if (!isTauri() || registered) return;
    registered = true;
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      getCurrentWindow().onDragDropEvent((event) => {
        if (event.payload.type === "drop") handler?.((event.payload as any).paths);
      });
    });
  }, []);
}

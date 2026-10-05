import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { ContextMenu, MenuItem } from "./ContextMenu";

type BlankHandler = (x: number, y: number) => MenuItem[];

interface BlankEvent {
  clientX: number;
  clientY: number;
  target: EventTarget | null;
  preventDefault?: () => void;
}

interface Ctx {
  /** 在任意空白区域弹出菜单（由 TasksTab 注册处理逻辑） */
  openBlankMenu: (e: BlankEvent) => void;
  /** 注册空白区域菜单的构建函数 */
  registerBlankHandler: (fn: BlankHandler) => () => void;
}

const ContextMenuContext = createContext<Ctx | null>(null);

export function useGlobalContextMenu() {
  return useContext(ContextMenuContext);
}

export function ContextMenuProvider({ children }: { children: React.ReactNode }) {
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const handlers = useRef<BlankHandler[]>([]);

  const current = () => handlers.current[handlers.current.length - 1] || null;

  const registerBlankHandler = useCallback((fn: BlankHandler) => {
    handlers.current.push(fn);
    return () => {
      handlers.current = handlers.current.filter((h) => h !== fn);
    };
  }, []);

  /** 挂在 window 上：任意空白区域右键都能触发（排除可交互元素与卡片） */
  useEffect(() => {
    const onContextMenu = (e: MouseEvent) => {
      const fn = current();
      if (!fn) return;
      const el = e.target as HTMLElement;
      if (el.closest("[data-card]")) return;
      if (el.closest("button, input, select, textarea, a, [role=button], [data-no-menu]")) return;
      e.preventDefault();
      const items = fn(e.clientX, e.clientY);
      if (items && items.length) setMenu({ x: e.clientX, y: e.clientY, items });
    };
    window.addEventListener("contextmenu", onContextMenu);
    return () => window.removeEventListener("contextmenu", onContextMenu);
  }, []);

  const value = useMemo<Ctx>(
    () => ({
      openBlankMenu: (e) => {
        const fn = current();
        if (!fn) return;
        e.preventDefault?.();
        const items = fn(e.clientX, e.clientY);
        if (items && items.length) setMenu({ x: e.clientX, y: e.clientY, items });
      },
      registerBlankHandler,
    }),
    [registerBlankHandler],
  );

  return (
    <ContextMenuContext.Provider value={value}>
      {children}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </ContextMenuContext.Provider>
  );
}

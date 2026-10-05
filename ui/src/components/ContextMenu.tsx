import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

export interface MenuItem {
  label: string;
  danger?: boolean;
  disabled?: boolean;
  divider?: boolean;
  onClick?: () => void;
}

const W = 190;

export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    // 顶部始终对齐鼠标 y，水平方向贴边防溢出
    setPos({
      left: Math.max(8, Math.min(x, window.innerWidth - r.width - 8)),
      top: Math.max(8, Math.min(y, window.innerHeight - r.height - 8)),
    });
    // 入场动画结束后再按最终尺寸校正一次
    const t = setTimeout(() => {
      const r2 = ref.current?.getBoundingClientRect();
      if (!r2) return;
      setPos({
        left: Math.max(8, Math.min(x, window.innerWidth - r2.width - 8)),
        top: Math.max(8, Math.min(y, window.innerHeight - r2.height - 8)),
      });
    }, 140);
    return () => clearTimeout(t);
  }, [x, y, items.length]);

  useEffect(() => {
    // 点击菜单外部才关闭（不在菜单内时立即关闭），保证菜单项能正常触发
    const onPointerDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const close = () => onClose();
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", close);
    window.addEventListener("wheel", close, { passive: true });
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", close);
      window.removeEventListener("wheel", close);
      window.removeEventListener("blur", close);
    };
  }, [onClose]);

  return createPortal(
    <AnimatePresence>
      <motion.div
        ref={ref}
        initial={{ opacity: 0, scale: 0.96 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.96 }}
        transition={{ duration: 0.12, ease: "easeOut" }}
        style={{ left: pos.left, top: pos.top, width: W, transformOrigin: "top left" }}
        className="fixed z-[9999] glass rounded-xl p-1 shadow-2xl"
        onContextMenu={(e) => e.preventDefault()}
      >
        {items.map((it, i) =>
          it.divider ? (
            <div key={i} className="my-1 h-px bg-slate-500/15 hover:bg-slate-500/25" />
          ) : (
            <button
              key={i}
              disabled={it.disabled}
              onMouseDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                if (it.disabled) return;
                it.onClick?.();
                onClose();
              }}
              className={`w-full truncate text-left px-3 py-1.5 rounded-lg text-sm transition disabled:opacity-35 ${
                it.danger ? "text-rose-600 dark:text-rose-300 hover:bg-rose-500/20" : "hover:bg-accent-soft hover:text-accent"
              }`}
            >
              {it.label}
            </button>
          ),
        )}
      </motion.div>
    </AnimatePresence>,
    document.body,
  );
}

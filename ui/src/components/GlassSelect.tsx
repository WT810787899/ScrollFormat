import React, { isValidElement, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";

export interface SelectOption {
  value: string;
  label: string;
}

function textOf(node: any): string {
  if (node == null || node === false) return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (isValidElement(node)) return textOf((node.props as any)?.children);
  return "";
}

/** 支持 <GlassSelect value onChange>{items.map(i => <option value={i}>{i}</option>)}</GlassSelect> */
export function GlassSelect({
  value,
  onChange,
  options,
  children,
  className = "",
}: {
  value: string;
  onChange: (v: string) => void;
  options?: SelectOption[];
  children?: React.ReactNode;
  className?: string;
}) {
  const resolved = useMemo<SelectOption[]>(() => {
    if (options) return options;
    const out: SelectOption[] = [];
    const walk = (node: any) => {
      if (node == null || node === false) return;
      if (Array.isArray(node)) return node.forEach(walk);
      if (isValidElement(node) && (node.type as any) === "option") {
        const props: any = node.props;
        const label = textOf(props.children);
        out.push({ value: String(props.value ?? label), label });
      }
    };
    walk(children);
    return out;
  }, [options, children]);
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [rect, setRect] = useState<{ left: number; top: number; width: number } | null>(null);

  const selected = resolved.find((o) => o.value === value);

  const place = useCallback(() => {
    const el = btn.current;
    const pop = list.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    // 优先使用弹层实际高度，避免估算误差导致向上弹出时错位
    const realH = pop ? pop.getBoundingClientRect().height : 0;
    const h = realH > 0 ? realH : Math.min(resolved.length * 30 + 10, 260);
    const below = window.innerHeight - r.bottom;
    const drop = below < h + 12 && r.top > h + 12;
    const width = Math.max(r.width, 148);
    const left = Math.max(8, Math.min(r.left, window.innerWidth - width - 8));
    const top = drop ? Math.max(8, r.top - h - 6) : Math.min(r.bottom + 6, window.innerHeight - h - 8);
    setRect((prev) => (prev && prev.left === left && prev.top === top && prev.width === width ? prev : { left, top, width }));
  }, [resolved.length]);

  // 打开时：先渲染弹层（隐藏），再按实际尺寸定位
  useLayoutEffect(() => {
    if (!open) {
      setRect(null);
      return;
    }
    place();
    requestAnimationFrame(place);
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    let raf2 = 0;
    requestAnimationFrame(() => {
      place();
      raf2 = requestAnimationFrame(place);
    });
    const ro = new ResizeObserver(place);
    if (btn.current) ro.observe(btn.current);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      cancelAnimationFrame(raf2);
      ro.disconnect();
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    const close = (e: Event) => {
      const t = e.target as Node;
      if (list.current?.contains(t) || btn.current?.contains(t)) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", close);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <>
      <button
        ref={btn}
        type="button"
        onClick={() => setOpen((v) => !v)}
        className={`w-full min-w-[132px] h-7 flex items-center gap-2 rounded-lg px-2.5 text-xs text-left transition bg-slate-500/15 hover:bg-slate-500/25 border border-white/10 focus:outline-none focus:border-accent/60 ${className}`}
      >
        <span className={`flex-1 truncate ${selected ? "t-1" : "t-3"}`}>{selected?.label ?? "—"}</span>
        <ChevronDown className={`w-3.5 h-3.5 flex-none t-3 transition-transform ${open ? "rotate-180" : ""}`} />
      </button>
      {open &&
        createPortal(
          <div
            ref={list}
            style={{
              left: rect?.left ?? 0,
              top: rect?.top ?? 0,
              minWidth: rect?.width ?? 148,
              visibility: rect ? "visible" : "hidden",
            }}
            className="fixed z-[9999] glass rounded-xl p-1 shadow-2xl max-h-[260px] overflow-auto"
          >
            {resolved.map((o) => (
              <button
                key={o.value}
                type="button"
                onClick={() => {
                  onChange(o.value);
                  setOpen(false);
                }}
                className={`w-full flex items-center gap-2 px-2.5 py-1 rounded-lg text-xs text-left transition ${
                  o.value === value ? "bg-accent-soft text-accent" : "t-2 hover:bg-accent-soft hover:text-accent"
                }`}
              >
                <span className="flex-1 truncate">{o.label}</span>
                {o.value === value && <Check className="w-3.5 h-3.5 flex-none" />}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}

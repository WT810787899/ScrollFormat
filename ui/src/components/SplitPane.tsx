import React, { useRef, useState } from "react";

export function SplitPane({
  initialLeft = 50,
  className = "",
  children,
}: {
  initialLeft?: number;
  className?: string;
  children: [React.ReactNode, React.ReactNode];
}) {
  const [leftPct, setLeftPct] = useState(() => Number(localStorage.getItem("sf-split") || initialLeft));
  const dragging = useRef(false);
  const container = useRef<HTMLDivElement>(null);

  const onMouseDown = () => {
    dragging.current = true;
    const onMove = (e: MouseEvent) => {
      if (!dragging.current || !container.current) return;
      const rect = container.current.getBoundingClientRect();
      const pct = Math.min(80, Math.max(20, ((e.clientX - rect.left) / rect.width) * 100));
      setLeftPct(Math.round(pct));
      localStorage.setItem("sf-split", String(Math.round(pct)));
      import("@tauri-apps/api/core").then(({ invoke }) => invoke("kv_set", { key: "sf-split", value: String(Math.round(pct)) }).catch(() => {}));
    };
    const onUp = () => {
      dragging.current = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  return (
    <div ref={container} className={`flex min-h-0 ${className}`}>
      <div style={{ width: `${leftPct}%` }} className="min-w-0 h-full">{children[0]}</div>
      <div onMouseDown={onMouseDown} className="w-1.5 shrink-0 cursor-col-resize bg-white/5 hover:bg-accent-soft rounded-full mx-0.5 transition" />
      <div className="flex-1 min-w-0 h-full">{children[1]}</div>
    </div>
  );
}

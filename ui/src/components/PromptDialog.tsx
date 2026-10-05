import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";

// ═══════════════════════════════════════════════════════
// PromptDialog — 替换原生 prompt/confirm 的现代毛玻璃弹窗
//   用法：const name = await promptDialog({ title: "预设名称" });
//   返回 Promise<string | null>（取消返回 null）
//   全局只需挂载一次 <PromptHost />（main.tsx 已挂载）
// ═══════════════════════════════════════════════════════

export interface PromptOptions {
  title: string;
  label?: string;
  placeholder?: string;
  defaultValue?: string;
  confirmText?: string;
  cancelText?: string;
  hint?: string;
  validate?: (v: string) => string | null;
}

interface PromptState extends PromptOptions {
  resolve: (v: string | null) => void;
}

let setter: ((s: PromptState | null) => void) | null = null;

export function promptDialog(opts: PromptOptions): Promise<string | null> {
  return new Promise((resolve) => {
    if (!setter) return resolve(null);
    setter({ ...opts, resolve });
  });
}

export function PromptHost() {
  const [state, setState] = useState<PromptState | null>(null);
  const [value, setValue] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setter = setState;
    return () => {
      setter = null;
    };
  }, []);

  useEffect(() => {
    if (!state) return;
    setValue(state.defaultValue ?? "");
    setErr(null);
    // 等入场动画开始后再聚焦，避免被动画遮挡
    const t = setTimeout(() => inputRef.current?.focus(), 40);
    return () => clearTimeout(t);
  }, [state]);

  const close = (result: string | null) => {
    state?.resolve(result);
    setState(null);
  };

  const submit = () => {
    if (!state) return;
    const v = value.trim();
    const msg = state.validate ? state.validate(v) : v ? null : "内容不能为空";
    if (msg) return setErr(msg);
    close(v);
  };

  return createPortal(
    <AnimatePresence>
      {state && (
        <motion.div
          className="fixed inset-0 z-[300] flex items-center justify-center bg-black/45 backdrop-blur-[2px]"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.14 }}
          onClick={() => close(null)}
        >
          <motion.div
            className="glass glass-fixed rounded-3xl w-[420px] max-w-[90vw] p-5 relative"
            initial={{ scale: 0.94, y: 14, opacity: 0 }}
            animate={{ scale: 1, y: 0, opacity: 1 }}
            exit={{ scale: 0.96, y: 8, opacity: 0 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-sm font-semibold pr-8">{state.title}</h3>
            {state.label && <p className="text-[11px] t-3 mt-1">{state.label}</p>}

            <input
              ref={inputRef}
              value={value}
              onChange={(e) => {
                setValue(e.target.value);
                if (err) setErr(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") submit();
                if (e.key === "Escape") close(null);
              }}
              placeholder={state.placeholder}
              className={`mt-3 h-8 w-full rounded-lg px-3 text-sm bg-slate-500/15 hover:bg-slate-500/25 border text-inherit outline-none focus:border-accent/70 transition ${
                err ? "border-rose-400/70" : "border-white/10"
              }`}
            />
            <div className="h-4 mt-1">
              {err && <p className="text-[11px] text-rose-500 dark:text-rose-300">{err}</p>}
              {!err && state.hint && <p className="text-[11px] t-3">{state.hint}</p>}
            </div>

            <div className="flex justify-end gap-2 mt-2">
              <button
                onClick={() => close(null)}
                className="h-7 px-3 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25"
              >
                {state.cancelText || "取消"}
              </button>
              <button
                onClick={submit}
                className="h-7 px-4 text-xs rounded-lg accent-grad text-white hover:opacity-90"
              >
                {state.confirmText || "确定"}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
// ═══════════════════════════════════════════════════════════
// TaskEditDialog —— 任务卡片「修改参数」弹窗
//   参数区模板与工作台 TAB 完全一致（共用 ParamPanel + paramModel）
//   保存：仅保存参数 / 保存并重新执行（失败、已取消、已完成的任务可直接重跑）
// ═══════════════════════════════════════════════════════════
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { Loader2, RotateCcw } from "lucide-react";
import { api, Preset, Task } from "../../lib/api";
import { Kind, KIND_META } from "../../lib/kinds";
import { useToasts } from "../../components/Toast";
import { validateForm } from "../../lib/validation";
import { ParamPanel } from "../workspace/ParamPanel";
import { ParamBag, buildTemplate, extraFromParams, paramsFromExtra, paramsFromPreset } from "../workspace/paramModel";

const STATUS_LABEL: Record<string, string> = {
  queued: "排队中",
  probing: "探测中",
  running: "进行中",
  paused: "已暂停",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

export function TaskEditDialog({ task, onClose, onSaved }: { task: Task; onClose: () => void; onSaved: () => void }) {
  const { push } = useToasts();
  const kind = (KIND_META[task.kind as Kind] ? (task.kind as Kind) : "custom") as Kind;
  const meta = KIND_META[kind];
  const [params, setParams] = useState<ParamBag>(() => paramsFromExtra(kind, task.options?.extra));
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [presets, setPresets] = useState<Preset[]>([]);
  const [presetId, setPresetId] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const locked = task.status === "running" || task.status === "probing";

  useEffect(() => {
    api.listPresets(kind).then(setPresets).catch(() => {});
  }, [kind]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, busy]);

  const patch = (p: ParamBag) => setParams((prev) => ({ ...prev, ...p }));

  const applyPreset = (id: string) => {
    setPresetId(id);
    const p = presets.find((x) => x.id === id);
    if (!p) return patch({ presetId: "" });
    patch(paramsFromPreset(kind, p, params));
    push(`已应用预设「${p.name}」`, "info");
  };

  const save = async (rerun: boolean) => {
    const outDir = params.outDirMode === "custom" ? params.outDir : params.outDirMode === "project_default" ? params.outDir : "";
    const v = validateForm({
      target: params.target,
      outDir: params.outDirMode === "source_dir" ? "ok" : outDir,
      quality: params.quality,
      crf: params.crf,
      namingTemplate: buildTemplate(params.naming),
    });
    setErrors(v.errors);
    if (!v.ok) return;
    setBusy(true);
    try {
      await api.updateTask(task.id, {
        options: {
          target_ext: params.target,
          quality: params.quality,
          preset: presetId || null,
          extra: extraFromParams(kind, params),
        },
        output_dir: outDir,
        output_dir_mode: params.outDirMode,
        naming: { ...params.naming, template: buildTemplate(params.naming) },
      });
      if (rerun) await api.retryTask(task.id);
      push(rerun ? "参数已保存并重新加入队列" : "参数已保存", "success");
      onSaved();
      onClose();
    } catch (e: any) {
      push(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const fileName = task.items[0]?.input.split(/[\\/]/).pop() || task.name;

  return createPortal(
    <AnimatePresence>
      <motion.div
        className="fixed inset-0 z-[300] flex items-center justify-center bg-black/55 backdrop-blur-[3px]"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.14 }}
        onClick={() => !busy && onClose()}
      >
        <motion.div
          className="glass glass-fixed rounded-3xl w-[720px] max-w-[94vw] max-h-[88vh] p-4 flex flex-col"
          initial={{ scale: 0.95, y: 14, opacity: 0 }}
          animate={{ scale: 1, y: 0, opacity: 1 }}
          exit={{ scale: 0.97, y: 8, opacity: 0 }}
          transition={{ duration: 0.16, ease: "easeOut" }}
          onClick={(e) => e.stopPropagation()}
        >
          {/* 标题：文件名 + 类型 + 状态 */}
          <div className="flex items-center gap-2 flex-none">
            <h3 className="text-sm font-semibold truncate" title={task.items[0]?.input}>
              修改参数
            </h3>
            <span className="text-[11px] t-3 truncate flex-1" title={fileName}>
              {fileName} · {meta.label}
            </span>
            <span className="text-xs px-2 py-0.5 rounded-full bg-slate-500/15 whitespace-nowrap">{STATUS_LABEL[task.status] || task.status}</span>
          </div>
          {locked && <p className="text-[11px] text-amber-600 dark:text-amber-300 mt-1">任务进行中，无法修改参数；请先暂停或等待完成。</p>}

          {/* 参数区：与工作台 TAB 同一模板 */}
          <div className="mt-3 flex-1 min-h-0 overflow-y-auto overflow-x-hidden pr-1">
            <ParamPanel
              kind={kind}
              value={params}
              onChange={patch}
              errors={errors}
              presets={presets}
              presetId={presetId}
              onPreset={applyPreset}
            />
          </div>

          <div className="flex justify-end gap-2 mt-3 flex-none">
            <button onClick={onClose} disabled={busy} className="h-7 px-3 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25 disabled:opacity-50">
              取消
            </button>
            <button
              onClick={() => save(false)}
              disabled={busy || locked}
              className="h-7 px-3 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25 disabled:opacity-40 disabled:hover:bg-slate-500/15"
            >
              仅保存参数
            </button>
            <button
              onClick={() => save(true)}
              disabled={busy || locked}
              className="h-7 px-4 text-xs rounded-lg accent-grad text-white hover:opacity-90 disabled:opacity-40 flex items-center gap-1.5"
            >
              {busy ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <RotateCcw className="w-3.5 h-3.5" />}
              保存并重新执行
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>,
    document.body,
  );
}

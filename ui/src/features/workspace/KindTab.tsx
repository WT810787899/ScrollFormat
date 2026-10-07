import React, { useEffect, useState } from "react";
import { Upload, X } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Kind, KIND_META } from "../../lib/kinds";
import { api, isTauri, Preset } from "../../lib/api";
import { useAppStore } from "../../store";
import { useToasts } from "../../components/Toast";
import { Card } from "../../components/Card";
import { validateForm } from "../../lib/validation";
import { PreviewPanel } from "./PreviewPanel";
import { SplitPane } from "../../components/SplitPane";
import { useDragDrop } from "../../lib/useDragDrop";
import { FileKindIcon } from "../../lib/fileIcon";
import { useGlobalContextMenu } from "../../components/ContextMenuProvider";
import { promptDialog } from "../../components/PromptDialog";
import { ParamPanel } from "./ParamPanel";
import { ParamBag, buildTemplate, defaultParams, extraFromParams, paramsFromPreset } from "./paramModel";

export function KindTab({ kind, onGoTasks }: { kind: Kind; onGoTasks: () => void }) {
  const meta = KIND_META[kind];
  const { push } = useToasts();
  const store = useAppStore();
  const files = store.filesByKind[kind] || [];
  const [presetId, setPresetId] = useState("");
  const [presets, setPresets] = useState<Preset[]>([]);
  // 参数区统一为一整张参数表：工作台与任务卡片「修改参数」弹窗共用同一模型
  const [params, setParams] = useState<ParamBag>(() => ({
    ...defaultParams(kind),
    outDirMode: (store.outDirModeByKind[kind] as any) || "custom",
    outDir: store.outDirByKind[kind] || store.defaultOutDir,
  }));
  const [submitting, setSubmitting] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState<string | null>(null);
  const [previewEnabled, setPreviewEnabled] = useState(() => localStorage.getItem("sf-preview") !== "off");

  const outDir = params.outDir;
  const outDirMode = params.outDirMode;
  const target = params.target;

  useEffect(() => {
    setParams((prev) => ({
      ...defaultParams(kind),
      // 输出目录按类型记忆，切换 TAB 不丢
      outDirMode: prev.outDirMode || "custom",
      outDir: prev.outDir || store.defaultOutDir,
    }));
    setErrors({});
  }, [kind]);

  useEffect(() => {
    api.listPresets(kind).then(setPresets).catch(() => {});
  }, [kind]);

  /** 参数区局部更新；输出目录同时写回 store（按类型记忆） */
  const patch = (p: ParamBag) => {
    setParams((prev) => ({ ...prev, ...p }));
    if (p.outDir !== undefined) store.setOutDir(kind, p.outDir);
    if (p.outDirMode !== undefined) store.setOutDirMode(kind, p.outDirMode);
  };

  useDragDrop((paths) => store.addFiles(kind, paths));

  const pickFiles = async () => {
    if (!isTauri()) return;
    const res = await openDialog({ multiple: true, filters: [{ name: meta.label, extensions: meta.inputs }] });
    if (typeof res === "string") store.addFiles(kind, [res]);
    else if (Array.isArray(res)) store.addFiles(kind, res);
  };

  const handlePreset = (id: string) => {
    setPresetId(id);
    const p = presets.find((p) => p.id === id);
    if (!p) return patch({ presetId: "" });
    api.setActivePreset(id).catch(() => {});
    patch(paramsFromPreset(kind, p, params));
    push(`已应用预设「${p.name}」`, "info");
  };

  const saveAsPreset = async () => {
    const name = await promptDialog({
      title: "保存为预设",
      label: "给这套参数起个名字，便于下次一键套用",
      placeholder: "例如：网页用 WebP 80",
      defaultValue: `${meta.label}-${target}`,
      confirmText: "保存",
      hint: "将保存当前参数区的全部参数（格式、质量、命名、目录与高级参数）",
    });
    if (!name) return;
    const preset: Preset = {
      id: `user_${Date.now()}`,
      name,
      builtin: false,
      kind,
      options: { target_ext: target, quality: params.quality, preset: presetId || null, extra: extraFromParams(kind, params) },
      out_dir_mode: outDirMode,
      out_dir: outDirMode === "custom" ? outDir : null,
      naming: { ...params.naming, template: buildTemplate(params.naming) },
      updated_at: new Date().toISOString(),
    };
    try {
      await api.savePreset(preset);
      setPresets(await api.listPresets(kind));
      push("预设已保存", "success");
    } catch (e: any) {
      push(String(e), "error");
    }
  };

  const deletePreset = async () => {
    await api.deletePreset(presetId).catch(() => {});
    setPresets(await api.listPresets(kind));
    setPresetId("");
    patch({ presetId: "" });
    push("预设已删除", "info");
  };

  const start = async () => {
    const effOutDir =
      outDirMode === "custom"
        ? outDir
        : outDirMode === "project_default"
          ? store.defaultOutDir || outDir
          : "";
    const v = validateForm({
      target,
      outDir: outDirMode === "source_dir" ? "ok" : effOutDir,
      quality: params.quality,
      crf: params.crf,
      namingTemplate: buildTemplate(params.naming),
    });
    setErrors(v.errors);
    if (!v.ok) return;
    if (files.length === 0) return push("请先添加文件", "error");
    setSubmitting(true);
    try {
      // 每个文件独立建一个任务，卡片平铺展示
      for (const f of files) {
        await api.createTask({
          kind,
          input_files: [f],
          output_dir: effOutDir,
          options: { target_ext: target, quality: params.quality, preset: presetId || null, extra: extraFromParams(kind, params) },
          priority: 0,
          output_dir_mode: outDirMode,
          naming: { ...params.naming, template: buildTemplate(params.naming) },
        });
      }
      push(`已创建 ${files.length} 个任务`, "success");
      store.clearFiles(kind);
      onGoTasks();
    } catch (e: any) {
      push(String(e), "error");
    } finally {
      setSubmitting(false);
    }
  };

  // 顶栏「开始转换」按钮触发
  // 只响应「新增的」触发计数：否则从任务列表切回本 TAB 时会因挂载而重复触发一次转换
  const startRef = React.useRef(start);
  startRef.current = start;
  const handledTrigger = React.useRef(0);
  useEffect(() => {
    const t = useAppStore.getState().startTrigger;
    if (t > 0 && t !== handledTrigger.current) {
      handledTrigger.current = t;
      startRef.current();
    }
  }, [store.startTrigger]);

  const togglePreview = (v: boolean) => {
    setPreviewEnabled(v);
    localStorage.setItem("sf-preview", v ? "on" : "off");
    import("@tauri-apps/api/core").then(({ invoke }) => invoke("kv_set", { key: "sf-preview", value: v ? "on" : "off" }).catch(() => {}));
  };

  /* ── 工作台空白区域右键菜单 ─────────────────── */
  const { registerBlankHandler } = useGlobalContextMenu() ?? {};
  useEffect(() => {
    if (!registerBlankHandler) return;
    return registerBlankHandler(() => {
      const cur = selected || files[0] || "";
      const effOutDir =
        outDirMode === "custom" ? outDir : outDirMode === "project_default" ? store.defaultOutDir || outDir : cur ? "" : "";
      const outDirResolved = outDirMode === "source_dir" && cur ? cur.replace(/[\\/][^\\/]+$/, "") : effOutDir;
      return [
        {
          label: meta.ready ? `开始转换（${files.length} 个文件）` : "开始转换（当前类型暂不可用）",
          disabled: !meta.ready || files.length === 0,
          onClick: () => startRef.current(),
        },
        { divider: true, label: "" },
        {
          label: files.length > 0 ? "全选文件" : "无文件",
          disabled: files.length === 0,
          onClick: () => setSelected(files[0] ?? null),
        },
        {
          label: "清空文件列表",
          disabled: files.length === 0,
          onClick: () => {
            store.clearFiles(kind);
            setSelected(null);
          },
        },
        { divider: true, label: "" },
        {
          label: "打开源目录",
          disabled: !selected,
          onClick: () => api.openInExplorer(selected!),
        },
        {
          label: "打开输出目录",
          disabled: !outDirResolved,
          onClick: () => api.openInExplorer(outDirResolved),
        },
        {
          label: "复制输入路径",
          disabled: !selected,
          onClick: () => {
            navigator.clipboard?.writeText(selected!).then(() => push("已复制路径", "success")).catch(() => push("复制失败", "error"));
          },
        },
        { divider: true, label: "" },
        {
          label: previewEnabled ? "关闭预览面板" : "开启预览面板",
          onClick: () => togglePreview(!previewEnabled),
        },
        {
          label: "打开设置",
          onClick: () => window.dispatchEvent(new CustomEvent("sf:open-settings")),
        },
      ];
    });
  }, [registerBlankHandler, files, selected, outDir, outDirMode, previewEnabled, kind, meta.ready, store]);

  return (
    <div className="h-full flex flex-col gap-2 min-h-0">
      <SplitPane initialLeft={45} className="flex-1 min-h-0">
      <div className="h-full flex flex-col gap-2 pr-1 min-h-0">
        <Card className="flex-1 min-h-0 flex flex-col">
          <div onClick={pickFiles} className="border-2 border-dashed border-white/20 rounded-xl p-3 text-center cursor-pointer hover:border-accent hover:bg-white/5 transition flex-none">
            <Upload className="w-5 h-5 mx-auto mb-1 opacity-70" />
            <p className="text-sm opacity-80">拖拽或点击选择{meta.label}文件</p>
            <p className="text-xs opacity-50 mt-1">{meta.inputs.join(", ") || "—"}</p>
            {files.length > 0 && (
              <div className="mt-2 pt-2 border-t border-white/10 flex flex-wrap justify-center gap-1">
                {Array.from(new Set(files.map((f) => f.split(".").pop()?.toLowerCase() || ""))).slice(0, 12).map((ext) => (
                  <span key={ext} className="inline-flex items-center gap-1 text-[11px] t-2 bg-white/5 rounded-full pl-1 pr-2 py-0.5">
                    <FileKindIcon name={`x.${ext}`} size="w-3 h-3" />
                    {ext}
                  </span>
                ))}
              </div>
            )}
          </div>
          <div className="mt-2 flex-1 min-h-0 overflow-auto flex flex-col gap-1.5">
            {files.map((f, i) => (
              <div key={i} onClick={() => setSelected(f)} className={`flex items-center gap-2 text-sm rounded-xl px-3 py-1.5 cursor-pointer ${selected === f ? "bg-accent-soft" : "bg-white/5"}`}>
                <FileKindIcon name={f} />
                <span className="truncate flex-1" title={f}>{f.split(/[\\/]/).pop()}</span>
                <button aria-label="移除" onClick={(e) => { e.stopPropagation(); store.removeFile(kind, i); }}>
                  <X className="w-4 h-4 opacity-60" />
                </button>
              </div>
            ))}
          </div>
          {/* 文件区底栏：左=预览开关，右=文件信息 */}
          <div className="mt-2 pt-2 border-t border-white/10 flex items-center gap-2 flex-none">
            <button
              onClick={() => togglePreview(!previewEnabled)}
              className={`h-6 px-2.5 text-[11px] rounded-lg whitespace-nowrap transition ${
                previewEnabled ? "bg-accent-soft text-accent" : "bg-slate-500/15 hover:bg-slate-500/25"
              }`}
              title={previewEnabled ? "关闭预览面板" : "开启预览面板"}
            >
              {previewEnabled ? "关闭预览面板" : "开启预览面板"}
            </button>
            <span className="flex-1" />
            <span className="text-[11px] t-3 truncate max-w-[200px]" title={`输出格式：${target}`}>
              {meta.label} · {files.length} 个文件 · → {target}
            </span>
          </div>
        </Card>
        {previewEnabled && (
          <div className="h-56 flex-none">
            <PreviewPanel kind={kind} path={selected} onClose={() => togglePreview(false)} />
          </div>
        )}
      </div>

      <div className="h-full flex flex-col gap-2 pl-1 min-h-0">
        <div className="flex-1 min-h-0 overflow-auto flex flex-col gap-2">
          <ParamPanel
            kind={kind}
            value={params}
            onChange={patch}
            errors={errors}
            presets={presets}
            presetId={presetId}
            onPreset={handlePreset}
            onSavePreset={saveAsPreset}
            onDeletePreset={deletePreset}
            defaultOutDirText={store.defaultOutDir}
          />
        </div>
      </div>
      </SplitPane>
    </div>
  );
}

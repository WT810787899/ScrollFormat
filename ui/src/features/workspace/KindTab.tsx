import React, { useEffect, useState } from "react";
import { Upload, X, FolderOpen } from "lucide-react";
import { GlassSelect } from "../../components/GlassSelect";
import { GlassCheckbox } from "../../components/GlassCheckbox";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Kind, KIND_META } from "../../lib/kinds";
import { api, isTauri, Preset } from "../../lib/api";
import { useAppStore } from "../../store";
import { useToasts } from "../../components/Toast";
import { ParamField } from "../../components/ParamField";
import { Card } from "../../components/Card";
import { validateForm } from "../../lib/validation";
import { PreviewPanel } from "./PreviewPanel";
import { SplitPane } from "../../components/SplitPane";
import { useDragDrop } from "../../lib/useDragDrop";
import { FileKindIcon } from "../../lib/fileIcon";
import { useGlobalContextMenu } from "../../components/ContextMenuProvider";
import { promptDialog } from "../../components/PromptDialog";

interface NamingRule {
  prefix: string;
  suffix: string;
  useIndex: boolean;
  useTimestamp: boolean;
  conflict: "auto_rename" | "overwrite" | "skip";
}

/** 由前缀/后缀/序号/时间拼出后端模板 */
function buildTemplate(n: NamingRule): string {
  return `${n.prefix}${n.useIndex ? "{index:03}" : ""}${n.useTimestamp ? "{timestamp}" : ""}{name}${n.suffix}`;
}

/** 从已存模板反解出界面状态 */
function parseTemplate(rule: any): NamingRule {
  const tpl: string = rule?.template ?? "";
  const prefix: string = rule?.prefix ?? "";
  const suffix: string = rule?.suffix ?? "";
  const inner = tpl.startsWith(prefix) && tpl.endsWith(suffix) ? tpl.slice(prefix.length, tpl.length - suffix.length || undefined) : tpl;
  return {
    prefix,
    suffix,
    useIndex: inner.includes("{index"),
    useTimestamp: inner.includes("{timestamp"),
    conflict: rule?.conflict ?? "auto_rename",
  };
}

const defaultNaming: NamingRule = {
  prefix: "",
  suffix: "",
  useIndex: false,
  useTimestamp: false,
  conflict: "auto_rename",
};

export function KindTab({ kind, onGoTasks }: { kind: Kind; onGoTasks: () => void }) {
  const meta = KIND_META[kind];
  const { push } = useToasts();
  const store = useAppStore();
  const files = store.filesByKind[kind] || [];
  const outDir = store.outDirByKind[kind] || store.defaultOutDir;
  const outDirMode = (store.outDirModeByKind[kind] as any) || "custom";
  const [target, setTarget] = useState(meta.outputs[0] || "");
  const [presetId, setPresetId] = useState("");
  const [presets, setPresets] = useState<Preset[]>([]);
  const [quality, setQuality] = useState(85);
  const [maxSide, setMaxSide] = useState("original");
  const [bitrate, setBitrate] = useState("192k");
  const [sampleRate, setSampleRate] = useState("original");
  const [channels, setChannels] = useState("2");
  const [crf, setCrf] = useState(23);
  const [vscale, setVscale] = useState("original");
  const [vcustomW, setVcustomW] = useState(1280);
  const [vfps, setVfps] = useState("original");
  const [encPreset, setEncPreset] = useState("veryfast");
  const [vcodec, setVcodec] = useState("");
  const [deinterlace, setDeinterlace] = useState(false);
  const [faststart, setFaststart] = useState(true);
  const [acodec, setAcodec] = useState("");
  const [abitrate, setAbitrate] = useState("");
  const [asamplerate, setAsamplerate] = useState("original");
  const [achannels, setAchannels] = useState("");
  const [audioCodec, setAudioCodec] = useState("");
  const [loudnorm, setLoudnorm] = useState("off");
  const [keepMeta, setKeepMeta] = useState(true);
  const [customArgs, setCustomArgs] = useState("");
  const [imgScaleMode, setImgScaleMode] = useState("long");
  const [imgPercent, setImgPercent] = useState(50);
  const [imgFilter, setImgFilter] = useState("lanczos3");
  const [imgDepth, setImgDepth] = useState("auto");
  const [imgBg, setImgBg] = useState("#ffffff");
  const [imgFlatten, setImgFlatten] = useState(false);
  const [naming, setNaming] = useState<NamingRule>(defaultNaming);
  const [submitting, setSubmitting] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState<string | null>(null);
  const [previewEnabled, setPreviewEnabled] = useState(() => localStorage.getItem("sf-preview") !== "off");

  useEffect(() => setTarget(meta.outputs[0] || ""), [kind]);
  useEffect(() => {
    api.listPresets(kind).then(setPresets).catch(() => {});
  }, [kind]);


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
    if (!p) return;
    api.setActivePreset(id).catch(() => {});
    const extra: any = (p.options as any).extra || {};
    const targetExt = extra.target_ext || p.options.target_ext;
    if (targetExt) setTarget(targetExt);
    const q = extra.quality ?? p.options.quality;
    if (q) setQuality(q);
    store.setOutDirMode(kind, extra.out_dir_mode || p.out_dir_mode);
    if (extra.out_dir || p.out_dir) store.setOutDir(kind, extra.out_dir || p.out_dir);
    if (extra.naming) setNaming(parseTemplate(extra.naming));
    else if (p.naming) setNaming(parseTemplate(p.naming));
    if (kind === "image") {
      if (extra.scale_mode) setImgScaleMode(extra.scale_mode);
      else if (extra.percent != null) setImgScaleMode("percent");
      else if (extra.max_side != null) setImgScaleMode("long");
      if (extra.percent != null) setImgPercent(extra.percent);
      if (extra.max_side != null) setMaxSide(String(extra.max_side));
      if (extra.filter) setImgFilter(extra.filter);
      if (extra.depth) setImgDepth(extra.depth);
      if (extra.flatten != null) setImgFlatten(extra.flatten);
      if (extra.bg) setImgBg(extra.bg);
    }
    if (kind === "audio") {
      if (extra.bitrate) setBitrate(extra.bitrate);
      if (extra.sample_rate) setSampleRate(extra.sample_rate);
      if (extra.channels) setChannels(String(extra.channels));
      if (extra.codec != null) setAudioCodec(extra.codec);
      if (extra.loudnorm) setLoudnorm(extra.loudnorm);
      if (extra.keep_meta != null) setKeepMeta(extra.keep_meta);
      if (extra.custom_args != null) setCustomArgs(extra.custom_args);
    }
    if (kind === "video") {
      if (extra.crf != null) setCrf(extra.crf);
      if (extra.scale_mode) setVscale(extra.scale_mode);
      else if (extra.scale) setVscale(extra.scale);
      if (extra.custom_width != null) setVcustomW(extra.custom_width);
      if (extra.fps) setVfps(extra.fps);
      if (extra.enc_preset) setEncPreset(extra.enc_preset);
      if (extra.v_codec != null) setVcodec(extra.v_codec);
      if (extra.deinterlace != null) setDeinterlace(extra.deinterlace);
      if (extra.faststart != null) setFaststart(extra.faststart);
      if (extra.a_codec != null) setAcodec(extra.a_codec);
      if (extra.a_bitrate != null) setAbitrate(extra.a_bitrate);
      if (extra.a_sample_rate) setAsamplerate(extra.a_sample_rate);
      if (extra.a_channels) setAchannels(String(extra.a_channels));
      else if (extra.a_channels === 0) setAchannels("");
      if (extra.custom_args != null) setCustomArgs(extra.custom_args);
    }
    push(`已应用预设「${p.name}」`, "info");
  };

  const extraParams = () => {
    let extra: any = {};
    if (kind === "image") {
      extra = {
        scale_mode: imgScaleMode,
        max_side: imgScaleMode === "long" && maxSide !== "original" ? Number(maxSide) : null,
        percent: imgScaleMode === "percent" ? imgPercent : null,
        filter: imgFilter,
        depth: imgDepth,
        flatten: imgFlatten,
        bg: imgBg,
      };
    }
    if (kind === "audio") {
      extra = {
        bitrate,
        sample_rate: sampleRate,
        channels: Number(channels),
        codec: audioCodec,
        loudnorm,
        keep_meta: keepMeta,
        custom_args: customArgs,
      };
    }
    if (kind === "video") {
      extra = {
        crf,
        scale_mode: vscale,
        custom_width: vcustomW,
        scale: vscale === "custom" ? `${vcustomW}:-2` : vscale,
        fps: vfps,
        enc_preset: encPreset,
        v_codec: vcodec,
        deinterlace,
        faststart,
        a_codec: acodec,
        a_bitrate: abitrate,
        a_sample_rate: asamplerate,
        a_channels: achannels ? Number(achannels) : 0,
        custom_args: customArgs,
      };
    }
    return {
      ...extra,
      target_ext: target,
      quality,
      naming: { ...naming, template: buildTemplate(naming) },
      out_dir_mode: outDirMode,
      out_dir: outDir,
    };
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
      options: { target_ext: target, quality, preset: presetId || null, extra: extraParams() },
      out_dir_mode: outDirMode,
      out_dir: outDirMode === "custom" ? outDir : null,
      naming: { ...naming, template: buildTemplate(naming) },
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
      quality,
      crf,
      namingTemplate: buildTemplate(naming),
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
          options: { target_ext: target, quality, preset: presetId || null, extra: extraParams() },
          priority: 0,
          output_dir_mode: outDirMode,
          naming: { ...naming, template: buildTemplate(naming) },
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
  const startRef = React.useRef(start);
  startRef.current = start;
  useEffect(() => {
    if (store.startTrigger > 0) startRef.current();
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
      const target = selected || files[0] || "";
      const effOutDir =
        outDirMode === "custom" ? outDir : outDirMode === "project_default" ? store.defaultOutDir || outDir : target ? "" : "";
      const outDirResolved = outDirMode === "source_dir" && target ? target.replace(/[\\/][^\\/]+$/, "") : effOutDir;
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
          disabled: !target,
          onClick: () => api.openInExplorer(target),
        },
        {
          label: "打开输出目录",
          disabled: !outDirResolved,
          onClick: () => api.openInExplorer(outDirResolved),
        },
        {
          label: "复制输入路径",
          disabled: !target,
          onClick: () => {
            navigator.clipboard?.writeText(target).then(() => push("已复制路径", "success")).catch(() => push("复制失败", "error"));
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
          <Card className="py-2">
            <div className="flex items-center gap-2">
              <span className="text-xs t-3 w-16 shrink-0 pl-1">预设</span>
              <GlassSelect value={presetId} onChange={(v: string) => handlePreset(v)}>
                <option value="">— 不使用 —</option>
                {presets.map((p) => <option key={p.id} value={p.id}>{p.builtin ? "★ " : ""}{p.name}</option>)}
              </GlassSelect>
              <button onClick={saveAsPreset} title="把当前参数区全部保存为预设" className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25 whitespace-nowrap">存为预设</button>
              {presetId && !presets.find((p) => p.id === presetId)?.builtin && (
                <button onClick={async () => { await api.deletePreset(presetId).catch(() => {}); setPresets(await api.listPresets(kind)); setPresetId(""); push("预设已删除", "info"); }} title="删除当前预设" className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-rose-500/25 whitespace-nowrap">删除</button>
              )}
            </div>
          </Card>

          <Card title="基础参数">
            <div className="flex flex-col gap-2">
              <ParamField label="输出格式" error={errors.target}>
                <GlassSelect value={target} onChange={(v: string) => setTarget(v)}>
                  {meta.outputs.map((o) => <option key={o} value={o}>{o}</option>)}
                </GlassSelect>
              </ParamField>
              <ParamField label="质量" error={errors.quality} wide>
                <div className="flex items-center gap-2">
                  <input type="range" min={10} max={100} value={quality} onChange={(e) => setQuality(+e.target.value)} className="flex-1" />
                  <span className="text-xs opacity-70 w-8">{quality}</span>
                </div>
              </ParamField>
              <ParamField label="输出目录" error={errors.outDir} wide>
                <div className="flex items-center gap-1.5">
                  {([
                    { v: "project_default", label: "项目默认" },
                    { v: "source_dir", label: "原文件地址" },
                    { v: "custom", label: "自定义" },
                  ] as const).map((m) => (
                    <button
                      key={m.v}
                      onClick={() => store.setOutDirMode(kind, m.v)}
                      title={`输出到${m.label}`}
                      className={`h-7 px-2.5 text-xs rounded-lg whitespace-nowrap transition ${
                        outDirMode === m.v ? "tab-active" : "bg-slate-500/15 hover:bg-slate-500/25"
                      }`}
                    >
                      {m.label}
                    </button>
                  ))}
                  {outDirMode === "custom" && (
                    <button
                      onClick={async () => { const r = await openDialog({ directory: true }); if (typeof r === "string") store.setOutDir(kind, r); }}
                      title="选择输出目录"
                      className="h-7 px-2.5 text-xs rounded-lg bg-slate-500/15 hover:bg-slate-500/25 flex items-center gap-1.5 min-w-0"
                    >
                      <FolderOpen className="w-3.5 h-3.5 flex-none" />
                      <span className="truncate max-w-[220px]" title={outDir}>{outDir ? outDir.split(/[\\/]/).pop() : "选择目录"}</span>
                    </button>
                  )}
                  {outDirMode !== "custom" && (
                    <span className="text-[11px] t-3 truncate">
                      {outDirMode === "source_dir" ? "输出到每个源文件所在目录" : `项目默认：${store.defaultOutDir || "未设置（请到设置中指定）"}`}
                    </span>
                  )}
                </div>
              </ParamField>
              <ParamField label="命名选项" error={errors.naming} wide>
                <div className="flex items-center gap-2">
                  <div className="flex items-center gap-1">
                    <span className="text-xs t-3">前缀</span>
                    <input
                      value={naming.prefix}
                      onChange={(e) => setNaming({ ...naming, prefix: e.target.value })}
                      placeholder="自定义"
                      className="h-7 w-20 bg-slate-500/15 hover:bg-slate-500/25 rounded-lg px-2 text-xs"
                    />
                  </div>
                  <div className="flex items-center gap-1">
                    <span className="text-xs t-3">后缀</span>
                    <input
                      value={naming.suffix}
                      onChange={(e) => setNaming({ ...naming, suffix: e.target.value })}
                      placeholder="自定义"
                      className="h-7 w-20 bg-slate-500/15 hover:bg-slate-500/25 rounded-lg px-2 text-xs"
                    />
                  </div>
                  <GlassCheckbox checked={naming.useIndex} onChange={(v) => setNaming({ ...naming, useIndex: v })} label="序号" title="在文件名中加入 001 形式的序号" />
                  <GlassCheckbox checked={naming.useTimestamp} onChange={(v) => setNaming({ ...naming, useTimestamp: v })} label="时间" title="在文件名中加入日期（yyyyMMdd）" />
                </div>
              </ParamField>
              <ParamField label="冲突策略">
                <GlassSelect value={naming.conflict} onChange={(v: string) => setNaming({ ...naming, conflict: v as any })}>
                  <option value="auto_rename">自动重命名</option>
                  <option value="overwrite">覆盖</option>
                  <option value="skip">跳过</option>
                </GlassSelect>
              </ParamField>
            </div>
          </Card>

          <Card title="高级参数">
            <div className="flex flex-col gap-2">
              {kind === "image" && (
                <>
                  <ParamField label="缩放方式" wide>
                    <div className="flex items-center gap-2">
                      <GlassSelect value={imgScaleMode} onChange={(v: string) => setImgScaleMode(v)}>
                        <option value="long">长边限制</option>
                        <option value="percent">按比例缩放</option>
                        <option value="none">不缩放</option>
                      </GlassSelect>
                      {imgScaleMode === "long" && (
                        <GlassSelect value={maxSide} onChange={(v: string) => setMaxSide(v)}>
                          <option value="original">原始</option>
                          <option value="4096">4096</option>
                          <option value="3840">3840</option>
                          <option value="2560">2560</option>
                          <option value="2048">2048</option>
                          <option value="1920">1920</option>
                          <option value="1600">1600</option>
                          <option value="1440">1440</option>
                          <option value="1280">1280</option>
                          <option value="1080">1080</option>
                          <option value="1024">1024</option>
                          <option value="800">800</option>
                          <option value="640">640</option>
                          <option value="512">512</option>
                          <option value="320">320</option>
                        </GlassSelect>
                      )}
                      {imgScaleMode === "percent" && (
                        <div className="flex items-center gap-2 flex-1">
                          <input type="range" min={5} max={200} value={imgPercent} onChange={(e) => setImgPercent(+e.target.value)} className="flex-1" />
                          <span className="text-xs t-3 w-10 text-right tabular-nums">{imgPercent}%</span>
                        </div>
                      )}
                    </div>
                  </ParamField>
                  <ParamField label="插值算法" hint="缩小时的采样质量">
                    <GlassSelect value={imgFilter} onChange={(v: string) => setImgFilter(v)}>
                      <option value="lanczos3">Lanczos3（默认）</option>
                      <option value="catmullrom">CatmullRom</option>
                      <option value="triangle">Triangle（双线性）</option>
                      <option value="nearest">Nearest（最近邻）</option>
                    </GlassSelect>
                  </ParamField>
                  <ParamField label="输出位深">
                    <GlassSelect value={imgDepth} onChange={(v: string) => setImgDepth(v)}>
                      <option value="auto">自动</option>
                      <option value="8">8 bit</option>
                      <option value="16">16 bit</option>
                    </GlassSelect>
                  </ParamField>
                  <ParamField label="背景色" hint="JPEG 等不支持透明通道时填充的颜色" wide>
                    <div className="flex items-center gap-2">
                      <input type="color" value={imgBg} onChange={(e) => setImgBg(e.target.value)} className="w-8 h-7 rounded-lg cursor-pointer" />
                      <GlassCheckbox checked={imgFlatten} onChange={setImgFlatten} label="透明区域填充" title="PNG/WebP 转 JPEG 时用背景色填充透明区域" />
                    </div>
                  </ParamField>
                </>
              )}
              {kind === "audio" && (
                <>
                  <ParamField label="采样率"><GlassSelect value={sampleRate} onChange={(v: string) => setSampleRate(v)}><option value="original">原始</option><option value="22050">22050</option><option value="32000">32000</option><option value="44100">44100</option><option value="48000">48000</option><option value="96000">96000</option></GlassSelect></ParamField>
                  <ParamField label="声道"><GlassSelect value={channels} onChange={(v: string) => setChannels(v)}><option value="1">单声道</option><option value="2">立体声</option><option value="6">5.1 环绕</option></GlassSelect></ParamField>
                  <ParamField label="编码器/码率" hint="编码器留空按格式自动" wide>
                    <div className="flex items-center gap-2">
                      <GlassSelect value={audioCodec} onChange={(v: string) => setAudioCodec(v)}>
                        <option value="">自动</option>
                        <option value="libmp3lame">MP3 (libmp3lame)</option>
                        <option value="libvorbis">Vorbis (libvorbis)</option>
                        <option value="libopus">Opus (libopus)</option>
                        <option value="aac">AAC</option>
                        <option value="flac">FLAC</option>
                        <option value="pcm_s16le">PCM 16bit</option>
                        <option value="wmav2">WMA</option>
                      </GlassSelect>
                      <GlassSelect value={bitrate} onChange={(v: string) => setBitrate(v)}>
                        <option value="64k">64k</option>
                        <option value="96k">96k</option>
                        <option value="128k">128k</option>
                        <option value="160k">160k</option>
                        <option value="192k">192k</option>
                        <option value="256k">256k</option>
                        <option value="320k">320k</option>
                        <option value="lossless">无损</option>
                      </GlassSelect>
                    </div>
                  </ParamField>
                  <ParamField label="音量归一化" hint="loudnorm 双声道响度标准化">
                    <div className="flex items-center gap-2">
                      <GlassSelect value={loudnorm} onChange={(v: string) => setLoudnorm(v)}>
                        <option value="off">关闭</option>
                        <option value="ebur128">EBU R128 (-14 LUFS)</option>
                        <option value="ebur128_16">EBU R128 峰值 -16</option>
                        <option value="dynaudnorm">动态模式</option>
                      </GlassSelect>
                    </div>
                  </ParamField>
                  <ParamField label="保留元数据">
                    <GlassCheckbox checked={keepMeta} onChange={setKeepMeta} label="复制封面与标签" />
                  </ParamField>
                </>
              )}
              {kind === "video" && (
                <>
                  <ParamField label="CRF" error={errors.crf} hint="数值越小画质越好、体积越大" wide>
                    <div className="flex items-center gap-2">
                      <input type="range" min={0} max={51} value={crf} onChange={(e) => setCrf(+e.target.value)} className="flex-1" />
                      <span className="text-xs t-3 w-8 text-right tabular-nums">{crf}</span>
                    </div>
                  </ParamField>
                  <ParamField label="分辨率">
                    <div className="flex items-center gap-2">
                      <GlassSelect value={vscale} onChange={(v: string) => setVscale(v)}>
                        <option value="original">原始</option>
                        <option value="3840:-2">4K 2160p</option>
                        <option value="2560:-2">2K 1440p</option>
                        <option value="1920:-2">1080p</option>
                        <option value="1600:-2">1200p</option>
                        <option value="1280:-2">720p</option>
                        <option value="960:-2">540p</option>
                        <option value="854:-2">480p</option>
                        <option value="640:-2">360p</option>
                        <option value="480:-2">240p</option>
                        <option value="iw*3/4">宽度 75%</option>
                        <option value="iw/2">宽度 50%</option>
                        <option value="custom">自定义宽度</option>
                      </GlassSelect>
                      {vscale === "custom" && (
                        <input type="number" min={16} value={vcustomW} onChange={(e) => setVcustomW(+e.target.value)} className="h-7 w-24 bg-slate-500/15 rounded-lg px-2 text-xs" />
                      )}
                    </div>
                  </ParamField>
                  <ParamField label="帧率">
                    <GlassSelect value={vfps} onChange={(v: string) => setVfps(v)}>
                      <option value="original">原始</option>
                      <option value="12">12</option>
                      <option value="15">15</option>
                      <option value="20">20</option>
                      <option value="23.976">23.976</option>
                      <option value="24">24</option>
                      <option value="25">25</option>
                      <option value="29.97">29.97</option>
                      <option value="30">30</option>
                      <option value="48">48</option>
                      <option value="50">50</option>
                      <option value="59.94">59.94</option>
                      <option value="60">60</option>
                      <option value="120">120</option>
                    </GlassSelect>
                  </ParamField>
                  <ParamField label="编码器/速度" hint="硬件编码需对应显卡支持" wide>
                    <div className="flex items-center gap-2">
                      <GlassSelect value={vcodec} onChange={(v: string) => setVcodec(v)}>
                        <option value="">自动（按容器）</option>
                        <option value="libx264">H.264 (libx264)</option>
                        <option value="libx265">H.265 (libx265)</option>
                        <option value="libvpx-vp9">VP9 (libvpx-vp9)</option>
                        <option value="libvpx">VP8 (libvpx)</option>
                        <option value="mpeg4">MPEG-4</option>
                        <option value="h264_nvenc">H.264 NVENC（显卡）</option>
                        <option value="hevc_nvenc">H.265 NVENC（显卡）</option>
                        <option value="h264_qsv">H.264 QSV（核显）</option>
                        <option value="h264_amf">H.264 AMF（AMD）</option>
                        <option value="libaom-av1">AV1 (libaom-av1)</option>
                      </GlassSelect>
                      <GlassSelect value={encPreset} onChange={(v: string) => setEncPreset(v)}>
                        <option value="ultrafast">ultrafast</option>
                        <option value="superfast">superfast</option>
                        <option value="veryfast">veryfast</option>
                        <option value="faster">faster</option>
                        <option value="fast">fast</option>
                        <option value="medium">medium</option>
                        <option value="slow">slow</option>
                        <option value="slower">slower</option>
                        <option value="veryslow">veryslow</option>
                      </GlassSelect>
                    </div>
                  </ParamField>
                  <ParamField label="去隔行">
                    <GlassCheckbox checked={deinterlace} onChange={setDeinterlace} label="启用 yadif 去隔行" title="使用 ffmpeg yadif 滤镜去除交错扫描的抖动" />
                  </ParamField>
                  <ParamField label="Web 优化">
                    <GlassCheckbox checked={faststart} onChange={setFaststart} label="moov 前置（+faststart）" title="MP4 索引前置，网页可边下边播" />
                  </ParamField>
                  <div className="h-px bg-white/10 my-0.5" />
                  <ParamField label="音频编码" wide>
                    <div className="flex items-center gap-2">
                      <GlassSelect value={acodec} onChange={(v: string) => setAcodec(v)}>
                        <option value="">自动</option>
                        <option value="aac">AAC</option>
                        <option value="libmp3lame">MP3</option>
                        <option value="libopus">Opus</option>
                        <option value="libvorbis">Vorbis</option>
                        <option value="flac">FLAC</option>
                        <option value="copy">直接复制（不重编码）</option>
                        <option value="none">去掉音轨</option>
                      </GlassSelect>
                      <GlassSelect value={abitrate} onChange={(v: string) => setAbitrate(v)}>
                        <option value="">默认码率</option>
                        <option value="64k">64k</option>
                        <option value="96k">96k</option>
                        <option value="128k">128k</option>
                        <option value="192k">192k</option>
                        <option value="256k">256k</option>
                        <option value="320k">320k</option>
                      </GlassSelect>
                    </div>
                  </ParamField>
                  <ParamField label="采样/声道" wide>
                    <div className="flex items-center gap-2">
                      <GlassSelect value={asamplerate} onChange={(v: string) => setAsamplerate(v)}>
                        <option value="original">原始采样率</option>
                        <option value="22050">22050</option>
                        <option value="32000">32000</option>
                        <option value="44100">44100</option>
                        <option value="48000">48000</option>
                        <option value="96000">96000</option>
                      </GlassSelect>
                      <GlassSelect value={achannels} onChange={(v: string) => setAchannels(v)}>
                        <option value="">原始声道</option>
                        <option value="1">单声道</option>
                        <option value="2">立体声</option>
                        <option value="6">5.1 环绕</option>
                        <option value="8">7.1 环绕</option>
                      </GlassSelect>
                    </div>
                  </ParamField>
                  </>
              )}
              {kind === "document" && <span className="opacity-60">文档参数占位，待接入 pandoc / LibreOffice。</span>}
              {(kind === "audio" || kind === "video") && (
                <>
                  <div className="h-px bg-white/10 my-0.5" />
                  <ParamField label="自定义参数" wide>
                    <input
                      value={customArgs}
                      onChange={(e) => setCustomArgs(e.target.value)}
                      placeholder={kind === "video" ? "-t 30 -profile:v high -movflags +faststart" : "-t 30 -map_metadata 0"}
                      className="h-7 w-full min-w-[240px] bg-slate-500/15 rounded-lg px-2 text-xs font-mono"
                    />
                  </ParamField>
                </>
              )}
            </div>
          </Card>
        </div>
      </div>
      </SplitPane>
    </div>
  );
}

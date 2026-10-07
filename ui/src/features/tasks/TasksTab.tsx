import { createPortal } from "react-dom";
import { useCallback, useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { RotateCcw, Trash2, Pause, Play, ChevronDown, ChevronRight, X, FolderInput, FolderOutput, ExternalLink, Settings2, ArrowRight } from "lucide-react";
import { api, Task, isTauri } from "../../lib/api";
import { useToasts } from "../../components/Toast";
import { ContextMenu, MenuItem } from "../../components/ContextMenu";
import { useGlobalContextMenu } from "../../components/ContextMenuProvider";
import { FileKindIcon, fileKind } from "../../lib/fileIcon";
import { formatSize, formatDateShort, formatDateTime } from "../../lib/format";
import { TaskEditDialog } from "./TaskEditDialog";

/** 卡片里各项之间的竖线分隔符 */
function Sep() {
  return <span className="flex-none w-px h-3 bg-current opacity-25" aria-hidden />;
}

const SUBTABS = [
  { key: "all", label: "全部" },
  { key: "queued", label: "排队中" },
  { key: "running", label: "进行中" },
  { key: "paused", label: "已暂停" },
  { key: "completed", label: "已完成" },
  { key: "failed", label: "失败" },
  { key: "cancelled", label: "已取消" },
];

const STATUS_LABEL: Record<string, string> = {
  queued: "排队中",
  probing: "探测中",
  running: "进行中",
  paused: "已暂停",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

const STATUS_COLOR: Record<string, string> = {
  queued: "bg-slate-500/15 hover:bg-slate-500/25",
  running: "bg-accent-soft text-accent",
  probing: "bg-accent-soft text-accent",
  paused: "bg-amber-400/25 text-amber-700 dark:text-amber-200",
  completed: "bg-emerald-400/25 text-emerald-700 dark:text-emerald-200",
  failed: "bg-rose-400/25 text-rose-700 dark:text-rose-200",
  cancelled: "bg-white/5 opacity-60",
};

export function TasksTab() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [sub, setSub] = useState("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [liveProgress, setLiveProgress] = useState<Record<string, number>>({});
  const [liveLogs, setLiveLogs] = useState<Record<string, string[]>>({});
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  /** 正在改参数的任务 id（弹窗内容始终取最新任务快照，避免状态过期） */
  const [editingId, setEditingId] = useState<string | null>(null);
  const editing = editingId ? tasks.find((t) => t.id === editingId) ?? null : null;
  const { push } = useToasts();

  const refresh = useCallback(() => api.listTasks().then(setTasks).catch(() => {}), []);

  useEffect(() => {
    refresh();
    if (!isTauri()) return;
    const unlistens: (() => void)[] = [];
    import("@tauri-apps/api/event").then(async ({ listen }) => {
      for (const ev of ["task://created", "task://updated", "task://completed", "task://failed"]) {
        unlistens.push(await listen(ev, () => refresh()));
      }
      unlistens.push(
        await listen<{ id: string; progress: number }>("task://progress", (e) => {
          setLiveProgress((m) => ({ ...m, [e.payload.id]: e.payload.progress }));
        }),
      );
      unlistens.push(
        await listen<{ id: string; line: string }>("task://log", (e) => {
          setLiveLogs((m) => ({ ...m, [e.payload.id]: [...(m[e.payload.id] || []).slice(-49), e.payload.line] }));
        }),
      );
    });
    return () => unlistens.forEach((u) => u());
  }, [refresh]);

  const filtered = tasks.filter((t) => {
    if (sub === "all") return true;
    if (sub === "running") return t.status === "running" || t.status === "probing";
    return t.status === sub;
  });

  const toggleExpand = (id: string) =>
    setExpanded((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  /** 单击 = 单选；Ctrl/Shift+单击 = 追加/切换多选；右键 = 同样进入选中态 */
  const selectCard = (id: string, e: React.MouseEvent) => {
    const additive = e.ctrlKey || e.metaKey || e.shiftKey;
    setSelected((prev) => {
      if (additive) {
        const n = new Set(prev);
        n.has(id) ? n.delete(id) : n.add(id);
        return n;
      }
      return prev.size === 1 && prev.has(id) ? new Set() : new Set([id]);
    });
  };

  /** 右键时若目标不在当前选区，先把该卡片设为选中 */
  const ensureSelected = (id: string, e: React.MouseEvent) => {
    if (e.ctrlKey || e.metaKey || e.shiftKey) return;
    setSelected((prev) => (prev.has(id) ? prev : new Set([id])));
  };

  const allSelected = filtered.length > 0 && filtered.every((t) => selected.has(t.id));
  const toggleSelectAll = () => setSelected(allSelected ? new Set() : new Set(filtered.map((t) => t.id)));

  const selectedIds = tasks.filter((t) => selected.has(t.id)).map((t) => t.id);
  const pausedIds = new Set(tasks.filter((t) => t.status === "paused").map((t) => t.id));
  const selectedPaused = selectedIds.length > 0 && selectedIds.every((id) => pausedIds.has(id));

  const act = async (action: string, label: string, ids?: string[]) => {
    const target = ids ?? (selected.size > 0 ? [...selected] : filtered.map((t) => t.id));
    if (target.length === 0) return push("没有可操作的任务", "info");
    try {
      await api.taskAction(action, target);
      push(`${label}完成`, "success");
    } catch (e: any) {
      push(String(e), "error");
    }
    setSelected(new Set());
    refresh();
  };

  const fileName = (t: Task) => t.items[0]?.input.split(/[\\/]/).pop() || t.name;

  /* ── 按住拖拽框选 ───────────────────────────── */
  const listRef = useRef<HTMLDivElement>(null);
  const [marquee, setMarquee] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const dragStart = useRef<{ x: number; y: number } | null>(null);
  const marqueeBase = useRef<Set<string> | null>(null);

  const onPointerDown = (e: React.PointerEvent) => {
    // 仅在空白处按下才启动框选；卡片/控件上按下走点击与多选逻辑
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("[data-card], button, input, select, textarea")) return;
    e.stopPropagation();
    e.preventDefault();
    const additive = e.ctrlKey || e.metaKey || e.shiftKey;
    const base = additive && !e.shiftKey ? new Set(selected) : new Set<string>();
    if (e.shiftKey) base.forEach((id) => base.add(id));
    marqueeBase.current = base;
    dragStart.current = { x: e.clientX, y: e.clientY };
    setMarquee({ x: e.clientX, y: e.clientY, w: 0, h: 0 });

    const collect = (fromX: number, fromY: number, toX: number, toY: number) => {
      const x1 = Math.min(fromX, toX);
      const y1 = Math.min(fromY, toY);
      const x2 = Math.max(fromX, toX);
      const y2 = Math.max(fromY, toY);
      const hit = new Set(marqueeBase.current || []);
      listRef.current?.querySelectorAll<HTMLElement>("[data-card]").forEach((el) => {
        const r = el.getBoundingClientRect();
        const overlap = r.left < x2 && r.right > x1 && r.top < y2 && r.bottom > y1;
        if (overlap) hit.add(el.dataset.card!);
      });
      return hit;
    };

    const move = (ev: PointerEvent) => {
      if (!dragStart.current) return;
      const x = Math.min(dragStart.current.x, ev.clientX);
      const y = Math.min(dragStart.current.y, ev.clientY);
      const w = Math.abs(ev.clientX - dragStart.current.x);
      const h = Math.abs(ev.clientY - dragStart.current.y);
      setMarquee({ x, y, w, h });
      // 实时命中：选框覆盖到的卡片立即进入选中态
      if (w > 2 || h > 2) setSelected(collect(dragStart.current.x, dragStart.current.y, ev.clientX, ev.clientY));
    };
    const up = (ev: PointerEvent) => {
      const start = dragStart.current;
      dragStart.current = null;
      setMarquee(null);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (!start) return;
      // 位移过小视为点击空白：清空选择
      if (Math.abs(ev.clientX - start.x) < 4 && Math.abs(ev.clientY - start.y) < 4) {
        setSelected(new Set());
        return;
      }
      const hit = collect(start.x, start.y, ev.clientX, ev.clientY);
      setSelected(hit);
      // 框选过程中禁用后续 click 的原生拖拽/文本选择副作用
      window.addEventListener("click", (ce) => ce.stopPropagation(), { once: true, capture: true });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  /** 空白区域菜单项（全应用任意空白处右键均可用） */
  const blankMenuItems = useCallback((): MenuItem[] => {
    const ids = filtered.map((t) => t.id);
    const pausedCount = filtered.filter((t) => t.status === "paused").length;
    const allPaused = pausedCount > 0 && pausedCount === filtered.length;
    return [
      { label: allPaused ? "继续全部" : "开始全部", disabled: ids.length === 0, onClick: () => act("resume", "开始", ids) },
      { label: allPaused ? "暂停其余" : "暂停全部", disabled: ids.length === 0, onClick: () => act("pause", "暂停", ids) },
      { label: "停止全部", danger: true, disabled: ids.length === 0, onClick: () => act("cancel", "取消", ids) },
      { divider: true, label: "" },
      { label: `${allSelected ? "取消全选" : "全选"}（${filtered.length}）`, disabled: filtered.length === 0, onClick: toggleSelectAll },
      {
        label: `重试失败任务（${tasks.filter((t) => t.status === "failed").length}）`,
        disabled: !tasks.some((t) => t.status === "failed"),
        onClick: () => act("retry", "重试", tasks.filter((t) => t.status === "failed").map((t) => t.id)),
      },
      { divider: true, label: "" },
      {
        label: `移除已完成（${tasks.filter((t) => t.status === "completed").length}）`,
        disabled: !tasks.some((t) => t.status === "completed"),
        onClick: () => api.clearTasks(["completed"]).then(() => { push("已移除已完成任务", "info"); refresh(); }),
      },
      {
        label: "移除失败与取消",
        disabled: !tasks.some((t) => t.status === "failed" || t.status === "cancelled"),
        onClick: () => api.clearTasks(["failed", "cancelled"]).then(() => { push("已移除失败与取消任务", "info"); refresh(); }),
      },
      {
        label: "清空任务列表",
        danger: true,
        disabled: tasks.length === 0,
        onClick: () => api.clearTasks([]).then(() => { push("任务列表已清空", "info"); refresh(); }),
      },
    ];
  }, [filtered, allSelected, tasks, selectedPaused, act, toggleSelectAll, push, refresh]);

  const { registerBlankHandler } = useGlobalContextMenu() ?? {};
  useEffect(() => {
    if (!registerBlankHandler) return;
    return registerBlankHandler(() => {
      setSelected(new Set());
      return blankMenuItems();
    });
  }, [registerBlankHandler, blankMenuItems]);

  const openMenu = (e: React.MouseEvent, t: Task) => {
    e.preventDefault();
    e.stopPropagation();
    ensureSelected(t.id, e);
    const ids = selected.has(t.id) && selected.size > 1 ? [...selected] : [t.id];
    const scope = ids.length > 1 ? `选中 ${ids.length} 项` : fileName(t);
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: ids.length > 1 ? `开始 / 继续（${ids.length}）` : "开始 / 继续", onClick: () => act("resume", "开始", ids) },
        { label: ids.length > 1 ? `暂停（${ids.length}）` : "暂停", onClick: () => act("pause", "暂停", ids) },
        { label: ids.length > 1 ? `重试（${ids.length}）` : "重试", onClick: () => act("retry", "重试", ids) },
        {
          label: "修改参数…",
          disabled: t.status === "running" || t.status === "probing",
          onClick: () => setEditingId(t.id),
        },
        { label: ids.length > 1 ? `取消任务（${ids.length}）` : "取消任务", danger: true, onClick: () => act("cancel", "取消", ids) },
        { divider: true, label: "" },
        {
          label: "打开输出文件",
          disabled: !t.items[0]?.output,
          onClick: () => api.openFile(t.items[0]!.output!).catch((err) => push(String(err), "error")),
        },
        { label: "打开源目录", onClick: () => api.openInExplorer(t.items[0]?.input || "") },
        { label: "打开输出目录", disabled: !t.items[0]?.output, onClick: () => api.openInExplorer(t.items[0]!.output!) },
        { label: expanded.has(t.id) ? "收起明细与日志" : "展开明细与日志", onClick: () => toggleExpand(t.id) },
        { divider: true, label: "" },
        {
          label: "移除已完成任务",
          disabled: !tasks.some((x) => x.status === "completed"),
          onClick: () => api.clearTasks(["completed"]).then(() => { push("已移除已完成任务", "info"); refresh(); }),
        },
        { label: `删除（${scope}）`, danger: true, onClick: () => act("delete", "删除", ids) },
      ],
    });
  };

  return (
    <div
      className="h-full glass rounded-2xl p-3 flex flex-col min-h-0"
      onClick={(e) => {
        // 仅点击空白区域（非卡片）时取消选中
        if (!(e.target as HTMLElement).closest("[data-card]") && selected.size > 0) setSelected(new Set());
      }}
      onContextMenu={(e) => {
        // 卡片自身有独立菜单；空白区域由全局 Provider 统一处理
        if ((e.target as HTMLElement).closest("[data-card]")) return;
      }}
    >
      <div className="flex gap-2 mb-2">
        <button onClick={() => act("resume", "全部开始")} className="w-24 accent-grad text-white rounded-xl py-1.5 text-sm text-center hover:opacity-90">{selected.size > 0 ? "开始选中" : "全部开始"}</button>
        <button
          onClick={() => (selectedPaused ? act("resume", "继续") : act("pause", "暂停"))}
          className={`w-24 rounded-xl py-1.5 text-sm text-center text-white ${selectedPaused ? "bg-emerald-500/80 hover:bg-emerald-400" : "bg-amber-500/80 hover:bg-amber-400"}`}
        >
          {selectedPaused ? "继续选中" : selected.size > 0 ? "暂停选中" : "全部暂停"}
        </button>
        <button onClick={() => act("cancel", "停止")} className="w-24 bg-rose-500/80 hover:bg-rose-400 text-white rounded-xl py-1.5 text-sm text-center">{selected.size > 0 ? "停止选中" : "全部停止"}</button>
        <button onClick={() => api.clearTasks(["failed"]).then(() => { push("已移除失败任务", "info"); refresh(); })} className="w-24 bg-slate-500/15 hover:bg-slate-500/25 hover:bg-slate-500/25 rounded-xl py-1.5 text-sm text-center">移除失败</button>
        <button
          onClick={() =>
            selected.size > 0
              ? act("delete", "移除选中")
              : api.clearTasks([]).then(() => { push("任务列表已清空", "info"); refresh(); })
          }
          className="w-24 bg-slate-500/15 hover:bg-slate-500/25 hover:bg-slate-500/25 rounded-xl py-1.5 text-sm text-center"
        >
          {selected.size > 0 ? "移除选中" : "清空任务"}
        </button>
        <button
          onClick={toggleSelectAll}
          title={allSelected ? "取消全选" : "选中当前筛选的全部任务"}
          className={`ml-auto w-24 rounded-xl py-1.5 text-sm text-center transition ${allSelected ? "tab-active" : "bg-slate-500/15 hover:bg-slate-500/25 hover:bg-slate-500/25"}`}
        >
          {allSelected ? "取消全选" : "全选"}
        </button>
      </div>

      <div className="flex gap-1.5 mb-2 flex-wrap">
        {SUBTABS.map((s) => (
          <button key={s.key} onClick={() => setSub(s.key)} className={`px-3 py-1 rounded-xl text-sm ${sub === s.key ? "tab-active" : "hover:bg-slate-500/15 hover:bg-slate-500/25"}`}>
            {s.label}
          </button>
        ))}
      </div>

      <div
        ref={listRef}
        onPointerDown={onPointerDown}
        onClick={(e) => e.stopPropagation()}
        onScroll={() => setMarquee(null)}
        className="flex-1 overflow-auto flex flex-col gap-2.5 pr-1 pb-4 relative select-none"
      >
        {filtered.length === 0 && <p className="text-sm opacity-50 text-center mt-8">暂无任务</p>}
        {filtered.map((t) => {
          const pct = ((liveProgress[t.id] ?? t.progress) * 100).toFixed(0);
          const itemSize = formatSize(t.items[0]?.size);
          const addedAt = formatDateShort(t.created_at);
          const addedFull = formatDateTime(t.created_at);
          return (
            <motion.div
              layout
              key={t.id}
              data-card={t.id}
              onClick={(e) => selectCard(t.id, e)}
              onContextMenu={(e) => openMenu(e, t)}
              className={`card-shadow bg-slate-500/10 rounded-2xl px-3 py-1.5 cursor-pointer transition border ${selected.has(t.id) ? "card-sel" : "border-transparent hover:bg-slate-500/15 hover:bg-slate-500/25"}`}
            >
              <div className="flex items-stretch gap-2">
                <div className="w-8 flex-none flex items-center justify-center">
                  <FileKindIcon name={fileName(t)} size="w-6 h-6" />
                </div>
                <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <button onClick={(e) => { e.stopPropagation(); toggleExpand(t.id); }} title={expanded.has(t.id) ? "收起明细" : "展开明细（转码日志）"} aria-label="展开">
                  {expanded.has(t.id) ? <ChevronDown className="w-3.5 h-3.5 opacity-60" /> : <ChevronRight className="w-3.5 h-3.5 opacity-60" />}
                </button>
                <div className="flex-1 min-w-0">
                  {/* 全部在同一行，竖线分隔：
                      文件名 │ 大小 │ 添加日期 │ →输出格式(关键色) │ 📁目录
                      只有文件名可截断，其余项 flex-none 始终完整可见 */}
                  <div className="flex items-center gap-2 min-w-0 text-xs">
                    <p className={`font-medium truncate text-sm ${selected.has(t.id) ? "text-accent" : ""}`} title={t.items[0]?.input}>{fileName(t)}</p>
                    {itemSize && (
                      <>
                        <Sep />
                        <span className="flex-none opacity-60 tabular-nums" title={`源文件大小 ${itemSize}`}>
                          {itemSize}
                        </span>
                      </>
                    )}
                    {addedAt && (
                      <>
                        <Sep />
                        <span className="flex-none opacity-60 tabular-nums" title={`添加于 ${addedFull}`}>
                          {addedAt}
                        </span>
                      </>
                    )}
                    <Sep />
                    <span className="flex-none flex items-center gap-1">
                      <ArrowRight className="w-3 h-3 opacity-60" />
                      <span className="text-accent font-medium">{t.options.target_ext}</span>
                    </span>
                    <Sep />
                    <span className="flex-none flex items-center gap-1 opacity-60 min-w-0">
                      <FolderOutput className="w-3 h-3 flex-none" />
                      <span className="truncate">{t.output_dir.split(/[\\/]/).pop()}</span>
                    </span>
                  </div>
                </div>
                <span className={`text-xs px-2 py-0.5 rounded-full whitespace-nowrap ${STATUS_COLOR[t.status] || "bg-slate-500/15 hover:bg-slate-500/25"}`}>
                  {STATUS_LABEL[t.status] || t.status}
                </span>
                {t.error && <span className="text-xs text-rose-600 dark:text-rose-300">[{t.error.code}]</span>}
                <button
                  title="修改该任务的转换参数（模板与工作台一致）"
                  onClick={(e) => { e.stopPropagation(); setEditingId(t.id); }}
                  className="h-7 px-2 flex items-center gap-1 rounded-lg bg-slate-500/15 hover:bg-accent-soft hover:text-accent transition text-xs whitespace-nowrap"
                >
                  <Settings2 className="w-3.5 h-3.5" />修改
                </button>
                <button
                  title={(t.items[0]?.outputs?.length || 0) > 1 ? `打开共 ${t.items[0]!.outputs!.length} 个产物所在目录` : "打开输出文件（系统默认程序）"}
                  onClick={(e) => {
                    e.stopPropagation();
                    const out = t.items[0]?.output;
                    if (!out) return push("文件尚未生成", "info");
                    // 多产物（PDF 拆页）时主输出是目录，直接在资源管理器里看
                    if ((t.items[0]?.outputs?.length || 0) > 1) return api.openInExplorer(out);
                    api.openFile(out).catch((err) => push(String(err), "error"));
                  }}
                  disabled={!t.items[0]?.output}
                  className="h-7 px-2 flex items-center gap-1 rounded-lg bg-slate-500/15 hover:bg-accent-soft hover:text-accent disabled:opacity-35 disabled:hover:bg-slate-500/15 disabled:hover:text-inherit transition text-xs whitespace-nowrap"
                >
                  <ExternalLink className="w-3.5 h-3.5" />打开
                </button>
                <button
                  title="打开源目录"
                  onClick={(e) => { e.stopPropagation(); api.openInExplorer(t.items[0]?.input || ""); }}
                  className="h-7 w-7 flex items-center justify-center rounded-lg hover:bg-slate-500/20 transition"
                >
                  <FolderInput className="w-4 h-4 t-2" />
                </button>
                <button
                  title="打开输出目录"
                  disabled={!t.items[0]?.output}
                  onClick={(e) => { e.stopPropagation(); api.openInExplorer(t.items[0]!.output!); }}
                  className="h-7 w-7 flex items-center justify-center rounded-lg hover:bg-emerald-500/25 hover:text-emerald-600 dark:hover:text-emerald-300 disabled:opacity-30 disabled:hover:bg-transparent disabled:hover:text-inherit transition"
                >
                  <FolderOutput className="w-4 h-4" />
                </button>
                <button
                  title={t.status === "paused" ? "继续任务" : "暂停任务"}
                  onClick={(e) => { e.stopPropagation(); (t.status === "paused" ? api.resumeTask(t.id) : api.pauseTask(t.id)).then(refresh); }}
                  className="w-14 h-7 flex items-center justify-center gap-1 rounded-lg hover:bg-slate-500/20 bg-white/5 transition text-xs whitespace-nowrap"
                >
                  {t.status === "paused" ? <><Play className="w-3 h-3" />继续</> : <><Pause className="w-3 h-3" />暂停</>}
                </button>
                <button aria-label="取消" title="取消任务" onClick={(e) => { e.stopPropagation(); api.cancelTask(t.id).then(refresh); }} className="p-1 rounded-lg hover:bg-slate-500/20 text-xs"><X className="w-3.5 h-3.5" /></button>
                <button aria-label="重试" title="重试任务" onClick={(e) => { e.stopPropagation(); api.retryTask(t.id).then(refresh); }} className="p-1 rounded-lg hover:bg-slate-500/20 text-xs"><RotateCcw className="w-3.5 h-3.5" /></button>
                <button aria-label="删除" title="删除任务" onClick={(e) => { e.stopPropagation(); api.deleteTask(t.id).then(refresh); }} className="p-1 rounded-lg hover:bg-slate-500/20 text-xs"><Trash2 className="w-3.5 h-3.5" /></button>
              </div>

              <div className="flex items-center gap-2 mt-1">
                <span className="text-xs opacity-70 w-10 text-right tabular-nums">{pct}%</span>
                <div className="flex-1 h-1.5 bg-slate-500/15 hover:bg-slate-500/25 rounded-full overflow-hidden">
                  <motion.div
                    className="h-full accent-grad rounded-full"
                    animate={{ width: `${pct}%` }}
                    transition={{ type: "spring", stiffness: 120, damping: 20 }}
                  />
                </div>
              </div>
                </div>
              </div>

              {expanded.has(t.id) && (
                <div className="mt-2 flex flex-col gap-1" onClick={(e) => e.stopPropagation()}>
                  {t.items.map((it, i) => (
                    <div key={i} className="text-xs bg-slate-500/10 hover:bg-slate-500/20 transition rounded-xl px-3 py-1.5 group">
                      <div className="flex items-center gap-2">
                        <FileKindIcon name={it.input} size="w-3.5 h-3.5" />
                        <span className="truncate flex-1" title={it.input}>{it.input.split(/[\\/]/).pop()}</span>
                        <span className="opacity-60 w-16 text-center">{STATUS_LABEL[it.status] || it.status}</span>
                        <span className="opacity-60 w-10 text-right tabular-nums">{(it.progress * 100).toFixed(0)}%</span>
                        <button title="打开源目录" onClick={() => api.openInExplorer(it.input)} className="opacity-50 group-hover:opacity-100 hover:text-accent transition"><FolderInput className="w-3.5 h-3.5" /></button>
                        {it.output && <button title="打开输出目录" onClick={() => api.openInExplorer(it.output!)} className="opacity-50 group-hover:opacity-100 hover:text-emerald-600 dark:text-emerald-300 transition"><FolderOutput className="w-3.5 h-3.5" /></button>}
                        {(it.outputs?.length || 0) > 1 ? (
                           <button title={`打开共 ${it.outputs!.length} 个产物所在目录`} onClick={() => api.openInExplorer(it.output!)} className="opacity-50 group-hover:opacity-100 hover:text-accent transition"><ExternalLink className="w-3.5 h-3.5" /></button>
                         ) : (
                           <button title="打开输出文件" onClick={() => api.openFile(it.output!).catch(() => {})} className="opacity-50 group-hover:opacity-100 hover:text-accent transition"><ExternalLink className="w-3.5 h-3.5" /></button>
                         )}
                        {it.status === "failed" && <button title="重试此任务" onClick={() => api.retryTask(t.id).then(refresh)} className="underline">重试</button>}
                      </div>
                      {it.error && <p className="text-rose-600 dark:text-rose-300">{it.error.message}</p>}
                      {it.output && ((it.outputs?.length || 0) > 1 ? (
                           <p className="opacity-60 truncate" title={it.outputs!.join("\n")}>→ 共 {it.outputs!.length} 个文件：{it.output.split(/[\\/]/).pop()}</p>
                         ) : (
                           <p className="opacity-50 truncate" title={it.output}>→ {it.output}</p>
                         ))}
                    </div>
                  ))}
                  <div className="mt-1 bg-slate-900/40 dark:bg-black/30 rounded-xl p-2 max-h-40 overflow-auto font-mono text-[11px] opacity-80">
                    <p className="opacity-60 mb-1">转码日志</p>
                    {(liveLogs[t.id] || []).length === 0 ? <p className="opacity-50">暂无日志</p> : (liveLogs[t.id] || []).map((line, i) => <div key={i}>{line}</div>)}
                  </div>
                </div>
              )}
            </motion.div>
          );
        })}
      </div>

      {marquee &&
        createPortal(
          <div
            className="fixed z-[150] pointer-events-none rounded-lg border border-dashed"
            style={{
              left: marquee.x,
              top: marquee.y,
              width: marquee.w,
              height: marquee.h,
              borderColor: "rgb(var(--accent))",
              backgroundColor: "rgb(var(--accent) / 0.14)",
            }}
          />,
          document.body,
        )}

      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}

      {editing && <TaskEditDialog task={editing} onClose={() => setEditingId(null)} onSaved={refresh} />}
    </div>
  );
}

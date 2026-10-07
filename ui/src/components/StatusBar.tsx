import { useEffect, useRef, useState } from "react";
import { Cpu, MemoryStick, Activity, Terminal, Gauge, Zap } from "lucide-react";
import { api, isTauri } from "../lib/api";

interface Perf {
  cpu_usage: number;
  mem_used_mb: number;
  mem_total_mb: number;
  gpu_usage: number | null;
  gpu_encoder_usage: number | null;
  gpu_mem_used_mb: number | null;
  gpu_mem_total_mb: number | null;
  gpu_name: string | null;
  gpu_available: boolean;
  running: number;
  queued: number;
  progress: number;
}

/**
 * 监控区的一格指标
 *   固定宽度 + overflow-hidden + 文本 truncate：内容再长也只会被省略号截断，
 *   绝不会溢出压到相邻格（转换时 GPU 会出现「编码100%」「5120/5120MB」等长文本）
 */
function Metric({
  icon,
  width,
  title,
  children,
}: {
  icon: React.ReactNode;
  width: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <span className={`flex items-center gap-1.5 flex-none overflow-hidden ${width}`} title={title}>
      {icon}
      <span className="truncate">{children}</span>
    </span>
  );
}

export function StatusBar() {
  const [perf, setPerf] = useState<Perf | null>(null);
  const [log, setLog] = useState("");
  // 资源检测开关（持久化到项目本地）
  const [monitor, setMonitor] = useState(() => localStorage.getItem("sf-monitor") !== "off");
  const [tick, setTick] = useState(0); // 打开开关时立即触发一次采样
  const unlisten = useRef<(() => void)[]>([]);

  useEffect(() => {
    localStorage.setItem("sf-monitor", monitor ? "on" : "off");
    if (!monitor) return;
    if (isTauri()) setTick((t) => t + 1);
  }, [monitor]);

  useEffect(() => {
    if (!monitor || !isTauri()) {
      setPerf(null);
      return;
    }
    let alive = true;
    const timer = setInterval(async () => {
      const p = await api.perfStats().catch(() => null);
      if (alive && p) setPerf(p);
    }, 1200);
    api.perfStats().then((p) => alive && setPerf(p)).catch(() => {});
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [monitor, tick]);

  useEffect(() => {
    if (!isTauri()) return;
    import("@tauri-apps/api/event").then(async ({ listen }) => {
      unlisten.current.push(
        await listen<{ line: string }>("task://log", (e) => {
          setLog((prev) => (prev ? `${prev}  ·  ${e.payload.line}` : e.payload.line));
        }),
      );
    });
    return () => {
      unlisten.current.forEach((u) => u());
      unlisten.current = [];
    };
  }, []);

  const gpuText = () => {
    if (!perf) return "--";
    if (!perf.gpu_available) return "不可用";
    const u = perf.gpu_usage != null ? `${perf.gpu_usage.toFixed(0)}%` : "--";
    // 编码引擎占用才是「显卡是否真在干活」的直接证据（NVENC 走独立引擎）
    const enc = perf.gpu_encoder_usage != null && perf.gpu_encoder_usage > 0 ? ` · 编码${perf.gpu_encoder_usage.toFixed(0)}%` : "";
    if (perf.gpu_mem_used_mb != null && perf.gpu_mem_total_mb) {
      // 显存用 GB 简写，避免这一格过长把相邻指标挤掉
      const fmt = (mb: number) => (mb >= 1024 ? `${(mb / 1024).toFixed(1)}G` : `${Math.round(mb)}M`);
      return `${u}${enc} · ${fmt(perf.gpu_mem_used_mb)}/${fmt(perf.gpu_mem_total_mb)}`;
    }
    return `${u}${enc}`;
  };

  return (
    <div className="glass rounded-xl mx-3 mb-3 px-3 h-8 flex items-center gap-2.5 text-xs flex-none overflow-hidden whitespace-nowrap">
      <button
        onClick={() => setMonitor((v) => !v)}
        title={monitor ? "关闭资源检测（停止 CPU/GPU 采样）" : "开启资源检测"}
        className={`w-6 h-6 flex-none flex items-center justify-center rounded-lg transition ${
          monitor ? "bg-accent-soft text-accent" : "bg-slate-500/15 t-3 hover:bg-slate-500/25"
        }`}
      >
        <Gauge className="w-3.5 h-3.5" />
      </button>

      <Metric icon={<Cpu className="w-3.5 h-3.5 flex-none" />} width="w-[92px]" title="CPU 占用">
        CPU&nbsp;{monitor && perf ? `${perf.cpu_usage.toFixed(0)}%` : "--"}
      </Metric>
      <Metric icon={<MemoryStick className="w-3.5 h-3.5 flex-none" />} width="w-[140px]" title="内存占用">
        内存&nbsp;{monitor && perf ? `${perf.mem_used_mb}/${perf.mem_total_mb}` : "--"}&nbsp;MB
      </Metric>
      <Metric
        icon={<Zap className="w-3.5 h-3.5 flex-none" />}
        width="w-[224px]"
        title={perf?.gpu_name ? `GPU：${perf.gpu_name}（nvidia-smi，含视频编解码引擎占用）` : "GPU 占用（需 NVIDIA 显卡与 nvidia-smi）"}
      >
        GPU&nbsp;{monitor ? gpuText() : "--"}
      </Metric>
      <Metric icon={<Activity className="w-3.5 h-3.5 flex-none" />} width="w-[86px]" title="正在转换 / 等待中（排队 + 已暂停）任务数">
        任务&nbsp;{monitor && perf ? perf.running : 0}&nbsp;/&nbsp;{monitor && perf ? perf.queued : 0}
      </Metric>
      <span className="w-11 flex-none text-center tabular-nums overflow-hidden" title="整体进度">
        {monitor && perf ? (perf.progress * 100).toFixed(0) : 0}%
      </span>
      <span className="flex items-center gap-1.5 flex-1 min-w-0 overflow-hidden" title="最近操作日志">
        <Terminal className="w-3.5 h-3.5 flex-none" />
        <span className="truncate">{log || (monitor ? "等待操作…" : "资源检测已关闭")}</span>
      </span>
    </div>
  );
}
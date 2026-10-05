import { useState } from "react";
import { api, EnvReport } from "../../lib/api";
import { CheckCircle2, XCircle, RefreshCw, Zap } from "lucide-react";

export function EnvTab() {
  const [report, setReport] = useState<EnvReport | null>(null);
  const [loading, setLoading] = useState(false);

  const run = async () => {
    setLoading(true);
    try {
      setReport(await api.probeEnv());
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="h-full glass rounded-2xl p-4 overflow-auto">
      <div className="flex items-center gap-3 mb-4">
        <h2 className="text-lg font-semibold">环境检测</h2>
        <button onClick={run} disabled={loading} className="flex items-center gap-1.5 accent-grad hover:opacity-90 disabled:opacity-40 text-white rounded-xl px-3 py-1.5 text-sm">
          <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />重新检测
        </button>
      </div>
      {!report && <p className="text-sm opacity-50">点击「重新检测」开始</p>}
      {report && (
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-sm">
            <Card label="系统" value={report.os} />
            <Card label="CPU 占用" value={`${report.cpu_usage.toFixed(0)}%`} />
            <Card label="CPU 核心" value={String(report.cpu_cores)} />
            <Card label="内存" value={`${report.memory_used_mb} / ${report.memory_total_mb} MB`} />
            <Card label="临时目录可写" value={report.temp_dir_writable ? "是" : "否"} />
          </div>
          {report.gpu && (
            <div className="bg-white/5 rounded-2xl p-3 flex items-start gap-2">
              {report.gpu.available ? (
                <Zap className="w-5 h-5 text-amber-400 shrink-0" />
              ) : (
                <XCircle className="w-5 h-5 text-slate-400 shrink-0" />
              )}
              <div className="min-w-0">
                <p className="font-medium">GPU{report.gpu.name ? ` · ${report.gpu.name}` : ""}</p>
                <p className="text-xs opacity-60 truncate">
                  {report.gpu.available
                    ? `占用 ${report.gpu.usage ?? "--"}% · 显存 ${report.gpu.mem_used_mb ?? "--"}/${report.gpu.mem_total_mb ?? "--"} MB · ${report.gpu.source}`
                    : `${report.gpu.source} — ${report.gpu.suggestion}`}
                </p>
              </div>
            </div>
          )}
          {report.warnings.length > 0 && (
            <div className="text-sm text-amber-700 dark:text-amber-300">{report.warnings.join("；")}</div>
          )}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
            {report.tools.map((t) => (
              <div key={t.name} className="bg-slate-500/10 rounded-2xl p-3 flex items-start gap-2">
                {t.available ? <CheckCircle2 className="w-5 h-5 text-emerald-400 shrink-0" /> : <XCircle className="w-5 h-5 text-rose-400 shrink-0" />}
                <div className="min-w-0 flex-1">
                  <p className="font-medium flex items-center gap-1.5">
                    {t.name}
                    {t.source === "手动指定" && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-accent-soft text-accent">手动指定</span>
                    )}
                  </p>
                  <p className="text-xs opacity-60 truncate" title={t.path || ""}>{t.available ? t.version || t.path : t.suggestion}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function Card({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-slate-500/10 rounded-2xl p-3">
      <p className="text-xs opacity-60">{label}</p>
      <p className="font-medium">{value}</p>
    </div>
  );
}

import { useEffect, useState } from "react";
import { api } from "../../lib/api";

export function LogsTab() {
  const [logs, setLogs] = useState<[string, string, string][]>([]);
  const refresh = () => api.getLogs(300).then(setLogs).catch(() => {});
  useEffect(() => {
    refresh();
  }, []);
  return (
    <div className="h-full glass rounded-2xl p-4 overflow-auto font-mono text-xs">
      <div className="flex items-center justify-between mb-2">
        <h2 className="text-lg font-semibold font-sans">日志</h2>
        <button onClick={refresh} className="bg-slate-500/15 hover:bg-slate-500/25 rounded-xl px-3 py-1">刷新</button>
      </div>
      {logs.map((l, i) => (
        <div key={i} className="py-0.5 border-b border-white/5">
          <span className="opacity-50">{l[0]}</span> <span className="text-accent">[{l[1]}]</span> {l[2]}
        </div>
      ))}
    </div>
  );
}

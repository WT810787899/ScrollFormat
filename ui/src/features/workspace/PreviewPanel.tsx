import { useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, isTauri } from "../../lib/api";
import { Kind } from "../../lib/kinds";

const cache = new Map<string, { thumb_path: string | null; extra: any }>();
const LIMIT = 128;

export function PreviewPanel({ kind, path, onClose }: { kind: Kind; path: string | null; onClose?: () => void }) {
  const [data, setData] = useState<{ thumb_path: string | null; extra: any } | null>(null);
  const [loading, setLoading] = useState(false);
  const last = useRef<string | null>(null);

  useEffect(() => {
    if (!path || !isTauri()) return setData(null);
    if (last.current === path) return;
    last.current = path;
    const cached = cache.get(path);
    if (cached) return setData(cached);
    setLoading(true);
    api.getPreview(path)
      .then((d) => {
        if (cache.size >= LIMIT) {
          const first = cache.keys().next().value;
          if (first) cache.delete(first);
        }
        cache.set(path, d);
        setData(d);
      })
      .catch(() => setData(null))
      .finally(() => setLoading(false));
  }, [path]);

  // 卸载时清理缓存，避免泄漏
  useEffect(() => () => cache.clear(), []);

  return (
    <div className="glass rounded-2xl p-3 h-full flex flex-col min-h-0 overflow-hidden">
      {onClose && (
        <button onClick={onClose} className="text-xs bg-slate-500/15 hover:bg-slate-500/25 rounded-full px-3 py-1 hover:bg-slate-500/25 mb-2 flex-none self-end">
          关闭预览面板
        </button>
      )}
      {!path && <p className="text-sm opacity-50">点击左侧文件查看预览</p>}
      {loading && <p className="text-sm opacity-50">加载中…</p>}
      {data && (
        <div className="flex-1 min-h-0 overflow-auto flex flex-col gap-3 rounded-xl">
          {data.thumb_path && (
            <img src={convertFileSrc(data.thumb_path)} alt="预览" className="rounded-xl max-h-32 object-contain bg-black/20" />
          )}
          <pre className="text-xs opacity-80 bg-slate-500/10 rounded-xl p-2 whitespace-pre-wrap">{JSON.stringify(data.extra, null, 2)}</pre>
        </div>
      )}
    </div>
  );
}

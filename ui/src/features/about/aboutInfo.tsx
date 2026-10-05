import { ScrollText, ExternalLink } from "lucide-react";
import { useToasts } from "../../components/Toast";
import { api } from "../../lib/api";

// ═══════════════════════════════════════════════════════
// aboutInfo.tsx — 「关于」面板的全部文字信息
//   改文案 / 版本号 / 作者信息 / 功能清单，只改这个文件，
//   无需触碰 SettingsTab.tsx 与其它任何 UI 代码。
// ═══════════════════════════════════════════════════════

export const ABOUT = {
  name: "格式卷轴 ScrollFormat",
  slogan: "一卷纳万格，转换自天成",
  version: "0.3.0",
  stack: "Tauri 2 + Rust + React 19 + Tailwind",
  intro:
    "本地优先的多格式转换工具：所有转换都在你自己的机器上完成，文件不上传、不外发。" +
    "统一任务队列、格式识别、转换前预览、批量处理、环境自检与参数预设。",
  features: [
    "图片转换：质量 / 长边限制 / 按比例缩放 / 插值算法 / 透明区域填充",
    "音频转换：码率 / 采样率 / 声道 / 编码器 / EBU R128 音量归一化",
    "视频转换：CRF / 分辨率（含按比例）/ 帧率 / 编码器（含硬件加速）/ 音频轨参数",
    "任务队列：进度、取消、暂停继续、重试、持久化与崩溃恢复",
    "预设系统：内置高质量 / 体积优先 / 快速 / 兼容优先，支持自建与一键还原",
    "命名规则：前后缀、序号、时间戳，自动重命名 / 覆盖 / 跳过",
    "环境检测：ffmpeg、ffprobe、pandoc、LibreOffice、calibre、7z 自动或手动指定",
  ],
  oss:
    "开源说明：本项目基于 MIT 许可的开源组件构建（Tauri、wry、React、Tokio、rusqlite、image、sysinfo 等）。" +
    "音视频转换依赖 FFmpeg（ffmpeg / ffprobe），可随包分发 sidecar，也可自行安装并加入 PATH。",
  license: "MIT License",
  dataDir: "项目本地 .scrollformat/（数据库、设置、预设、UI 状态）",
  author: {
    label: "Bilibili：音乐制作王某人",
    url: "https://space.bilibili.com/80192671",
  },
};

export async function openUrl(url: string) {
  return api.openUrl(url);
}

export function AboutPanel() {
  const { push } = useToasts();
  const a = ABOUT;

  const openAuthor = async () => {
    try {
      await openUrl(a.author.url);
    } catch (e: any) {
      // 打不开时至少把链接复制到剪贴板
      navigator.clipboard?.writeText(a.author.url).catch(() => {});
      push(String(e), "error");
    }
  };

  return (
    <div className="flex flex-col gap-3 text-sm">
      {/* 头部：图标 + 名称 + 版本 */}
      <div className="flex items-center gap-3">
        <div className="w-11 h-11 rounded-2xl flex items-center justify-center bg-accent-soft text-accent flex-none">
          <ScrollText className="w-6 h-6" />
        </div>
        <div className="min-w-0">
          <p className="font-semibold text-sm truncate">{a.name}</p>
          <p className="text-[11px] t-3 truncate">版本 {a.version} · {a.stack}</p>
        </div>
      </div>

      <p className="text-[11px] t-3 italic">{a.slogan}</p>
      <p className="text-xs t-2 leading-relaxed">{a.intro}</p>

      <ul className="flex flex-col gap-1">
        {a.features.map((f, i) => (
          <li key={i} className="text-[11px] t-2 flex gap-1.5">
            <span className="text-accent flex-none">·</span>
            <span className="leading-relaxed">{f}</span>
          </li>
        ))}
      </ul>

      <div className="h-px bg-white/10" />

      {/* 作者 */}
      <div className="flex flex-col gap-0.5">
        <p className="text-[11px] font-semibold uppercase tracking-wider t-3">作者</p>
        <button
          onClick={openAuthor}
          title="打开主页"
          className="text-[11px] underline text-left w-fit inline-flex items-center gap-1 text-accent"
        >
          {a.author.label}
          <ExternalLink className="w-3 h-3" />
        </button>
        <p className="text-[11px] t-3 select-text break-all">{a.author.url}</p>
      </div>

      {/* 运行信息 */}
      <div className="grid grid-cols-1 gap-1 text-[11px]">
        <Row label="许可" value={a.license} />
        <Row label="数据目录" value={a.dataDir} />
        <Row label="外部工具" value="ffmpeg / ffprobe（自动检测或手动指定）" />
      </div>

      <p className="text-[11px] t-2 leading-relaxed">{a.oss}</p>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex gap-2">
      <span className="t-3 w-16 shrink-0">{label}</span>
      <span className="t-2 truncate" title={value}>{value}</span>
    </div>
  );
}
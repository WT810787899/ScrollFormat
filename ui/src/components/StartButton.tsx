import { Play, Zap } from "lucide-react";

/**
 * 顶栏「开始转换」按钮
 *   - 工作台 TAB：把当前文件按参数加入队列并开始
 *   - 任务列表 TAB：继续所有「已暂停 / 未开始」的任务（失败任务自动跳过）
 * 有待继续任务时按钮会呼吸发光并显示数量，点击手感：悬停扫光 + 下沉回弹
 */
export function StartButton({
  onStart,
  pendingTasks,
  stagedFiles,
  queueMode,
  className = "ml-10",
}: {
  onStart: () => void;
  /** 待继续的任务数（已暂停 + 排队中） */
  pendingTasks: number;
  /** 文件区已添加、尚未转化的文件数 */
  stagedFiles: number;
  /** 是否处于任务列表 TAB（决定行为与图标） */
  queueMode: boolean;
  className?: string;
}) {
  const badge = pendingTasks + stagedFiles;
  return (
    <span className={`${className} relative inline-flex flex-none`}>
      <button
        onClick={onStart}
        className={`group relative flex items-center gap-2 overflow-hidden rounded-xl px-5 py-2 text-sm font-medium text-white
          accent-grad sf-glow sf-glow-hover sf-glow-press
          transition-[filter,box-shadow,transform] duration-150
          before:content-[''] before:absolute before:inset-y-0 before:left-0 before:w-1/2 before:-skew-x-12
          before:bg-gradient-to-r before:from-transparent before:via-white/40 before:to-transparent
          before:translate-x-[-180%] group-hover:before:translate-x-[340%] before:transition-transform before:duration-700 before:ease-out
          ${badge > 0 ? "sf-start-pulse" : ""}`}
      >
        {queueMode ? (
          <Play className="w-4 h-4 transition-transform group-hover:scale-110" />
        ) : (
          <Zap className="w-4 h-4 transition-transform group-hover:scale-110 group-hover:rotate-12" />
        )}
        <span>开始转换</span>
      </button>
      {/* 角标绝对定位在按钮右上角：数字再大也不会撑长按钮 */}
      {badge > 0 && (
        <span
          className="pointer-events-none absolute -top-1.5 -right-1.5 min-w-[18px] h-[18px] px-1 grid place-items-center
            rounded-full bg-white text-[11px] font-bold tabular-nums leading-none
            text-[rgb(var(--accent))] ring-2 ring-[rgb(var(--accent))] shadow"
        >
          {badge > 99 ? "99+" : badge}
        </span>
      )}
    </span>
  );
}

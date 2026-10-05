import React from "react";

export function ParamField({
  label,
  hint,
  error,
  wide,
  children,
}: {
  label: string;
  hint?: string;
  error?: string | null;
  /** 宽控件（滑块、颜色+多开关等）：提示置于下方；否则置于控件右侧并右对齐 */
  wide?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className="w-full">
      <div className="flex items-center gap-2 w-full">
        <div className="w-20 shrink-0 text-xs t-2">{label}</div>
        <div className={`min-w-0 ${wide ? "flex-1" : "shrink"} ${error ? "ring-1 ring-rose-400/60 rounded-lg" : ""}`}>{children}</div>
        {/* 提示列：固定占位 + 右对齐，始终贴齐面板右边；无提示则不占位 */}
        {!wide && (hint || error) && (
          <div className="flex-1 min-w-0 flex justify-end">
            {(hint || error) && (
              <span
                className={`text-[11px] truncate text-right ${error ? "text-rose-500 dark:text-rose-300" : "t-3"}`}
                title={error || hint}
              >
                {error || hint}
              </span>
            )}
          </div>
        )}
      </div>
      {wide && hint && <p className="text-[11px] t-3 w-full text-right mt-0.5">{hint}</p>}
    </div>
  );
}

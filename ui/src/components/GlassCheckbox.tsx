import { Check } from "lucide-react";

export function GlassCheckbox({
  checked,
  onChange,
  label,
  title,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  title?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      title={title}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      className={`inline-flex items-center gap-1.5 select-none transition ${
        disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"
      } text-xs t-2`}
    >
      <span
        className={`w-3.5 h-3.5 rounded-[5px] flex items-center justify-center transition-all border ${
          checked
            ? "bg-accent border-accent shadow-[0_0_10px_-2px_rgb(var(--accent))]"
            : "bg-slate-500/15 border-white/20 hover:border-accent/60"
        }`}
      >
        {checked && <Check className="w-2.5 h-2.5 text-white" strokeWidth={3} />}
      </span>
      {label && <span>{label}</span>}
    </button>
  );
}
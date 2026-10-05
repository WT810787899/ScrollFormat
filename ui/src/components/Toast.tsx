import { create } from "zustand";

interface Toast {
  id: number;
  message: string;
  level: "success" | "error" | "info";
}

interface ToastState {
  toasts: Toast[];
  push: (message: string, level?: Toast["level"]) => void;
  remove: (id: number) => void;
}

let seq = 0;

export const useToasts = create<ToastState>((set) => ({
  toasts: [],
  push: (message, level = "info") => {
    const id = ++seq;
    set((s) => ({ toasts: [...s.toasts, { id, message, level }] }));
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), 3000);
  },
  remove: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

export function ToastHost() {
  const { toasts } = useToasts();
  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-[100] flex flex-col gap-2 items-center">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`px-4 py-2 rounded-xl text-sm shadow-lg ${
            t.level === "success" ? "bg-emerald-500/80 text-white" : t.level === "error" ? "bg-rose-500/80 text-white" : "bg-slate-700/90 text-white"
          }`}
        >
          {t.message}
        </div>
      ))}
    </div>
  );
}

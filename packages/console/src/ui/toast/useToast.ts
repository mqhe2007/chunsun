import { reactive, readonly } from "vue";

export type ToastSeverity = "success" | "error" | "warn" | "info";

export type ToastItem = {
  id: number;
  severity: ToastSeverity;
  title: string;
  life: number;
};

type ToastInput = {
  severity?: ToastSeverity;
  summary?: string;
  detail?: string;
  life?: number;
};

const state = reactive<{ items: ToastItem[]; last?: string; lastAt: number }>({
  items: [],
  lastAt: 0,
});

let nextId = 1;

function formatToastLine(title: string, detail?: string): string {
  const t = title.trim();
  const d = (detail ?? "").trim();
  if (t && d && t !== d) return `${t}，${d}`;
  return t || d;
}

function push(severity: ToastSeverity, title: string, detail?: string, life = 3000) {
  const text = formatToastLine(title, detail);
  const dedupeKey = `${severity}:${text}`;
  const now = Date.now();
  if (state.last === dedupeKey && now - state.lastAt < 3000) return;
  state.last = dedupeKey;
  state.lastAt = now;

  const id = nextId++;
  state.items.push({ id, severity, title: text, life });
  window.setTimeout(() => dismiss(id), life);
}

function dismiss(id: number) {
  const idx = state.items.findIndex(t => t.id === id);
  if (idx >= 0) state.items.splice(idx, 1);
}

export function useToast() {
  return {
    add(input: ToastInput) {
      push(
        input.severity ?? "info",
        input.summary ?? "",
        input.detail,
        input.life ?? 3000,
      );
    },
    success(title: string, detail?: string, life?: number) {
      push("success", title, detail, life);
    },
    error(title: string, detail?: string, life?: number) {
      push("error", title, detail, life);
    },
    warn(title: string, detail?: string, life?: number) {
      push("warn", title, detail, life);
    },
    info(title: string, detail?: string, life?: number) {
      push("info", title, detail, life);
    },
    dismiss,
  };
}

export const toastStore = readonly(state);

/** 供 axios 拦截器等非组件上下文使用 */
export const appToast = useToast();

import { reactive, readonly } from "vue";

export type ToastSeverity = "success" | "error" | "warn" | "info";

type ToastItem = {
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

const state = reactive<{ items: ToastItem[] }>({ items: [] });
let nextId = 1;

function formatToastLine(title: string, detail?: string): string {
  const t = title.trim();
  const d = (detail ?? "").trim();
  if (t && d && t !== d) return `${t}，${d}`;
  return t || d;
}

export function useSiteToast() {
  return {
    add(input: ToastInput) {
      const id = nextId++;
      const life = input.life ?? 3000;
      state.items.push({
        id,
        severity: input.severity ?? "info",
        title: formatToastLine(input.summary ?? "", input.detail),
        life,
      });
      window.setTimeout(() => {
        const idx = state.items.findIndex(t => t.id === id);
        if (idx >= 0) state.items.splice(idx, 1);
      }, life);
    },
  };
}

export const siteToastStore = readonly(state);

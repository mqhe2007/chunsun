import { reactive } from "vue";

export type ConfirmOptions = {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
};

type Pending = ConfirmOptions & {
  resolve: (value: boolean) => void;
};

const state = reactive<{ pending: Pending | null }>({ pending: null });

export function confirm(options: ConfirmOptions): Promise<boolean> {
  if (state.pending) {
    state.pending.resolve(false);
  }
  return new Promise(resolve => {
    state.pending = { ...options, resolve };
  });
}

export function acceptConfirm() {
  state.pending?.resolve(true);
  state.pending = null;
}

export function rejectConfirm() {
  state.pending?.resolve(false);
  state.pending = null;
}

export function useConfirmState() {
  return state;
}

import { create } from 'zustand';

export interface Toast {
  id: number;
  text: string;
  tone: 'info' | 'error';
  action?: { label: string; run: () => void };
}

interface ToastState {
  toasts: Toast[];
  push: (toast: Omit<Toast, 'id'>, ms?: number) => number;
  dismiss: (id: number) => void;
}

let next = 1;

export const useToasts = create<ToastState>()((set, get) => ({
  toasts: [],
  push: (toast, ms = 6000) => {
    const id = next++;
    set((s) => ({ toasts: [...s.toasts.slice(-3), { ...toast, id }] }));
    if (ms > 0) setTimeout(() => get().dismiss(id), ms);
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

export function toast(text: string, action?: Toast['action']): void {
  useToasts.getState().push({ text, tone: 'info', action });
}

export function toastError(error: unknown): void {
  const text =
    typeof error === 'object' && error && 'message' in error
      ? String((error as { message: unknown }).message)
      : String(error);
  useToasts.getState().push({ text, tone: 'error' }, 9000);
}

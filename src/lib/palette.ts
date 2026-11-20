import { create } from 'zustand';

interface PaletteState {
  open: boolean;
  initial: string;
  show: (initial?: string) => void;
  hide: () => void;
  toggle: () => void;
}

export const usePalette = create<PaletteState>()((set, get) => ({
  open: false,
  initial: '',
  show: (initial = '') => set({ open: true, initial }),
  hide: () => set({ open: false }),
  toggle: () => (get().open ? set({ open: false }) : set({ open: true, initial: '' })),
}));

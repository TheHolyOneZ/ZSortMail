import { create } from 'zustand';

export const useMailSearch = create<{ query: string; set: (query: string) => void }>()((set) => ({
  query: '',
  set: (query) => set({ query }),
}));

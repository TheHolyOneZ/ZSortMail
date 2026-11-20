import { create } from 'zustand';
import type { CoreEvent, LinkPhase, SyncPhase } from '@/lib/bindings';
import { toast } from '@/lib/toasts';

interface BackendState {
  link: { phase: LinkPhase | 'idle'; message: string };
  sync: { phase: SyncPhase; done: number; total: number };
  setLink: (phase: LinkPhase | 'idle', message?: string) => void;
  apply: (event: CoreEvent) => void;
}

export const useBackend = create<BackendState>()((set) => ({
  link: { phase: 'idle', message: '' },
  sync: { phase: 'idle', done: 0, total: 0 },
  setLink: (phase, message = '') => set({ link: { phase, message } }),
  apply: (event) => {
    switch (event.type) {
      case 'link_state':
        set({ link: { phase: event.phase, message: event.message } });
        break;
      case 'sync_progress':
        set({ sync: { phase: event.phase, done: event.done, total: event.total } });
        break;
      case 'notice':
        if (event.level === 'info') toast(event.text);
        else toast(event.text);
        break;
      default:
        break;
    }
  },
}));

import { emit } from '@tauri-apps/api/event';
import { mockIPC } from '@tauri-apps/api/mocks';
import type { CoreEvent } from '@/lib/bindings';

export function installWebBackend(): void {
  if (typeof window === 'undefined' || '__TAURI_INTERNALS__' in window) return;
  window.__ZSM_WEB__ = true;

  mockIPC(
    async (cmd, payload) => {
      const response = await fetch(`/ipc/${cmd}`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(payload ?? {}),
      });
      const body: unknown = await response.json().catch(() => null);
      if (!response.ok)
        throw body ?? { kind: 'internal', message: `Backend answered ${response.status}` };
      return body;
    },
    { shouldMockEvents: true },
  );

  const stream = new EventSource('/events');
  stream.onmessage = (message) => {
    try {
      const event = JSON.parse(message.data) as CoreEvent;
      void emit('zsm-event', event);
    } catch {}
  };
}

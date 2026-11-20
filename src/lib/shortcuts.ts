import { useEffect, useRef } from 'react';
import { type ActionId, bindingFor, matches } from '@/lib/keymap';
import { useUi } from '@/lib/store';

export function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  return (
    el instanceof HTMLInputElement ||
    el instanceof HTMLTextAreaElement ||
    el instanceof HTMLSelectElement ||
    el?.isContentEditable === true
  );
}

export interface Shortcut {
  key: string;
  mod?: boolean;
  whileTyping?: boolean;
  run: (event: KeyboardEvent) => void;
}

export function useShortcuts(shortcuts: Shortcut[], enabled = true): void {
  const latest = useRef(shortcuts);
  latest.current = shortcuts;
  useEffect(() => {
    if (!enabled) return;
    function onKey(event: KeyboardEvent) {
      for (const shortcut of latest.current) {
        const binding = shortcut.mod ? `Ctrl+${shortcut.key}` : shortcut.key;
        if (!matches(binding, event)) continue;
        if (isTyping(event.target) && !shortcut.whileTyping) continue;
        if (document.querySelector('dialog[open]')) continue;
        event.preventDefault();
        shortcut.run(event);
        return;
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [enabled]);
}

export function useActions(handlers: Partial<Record<ActionId, () => void>>, enabled = true): void {
  const keys = useUi((s) => s.keys);
  const ids = Object.keys(handlers) as ActionId[];
  useShortcuts(
    ids.map((id) => ({ key: bindingFor(keys, id), run: () => handlers[id]?.() })),
    enabled,
  );
}

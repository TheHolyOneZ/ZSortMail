import { type ReactNode, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '@/lib/cn';
import type { MenuItem } from './Menu';

interface Open {
  x: number;
  y: number;
  items: MenuItem[];
}

export function useContextMenu(): {
  onContextMenu: (items: () => MenuItem[] | Promise<MenuItem[]>) => (e: React.MouseEvent) => void;
  menu: ReactNode;
} {
  const [open, setOpen] = useState<Open | null>(null);
  const onContextMenu = useCallback(
    (items: () => MenuItem[] | Promise<MenuItem[]>) => (e: React.MouseEvent) => {
      const t = e.target as HTMLElement;
      if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t.isContentEditable)
        return;
      e.preventDefault();
      e.stopPropagation();
      const { clientX: x, clientY: y } = e;
      void Promise.resolve(items()).then((list) => {
        if (list.length > 0) setOpen({ x, y, items: list });
      });
    },
    [],
  );
  const menu = open ? <Panel open={open} onClose={() => setOpen(null)} /> : null;
  return { onContextMenu, menu };
}

function Panel({ open, onClose }: { open: Open; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState({ x: open.x, y: open.y });
  const enabled = open.items.map((i, n) => (i.disabled ? -1 : n)).filter((n) => n >= 0);
  const [active, setActive] = useState(enabled[0] ?? 0);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setAt({
      x: Math.max(8, Math.min(open.x, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(open.y, window.innerHeight - r.height - 8)),
    });
    el.focus();
  }, [open]);

  useEffect(() => {
    function away(e: Event) {
      if (!ref.current?.contains(e.target as Node)) onClose();
    }
    document.addEventListener('pointerdown', away, true);
    window.addEventListener('blur', onClose);
    window.addEventListener('resize', onClose);
    document.addEventListener('scroll', onClose, true);
    return () => {
      document.removeEventListener('pointerdown', away, true);
      window.removeEventListener('blur', onClose);
      window.removeEventListener('resize', onClose);
      document.removeEventListener('scroll', onClose, true);
    };
  }, [onClose]);

  function choose(i: number) {
    const item = open.items[i];
    if (!item || item.disabled) return;
    onClose();
    item.onSelect();
  }

  return createPortal(
    <div
      ref={ref}
      role="menu"
      tabIndex={-1}
      aria-label="Actions"
      style={{ left: at.x, top: at.y }}
      onContextMenu={(e) => e.preventDefault()}
      onKeyDown={(e) => {
        const at = enabled.indexOf(active);
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          setActive(enabled[(at + 1) % enabled.length] ?? 0);
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          setActive(enabled[(at - 1 + enabled.length) % enabled.length] ?? 0);
        } else if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          choose(active);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          onClose();
        }
      }}
      className="fixed z-50 min-w-[210px] rounded-pop bg-sheet py-1 shadow-pop outline-none"
    >
      {open.items.map((item, i) => (
        <div key={item.label} role="none">
          {item.separated && <hr className="my-1 h-px border-0 bg-rule" />}
          <button
            type="button"
            role="menuitem"
            tabIndex={-1}
            disabled={item.disabled}
            onMouseEnter={() => !item.disabled && setActive(i)}
            onClick={() => choose(i)}
            className={cn(
              'flex w-full items-center gap-6 px-3 py-1.5 text-left text-ui',
              item.danger ? 'text-red' : 'text-text',
              active === i && 'bg-hover',
              item.disabled && 'opacity-45',
            )}
          >
            <span className="flex-1">{item.label}</span>
            {item.hint && <span className="text-meta text-text-2">{item.hint}</span>}
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}

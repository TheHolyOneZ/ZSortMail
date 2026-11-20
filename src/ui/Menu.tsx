import {
  type KeyboardEvent,
  type ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { cn } from '@/lib/cn';

export interface MenuItem {
  label: string;
  hint?: string;
  onSelect: () => void;
  danger?: boolean;
  separated?: boolean;
  disabled?: boolean;
}

export function Menu({
  trigger,
  items,
  label,
  align = 'end',
  side = 'down',
  triggerClassName,
  className,
}: {
  trigger: ReactNode;
  items: MenuItem[];
  label: string;
  align?: 'start' | 'end';
  side?: 'down' | 'up';
  triggerClassName?: string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const id = useId();
  const enabled = items.map((item, i) => (item.disabled ? -1 : i)).filter((i) => i >= 0);

  useEffect(() => {
    if (!open) return;
    function onPointer(e: PointerEvent) {
      const target = e.target as Node;
      if (!panel.current?.contains(target) && !button.current?.contains(target)) setOpen(false);
    }
    document.addEventListener('pointerdown', onPointer);
    return () => document.removeEventListener('pointerdown', onPointer);
  }, [open]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: the highlight resets only when the menu opens.
  useLayoutEffect(() => {
    if (open) {
      setActive(enabled[0] ?? 0);
      panel.current?.focus();
    }
  }, [open]);

  function close(refocus = true) {
    setOpen(false);
    if (refocus) button.current?.focus();
  }

  function onKeyDown(e: KeyboardEvent) {
    const at = enabled.indexOf(active);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActive(enabled[(at + 1) % enabled.length] ?? 0);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive(enabled[(at - 1 + enabled.length) % enabled.length] ?? 0);
    } else if (e.key === 'Home') {
      e.preventDefault();
      setActive(enabled[0] ?? 0);
    } else if (e.key === 'End') {
      e.preventDefault();
      setActive(enabled[enabled.length - 1] ?? 0);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      const item = items[active];
      if (item && !item.disabled) {
        close();
        item.onSelect();
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'Tab') {
      close(false);
    }
  }

  return (
    <div className={cn('relative', className)}>
      <button
        ref={button}
        type="button"
        aria-label={label}
        title={label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => setOpen((v) => !v)}
        className={cn(
          'inline-flex size-8 items-center justify-center rounded-control text-text-2 transition-colors hover:bg-hover hover:text-text active:bg-press',
          triggerClassName,
        )}
      >
        {trigger}
      </button>
      {open && (
        <div
          ref={panel}
          id={id}
          role="menu"
          aria-label={label}
          tabIndex={-1}
          onKeyDown={onKeyDown}
          className={cn(
            'absolute z-30 min-w-[200px] rounded-pop bg-sheet py-1 shadow-lift outline-none',
            side === 'down' ? 'top-full mt-1' : 'bottom-full mb-1',
            align === 'end' ? 'right-0' : 'left-0',
          )}
        >
          {items.map((item, i) => (
            <div key={item.label} role="none">
              {item.separated && <hr className="my-1 h-px border-0 bg-rule" />}
              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                disabled={item.disabled}
                onMouseEnter={() => !item.disabled && setActive(i)}
                onClick={() => {
                  close();
                  item.onSelect();
                }}
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
        </div>
      )}
    </div>
  );
}

import { CaretDown, Check } from '@phosphor-icons/react';
import { type ReactNode, useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '@/lib/cn';

export interface SelectOption<T extends string | number> {
  value: T;
  label: string;
  hint?: string;
  leading?: ReactNode;
  disabled?: boolean;
}

export function Select<T extends string | number>({
  value,
  onChange,
  options,
  label,
  placeholder = 'Choose',
  className,
  align = 'start',
  size = 'md',
}: {
  value: T;
  onChange: (value: T) => void;
  options: SelectOption<T>[];
  label: string;
  placeholder?: string;
  className?: string;
  align?: 'start' | 'end';
  size?: 'sm' | 'md';
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const typed = useRef({ text: '', at: 0 });
  const id = useId();
  const [box, setBox] = useState<{
    left: number;
    top?: number;
    bottom?: number;
    width: number;
    max: number;
  }>({
    left: 0,
    width: 0,
    max: 280,
  });
  const selectedIndex = options.findIndex((o) => o.value === value);
  const selected = options[selectedIndex];

  function show() {
    setActive(Math.max(selectedIndex, 0));
    setOpen(true);
  }

  useLayoutEffect(() => {
    if (!open) return;
    function place() {
      if (!button.current) return;
      const r = button.current.getBoundingClientRect();
      if (r.bottom < 0 || r.top > window.innerHeight) {
        setOpen(false);
        return;
      }
      const below = window.innerHeight - r.bottom - 12;
      const above = r.top - 12;
      const up = below < 180 && above > below;
      const width = Math.max(r.width, 200);
      setBox({
        left: Math.max(
          8,
          Math.min(align === 'end' ? r.right - width : r.left, window.innerWidth - width - 8),
        ),
        top: up ? undefined : r.bottom + 4,
        bottom: up ? window.innerHeight - r.top + 4 : undefined,
        width,
        max: Math.min(320, Math.max(120, up ? above : below)),
      });
    }
    place();
    list.current?.focus({ preventScroll: true });
    window.addEventListener('scroll', place, true);
    window.addEventListener('resize', place);
    return () => {
      window.removeEventListener('scroll', place, true);
      window.removeEventListener('resize', place);
    };
  }, [open, align]);

  useEffect(() => {
    if (!open) return;
    function away(e: Event) {
      const t = e.target as Node;
      if (!list.current?.contains(t) && !button.current?.contains(t)) setOpen(false);
    }
    document.addEventListener('pointerdown', away, true);
    return () => document.removeEventListener('pointerdown', away, true);
  }, [open]);

  useEffect(() => {
    if (open) document.getElementById(`${id}-${active}`)?.scrollIntoView({ block: 'nearest' });
  }, [open, active, id]);

  function choose(i: number) {
    const o = options[i];
    if (!o || o.disabled) return;
    onChange(o.value);
    setOpen(false);
    button.current?.focus();
  }

  const enabled = options.map((o, i) => (o.disabled ? -1 : i)).filter((i) => i >= 0);
  function move(by: number) {
    const at = enabled.indexOf(active);
    setActive(enabled[Math.min(enabled.length - 1, Math.max(0, at + by))] ?? 0);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      move(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      move(-1);
    } else if (e.key === 'Home') {
      e.preventDefault();
      setActive(enabled[0] ?? 0);
    } else if (e.key === 'End') {
      e.preventDefault();
      setActive(enabled[enabled.length - 1] ?? 0);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      choose(active);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      button.current?.focus();
    } else if (e.key === 'Tab') {
      setOpen(false);
    } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) {
      const t = typed.current;
      const nowMs = Date.now();
      t.text = nowMs - t.at > 600 ? e.key.toLowerCase() : t.text + e.key.toLowerCase();
      t.at = nowMs;
      const hit = options.findIndex(
        (o, i) =>
          !o.disabled &&
          o.label.toLowerCase().startsWith(t.text) &&
          (t.text.length > 1 || i > active),
      );
      const first =
        hit >= 0
          ? hit
          : options.findIndex((o) => !o.disabled && o.label.toLowerCase().startsWith(t.text));
      if (first >= 0) setActive(first);
    }
  }

  return (
    <>
      <button
        ref={button}
        type="button"
        role="combobox"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={(e) => {
          if (!open && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
            e.preventDefault();
            show();
          }
        }}
        className={cn(
          'select-trigger inline-flex w-full items-center justify-between gap-2 rounded-control border border-transparent bg-hover text-left text-text transition-colors hover:border-rule-strong',
          size === 'sm' ? 'h-8 px-2.5 text-ui' : 'h-9 px-3 text-body',
          open && 'border-blue',
          className,
        )}
      >
        <span className="flex min-w-0 items-center gap-2">
          {selected?.leading}
          <span dir="auto" className={cn('truncate', !selected && 'text-text-2')}>
            {selected?.label ?? placeholder}
          </span>
        </span>
        <CaretDown
          size={12}
          weight="bold"
          className={cn('shrink-0 text-text-2 transition-transform', open && 'rotate-180')}
        />
      </button>
      {open &&
        createPortal(
          <div
            ref={list}
            id={id}
            role="listbox"
            aria-label={label}
            tabIndex={-1}
            aria-activedescendant={`${id}-${active}`}
            onKeyDown={onKeyDown}
            style={{
              left: box.left,
              top: box.top,
              bottom: box.bottom,
              width: box.width,
              maxHeight: box.max,
            }}
            className="select-list fixed z-[60] overflow-auto rounded-pop bg-sheet py-1 shadow-pop outline-none"
          >
            {options.map((o, i) => (
              // biome-ignore lint/a11y/useKeyWithClickEvents: the listbox owns the keyboard (arrows, Enter); the row is a pointer shortcut.
              <div
                key={String(o.value)}
                id={`${id}-${i}`}
                role="option"
                aria-selected={o.value === value}
                aria-disabled={o.disabled}
                tabIndex={-1}
                onMouseEnter={() => !o.disabled && setActive(i)}
                onClick={() => choose(i)}
                className={cn(
                  'flex cursor-pointer items-center gap-2.5 px-3 py-1.5',
                  active === i && 'bg-hover',
                  o.disabled && 'cursor-default opacity-45',
                )}
              >
                {o.leading}
                <span className="min-w-0 flex-1">
                  <span dir="auto" className="block truncate text-ui font-medium">
                    {o.label}
                  </span>
                  {o.hint && <span className="block truncate text-meta text-text-2">{o.hint}</span>}
                </span>
                {o.value === value && (
                  <Check size={14} weight="bold" className="shrink-0 text-blue" />
                )}
              </div>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}

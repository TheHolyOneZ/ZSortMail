import { type KeyboardEvent, type ReactNode, useRef } from 'react';
import { cn } from '@/lib/cn';

export interface TabItem<T extends string> {
  value: T;
  label: string;
  count?: number;
}

export function Tabs<T extends string>({
  value,
  onChange,
  items,
  label,
  className,
  panelId,
}: {
  value: T;
  onChange: (value: T) => void;
  items: TabItem<T>[];
  label: string;
  className?: string;
  panelId: string;
}): ReactNode {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  function onKeyDown(e: KeyboardEvent, index: number) {
    const last = items.length - 1;
    let next = index;
    if (e.key === 'ArrowRight') next = index === last ? 0 : index + 1;
    else if (e.key === 'ArrowLeft') next = index === 0 ? last : index - 1;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = last;
    else return;
    e.preventDefault();
    const item = items[next];
    if (item) {
      onChange(item.value);
      refs.current[next]?.focus();
    }
  }

  return (
    <div role="tablist" aria-label={label} className={cn('flex gap-1', className)}>
      {items.map((item, i) => {
        const selected = item.value === value;
        return (
          <button
            key={item.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="tab"
            id={`${panelId}-tab-${item.value}`}
            aria-selected={selected}
            aria-controls={panelId}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(item.value)}
            onKeyDown={(e) => onKeyDown(e, i)}
            className={cn(
              'inline-flex h-7 items-center gap-1.5 rounded-full px-3 text-ui font-medium transition-colors',
              selected ? 'bg-text text-desk' : 'text-text-2 hover:bg-hover hover:text-text',
            )}
          >
            {item.label}
            {item.count !== undefined && item.count > 0 && (
              <span className={cn('tabular text-meta', selected ? 'opacity-70' : 'text-text-2')}>
                {item.count}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

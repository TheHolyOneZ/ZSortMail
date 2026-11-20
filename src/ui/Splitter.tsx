import { useRef, useState } from 'react';
import { cn } from '@/lib/cn';

export function Splitter({
  value,
  onChange,
  onDefault,
  min,
  max,
  label,
  className,
  edge = 'right',
  style,
  onDragging,
}: {
  value: number;
  onChange: (width: number) => void;
  onDefault: () => void;
  min: number;
  max: number;
  label: string;
  className?: string;
  edge?: 'left' | 'right';
  style?: React.CSSProperties;
  onDragging?: (dragging: boolean) => void;
}) {
  const [dragging, setDragging] = useState(false);
  const start = useRef({ x: 0, width: 0 });
  const clamp = (n: number) => Math.round(Math.min(max, Math.max(min, n)));

  return (
    // biome-ignore lint/a11y/useSemanticElements: an <hr> cannot take focus or the arrow keys; this is the pattern for a window splitter.
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      tabIndex={0}
      style={style}
      onDoubleClick={onDefault}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        start.current = { x: e.clientX, width: value };
        setDragging(true);
        onDragging?.(true);
      }}
      onPointerMove={(e) => {
        if (!dragging) return;
        const dx = e.clientX - start.current.x;
        onChange(clamp(start.current.width + (edge === 'right' ? dx : -dx)));
      }}
      onPointerUp={(e) => {
        e.currentTarget.releasePointerCapture(e.pointerId);
        setDragging(false);
        onDragging?.(false);
      }}
      onKeyDown={(e) => {
        const step = e.shiftKey ? 48 : 16;
        const sign = edge === 'right' ? 1 : -1;
        if (e.key === 'ArrowRight') onChange(clamp(value + sign * step));
        else if (e.key === 'ArrowLeft') onChange(clamp(value - sign * step));
        else if (e.key === 'Home') onChange(min);
        else if (e.key === 'End') onChange(max);
        else if (e.key === 'Enter') onDefault();
        else return;
        e.preventDefault();
      }}
      className={cn(
        'group absolute inset-y-0 z-30 w-2 cursor-col-resize touch-none outline-none',
        className,
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          'absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-transparent transition-[background-color,width] group-hover:w-0.5 group-hover:bg-blue group-focus-visible:w-0.5 group-focus-visible:bg-blue',
          dragging && 'w-0.5 bg-blue',
        )}
      />
    </div>
  );
}

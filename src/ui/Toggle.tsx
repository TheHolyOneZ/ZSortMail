import { cn } from '@/lib/cn';

export function Toggle({
  checked,
  onChange,
  label,
  className,
  disabled,
}: {
  checked: boolean;
  onChange: (on: boolean) => void;
  label: string;
  className?: string;
  disabled?: boolean;
}) {
  return (
    <span
      className={cn('toggle relative inline-flex h-[26px] w-[46px] shrink-0', className)}
      data-on={checked}
    >
      <input
        type="checkbox"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
        className="toggle-input peer absolute inset-0 z-10 m-0 size-full cursor-pointer opacity-0 disabled:cursor-default"
      />
      <span className="toggle-track pointer-events-none absolute inset-0 rounded-full bg-rule-strong transition-colors duration-200 peer-checked:bg-blue peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-blue peer-disabled:opacity-45" />
      <span className="toggle-knob pointer-events-none absolute top-[3px] left-[3px] size-[20px] rounded-full bg-sheet shadow-[0_1px_3px_rgb(0_0_0/0.35)] transition-transform duration-200 ease-[cubic-bezier(0.3,0,0.2,1)] peer-checked:translate-x-[20px] peer-disabled:opacity-45" />
    </span>
  );
}

import { MagnifyingGlass, X } from '@phosphor-icons/react';
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode } from 'react';
import { cn } from '@/lib/cn';

type ButtonVariant = 'primary' | 'paperPrimary' | 'quiet' | 'ink';

const variants: Record<ButtonVariant, string> = {
  primary: 'bg-blue text-on-blue hover:brightness-110 active:brightness-95',
  paperPrimary: 'bg-paper-blue text-on-paper-blue hover:brightness-125 active:brightness-110',
  quiet: 'text-text hover:bg-hover active:bg-press',
  ink: 'bg-text text-desk hover:brightness-125 active:brightness-110',
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
}

export function Button({ variant = 'quiet', className, type = 'button', ...props }: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        'inline-flex h-8 shrink-0 items-center justify-center gap-2 rounded-control px-3 text-ui font-medium whitespace-nowrap',
        'transition-[background-color,filter] disabled:pointer-events-none disabled:opacity-45',
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
}

export function IconButton({
  label,
  className,
  type = 'button',
  children,
  ...props
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={cn(
        'inline-flex size-8 items-center justify-center rounded-control text-text-2',
        'transition-colors hover:bg-hover hover:text-text active:bg-press disabled:opacity-45',
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}

export function Kbd({
  children,
  className,
  tone = 'desk',
}: {
  children: ReactNode;
  className?: string;
  tone?: 'desk' | 'paper';
}) {
  return (
    <kbd
      className={cn(
        'inline-flex h-[18px] min-w-[18px] items-center justify-center rounded-[4px] border px-1',
        'font-ui text-meta font-medium',
        tone === 'paper'
          ? 'border-paper-rule bg-paper text-paper-text-2'
          : 'border-rule-strong bg-sheet text-text-2',
        className,
      )}
    >
      {children}
    </kbd>
  );
}

export interface ChipProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  active?: boolean;
  count?: number;
}

export function Chip({
  active = false,
  count,
  className,
  children,
  type = 'button',
  ...props
}: ChipProps) {
  return (
    <button
      type={type}
      aria-pressed={active}
      className={cn(
        'inline-flex h-7 shrink-0 items-center gap-1.5 rounded-full border px-2.5 text-meta font-medium',
        'transition-colors',
        active
          ? 'border-text bg-text text-desk'
          : 'border-rule-strong text-text-2 hover:bg-hover hover:text-text',
        className,
      )}
      {...props}
    >
      {children}
      {count !== undefined && count > 0 && (
        <span className={cn('tabular text-meta', active ? 'opacity-70' : 'text-text-2')}>
          {count}
        </span>
      )}
    </button>
  );
}

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
}

export function Segmented<T extends string>({
  value,
  onChange,
  options,
  label,
  className,
}: {
  value: T;
  onChange: (value: T) => void;
  options: SegmentedOption<T>[];
  label: string;
  className?: string;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className={cn('inline-flex rounded-control bg-hover p-0.5', className)}
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          // biome-ignore lint/a11y/useSemanticElements: a styled button is the control; a native radio cannot be styled like this.
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(option.value)}
            className={cn(
              'h-7 rounded-[6px] px-3 text-ui font-medium transition-colors',
              selected ? 'bg-sheet text-text shadow-control' : 'text-text-2 hover:text-text',
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

export interface SearchFieldProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'value'> {
  value: string;
  onChange: (value: string) => void;
  label: string;
}

export function SearchField({ value, onChange, label, className, ...props }: SearchFieldProps) {
  return (
    <label className={cn('relative block', className)}>
      <span className="sr-only">{label}</span>
      <MagnifyingGlass
        size={16}
        className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-text-2"
      />
      <input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={label}
        autoComplete="off"
        spellCheck={false}
        className={cn(
          'h-8 w-full rounded-control bg-hover pr-8 pl-8 text-ui text-text outline-none',
          'placeholder:text-text-2 focus-visible:outline-2 focus-visible:outline-blue',
          '[&::-webkit-search-cancel-button]:hidden',
        )}
        {...props}
      />
      {value && (
        <button
          type="button"
          aria-label="Clear search"
          onClick={() => onChange('')}
          className="absolute top-1/2 right-1.5 grid size-5 -translate-y-1/2 place-items-center rounded-full text-text-2 hover:bg-press hover:text-text"
        >
          <X size={12} weight="bold" />
        </button>
      )}
    </label>
  );
}

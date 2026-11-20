import { X } from '@phosphor-icons/react';
import type { FormEvent, ReactNode } from 'react';
import { cn } from '@/lib/cn';

export function Modal({
  title,
  description,
  children,
  footer,
  onClose,
  onSubmit,
  tone = 'sheet',
  className,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  onClose: () => void;
  onSubmit?: (e: FormEvent) => void;
  tone?: 'sheet';
  className?: string;
}) {
  const Root = (onSubmit ? 'form' : 'div') as 'form';
  return (
    <Root
      {...(onSubmit ? { onSubmit } : {})}
      data-tone={tone}
      className={cn('modal flex max-h-[min(88vh,820px)] flex-col overflow-hidden', className)}
    >
      <div className="composer-stripe shrink-0" aria-hidden="true" />
      <header className="flex shrink-0 items-start gap-4 px-6 pt-5 pb-3">
        <div className="min-w-0 flex-1">
          <h2 className="text-name font-semibold tracking-[-0.01em]">{title}</h2>
          {description && <p className="mt-1 max-w-[60ch] text-ui text-text-2">{description}</p>}
        </div>
        <button
          type="button"
          aria-label="Close"
          title="Close (Esc)"
          onClick={onClose}
          className="-me-2 -mt-1 grid size-8 shrink-0 place-items-center rounded-control text-text-2 transition-colors hover:bg-hover hover:text-text"
        >
          <X size={16} />
        </button>
      </header>
      <div className="scroll-y min-h-0 flex-1 px-6 pb-5">{children}</div>
      {footer && (
        <footer className="flex shrink-0 items-center justify-end gap-2 border-t border-rule bg-desk/60 px-6 py-3">
          {footer}
        </footer>
      )}
    </Root>
  );
}

export function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    // biome-ignore lint/a11y/noLabelWithoutControl: the control is passed in as children, inside this label.
    <label className={cn('block', className)}>
      <span className="block text-ui font-medium">{label}</span>
      <span className="mt-1 block">{children}</span>
      {hint && <span className="mt-1 block text-meta text-text-2">{hint}</span>}
    </label>
  );
}

export function Section({
  title,
  hint,
  children,
  className,
}: {
  title: string;
  hint?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <fieldset className={cn('mt-6 first:mt-1', className)}>
      <legend className="text-ui font-semibold">{title}</legend>
      {hint && <p className="mt-0.5 text-meta text-text-2">{hint}</p>}
      <div className="mt-2.5">{children}</div>
    </fieldset>
  );
}

export function Choice({
  type = 'checkbox',
  checked,
  onChange,
  title,
  hint,
  name,
  children,
}: {
  type?: 'checkbox' | 'radio';
  checked: boolean;
  onChange: (on: boolean) => void;
  title: string;
  hint?: ReactNode;
  name?: string;
  children?: ReactNode;
}) {
  return (
    <label
      className={cn(
        'flex cursor-pointer items-start gap-3 rounded-control border px-3 py-2.5 transition-colors',
        checked ? 'border-blue bg-blue-wash' : 'border-rule-strong hover:bg-hover',
      )}
    >
      <input
        type={type}
        name={name}
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-1 size-4 shrink-0"
      />
      <span className="min-w-0 flex-1">
        <span className="block text-ui font-medium">{title}</span>
        {hint && <span className="block text-meta text-text-2">{hint}</span>}
        {children}
      </span>
    </label>
  );
}

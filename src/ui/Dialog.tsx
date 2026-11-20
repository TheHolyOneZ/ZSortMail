import { type ReactNode, useEffect, useRef } from 'react';
import { cn } from '@/lib/cn';

export function Dialog({
  open,
  onClose,
  label,
  children,
  className,
  placement = 'center',
}: {
  open: boolean;
  onClose: () => void;
  label: string;
  children: ReactNode;
  className?: string;
  placement?: 'center' | 'top';
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: Escape already closes a modal dialog natively; clicking the backdrop is a pointer shortcut for the same thing.
    <dialog
      ref={ref}
      aria-label={label}
      onClose={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
      className={cn(
        'zsm-dialog m-auto w-[min(92vw,var(--dialog-w,520px))]',
        placement === 'top' && 'mt-[12vh] mb-auto',
        className,
      )}
    >
      {open ? children : null}
    </dialog>
  );
}

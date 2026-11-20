import { X } from '@phosphor-icons/react';
import { cn } from '@/lib/cn';
import { useToasts } from '@/lib/toasts';

export function Toaster() {
  const toasts = useToasts((s) => s.toasts);
  const dismiss = useToasts((s) => s.dismiss);
  return (
    <div
      aria-live="polite"
      className="pointer-events-none fixed inset-x-0 bottom-4 z-50 flex flex-col items-center gap-2 px-4"
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          role={t.tone === 'error' ? 'alert' : 'status'}
          className={cn(
            'toast pointer-events-auto flex max-w-[min(92vw,520px)] items-center gap-3 rounded-pop py-2 ps-4 pe-2 text-ui shadow-lift',
            t.tone === 'error' ? 'bg-red text-on-stamp' : 'bg-text text-desk',
          )}
        >
          <span className="min-w-0 flex-1">{t.text}</span>
          {t.action && (
            <button
              type="button"
              className="rounded-control px-2 py-1 font-semibold underline-offset-2 hover:underline"
              onClick={() => {
                t.action?.run();
                dismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          )}
          <button
            type="button"
            aria-label="Dismiss"
            className="grid size-6 place-items-center rounded-control opacity-70 hover:opacity-100"
            onClick={() => dismiss(t.id)}
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}

import { useBackend } from '@/lib/backend';
import { cn } from '@/lib/cn';

const number = (n: number) => new Intl.NumberFormat().format(n);

export function SyncNotice({ className }: { className?: string }) {
  const sync = useBackend((s) => s.sync);
  if (sync.phase !== 'reading' && sync.phase !== 'backfilling') return null;

  const reading = sync.phase === 'reading';
  const known = reading && sync.total > 0;
  const percent = known ? Math.min(100, Math.round((sync.done / sync.total) * 100)) : null;

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn('overflow-hidden rounded-control bg-blue-wash', className)}
    >
      <div className="px-3 pt-2 pb-2">
        <p className="text-ui font-medium">
          {reading ? 'Reading your mailbox' : 'Loading older mail'}
        </p>
        <p className="mt-0.5 text-meta text-text-2">
          {reading
            ? known
              ? `${number(sync.done)} of ${number(sync.total)} messages. The newest come first, so recent people are here already.`
              : 'Asking Gmail what is there. This takes a moment.'
            : `${number(sync.done)} messages are here so far. You can keep working; older conversations appear as they arrive.`}
        </p>
      </div>
      <div className="relative h-[3px] w-full overflow-hidden bg-rule" aria-hidden="true">
        {percent !== null ? (
          <div
            className="airmail h-full transition-[width] duration-300 ease-out"
            data-moving="true"
            style={{ width: `${percent}%` }}
          />
        ) : (
          <div className="sync-sweep absolute inset-y-0 w-1/3 airmail" data-moving="true" />
        )}
      </div>
    </div>
  );
}

const SKELETON_ROWS = [0, 1, 2, 3, 4, 5, 6];

export function PeopleSkeleton() {
  return (
    <div aria-hidden="true" className="px-4">
      {SKELETON_ROWS.map((i) => (
        <div
          key={`row-${i}`}
          className="skeleton-row flex items-center gap-3 py-2.5"
          style={{ ['--i' as string]: i }}
        >
          <span className="skeleton size-10 shrink-0 rounded-sheet" />
          <span className="min-w-0 flex-1">
            <span className="skeleton block h-3.5 w-1/2 rounded" />
            <span className="skeleton mt-2 block h-3 w-4/5 rounded" />
          </span>
        </div>
      ))}
    </div>
  );
}

import { useEffect, useState } from 'react';
import { cn } from '@/lib/cn';
import { useT } from '@/lib/i18n';
import { isTauri } from '@/lib/tauri';

type Edge =
  | 'North'
  | 'South'
  | 'East'
  | 'West'
  | 'NorthEast'
  | 'NorthWest'
  | 'SouthEast'
  | 'SouthWest';

const isMac = typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform);

async function win() {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow();
}

function Glyph({ kind }: { kind: 'min' | 'max' | 'restore' | 'close' }) {
  const common = {
    width: 10,
    height: 10,
    viewBox: '0 0 10 10',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 1.1,
  } as const;
  switch (kind) {
    case 'min':
      return (
        <svg {...common} aria-hidden="true">
          <path d="M1 5.5h8" />
        </svg>
      );
    case 'max':
      return (
        <svg {...common} aria-hidden="true">
          <rect x="1.5" y="1.5" width="7" height="7" />
        </svg>
      );
    case 'restore':
      return (
        <svg {...common} aria-hidden="true">
          <rect x="1.5" y="3" width="5.5" height="5.5" />
          <path d="M3.5 3V1.5h5V6.5H7" />
        </svg>
      );
    case 'close':
      return (
        <svg {...common} aria-hidden="true">
          <path d="M1.5 1.5l7 7M8.5 1.5l-7 7" />
        </svg>
      );
  }
}

export function TitleBar({ children }: { children?: React.ReactNode }) {
  const t = useT();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let off: (() => void) | undefined;
    let stopped = false;
    void win().then(async (w) => {
      const sync = async () => !stopped && setMaximized(await w.isMaximized());
      await sync();
      off = await w.onResized(sync);
      if (stopped) off();
    });
    return () => {
      stopped = true;
      off?.();
    };
  }, []);

  const act = (fn: (w: Awaited<ReturnType<typeof win>>) => Promise<void>) => () =>
    void win().then(fn);
  const buttons = (
    <div className={cn('flex h-full items-stretch', isMac ? 'order-first ps-1' : '')}>
      {(
        [
          [t('window.minimize'), 'min', act((w) => w.minimize())],
          [
            maximized ? 'Restore' : 'Maximize',
            maximized ? 'restore' : 'max',
            act((w) => w.toggleMaximize()),
          ],
          [t('window.close'), 'close', act((w) => w.close())],
        ] as const
      ).map(([label, kind, run]) => (
        <button
          key={label}
          type="button"
          aria-label={label}
          title={label}
          onClick={run}
          className={cn(
            'grid w-11 place-items-center text-text-2 transition-colors',
            kind === 'close'
              ? 'hover:bg-red hover:text-on-stamp'
              : 'hover:bg-hover hover:text-text',
          )}
        >
          <Glyph kind={kind} />
        </button>
      ))}
    </div>
  );

  return (
    <header className="relative z-40 flex h-9 shrink-0 select-none items-stretch border-b border-rule bg-desk">
      {isMac && buttons}
      <div data-tauri-drag-region className="flex min-w-0 flex-1 items-center gap-2 ps-3">
        <img
          src="/icon.png"
          alt=""
          width={18}
          height={18}
          className="pointer-events-none size-[18px]"
        />
        <span
          data-tauri-drag-region
          className="text-meta font-semibold tracking-[0.01em] text-text-2"
        >
          ZSortMail
        </span>
        {children}
      </div>
      {!isMac && buttons}
      <span
        className="airmail-edge-bottom pointer-events-none absolute inset-x-0 -bottom-px h-[2px]"
        aria-hidden="true"
      />
    </header>
  );
}

const EDGES: { edge: Edge; className: string; cursor: string }[] = [
  { edge: 'North', className: 'inset-x-2 top-0 h-1', cursor: 'ns-resize' },
  { edge: 'South', className: 'inset-x-2 bottom-0 h-1', cursor: 'ns-resize' },
  { edge: 'West', className: 'inset-y-2 left-0 w-1', cursor: 'ew-resize' },
  { edge: 'East', className: 'inset-y-2 right-0 w-1', cursor: 'ew-resize' },
  { edge: 'NorthWest', className: 'left-0 top-0 size-2', cursor: 'nwse-resize' },
  { edge: 'NorthEast', className: 'right-0 top-0 size-2', cursor: 'nesw-resize' },
  { edge: 'SouthWest', className: 'bottom-0 left-0 size-2', cursor: 'nesw-resize' },
  { edge: 'SouthEast', className: 'bottom-0 right-0 size-2', cursor: 'nwse-resize' },
];

export function ResizeEdges() {
  return (
    <>
      {EDGES.map(({ edge, className, cursor }) => (
        <div
          key={edge}
          aria-hidden="true"
          className={cn('fixed z-50', className)}
          style={{ cursor }}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            e.preventDefault();
            void win().then((w) => w.startResizeDragging(edge));
          }}
        />
      ))}
    </>
  );
}

export function Frame({ children }: { children: React.ReactNode }) {
  if (!isTauri()) return <>{children}</>;
  return (
    <div className="flex h-full flex-col">
      <TitleBar />
      <div className="min-h-0 flex-1">{children}</div>
      <ResizeEdges />
    </div>
  );
}

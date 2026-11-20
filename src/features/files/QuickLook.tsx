import { ArrowLeft, ArrowRight, DownloadSimple, X } from '@phosphor-icons/react';
import { lazy, Suspense, useEffect, useState } from 'react';
import { api } from '@/lib/api';
import { formatSize } from '@/lib/format';
import { isTauri, zsmUrl } from '@/lib/tauri';
import { toast, toastError } from '@/lib/toasts';
import { IconButton } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { canPreview, fileKind, kindIcon } from './fileKind';

const PdfView = lazy(() => import('./PdfView'));

export interface PreviewItem {
  id: number;
  name: string;
  mime: string;
  size: number;
  caption?: string;
  conversationId?: number;
}

export function QuickLook({
  items,
  index,
  onIndex,
  onClose,
  onShowConversation,
}: {
  items: PreviewItem[];
  index: number | null;
  onIndex: (index: number) => void;
  onClose: () => void;
  onShowConversation?: (conversationId: number) => void;
}) {
  const open = index !== null && items[index] !== undefined;
  const item = open ? items[index as number] : undefined;

  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (index === null) return;
      if (e.key === 'ArrowRight' && index < items.length - 1) onIndex(index + 1);
      else if (e.key === 'ArrowLeft' && index > 0) onIndex(index - 1);
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, index, items.length, onIndex]);

  return (
    <Dialog open={open} onClose={onClose} label="Preview" className="[--dialog-w:960px]">
      {item && (
        <div className="flex max-h-[86vh] flex-col overflow-hidden rounded-pop bg-sheet shadow-lift">
          <header className="flex items-center gap-2 border-b border-rule px-4 py-2.5">
            <div className="min-w-0 flex-1">
              <h2 dir="auto" className="truncate text-body font-semibold">
                {item.name}
              </h2>
              <p className="truncate text-meta text-text-2">
                {formatSize(item.size)}
                {item.caption ? `, ${item.caption}` : ''}
                {items.length > 1 && index !== null ? `, ${index + 1} of ${items.length}` : ''}
              </p>
            </div>
            {item.conversationId !== undefined && onShowConversation && (
              <button
                type="button"
                className="rounded-control px-2 py-1 text-ui font-medium text-blue hover:bg-hover"
                onClick={() => {
                  onShowConversation(item.conversationId as number);
                  onClose();
                }}
              >
                Show in conversation
              </button>
            )}
            <SaveButton item={item} />
            <IconButton
              label="Previous file"
              disabled={index === 0}
              onClick={() => onIndex((index as number) - 1)}
            >
              <ArrowLeft size={18} />
            </IconButton>
            <IconButton
              label="Next file"
              disabled={index === items.length - 1}
              onClick={() => onIndex((index as number) + 1)}
            >
              <ArrowRight size={18} />
            </IconButton>
            <IconButton label="Close preview" onClick={onClose}>
              <X size={18} />
            </IconButton>
          </header>
          <div className="grid min-h-[320px] flex-1 place-items-center overflow-auto bg-desk p-4">
            <Preview key={item.id} item={item} />
          </div>
        </div>
      )}
    </Dialog>
  );
}

function SaveButton({ item }: { item: PreviewItem }) {
  const url = zsmUrl(`att/${item.id}`);
  if (!isTauri()) {
    return (
      <a
        href={url}
        download={item.name}
        aria-label="Save file"
        title="Save file"
        className="inline-flex size-8 items-center justify-center rounded-control text-text-2 hover:bg-hover hover:text-text"
      >
        <DownloadSimple size={18} />
      </a>
    );
  }
  return (
    <IconButton
      label="Save to Downloads"
      onClick={() => {
        api
          .saveAttachment(item.id)
          .then((path) => toast(`Saved to ${path}`))
          .catch(toastError);
      }}
    >
      <DownloadSimple size={18} />
    </IconButton>
  );
}

function Preview({ item }: { item: PreviewItem }) {
  const kind = fileKind(item.mime, item.name);
  const src = zsmUrl(`att/${item.id}?inline=1`);
  const [text, setText] = useState<string | null>(null);
  const [state, setState] = useState<'preparing' | 'ready' | 'failed'>('preparing');
  useEffect(() => {
    let live = true;
    api
      .prepareAttachment(item.id)
      .then(() => live && setState('ready'))
      .catch((e) => {
        if (!live) return;
        setState('failed');
        toastError(e);
      });
    return () => {
      live = false;
    };
  }, [item.id]);
  useEffect(() => {
    if (kind !== 'text' || state !== 'ready') return;
    let live = true;
    fetch(src)
      .then((r) => r.text())
      .then((t) => live && setText(t.slice(0, 200_000)))
      .catch(() => live && setText(''));
    return () => {
      live = false;
    };
  }, [kind, src, state]);

  if (state === 'preparing') return <p className="text-ui text-text-2">Downloading</p>;
  if (state === 'failed') {
    return (
      <p className="text-ui text-text-2">
        This file could not be downloaded. Try again when Gmail is reachable.
      </p>
    );
  }
  if (kind === 'image') {
    return (
      <img
        src={src}
        alt={item.name}
        className="max-h-[68vh] max-w-full rounded-sheet bg-paper object-contain shadow-sheet"
      />
    );
  }
  if (kind === 'pdf') {
    return (
      <Suspense fallback={<p className="text-ui text-text-2">Opening the PDF</p>}>
        <PdfView src={src} name={item.name} />
      </Suspense>
    );
  }
  if (kind === 'text') {
    return (
      <pre
        dir="auto"
        className="max-h-[68vh] w-full overflow-auto rounded-sheet bg-paper p-4 font-read text-read whitespace-pre-wrap text-paper-text select-text"
      >
        {text ?? 'Loading'}
      </pre>
    );
  }
  if (kind === 'audio') {
    // biome-ignore lint/a11y/useMediaCaption: an attachment has no caption track.
    return <audio controls src={src} className="w-full max-w-[520px]" />;
  }
  if (kind === 'video') {
    // biome-ignore lint/a11y/useMediaCaption: an attachment has no caption track.
    return <video controls src={src} className="max-h-[68vh] max-w-full rounded-sheet bg-black" />;
  }
  const Icon = kindIcon[kind];
  return (
    <div className="text-center">
      <Icon size={56} weight="regular" />
      <p className="mt-3 text-body font-medium">{item.name}</p>
      <p className="mt-1 text-ui text-text-2">
        {canPreview(kind)
          ? 'This file cannot be shown in the window.'
          : 'There is no preview for this kind of file.'}
      </p>
    </div>
  );
}

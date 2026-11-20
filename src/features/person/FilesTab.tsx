import { useNavigate } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import type { FileEntry } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { formatMessageTime, formatSize } from '@/lib/format';
import { useFiles } from '@/lib/queries';
import { zsmUrl } from '@/lib/tauri';
import { Segmented } from '@/ui/controls';
import { fileKind, kindIcon } from '../files/fileKind';
import { type PreviewItem, QuickLook } from '../files/QuickLook';

function toPreview(entry: FileEntry, nowTs: number): PreviewItem {
  return {
    id: entry.attachment.id,
    name: entry.attachment.name,
    mime: entry.attachment.mime,
    size: entry.attachment.size,
    caption: `${entry.from.is_me ? 'You' : entry.from.name}, ${formatMessageTime(entry.sent_at, nowTs)}`,
    conversationId: entry.conversation_id,
  };
}

function Thumb({ entry, size }: { entry: FileEntry; size: 'list' | 'grid' }) {
  const kind = fileKind(entry.attachment.mime, entry.attachment.name);
  const Icon = kindIcon[kind];
  const [broken, setBroken] = useState(false);
  return (
    <span
      className={
        size === 'grid'
          ? 'grid aspect-square w-full place-items-center overflow-hidden rounded-sheet bg-desk text-text-2'
          : 'grid size-11 shrink-0 place-items-center overflow-hidden rounded-sheet bg-desk text-text-2'
      }
    >
      {kind === 'image' && !broken ? (
        <img
          src={zsmUrl(`att/${entry.attachment.id}?inline=1`)}
          alt=""
          loading="lazy"
          decoding="async"
          onError={() => setBroken(true)}
          className="size-full object-cover"
        />
      ) : (
        <Icon size={size === 'grid' ? 34 : 22} />
      )}
    </span>
  );
}

export function FilesTab({ personId }: { personId: number }) {
  const { data: files = [], isLoading } = useFiles(personId);
  const navigate = useNavigate();
  const [open, setOpen] = useState<number | null>(null);
  const [layout, setLayout] = useState<'list' | 'grid'>('list');
  const nowTs = now();
  const items = useMemo(() => files.map((f) => toPreview(f, nowTs)), [files, nowTs]);

  if (!isLoading && files.length === 0) {
    return (
      <p className="px-4 py-8 text-center text-ui text-text-2">
        No files yet. Attachments in your conversations with this person collect here.
      </p>
    );
  }

  return (
    <>
      <div className="flex justify-end px-4 pt-2">
        <Segmented
          label="Show files as"
          value={layout}
          onChange={setLayout}
          options={[
            { value: 'list', label: 'List' },
            { value: 'grid', label: 'Grid' },
          ]}
        />
      </div>
      <ul aria-label="Files" className={layout === 'grid' ? 'grid grid-cols-3 gap-2 p-3' : 'py-1'}>
        {files.map((entry, i) => (
          <li key={entry.attachment.id}>
            <button
              type="button"
              onClick={() => setOpen(i)}
              title={entry.attachment.name}
              className={
                layout === 'grid'
                  ? 'skip-offscreen flex w-full flex-col gap-1.5 rounded-sheet p-1.5 text-left transition-colors hover:bg-hover'
                  : 'skip-offscreen flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-hover'
              }
            >
              <Thumb entry={entry} size={layout} />
              <span className="min-w-0 flex-1">
                <span dir="auto" className="block truncate text-ui font-medium">
                  {entry.attachment.name}
                </span>
                <span dir="auto" className="block truncate text-meta text-text-2">
                  {formatSize(entry.attachment.size)}
                  {layout === 'list' &&
                    `, ${entry.from.is_me ? 'you' : entry.from.name.split(' ')[0]}, ${formatMessageTime(entry.sent_at, nowTs)}`}
                </span>
                {layout === 'list' && (
                  <span dir="auto" className="block truncate text-meta text-text-2">
                    {entry.conversation_subject}
                  </span>
                )}
              </span>
            </button>
          </li>
        ))}
      </ul>
      <QuickLook
        items={items}
        index={open}
        onIndex={setOpen}
        onClose={() => setOpen(null)}
        onShowConversation={(conversationId) =>
          navigate({
            to: '/people/$personId/c/$conversationId',
            params: { personId: String(personId), conversationId: String(conversationId) },
          })
        }
      />
    </>
  );
}

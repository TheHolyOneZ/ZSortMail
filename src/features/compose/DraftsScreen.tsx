import { Clock, NotePencil, Trash } from '@phosphor-icons/react';
import { addressLabel } from '@/lib/addresses';
import { api } from '@/lib/api';
import type { Draft } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { compose, openDraft } from '@/lib/composer';
import { formatListTime, plural } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { useDrafts } from '@/lib/queries';
import { toast, toastError } from '@/lib/toasts';
import { Button, IconButton } from '@/ui/controls';

export function draftSnippet(html: string): string {
  const doc = new DOMParser().parseFromString(html, 'text/html');
  for (const p of Array.from(doc.querySelectorAll('blockquote'))) p.remove();
  const text = (doc.body.textContent ?? '').replace(/\s+/g, ' ').trim();
  return (
    text
      .split(/\s--\s/)[0]
      ?.slice(0, 140)
      .trim() ?? ''
  );
}

function recipients(draft: Draft): string {
  const all = [...draft.to, ...draft.cc, ...draft.bcc];
  if (all.length === 0) return 'No recipient yet';
  const first = addressLabel(all[0] as (typeof all)[number]);
  return all.length === 1 ? first : `${first} and ${plural(all.length - 1, 'other')}`;
}

function scheduledFor(at: number): string {
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(at);
}

export function DraftsScreen() {
  const t = useT();
  const { data: drafts = [], isSuccess } = useDrafts();
  const scheduled = drafts.filter((d) => d.send_at !== null);
  const loose = drafts.filter((d) => d.send_at === null);

  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Drafts">
      <div className="mx-auto max-w-[820px]">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-title font-semibold tracking-[-0.01em]">{t('drafts.title')}</h1>
            <p className="mt-1 max-w-[56ch] text-ui text-text-2">
              Emails you started. They are kept on this computer and saved as you type.
            </p>
          </div>
          <Button variant="primary" className="shrink-0" onClick={() => void compose()}>
            <NotePencil size={16} weight="bold" />
            New message
          </Button>
        </div>

        {isSuccess && drafts.length === 0 && (
          <div className="mt-12 max-w-[56ch]">
            <h2 className="text-name font-semibold">No drafts</h2>
            <p className="mt-1 text-ui text-text-2">
              Press C anywhere to write a message. If you close it before sending, it waits here.
            </p>
          </div>
        )}

        <DraftList title="Scheduled" drafts={scheduled} />
        <DraftList title="Drafts" drafts={loose} />
      </div>
    </section>
  );
}

function DraftList({ title, drafts }: { title: string; drafts: Draft[] }) {
  if (drafts.length === 0) return null;
  const nowTs = now();
  return (
    <section className="mt-8" aria-label={title}>
      <h2 className="text-name font-semibold">
        {title} ({drafts.length})
      </h2>
      <ul className="mt-2 border-t border-rule">
        {drafts.map((d) => (
          <li key={d.id} className="group flex items-center border-b border-rule hover:bg-hover">
            <button
              type="button"
              onClick={() => openDraft(d.id)}
              className="flex min-w-0 flex-1 items-start gap-4 py-3 text-left"
            >
              <span className="min-w-0 flex-1">
                <span className="flex items-baseline gap-2">
                  <span dir="auto" className="truncate text-body font-medium">
                    {recipients(d)}
                  </span>
                  {d.send_at !== null && (
                    <span className="inline-flex shrink-0 items-center gap-1 text-meta text-text-2">
                      <Clock size={12} />
                      {scheduledFor(d.send_at)}
                    </span>
                  )}
                </span>
                <span dir="auto" className="block truncate text-ui">
                  {d.subject.trim() || 'No subject'}
                </span>
                <span dir="auto" className="block truncate text-ui text-text-2">
                  {draftSnippet(d.body_html) || 'Nothing written yet'}
                  {d.attachments.length > 0 && `. ${plural(d.attachments.length, 'file')}`}
                </span>
              </span>
              <time className="tabular shrink-0 text-meta text-text-2">
                {formatListTime(d.updated_at, nowTs)}
              </time>
            </button>
            <IconButton
              label={`Discard the draft to ${recipients(d)}`}
              className="ms-2 shrink-0 opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
              onClick={() =>
                void api.deleteDraft(d.id).then(() => toast('Draft discarded.'), toastError)
              }
            >
              <Trash size={17} />
            </IconButton>
          </li>
        ))}
      </ul>
    </section>
  );
}

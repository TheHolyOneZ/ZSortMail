import { ArrowRight, Copy, HourglassMedium, Pause, Play, Plus, Trash } from '@phosphor-icons/react';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { api } from '@/lib/api';
import type { Nudge, WatchView } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { elapsedLabel, formatMessageTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { useNudges, useWatches } from '@/lib/queries';
import { toast, toastError } from '@/lib/toasts';
import { Button, IconButton } from '@/ui/controls';
import { WatchDialog } from './WatchDialog';

const EXAMPLES = ['xbox 394', 'from:dhl', 'passport appointment', 'invoice has:attachment'];

function summaryOf(w: WatchView): string {
  const parts = [w.mode === 'keep' ? 'keeps watching' : 'watches once'];
  if (w.priority === 'urgent') parts.push('urgent');
  if (w.expires_at)
    parts.push(
      `stops ${new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short' }).format(w.expires_at)}`,
    );
  return parts.join(', ');
}

export function WaitingScreen() {
  const t = useT();
  const { data: watches = [], isSuccess } = useWatches();
  const { data: nudges = [] } = useNudges();
  const [dialog, setDialog] = useState<{ query: string; editing?: WatchView } | null>(null);
  const waiting = watches.filter((w) => w.status === 'waiting');
  const arrived = watches.filter((w) => w.status === 'arrived');
  const rest = watches.filter((w) => w.status === 'paused' || w.status === 'expired');

  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Waiting for">
      <div className="mx-auto max-w-[800px]">
        <div className="flex items-end justify-between gap-4">
          <div>
            <h1 className="text-title font-semibold tracking-[-0.01em]">{t('waiting.title')}</h1>
            <p className="mt-1 max-w-[52ch] text-ui text-text-2">
              Tell ZSortMail what mail you expect, like “xbox 394”, and it tells you the moment it
              arrives.
            </p>
          </div>
          <Button variant="primary" className="shrink-0" onClick={() => setDialog({ query: '' })}>
            <Plus size={16} weight="bold" />
            Watch for an email
          </Button>
        </div>

        {isSuccess && watches.length === 0 && nudges.length === 0 && (
          <div className="mt-14 max-w-[52ch]">
            <HourglassMedium size={40} className="text-text-3" />
            <h2 className="mt-3 text-name font-semibold">Nothing to wait for yet</h2>
            <p className="mt-1 text-ui text-text-2">
              Waiting for a parcel, an appointment, an invoice? Describe it and carry on. You can
              type
              <kbd className="mx-1 rounded-[4px] border border-rule-strong px-1 font-ui text-meta">
                watch xbox 394
              </kbd>
              in the search box (Ctrl or Cmd and K) as a shortcut.
            </p>
            <div className="mt-4 flex flex-wrap gap-2">
              {EXAMPLES.map((example) => (
                <button
                  key={example}
                  type="button"
                  onClick={() => setDialog({ query: example })}
                  className="rounded-full border border-rule-strong px-3 py-1 text-ui text-text-2 transition-colors hover:bg-hover hover:text-text"
                >
                  {example}
                </button>
              ))}
            </div>
          </div>
        )}

        {waiting.length > 0 && (
          <Group title={`Waiting (${waiting.length})`}>
            {waiting.map((w) => (
              <WatchRow
                key={w.id}
                watch={w}
                onEdit={() => setDialog({ query: w.query, editing: w })}
              />
            ))}
          </Group>
        )}
        {nudges.length > 0 && (
          <Group title={`Waiting for a reply (${nudges.length})`}>
            {nudges.map((n) => (
              <NudgeRow key={n.id} nudge={n} />
            ))}
          </Group>
        )}
        {arrived.length > 0 && (
          <Group title="Arrived">
            {arrived.map((w) => (
              <WatchRow
                key={w.id}
                watch={w}
                onEdit={() => setDialog({ query: w.query, editing: w })}
              />
            ))}
          </Group>
        )}
        {rest.length > 0 && (
          <Group title="Paused and ended">
            {rest.map((w) => (
              <WatchRow
                key={w.id}
                watch={w}
                onEdit={() => setDialog({ query: w.query, editing: w })}
              />
            ))}
          </Group>
        )}
      </div>
      {dialog && (
        <WatchDialog
          open
          onClose={() => setDialog(null)}
          initialQuery={dialog.query}
          editing={dialog.editing}
        />
      )}
    </section>
  );
}

function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mt-8" aria-label={title}>
      <h2 className="text-name font-semibold">{title}</h2>
      <ul className="mt-2 border-t border-rule">{children}</ul>
    </section>
  );
}

function WatchRow({ watch: w, onEdit }: { watch: WatchView; onEdit: () => void }) {
  const navigate = useNavigate();
  const hit = w.hits[0];
  const urgent = w.priority === 'urgent' && w.status === 'waiting';
  const inactive = w.status === 'paused' || w.status === 'expired';

  const status =
    w.status === 'waiting'
      ? `waiting ${elapsedLabel(now() - w.created_at)}`
      : w.status === 'arrived'
        ? `arrived ${formatMessageTime(w.arrived_at ?? w.last_hit_at ?? w.created_at, now())}`
        : w.status === 'paused'
          ? 'paused'
          : 'ended';

  function open() {
    if (!hit) return;
    void api.markConversationSeen(hit.conversation_id).catch(() => {});
    if (hit.person_id !== null) {
      void navigate({
        to: '/people/$personId/c/$conversationId',
        params: { personId: String(hit.person_id), conversationId: String(hit.conversation_id) },
      });
    }
  }

  return (
    <li
      className={cn(
        'airmail-edge border-b border-rule py-4 ps-5',
        inactive && 'opacity-70',
        !urgent && 'before:hidden',
      )}
    >
      <div className="flex items-start gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-baseline gap-x-3">
            <h3 dir="auto" className="text-body font-semibold">
              {w.name}
            </h3>
            <span className="text-ui text-text-2">{summaryOf(w)}</span>
          </div>
          {w.hits.some((h) => !h.seen) && w.status === 'arrived' && (
            <p className="mt-0.5 text-meta font-medium text-blue">New, not opened yet</p>
          )}
          {hit && (
            <div className="mt-2">
              <p dir="auto" className="truncate text-ui">
                <span className="font-medium">{hit.person_name}</span>: {hit.subject}
              </p>
              {hit.extracted.length > 0 && (
                <ul className="mt-2 flex flex-wrap gap-2" aria-label="Found in the message">
                  {hit.extracted.map((e) => (
                    <li key={`${e.kind}-${e.value}`}>
                      <button
                        type="button"
                        title="Copy"
                        onClick={() => {
                          void navigator.clipboard
                            ?.writeText(e.value)
                            .then(() => toast(`${e.value} copied.`));
                        }}
                        className="inline-flex items-center gap-2 rounded-sheet border border-rule-strong px-2.5 py-1 text-ui transition-colors hover:bg-hover"
                      >
                        <span className="text-text-2">
                          {e.kind === 'tracking'
                            ? 'Tracking'
                            : e.kind === 'order'
                              ? 'Order'
                              : 'Reference'}
                        </span>
                        <span dir="ltr" className="font-medium select-text">
                          {e.value}
                        </span>
                        <Copy size={13} className="text-text-3" />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>
        <div className="flex shrink-0 flex-col items-end gap-2">
          <span className="tabular text-ui text-text-2">{status}</span>
          <div className="flex items-center">
            {hit && hit.person_id !== null && (
              <Button onClick={open}>
                Open <ArrowRight size={14} />
              </Button>
            )}
            {w.status === 'waiting' && (
              <IconButton
                label="Pause"
                onClick={() => void api.setWatchActive(w.id, false).catch(toastError)}
              >
                <Pause size={17} />
              </IconButton>
            )}
            {(w.status === 'paused' || w.status === 'arrived' || w.status === 'expired') && (
              <IconButton
                label="Watch again"
                onClick={() => void api.setWatchActive(w.id, true).catch(toastError)}
              >
                <Play size={17} />
              </IconButton>
            )}
            <IconButton label="Change" onClick={onEdit}>
              <HourglassMedium size={17} />
            </IconButton>
            <IconButton label="Remove" onClick={() => void api.deleteWatch(w.id).catch(toastError)}>
              <Trash size={17} />
            </IconButton>
          </div>
        </div>
      </div>
    </li>
  );
}

function NudgeRow({ nudge }: { nudge: Nudge }) {
  const navigate = useNavigate();
  const nowTs = now();
  const due = new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  }).format(nudge.due_at);
  return (
    <li className="flex items-start gap-4 border-b border-rule py-3">
      <div className="min-w-0 flex-1">
        <h3 dir="auto" className="truncate text-body font-semibold">
          {nudge.who}
        </h3>
        <p dir="auto" className="truncate text-ui text-text-2">
          {nudge.subject}
        </p>
        <p className="mt-0.5 text-meta text-text-2">
          Sent {elapsedLabel(nowTs - nudge.sent_at)} ago. You will be reminded on {due} if there is
          no reply.
        </p>
      </div>
      <div className="flex shrink-0 items-center">
        {nudge.person_id !== null && (
          <Button
            onClick={() =>
              void navigate({
                to: '/people/$personId/c/$conversationId',
                params: {
                  personId: String(nudge.person_id),
                  conversationId: String(nudge.conversation_id),
                },
              })
            }
          >
            Open
            <ArrowRight size={14} />
          </Button>
        )}
        <IconButton
          label="Stop the reminder"
          onClick={() =>
            void api.cancelNudge(nudge.id).then(() => toast('Reminder stopped.'), toastError)
          }
        >
          <Trash size={17} />
        </IconButton>
      </div>
    </li>
  );
}

import { PencilSimpleLine } from '@phosphor-icons/react';
import { Link } from '@tanstack/react-router';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { ConversationRow } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { compose } from '@/lib/composer';
import { elapsedLabel, formatMessageTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { useToday } from '@/lib/queries';
import { Button } from '@/ui/controls';
import { ZMark } from '@/ui/icons';
import { SyncNotice } from '@/ui/SyncNotice';
import { ConversationLine } from './ConversationLine';
import { SnoozeMenu } from './SnoozeMenu';

function dateLine(): string {
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
  }).format(now());
}

export function TodayScreen() {
  const t = useT();
  const { data, isSuccess } = useToday();
  const owed = data?.needs_reply ?? [];
  const quiet = data?.waiting_on ?? [];
  const snoozed = data?.snoozed ?? [];
  const watches = (data?.watches ?? []).filter(
    (w) => w.status === 'waiting' || (w.status === 'arrived' && w.hits.some((h) => !h.seen)),
  );
  const arrivedNew = watches.filter((w) => w.status === 'arrived');
  const nothing =
    isSuccess &&
    owed.length + quiet.length + snoozed.length + watches.length + (data?.new_senders ?? 0) === 0;

  const parts: string[] = [];
  if (owed.length) parts.push(`${owed.length} ${owed.length === 1 ? 'reply' : 'replies'} to write`);
  if (arrivedNew.length)
    parts.push(
      `${arrivedNew.length} you were waiting for ${arrivedNew.length === 1 ? 'has' : 'have'} arrived`,
    );
  if (data?.new_senders)
    parts.push(
      `${data.new_senders} ${data.new_senders === 1 ? 'person' : 'people'} waiting to be let in`,
    );

  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Today">
      <div className="mx-auto max-w-[760px]">
        <div className="flex items-center justify-between gap-3">
          <h1 className="text-title font-semibold tracking-[-0.01em]">{t('today.title')}</h1>
          <Button variant="primary" onClick={() => void compose()}>
            <PencilSimpleLine size={15} />
            {t('nav.write')}
          </Button>
        </div>
        <SyncNotice className="mt-3" />
        <p className="mt-1 text-ui text-text-2">
          {dateLine()}
          {parts.length > 0 ? `. ${parts.join(', ')}.` : '.'}
        </p>

        {nothing && (
          <div className="mt-16 max-w-[44ch]">
            <ZMark size={44} className="text-text-3" />
            <h2 className="mt-4 text-name font-semibold">Nothing needs you</h2>
            <p className="mt-1 text-ui text-text-2">
              No replies owed, nobody waiting to be let in, nothing you are waiting for. When
              something does, it appears here.
            </p>
          </div>
        )}

        {data && data.new_senders > 0 && (
          <Section title="New senders">
            <li className="flex items-center justify-between gap-4 border-b border-rule py-3">
              <span className="text-ui">
                {data.new_senders} {data.new_senders === 1 ? 'person has' : 'people have'} never
                written to you before.
              </span>
              <Link
                to="/new-senders"
                className="inline-flex h-8 items-center rounded-control bg-blue px-3 text-ui font-medium text-on-blue hover:brightness-110"
              >
                Decide
              </Link>
            </li>
          </Section>
        )}

        {owed.length > 0 && (
          <Section title="Needs you">
            {owed.map((row) => (
              <ConversationLine
                key={row.conversation.id}
                row={row}
                meta={ageOf(row)}
                actions={<SnoozeMenu conversationId={row.conversation.id} />}
              />
            ))}
          </Section>
        )}

        {watches.length > 0 && (
          <Section title="Waiting for">
            {watches.map((w) => (
              <li key={w.id} className="flex items-center gap-4 border-b border-rule py-3">
                <div className="min-w-0 flex-1">
                  <p dir="auto" className="truncate text-body font-medium">
                    {w.name}
                  </p>
                  <p className="truncate text-ui text-text-2">
                    {w.status === 'arrived' && w.hits[0]
                      ? `${w.hits[0].person_name}: ${w.hits[0].subject}`
                      : `waiting ${elapsedLabel(now() - w.created_at)}`}
                  </p>
                </div>
                <Link to="/waiting" className="text-ui font-medium text-blue">
                  {w.status === 'arrived' ? 'See what came' : 'Open'}
                </Link>
              </li>
            ))}
          </Section>
        )}

        {quiet.length > 0 && (
          <Section title="Waiting on them">
            {quiet.map((row) => (
              <ConversationLine
                key={row.conversation.id}
                row={row}
                meta={`quiet for ${ageOf(row)}`}
              />
            ))}
          </Section>
        )}

        {snoozed.length > 0 && (
          <Section title="Snoozed">
            {snoozed.map((row) => (
              <ConversationLine
                key={row.conversation.id}
                row={row}
                meta={
                  row.snoozed_until ? `back ${formatMessageTime(row.snoozed_until, now())}` : ''
                }
                actions={
                  <Button
                    onClick={() =>
                      void act(() => api.snoozeConversation(row.conversation.id, null))
                    }
                  >
                    Bring back
                  </Button>
                }
              />
            ))}
          </Section>
        )}
      </div>
    </section>
  );
}

function ageOf(row: ConversationRow): string {
  return elapsedLabel(Math.max(0, now() - row.conversation.last_at));
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mt-8" aria-label={title}>
      <h2 className="text-name font-semibold">{title}</h2>
      <ul className="mt-2 border-t border-rule">{children}</ul>
    </section>
  );
}

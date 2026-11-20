import { useNavigate } from '@tanstack/react-router';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { HiddenConversation, HiddenPerson } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { formatListTime, plural } from '@/lib/format';
import { useHidden } from '@/lib/queries';
import { Button } from '@/ui/controls';
import { Stamp } from '@/ui/Stamp';

function until(ms: number | null): string | null {
  if (ms === null) return null;
  return `until ${new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short' }).format(ms)}`;
}

export function HiddenScreen() {
  const { data, isSuccess } = useHidden();
  const people = data?.people.filter((p) => !p.blocked) ?? [];
  const blocked = data?.people.filter((p) => p.blocked) ?? [];
  const conversations = data?.conversations ?? [];
  const empty = isSuccess && people.length + blocked.length + conversations.length === 0;

  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Hidden">
      <div className="mx-auto max-w-[760px]">
        <h1 className="text-title font-semibold tracking-[-0.01em]">Hidden</h1>
        <p className="mt-1 max-w-[56ch] text-ui text-text-2">
          What you have put out of sight. It stays out of lists, search and notifications until you
          bring it back.
        </p>

        {empty && (
          <p className="mt-10 max-w-[44ch] text-ui text-text-2">
            Nothing is hidden. Hide a person from their page, or a conversation with{' '}
            <kbd className="font-ui">H</kbd>.
          </p>
        )}

        {people.length > 0 && (
          <Section title="People you hid">
            {people.map((h) => (
              <PersonRow
                key={h.person.id}
                item={h}
                action="Unhide"
                onAction={() => void act(() => api.hidePerson(h.person.id, false))}
              />
            ))}
          </Section>
        )}

        {conversations.length > 0 && (
          <Section title="Conversations you hid">
            {conversations.map((h) => (
              <ConversationRow key={h.conversation.id} item={h} />
            ))}
          </Section>
        )}

        {blocked.length > 0 && (
          <Section title="Blocked senders">
            {blocked.map((h) => (
              <PersonRow
                key={h.person.id}
                item={h}
                action="Unblock"
                onAction={() => void act(() => api.hidePerson(h.person.id, false))}
              />
            ))}
          </Section>
        )}
      </div>
    </section>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mt-8" aria-label={title}>
      <h2 className="text-name font-semibold">{title}</h2>
      <ul className="mt-2 border-t border-rule">{children}</ul>
    </section>
  );
}

function PersonRow({
  item,
  action,
  onAction,
}: {
  item: HiddenPerson;
  action: string;
  onAction: () => void;
}) {
  const p = item.person;
  const when = until(item.until);
  return (
    <li className="flex items-center gap-3 border-b border-rule py-3">
      <Stamp name={p.name} ink={p.ink} kind={p.kind} size={36} />
      <div className="min-w-0 flex-1">
        <p dir="auto" className="truncate text-body font-medium">
          {p.name}
        </p>
        <p dir="ltr" className="truncate text-meta text-text-2">
          {item.address}
          {when ? `, ${when}` : ''}
        </p>
      </div>
      <Button onClick={onAction}>{action}</Button>
    </li>
  );
}

function ConversationRow({ item }: { item: HiddenConversation }) {
  const navigate = useNavigate();
  const c = item.conversation;
  const when = until(item.until);
  return (
    <li className="flex items-center gap-3 border-b border-rule py-3">
      <div className="min-w-0 flex-1">
        <p dir="auto" className="truncate text-body font-medium">
          {c.subject}
        </p>
        <p dir="auto" className="truncate text-meta text-text-2">
          {item.person_name ? `${item.person_name}, ` : ''}
          {plural(c.message_count, 'message')}, {formatListTime(c.last_at, now())}
          {when ? `, ${when}` : ''}
        </p>
      </div>
      <Button
        onClick={() => {
          void act(() => api.hideConversation(c.id, false)).then((ok) => {
            if (ok && item.person_id !== null) {
              void navigate({
                to: '/people/$personId/c/$conversationId',
                params: { personId: String(item.person_id), conversationId: String(c.id) },
              });
            }
          });
        }}
      >
        Show again
      </Button>
    </li>
  );
}

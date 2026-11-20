import { ArrowLeft, DotsThree, Paperclip, PencilSimple, PushPin } from '@phosphor-icons/react';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { ConversationSummary } from '@/lib/bindings';
import { DAY, now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { writeToPerson } from '@/lib/composer';
import { formatListTime, plural, type TimeGroup, timeGroup } from '@/lib/format';
import { stampColor } from '@/lib/ink';
import { conversationMenu, copyText, personMenu } from '@/lib/menus';
import { useConversations, usePerson, usePersonFlags } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { useContextMenu } from '@/ui/ContextMenu';
import { Button, IconButton } from '@/ui/controls';
import { Menu } from '@/ui/Menu';
import { Stamp } from '@/ui/Stamp';
import { Tabs } from '@/ui/Tabs';
import { EditPerson } from '../person/EditPerson';
import { FilesTab } from '../person/FilesTab';
import { MergeDialog } from '../person/MergeDialog';
import { NotesTab } from '../person/NotesTab';
import { TimelineTab } from '../person/TimelineTab';

const GROUP_ORDER: TimeGroup[] = ['Today', 'Yesterday', 'This week', 'This month', 'Earlier'];

function groupConversations(items: ConversationSummary[], nowTs: number) {
  const groups = new Map<TimeGroup, ConversationSummary[]>();
  for (const item of items) {
    const key = timeGroup(item.last_at, nowTs);
    const list = groups.get(key);
    if (list) list.push(item);
    else groups.set(key, [item]);
  }
  return GROUP_ORDER.flatMap((name) => {
    const list = groups.get(name);
    return list ? [{ name, items: list }] : [];
  });
}

type PersonTab = 'conversations' | 'timeline' | 'files' | 'notes';

export function ConversationsPane({
  personId,
  selectedId,
  showBack,
}: {
  personId?: number;
  selectedId?: number;
  showBack?: boolean;
}) {
  if (personId === undefined) {
    return (
      <section className="grid h-full place-items-center bg-pane px-8" aria-label="Conversations">
        <p className="max-w-[26ch] text-center text-ui text-text-2">
          Choose someone to see their conversations.
        </p>
      </section>
    );
  }
  return (
    <PersonPane key={personId} personId={personId} selectedId={selectedId} showBack={showBack} />
  );
}

function PersonPane({
  personId,
  selectedId,
  showBack,
}: {
  personId: number;
  selectedId?: number;
  showBack?: boolean;
}) {
  const navigate = useNavigate();
  const { data: person } = usePerson(personId);
  const { data: conversations = [] } = useConversations(personId);
  const { data: flags } = usePersonFlags(personId);
  const [tab, setTab] = useState<PersonTab>('conversations');
  const [editing, setEditing] = useState(false);
  const [merging, setMerging] = useState(false);
  const { onContextMenu, menu } = useContextMenu();

  if (!person) {
    return <section className="h-full bg-pane" aria-label="Conversations" />;
  }

  const s = person.summary;
  const nowTs = now();
  const openConversation = (id: number) =>
    navigate({
      to: '/people/$personId/c/$conversationId',
      params: { personId: String(personId), conversationId: String(id) },
    });
  const groups = groupConversations(conversations, nowTs);
  const subtitle = [s.title, person.organization].filter(Boolean).join(', ');
  const since = person.first_contact_at
    ? new Intl.DateTimeFormat(undefined, { month: 'short', year: 'numeric' }).format(
        person.first_contact_at,
      )
    : null;

  return (
    <section className="flex h-full min-w-0 flex-col bg-pane" aria-label="Conversations">
      {/* biome-ignore lint/a11y/noStaticElementInteractions: right-click is a shortcut to the same actions as the More menu, which the keyboard reaches. */}
      <header
        className="shrink-0 px-4 pt-4 pb-3"
        onContextMenu={onContextMenu(() =>
          personMenu(s, {
            edit: () => setEditing(true),
            merge: () => setMerging(true),
            afterHide: () => void navigate({ to: '/people' }),
          }),
        )}
      >
        <div className="flex items-center">
          {showBack && (
            <IconButton
              label="Back to people"
              className="-ml-2"
              onClick={() => navigate({ to: '/people' })}
            >
              <ArrowLeft size={18} />
            </IconButton>
          )}
          <span className="flex-1" />
          <IconButton
            label={s.pinned ? 'Unpin' : 'Pin to the top'}
            aria-pressed={s.pinned}
            onClick={() => api.setPersonPinned(s.id, !s.pinned).catch(toastError)}
          >
            <PushPin size={18} weight={s.pinned ? 'fill' : 'regular'} />
          </IconButton>
          <IconButton label="Edit person" onClick={() => setEditing(true)}>
            <PencilSimple size={18} />
          </IconButton>
          <Menu
            label="More about this person"
            className="-mr-2"
            trigger={<DotsThree size={20} weight="bold" />}
            items={[
              {
                label: flags?.vip ? 'Remove from favourites' : 'Add to favourites',
                onSelect: () =>
                  flags && void act(() => api.setPersonFlags(s.id, { ...flags, vip: !flags.vip })),
              },
              {
                label: flags?.muted ? 'Unmute notifications' : 'Mute notifications',
                onSelect: () =>
                  flags &&
                  void act(() => api.setPersonFlags(s.id, { ...flags, muted: !flags.muted })),
              },
              {
                label: flags?.whitelisted ? 'Stop always allowing' : 'Always allow',
                hint: 'skips rules',
                onSelect: () =>
                  flags &&
                  void act(() =>
                    api.setPersonFlags(s.id, { ...flags, whitelisted: !flags.whitelisted }),
                  ),
              },
              ...(s.kind === 'person'
                ? [
                    {
                      label: 'Merge into another person…',
                      separated: true,
                      onSelect: () => setMerging(true),
                    },
                  ]
                : []),
              {
                label: `Hide ${s.name.split(' ')[0]}`,
                separated: s.kind !== 'person',
                onSelect: () =>
                  void act(() => api.hidePerson(s.id, true)).then((ok) => {
                    if (ok) void navigate({ to: '/people' });
                  }),
              },
              {
                label: 'Hide for a week',
                onSelect: () =>
                  void act(() => api.hidePerson(s.id, true, now() + 7 * DAY)).then((ok) => {
                    if (ok) void navigate({ to: '/people' });
                  }),
              },
              {
                label: `Block ${s.name.split(' ')[0]}`,
                danger: true,
                onSelect: () =>
                  void act(() => api.decideSender(s.id, 'block')).then((ok) => {
                    if (ok) void navigate({ to: '/people' });
                  }),
              },
            ]}
          />
        </div>
        <div className="mt-1 flex items-start gap-3">
          <Stamp name={s.name} ink={s.ink} kind={s.kind} size={64} favourite={s.vip} />
          <div className="min-w-0 flex-1">
            <h2 dir="auto" className="line-clamp-2 text-title font-semibold tracking-[-0.01em]">
              {s.name}
            </h2>
            {subtitle && (
              <p dir="auto" className="mt-0.5 line-clamp-2 text-ui text-text-2">
                {subtitle}
              </p>
            )}
          </div>
        </div>
        {person.description && (
          <p dir="auto" className="mt-3 line-clamp-3 text-ui">
            {person.description}
          </p>
        )}
        <p className="mt-3 text-meta text-text-2">
          {plural(person.conversation_count, 'conversation')}, {plural(person.file_count, 'file')}
          {since ? `, writing since ${since}` : ''}
        </p>
        <ul aria-label="Email addresses" className="mt-1 flex flex-col gap-0.5">
          {person.addresses.map((address) => (
            <li key={address} className="flex items-center gap-2">
              <span dir="ltr" className="min-w-0 truncate text-meta text-text-2 select-text">
                {address}
              </span>
              <button
                type="button"
                onClick={() => void copyText(address, address)}
                className="shrink-0 rounded-[4px] px-1 text-meta text-blue hover:bg-hover"
                aria-label={`Copy ${address}`}
              >
                Copy
              </button>
            </li>
          ))}
        </ul>
        <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2">
          <Button
            variant="primary"
            onClick={() => void writeToPerson(s.id, s.name)}
            disabled={person.addresses.length === 0}
          >
            <PencilSimple size={15} />
            Write to {s.name.split(' ')[0]}
          </Button>
          {s.unread > 0 && (
            <button
              type="button"
              onClick={() => void act(() => api.markAllRead(s.id))}
              className="text-meta font-medium text-blue underline-offset-2 hover:underline"
            >
              Mark all {s.unread} unread as read
            </button>
          )}
        </div>
        <Tabs
          className="mt-5"
          label="About this person"
          panelId="person-panel"
          value={tab}
          onChange={setTab}
          items={[
            { value: 'conversations', label: 'Conversations' },
            { value: 'timeline', label: 'Timeline' },
            { value: 'files', label: 'Files', count: person.file_count },
            { value: 'notes', label: 'Notes' },
          ]}
        />
      </header>

      <div
        id="person-panel"
        role="tabpanel"
        aria-labelledby={`person-panel-tab-${tab}`}
        className="scroll-y min-h-0 flex-1 border-t border-rule"
      >
        {tab === 'conversations' &&
          groups.map((group) => (
            <section key={group.name} aria-label={group.name}>
              <h3 className="sticky top-0 z-10 bg-pane px-4 py-1.5 text-meta font-medium text-text-2">
                {group.name}
              </h3>
              {group.items.map((c) => (
                <ConversationRow
                  key={c.id}
                  conversation={c}
                  ink={s.ink}
                  nowTs={nowTs}
                  selected={c.id === selectedId}
                  onContext={onContextMenu(() =>
                    conversationMenu(c, {
                      open: () => openConversation(c.id),
                      afterRemove: () =>
                        c.id === selectedId &&
                        void navigate({
                          to: '/people/$personId',
                          params: { personId: String(personId) },
                        }),
                    }),
                  )}
                  onOpen={() => openConversation(c.id)}
                />
              ))}
            </section>
          ))}
        {tab === 'timeline' && <TimelineTab personId={personId} />}
        {tab === 'files' && <FilesTab personId={personId} />}
        {tab === 'notes' && <NotesTab personId={personId} />}
      </div>
      <EditPerson person={person} open={editing} onClose={() => setEditing(false)} />
      {merging && <MergeDialog id={personId} onClose={() => setMerging(false)} />}
      {menu}
    </section>
  );
}

function ConversationRow({
  conversation: c,
  ink,
  nowTs,
  selected,
  onOpen,
  onContext,
}: {
  conversation: ConversationSummary;
  ink: number;
  nowTs: number;
  selected: boolean;
  onOpen: () => void;
  onContext?: (e: React.MouseEvent) => void;
}) {
  const unread = c.unread > 0;
  return (
    <button
      type="button"
      aria-current={selected ? 'true' : undefined}
      onClick={onOpen}
      onContextMenu={onContext}
      className={cn(
        'skip-offscreen relative block w-full px-4 py-3 text-left transition-colors',
        selected ? 'bg-blue-wash' : 'hover:bg-hover',
      )}
    >
      {selected && <span className="absolute inset-y-3 left-0 w-[3px] rounded-r-full bg-blue" />}
      <span className="flex items-baseline gap-2">
        {unread && (
          <span
            className="size-2 shrink-0 self-center rounded-full"
            style={{ background: stampColor(ink) }}
            role="img"
            aria-label="Unread"
          />
        )}
        <span
          dir="auto"
          className={cn('truncate text-body', unread ? 'font-semibold' : 'font-medium')}
        >
          {c.subject}
        </span>
        <time className="tabular ml-auto shrink-0 font-stretch-semi-condensed text-meta text-text-2">
          {formatListTime(c.last_at, nowTs)}
        </time>
      </span>
      <span dir="auto" className="mt-0.5 line-clamp-1 text-ui text-text-2">
        {c.snippet}
      </span>
      <span className="mt-1.5 flex items-center gap-3 text-meta text-text-2">
        {c.status === 'needs_reply' && (
          <span className="inline-flex items-center gap-1.5">
            <span className="size-2 rounded-full bg-red" />
            Needs reply
          </span>
        )}
        {c.status === 'waiting_on_them' && (
          <span className="inline-flex items-center gap-1.5">
            <span className="size-2 rounded-full border border-text-2" />
            Waiting on them
          </span>
        )}
        <span className="tabular">{plural(c.message_count, 'message')}</span>
        {c.has_files && <Paperclip size={13} aria-label="Has files" />}
        {c.others.length > 0 && (
          <span dir="auto" className="truncate">
            with {c.others.map((o) => o.name.replace(/\s*[([].*$/u, '').split(' ')[0]).join(', ')}
          </span>
        )}
      </span>
    </button>
  );
}

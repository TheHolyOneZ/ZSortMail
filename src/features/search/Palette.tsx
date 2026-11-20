import {
  ArrowRight,
  EyeSlash,
  Funnel,
  GearSix,
  HourglassMedium,
  MagnifyingGlass,
  NotePencil,
  Repeat,
  Sun,
  Tray,
  UserPlus,
  UsersThree,
} from '@phosphor-icons/react';
import { useNavigate } from '@tanstack/react-router';
import { type ComponentType, useEffect, useId, useMemo, useRef, useState } from 'react';
import { api } from '@/lib/api';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { compose } from '@/lib/composer';
import { formatListTime } from '@/lib/format';
import { usePalette } from '@/lib/palette';
import { useSearch } from '@/lib/queries';
import { toast, toastError } from '@/lib/toasts';
import { Kbd } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Stamp } from '@/ui/Stamp';
import { highlight } from './highlight';

type Go =
  | '/today'
  | '/people'
  | '/river'
  | '/new-senders'
  | '/hidden'
  | '/rules'
  | '/drafts'
  | '/waiting'
  | '/routines'
  | '/settings';

interface Command {
  id: string;
  label: string;
  hint: string;
  words: string;
  icon: ComponentType<{ size?: number }>;
  to?: Go;
  run?: () => void;
}

const COMMANDS: Command[] = [
  {
    id: 'compose',
    label: 'New message',
    hint: 'Write an email',
    words: 'new message compose write email',
    icon: NotePencil,
    run: () => void compose(),
  },
  {
    id: 'today',
    label: 'Go to Today',
    hint: 'What needs you',
    words: 'today home',
    icon: Sun,
    to: '/today',
  },
  {
    id: 'people',
    label: 'Go to People',
    hint: 'Everyone you write to',
    words: 'people contacts',
    icon: UsersThree,
    to: '/people',
  },
  {
    id: 'mail',
    label: 'Go to Mail',
    hint: 'Every message, newest first',
    words: 'mail inbox messages',
    icon: Tray,
    to: '/river',
  },
  {
    id: 'new',
    label: 'Go to New senders',
    hint: 'Allow, hide or block',
    words: 'new senders screener',
    icon: UserPlus,
    to: '/new-senders',
  },
  {
    id: 'hidden',
    label: 'Go to Hidden',
    hint: 'What you put out of sight',
    words: 'hidden blocked unhide',
    icon: EyeSlash,
    to: '/hidden',
  },
  {
    id: 'drafts',
    label: 'Go to Drafts',
    hint: 'Emails you started',
    words: 'drafts unsent started scheduled',
    icon: NotePencil,
    to: '/drafts',
  },
  {
    id: 'rules',
    label: 'Go to Rules',
    hint: 'Sort mail as it arrives',
    words: 'rules filters automate sort',
    icon: Funnel,
    to: '/rules',
  },
  {
    id: 'waiting',
    label: 'Go to Waiting for',
    hint: 'Mail you expect',
    words: 'waiting watch alerts',
    icon: HourglassMedium,
    to: '/waiting',
  },
  {
    id: 'routines',
    label: 'Go to Routines',
    hint: 'Recurring emails',
    words: 'routines recurring',
    icon: Repeat,
    to: '/routines',
  },
  {
    id: 'settings',
    label: 'Open Settings',
    hint: 'Theme, density, motion',
    words: 'settings preferences theme',
    icon: GearSix,
    to: '/settings',
  },
];

type Row =
  | { kind: 'command'; key: string; command: Command }
  | { kind: 'watch'; key: string; text: string }
  | { kind: 'person'; key: string; personId: number }
  | { kind: 'message'; key: string; personId: number | null; conversationId: number };

export function Palette() {
  const open = usePalette((s) => s.open);
  const hide = usePalette((s) => s.hide);
  const initial = usePalette((s) => s.initial);
  return (
    <Dialog
      open={open}
      onClose={hide}
      label="Search and commands"
      placement="top"
      className="[--dialog-w:640px]"
    >
      <PaletteBody key={String(open)} initial={initial} onClose={hide} />
    </Dialog>
  );
}

function PaletteBody({ initial, onClose }: { initial: string; onClose: () => void }) {
  const navigate = useNavigate();
  const [query, setQuery] = useState(initial);
  const [active, setActive] = useState(0);
  const listId = useId();
  const input = useRef<HTMLInputElement>(null);
  const { data } = useSearch(query);
  const nowTs = now();

  useEffect(() => input.current?.focus(), []);

  const q = query.trim().toLowerCase();
  const commands = useMemo(
    () =>
      COMMANDS.filter((c) => q === '' || `${c.label} ${c.words}`.toLowerCase().includes(q)).slice(
        0,
        q === '' ? 7 : 3,
      ),
    [q],
  );

  const watchText = /^watch\s+(.+)$/i.exec(query.trim())?.[1]?.trim();

  const rows = useMemo<Row[]>(() => {
    const out: Row[] = [];
    if (watchText) out.push({ kind: 'watch', key: 'watch', text: watchText });
    out.push(
      ...commands.map((command): Row => ({ kind: 'command', key: `c-${command.id}`, command })),
    );
    for (const p of data?.people ?? [])
      out.push({ kind: 'person', key: `p-${p.id}`, personId: p.id });
    for (const m of data?.messages ?? []) {
      out.push({
        kind: 'message',
        key: `m-${m.message_id}`,
        personId: m.person_id,
        conversationId: m.conversation_id,
      });
    }
    return out;
  }, [commands, data, watchText]);

  const last = Math.max(rows.length - 1, 0);
  const current = Math.min(active, last);

  useEffect(() => {
    document.getElementById(`${listId}-${current}`)?.scrollIntoView({ block: 'nearest' });
  }, [current, listId]);

  function choose(row: Row | undefined) {
    if (!row) return;
    onClose();
    if (row.kind === 'watch') {
      api
        .saveWatch({
          id: null,
          name: '',
          query: row.text,
          mode: 'once',
          priority: 'normal',
          expires_at: null,
          include_existing: false,
        })
        .then(() => {
          toast(`Waiting for “${row.text}”.`);
          void navigate({ to: '/waiting' });
        })
        .catch(toastError);
    } else if (row.kind === 'command') {
      if (row.command.run) row.command.run();
      else if (row.command.to) void navigate({ to: row.command.to });
    } else if (row.kind === 'person') {
      void navigate({ to: '/people/$personId', params: { personId: String(row.personId) } });
    } else if (row.personId !== null) {
      void navigate({
        to: '/people/$personId/c/$conversationId',
        params: { personId: String(row.personId), conversationId: String(row.conversationId) },
      });
    }
  }

  const peopleById = new Map((data?.people ?? []).map((p) => [p.id, p]));
  const messagesById = new Map((data?.messages ?? []).map((m) => [`m-${m.message_id}`, m]));
  const searching = q.length > 0;
  const nothing = searching && data !== undefined && rows.length === 0;

  return (
    <div className="overflow-hidden rounded-pop bg-sheet shadow-lift">
      <div className="flex items-center gap-3 border-b border-rule px-4">
        <MagnifyingGlass size={18} className="shrink-0 text-text-2" />
        <input
          ref={input}
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-activedescendant={rows.length > 0 ? `${listId}-${current}` : undefined}
          aria-label="Search people and messages"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setActive(Math.min(current + 1, last));
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              setActive(Math.max(current - 1, 0));
            } else if (e.key === 'Enter') {
              e.preventDefault();
              choose(rows[current]);
            }
          }}
          placeholder="Search people and messages, or go somewhere"
          autoComplete="off"
          spellCheck={false}
          className="h-12 w-full bg-transparent text-body text-text outline-none placeholder:text-text-2"
        />
        <Kbd>Esc</Kbd>
      </div>

      <div
        id={listId}
        role="listbox"
        aria-label="Results"
        className="scroll-y max-h-[min(56vh,440px)] py-1"
      >
        {rows.map((row, i) => {
          const selected = i === current;
          const base = cn(
            'flex w-full items-center gap-3 px-4 py-2 text-left',
            selected ? 'bg-blue-wash' : 'hover:bg-hover',
          );
          if (row.kind === 'watch') {
            return (
              <Option
                key={row.key}
                id={`${listId}-${i}`}
                selected={selected}
                onPick={() => choose(row)}
                className={base}
              >
                <span className="grid size-8 shrink-0 place-items-center rounded-control bg-desk text-text-2">
                  <HourglassMedium size={18} />
                </span>
                <span className="min-w-0 flex-1">
                  <span dir="auto" className="block truncate text-body font-medium">
                    Wait for “{row.text}”
                  </span>
                  <span className="block truncate text-meta text-text-2">
                    Tell me the moment it arrives
                  </span>
                </span>
                <ArrowRight size={14} className="shrink-0 text-text-3" />
              </Option>
            );
          }
          if (row.kind === 'command') {
            const Icon = row.command.icon;
            return (
              <Option
                key={row.key}
                id={`${listId}-${i}`}
                selected={selected}
                onPick={() => choose(row)}
                className={base}
              >
                <span className="grid size-8 shrink-0 place-items-center rounded-control bg-desk text-text-2">
                  <Icon size={18} />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-body font-medium">{row.command.label}</span>
                  <span className="block truncate text-meta text-text-2">{row.command.hint}</span>
                </span>
                <ArrowRight size={14} className="shrink-0 text-text-3" />
              </Option>
            );
          }
          if (row.kind === 'person') {
            const p = peopleById.get(row.personId);
            if (!p) return null;
            return (
              <Option
                key={row.key}
                id={`${listId}-${i}`}
                selected={selected}
                onPick={() => choose(row)}
                className={base}
              >
                <Stamp name={p.name} ink={p.ink} kind={p.kind} size={32} />
                <span className="min-w-0 flex-1">
                  <span dir="auto" className="block truncate text-body font-medium">
                    {highlight(p.name, query)}
                  </span>
                  <span dir="auto" className="block truncate text-meta text-text-2">
                    {p.last_subject}
                  </span>
                </span>
                <span className="shrink-0 text-meta text-text-2">Person</span>
              </Option>
            );
          }
          const m = messagesById.get(row.key);
          if (!m) return null;
          return (
            <Option
              key={row.key}
              id={`${listId}-${i}`}
              selected={selected}
              onPick={() => choose(row)}
              className={base}
            >
              <Stamp name={m.from.name} ink={m.from.ink} kind={m.from.kind} size={32} />
              <span className="min-w-0 flex-1">
                <span dir="auto" className="block truncate text-body font-medium">
                  {highlight(m.subject, query)}
                </span>
                <span dir="auto" className="block truncate text-meta text-text-2">
                  {m.from.is_me ? 'You' : m.from.name.split(' ')[0]}: {highlight(m.snippet, query)}
                </span>
              </span>
              <time className="tabular shrink-0 text-meta text-text-2">
                {formatListTime(m.sent_at, nowTs)}
              </time>
            </Option>
          );
        })}
        {nothing && (
          <p className="px-4 py-6 text-center text-ui text-text-2">
            Nothing matches “{query.trim()}”. Search looks at names, addresses, subjects and message
            text.
          </p>
        )}
      </div>
    </div>
  );
}

function Option({
  id,
  selected,
  onPick,
  className,
  children,
}: {
  id: string;
  selected: boolean;
  onPick: () => void;
  className: string;
  children: React.ReactNode;
}) {
  return (
    <div
      id={id}
      role="option"
      aria-selected={selected}
      tabIndex={-1}
      onClick={onPick}
      onKeyDown={() => {}}
      className={cn(className, 'cursor-pointer')}
    >
      {children}
    </div>
  );
}

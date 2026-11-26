import {
  Archive,
  BookmarkSimple,
  EnvelopeSimple,
  EyeSlash,
  PaperPlaneTilt,
  PencilSimpleLine,
  Star,
  Tag,
  Trash,
  Tray,
  X,
} from '@phosphor-icons/react';
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { BulkAction, Category, ConversationRow } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { compose } from '@/lib/composer';
import { formatListTime, plural } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { useMailSearch } from '@/lib/mailSearch';
import { useInbox, useSavedViews, useTags, useView } from '@/lib/queries';
import { cleanSidebar, setLabelShown, visibleLabels } from '@/lib/sidebar';
import { useUi } from '@/lib/store';
import { toast, toastError } from '@/lib/toasts';
import type { Key } from '@/locales/en';
import { Button, Chip, IconButton, SearchField } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Field, Modal } from '@/ui/Modal';
import { SyncNotice } from '@/ui/SyncNotice';
import { StrandView } from '../strand/StrandView';
import { ConversationLine } from '../today/ConversationLine';
import { SnoozeMenu } from '../today/SnoozeMenu';

const TABS: { value: Category | null; label: Key; index: number | null }[] = [
  { value: null, label: 'mailtab.all', index: null },
  { value: 'person', label: 'mailtab.people', index: 0 },
  { value: 'newsletter', label: 'mailtab.newsletters', index: 1 },
  { value: 'receipt', label: 'mailtab.receipts', index: 2 },
  { value: 'automated', label: 'mailtab.automated', index: 3 },
];

const PAGE = 100;

const BOXES: { id: string; label: Key; query: string; icon: typeof Tray }[] = [
  { id: 'inbox', label: 'mail.inbox', query: '', icon: Tray },
  { id: 'unread', label: 'mail.unread', query: 'is:unread in:inbox', icon: EnvelopeSimple },
  { id: 'starred', label: 'mail.starred', query: 'is:starred', icon: Star },
  { id: 'sent', label: 'mail.sent', query: 'is:sent', icon: PaperPlaneTilt },
  { id: 'archived', label: 'mail.all', query: 'is:archived OR in:inbox', icon: Archive },
  { id: 'trash', label: 'mail.trash', query: 'in:trash', icon: Trash },
];

const WIDE = '(min-width: 1180px)';

function useWide(): boolean {
  return useSyncExternalStore(
    (notify) => {
      const m = window.matchMedia(WIDE);
      m.addEventListener('change', notify);
      return () => m.removeEventListener('change', notify);
    },
    () => window.matchMedia(WIDE).matches,
    () => false,
  );
}

function labelQuery(name: string): string {
  return /\s/.test(name) ? `label:"${name}"` : `label:${name}`;
}

const BULK: { action: BulkAction; label: Key }[] = [
  { action: 'read', label: 'bulk.read' },
  { action: 'archive', label: 'bulk.archive' },
  { action: 'hide', label: 'bulk.hide' },
  { action: 'trash', label: 'bulk.trash' },
];

export function MailScreen() {
  const t = useT();
  const [category, setCategory] = useState<Category | null>(null);
  const [limit, setLimit] = useState(PAGE);
  const query = useMailSearch((s) => s.query);
  const setQuery = useMailSearch((s) => s.set);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [naming, setNaming] = useState(false);
  const [viewName, setViewName] = useState('');
  const searching = query.trim().length > 0;
  const [box, setBox] = useState('inbox');
  const [reading, setReading] = useState<{ id: number; person: number | null } | null>(null);
  const wide = useWide();
  const { data: allTags = [] } = useTags();
  const sidebar = cleanSidebar(useUi((u) => u.sidebar));
  const setSidebar = useUi((u) => u.setSidebar);
  const tags = visibleLabels(allTags, sidebar);
  const labelBox = box.startsWith('label:') ? box.slice(6) : null;
  const boxQuery = labelBox ? labelQuery(labelBox) : (BOXES.find((b) => b.id === box)?.query ?? '');
  const inInbox = box === 'inbox' && !searching;

  const inbox = useInbox(category, limit);
  const found = useView(searching ? query : '');
  const boxed = useView(searching || inInbox ? '' : boxQuery);
  const { data: views = [] } = useSavedViews();

  const rows: ConversationRow[] = searching
    ? (found.data ?? [])
    : inInbox
      ? (inbox.data?.rows ?? [])
      : (boxed.data ?? []);
  const counts = inbox.data?.counts ?? [];
  const total = counts.reduce((a, b) => a + b, 0);
  const ready = searching
    ? found.data !== undefined
    : inInbox
      ? inbox.data !== undefined
      : boxed.data !== undefined;
  const visibleIds = useMemo(() => new Set(rows.map((r) => r.conversation.id)), [rows]);
  const selected = [...picked].filter((id) => visibleIds.has(id));

  const last = useRef<number | null>(null);
  function toggle(id: number, shift: boolean) {
    const order = rows.map((r) => r.conversation.id);
    const from = last.current;
    setPicked((prev) => {
      const next = new Set(prev);
      if (shift && from !== null && order.includes(from)) {
        const [a, b] = [order.indexOf(from), order.indexOf(id)].sort((x, y) => x - y) as [
          number,
          number,
        ];
        const on = !prev.has(id);
        for (const between of order.slice(a, b + 1)) {
          if (on) next.add(between);
          else next.delete(between);
        }
      } else if (!next.delete(id)) next.add(id);
      return next;
    });
    last.current = id;
  }

  const allOn = rows.length > 0 && selected.length === rows.length;
  const someOn = selected.length > 0 && !allOn;
  const allBox = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (allBox.current) allBox.current.indeterminate = someOn;
  }, [someOn]);

  async function run(action: BulkAction) {
    const ids = selected;
    setPicked(new Set());
    await act(() => api.bulkConversations(ids, action));
  }

  async function saveView() {
    try {
      await api.saveView(viewName, query);
      toast(`Saved the view “${viewName.trim() || query.trim()}”.`);
      setNaming(false);
      setViewName('');
    } catch (e) {
      toastError(e);
    }
  }

  return (
    <section className="flex h-full min-w-0" aria-label="Mail">
      <nav
        aria-label="Mailboxes"
        className="scroll-y hidden w-[190px] shrink-0 border-e border-rule bg-desk px-2 py-6 sm:block"
      >
        <ul className="flex flex-col gap-0.5">
          {BOXES.map((b) => {
            const Icon = b.icon;
            const on = box === b.id && !searching;
            return (
              <li key={b.id}>
                <button
                  type="button"
                  aria-current={on ? 'true' : undefined}
                  onClick={() => {
                    setBox(b.id);
                    setQuery('');
                    setPicked(new Set());
                    setLimit(PAGE);
                  }}
                  className={cn(
                    'flex h-10 w-full items-center gap-3 rounded-control px-3 text-start text-ui font-medium transition-colors',
                    on ? 'bg-blue-wash text-text' : 'text-text-2 hover:bg-hover hover:text-text',
                  )}
                >
                  <Icon size={18} weight={on ? 'fill' : 'regular'} />
                  <span className="min-w-0 flex-1 truncate">{t(b.label)}</span>
                  {b.id === 'inbox' && total > 0 && (
                    <span className="tabular text-meta text-text-2">{total}</span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
        {tags.length > 0 && (
          <>
            <div className="mt-4 mb-1 ps-3 text-meta font-medium text-text-2">
              {t('mail.labels')}
            </div>
            <ul className="flex flex-col gap-0.5">
              {tags.map((tag) => {
                const on = box === `label:${tag.name}` && !searching;
                return (
                  <li key={tag.id} className="group relative">
                    <button
                      type="button"
                      aria-current={on ? 'true' : undefined}
                      onClick={() => {
                        setBox(`label:${tag.name}`);
                        setQuery('');
                        setPicked(new Set());
                      }}
                      className={cn(
                        'flex h-10 w-full items-center gap-3 rounded-control ps-3 pe-10 text-start text-ui font-medium transition-colors',
                        on
                          ? 'bg-blue-wash text-text'
                          : 'text-text-2 hover:bg-hover hover:text-text',
                      )}
                    >
                      <Tag size={18} weight={on ? 'fill' : 'regular'} />
                      <span className="min-w-0 flex-1 truncate">{tag.name}</span>
                    </button>
                    <IconButton
                      label={t('mail.hide_label')}
                      className="absolute end-1 top-1 size-8 opacity-0 focus-visible:opacity-100 group-hover:opacity-100"
                      onClick={() => {
                        if (on) setBox('inbox');
                        setSidebar(setLabelShown(sidebar, tag.name, false));
                      }}
                    >
                      <EyeSlash size={16} />
                    </IconButton>
                  </li>
                );
              })}
            </ul>
          </>
        )}
      </nav>
      <div className="scroll-y min-w-0 flex-1 bg-pane px-6 py-6">
        <div className="flex items-center justify-between gap-3">
          <h1 className="text-title font-semibold tracking-[-0.01em]">{t('mail.title')}</h1>
          <Button variant="primary" onClick={() => void compose()}>
            <PencilSimpleLine size={15} />
            {t('nav.write')}
          </Button>
        </div>
        <SyncNotice className="mt-3" />
        <p className="mt-1 text-ui text-text-2">
          {inInbox
            ? 'Every conversation in your inbox, newest first. Archived mail is still under each person.'
            : searching
              ? 'Results from all your mail.'
              : `${labelBox ?? t(BOXES.find((b) => b.id === box)?.label ?? 'nav.mail')}, newest first.`}
        </p>

        <SearchField
          className="mt-4"
          label={t('mail.search')}
          value={query}
          onChange={(v) => {
            setQuery(v);
            setPicked(new Set());
          }}
        />

        <div className="mt-3 flex flex-wrap items-center gap-2">
          {views.map((v) => (
            <span
              key={v.id}
              className={cn(
                'inline-flex items-center rounded-full border text-ui',
                query === v.query ? 'border-blue bg-blue-wash' : 'border-rule-strong',
              )}
            >
              <button
                type="button"
                onClick={() => setQuery(v.query)}
                title={v.query}
                className="inline-flex items-center gap-1.5 rounded-s-full py-1 ps-3 pe-2 hover:bg-hover"
              >
                <BookmarkSimple size={13} weight="fill" className="text-text-2" />
                {v.name}
              </button>
              <button
                type="button"
                aria-label={`Remove the view ${v.name}`}
                onClick={() => void api.deleteView(v.id).catch(toastError)}
                className="grid size-7 place-items-center rounded-e-full text-text-2 hover:bg-hover hover:text-text"
              >
                <X size={11} weight="bold" />
              </button>
            </span>
          ))}
          {searching && !views.some((v) => v.query === query.trim()) && (
            <Button onClick={() => setNaming(true)}>
              <BookmarkSimple size={14} />
              Save as a view
            </Button>
          )}
        </div>

        {inInbox && (
          <fieldset className="mt-4 flex flex-wrap gap-2 border-0 p-0">
            <legend className="sr-only">Show</legend>
            {TABS.map((tab) => (
              <Chip
                key={tab.label}
                active={category === tab.value}
                count={tab.index === null ? total : counts[tab.index]}
                onClick={() => {
                  setCategory(tab.value);
                  setLimit(PAGE);
                  setPicked(new Set());
                }}
              >
                {t(tab.label)}
              </Chip>
            ))}
          </fieldset>
        )}

        <div className="mt-4 flex items-center gap-1 border-b border-rule pb-1">
          <label className="flex h-10 cursor-pointer items-center gap-3 pe-3 ps-3 text-ui font-medium">
            <input
              ref={allBox}
              type="checkbox"
              aria-label="Select all conversations"
              checked={allOn}
              disabled={rows.length === 0}
              onChange={() =>
                setPicked(allOn ? new Set() : new Set(rows.map((r) => r.conversation.id)))
              }
              className="size-[20px]"
            />
            {selected.length > 0
              ? `${plural(selected.length, 'conversation')} selected`
              : t('mail.select_all')}
          </label>
        </div>

        {selected.length > 0 && (
          <div
            role="toolbar"
            aria-label="Selected conversations"
            className="sticky top-0 z-20 mt-2 flex flex-wrap items-center gap-2 rounded-control border border-rule-strong bg-sheet px-3 py-2 shadow-sheet"
          >
            <span className="me-auto text-ui font-medium">{t('bulk.what')}</span>
            {BULK.map((b) => (
              <Button key={b.action} onClick={() => void run(b.action)}>
                {t(b.label)}
              </Button>
            ))}
            <Button onClick={() => setPicked(new Set())}>{t('bulk.clear')}</Button>
          </div>
        )}

        <ul className="border-t border-rule" aria-label="Conversations">
          {rows.map((row) => {
            const id = row.conversation.id;
            return (
              <ConversationLine
                key={id}
                row={row}
                selected={wide && reading?.id === id}
                onOpen={wide ? () => setReading({ id, person: row.person_id }) : undefined}
                meta={formatListTime(row.conversation.last_at, now())}
                actions={
                  <>
                    <IconButton
                      label="Archive"
                      className="size-[40px]"
                      onClick={() => void act(() => api.archiveConversation(id, true))}
                    >
                      <Archive size={20} />
                    </IconButton>
                    <SnoozeMenu conversationId={id} big />
                    <IconButton
                      label="Move to trash"
                      className="size-[40px]"
                      onClick={() => void act(() => api.trashConversation(id, true))}
                    >
                      <Trash size={20} />
                    </IconButton>
                  </>
                }
                leading={
                  <label className="grid size-[40px] shrink-0 cursor-pointer place-items-center">
                    <span className="sr-only">Select {row.conversation.subject}</span>
                    <input
                      type="checkbox"
                      checked={picked.has(id)}
                      onChange={() => {}}
                      onClick={(e) => toggle(id, e.shiftKey)}
                      className="size-[20px]"
                    />
                  </label>
                }
              />
            );
          })}
        </ul>
        {rows.length === 0 && ready && (
          <p className="mt-8 text-ui text-text-2">
            {searching
              ? 'Nothing matches that search. Try from:, subject:, has:attachment, is:unread, newer_than:7d.'
              : 'Nothing here. Everything in this view has been dealt with.'}
          </p>
        )}
        {!searching && rows.length >= limit && (
          <div className="mt-4">
            <Button onClick={() => setLimit(limit + PAGE)}>{t('mail.show_more')}</Button>
          </div>
        )}
      </div>
      {wide && (
        <div className="w-[46%] min-w-[420px] max-w-[760px] shrink-0 border-s border-rule">
          <StrandView conversationId={reading?.id} personId={reading?.person ?? undefined} />
        </div>
      )}

      <Dialog open={naming} onClose={() => setNaming(false)} label="Save as a view">
        <Modal
          title="Save as a view"
          description="A view is a search you keep. It appears as a button on this screen."
          onClose={() => setNaming(false)}
          onSubmit={(e) => {
            e.preventDefault();
            void saveView();
          }}
          footer={
            <>
              <Button onClick={() => setNaming(false)}>Cancel</Button>
              <Button type="submit" variant="primary">
                Save view
              </Button>
            </>
          }
        >
          <Field label="Name" hint={`Searches for: ${query.trim()}`}>
            <input
              value={viewName}
              onChange={(e) => setViewName(e.target.value)}
              placeholder="Parcels"
              autoFocus
            />
          </Field>
        </Modal>
      </Dialog>
    </section>
  );
}

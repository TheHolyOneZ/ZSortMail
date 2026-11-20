import { CaretLeft, PushPin, UserPlus } from '@phosphor-icons/react';
import { useNavigate } from '@tanstack/react-router';
import { useVirtualizer } from '@tanstack/react-virtual';
import { LayoutGroup, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import { useBackend } from '@/lib/backend';
import type { PeopleFilter, PersonSummary } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { formatListTime } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { stampColor } from '@/lib/ink';
import { personMenu } from '@/lib/menus';
import { useGroups, usePeopleIn } from '@/lib/queries';
import { rowHeights, useUi } from '@/lib/store';
import type { Key } from '@/locales/en';
import { useMotionGovernor } from '@/motion/MotionProvider';
import { settleSpring } from '@/motion/tokens';
import { useContextMenu } from '@/ui/ContextMenu';
import { Button, Chip, SearchField, Segmented } from '@/ui/controls';
import { Stamp } from '@/ui/Stamp';
import { PeopleSkeleton, SyncNotice } from '@/ui/SyncNotice';
import { EditPersonById } from '../person/EditPerson';
import { MergeDialog } from '../person/MergeDialog';
import { CategoryEditor } from './CategoryEditor';
import { CategoryGallery } from './CategoryGallery';
import { CategoryPeople } from './CategoryPeople';
import { PersonCategories } from './PersonCategories';

const FILTERS: Array<{ value: PeopleFilter; label: Key }> = [
  { value: 'all', label: 'filter.all' },
  { value: 'needs_reply', label: 'filter.needs_reply' },
  { value: 'waiting_on_them', label: 'filter.waiting' },
  { value: 'has_files', label: 'filter.files' },
  { value: 'pinned', label: 'filter.pinned' },
];

function emptyMessage(filter: PeopleFilter, query: string): string {
  if (query.trim()) return `No one matches “${query.trim()}”.`;
  switch (filter) {
    case 'needs_reply':
      return 'Nobody is waiting on a reply from you.';
    case 'waiting_on_them':
      return 'Nobody owes you a reply.';
    case 'has_files':
      return 'No files from anyone yet.';
    case 'pinned':
      return 'Pin someone to keep them at the top.';
    default:
      return 'No people yet. Link a Gmail account to see everyone you write with.';
  }
}

export function PeoplePane({ selectedId }: { selectedId?: number }) {
  const category = useUi((s) => s.peopleCategory);
  const first = useUi((s) => s.categoriesFirst);
  const picker = category === 'gallery' || (category === null && first && selectedId === undefined);
  if (picker) return <CategoryGallery />;
  const shown = category === null || category === 'gallery' ? 'everyone' : category;
  return <PeopleList category={shown} selectedId={selectedId} />;
}

function PeopleList({ category, selectedId }: { category: string; selectedId?: number }) {
  const t = useT();
  const setCategory = useUi((s) => s.setPeopleCategory);
  const { data: groups = [] } = useGroups();
  const group = groups.find((g) => `group:${g.id}` === category);
  const title =
    category === 'favourites' ? t('categories.favourites') : group ? group.name : t('people.title');
  const filter = useUi((s) => s.peopleFilter);
  const query = useUi((s) => s.peopleQuery);
  const setFilter = useUi((s) => s.setPeopleFilter);
  const setQuery = useUi((s) => s.setPeopleQuery);
  const density = useUi((s) => s.density);
  const view = useUi((s) => s.peopleView);
  const setView = useUi((s) => s.setPeopleView);
  const rowHeight = rowHeights[density].people;

  const { data: people = [] } = usePeopleIn(category, filter, query);
  const { data: everyone = [] } = usePeopleIn(category, 'all', '');
  const counts = {
    needs_reply: everyone.filter((p) => p.needs_reply).length,
    waiting_on_them: everyone.filter((p) => p.waiting_on_them).length,
  };

  const navigate = useNavigate();
  const loading = useBackend((b) => b.sync.phase === 'reading');
  const { onContextMenu, menu } = useContextMenu();
  const [editing, setEditing] = useState<number | null>(null);
  const [merging, setMerging] = useState<number | null>(null);
  const [categorising, setCategorising] = useState<number | null>(null);
  const [adding, setAdding] = useState(false);
  const [editingGroup, setEditingGroup] = useState(false);
  const unreadTotal = everyone.reduce((n, p) => n + p.unread, 0);
  const menuFor = (p: PersonSummary) =>
    onContextMenu(() =>
      personMenu(p, {
        open: () => open(p),
        edit: () => setEditing(p.id),
        merge: () => setMerging(p.id),
        categories: () => setCategorising(p.id),
        afterHide: () => p.id === selectedId && void navigate({ to: '/people' }),
      }),
    );
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: people.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 8,
  });

  const selectedIndex = people.findIndex((p) => p.id === selectedId);

  function open(person: PersonSummary) {
    navigate({ to: '/people/$personId', params: { personId: String(person.id) } });
  }

  function onKeyDown(event: React.KeyboardEvent) {
    if (people.length === 0) return;
    let next = -1;
    if (event.key === 'ArrowDown') next = Math.min(people.length - 1, selectedIndex + 1);
    else if (event.key === 'ArrowUp') next = Math.max(0, selectedIndex - 1);
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = people.length - 1;
    const target = next >= 0 ? people[next] : undefined;
    if (!target) return;
    event.preventDefault();
    virtualizer.scrollToIndex(next);
    open(target);
  }

  useEffect(() => {
    if (selectedIndex >= 0) virtualizer.scrollToIndex(selectedIndex, { align: 'auto' });
  }, [selectedIndex, virtualizer]);

  const nowTs = now();

  return (
    <section className="flex h-full min-w-0 flex-col bg-pane" aria-label="People">
      <header className="shrink-0 px-4 pt-4 pb-3">
        <div className="flex items-baseline justify-between">
          <h2 className="flex min-w-0 items-center gap-1 text-name font-semibold">
            <button
              type="button"
              aria-label={t('categories.back')}
              title={t('categories.back')}
              onClick={() => setCategory('gallery')}
              className="-ms-2 grid size-[36px] shrink-0 place-items-center self-center rounded-control text-text-2 hover:bg-hover hover:text-text"
            >
              <CaretLeft size={16} weight="bold" />
            </button>
            <span className="truncate">{title}</span>
          </h2>
          <span className="flex items-center gap-3">
            <span className="tabular text-meta text-text-2">{everyone.length}</span>
            <Segmented
              label="Show people as"
              value={view}
              onChange={setView}
              options={[
                { value: 'list', label: t('people.view_list') },
                { value: 'wall', label: t('people.view_wall') },
              ]}
            />
          </span>
        </div>
        {group && (
          <div className="mt-3 flex flex-wrap items-center gap-2">
            <Button variant="primary" onClick={() => setAdding(true)}>
              <UserPlus size={14} />
              {t('people.add')}
            </Button>
            <Button onClick={() => setEditingGroup(true)}>{t('people.edit_category')}</Button>
          </div>
        )}
        <SyncNotice className="mt-3" />
        <SearchField
          className="mt-3"
          label={t('people.search')}
          value={query}
          onChange={setQuery}
        />
        {unreadTotal > 0 && (
          <button
            type="button"
            onClick={() => void act(() => api.markAllRead())}
            className="mt-2 text-meta font-medium text-blue underline-offset-2 hover:underline"
          >
            {t('people.mark_all', { count: unreadTotal })}
          </button>
        )}
        <div className="mt-3 flex flex-wrap gap-1.5">
          {FILTERS.map((f) => (
            <Chip
              key={f.value}
              active={filter === f.value}
              count={
                f.value === 'needs_reply' || f.value === 'waiting_on_them'
                  ? counts[f.value]
                  : undefined
              }
              onClick={() => setFilter(f.value)}
            >
              {t(f.label)}
            </Chip>
          ))}
        </div>
      </header>

      {people.length === 0 && loading ? (
        <PeopleSkeleton />
      ) : people.length === 0 ? (
        <div className="px-4 py-8 text-ui text-text-2">
          <p>
            {group && !query.trim() && filter === 'all'
              ? 'Nobody is in this category yet.'
              : emptyMessage(filter, query)}
          </p>
          {group && !query.trim() && filter === 'all' && (
            <Button className="mt-3" variant="primary" onClick={() => setAdding(true)}>
              <UserPlus size={14} />
              {t('people.add')}
            </Button>
          )}
        </div>
      ) : view === 'wall' ? (
        <LayoutGroup id="people">
          <Wall
            people={people}
            selectedId={selectedId}
            nowTs={nowTs}
            onOpen={open}
            menuFor={menuFor}
          />
        </LayoutGroup>
      ) : (
        <div
          ref={scrollRef}
          role="listbox"
          aria-label="People"
          tabIndex={0}
          onKeyDown={onKeyDown}
          className="scroll-y min-h-0 flex-1 outline-none focus-visible:outline-none"
        >
          <LayoutGroup id="people">
            <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
              {virtualizer.getVirtualItems().map((item) => {
                const person = people[item.index];
                if (!person) return null;
                return (
                  <PersonRow
                    key={person.id}
                    person={person}
                    selected={person.id === selectedId}
                    nowTs={nowTs}
                    onOpen={() => open(person)}
                    onContext={menuFor(person)}
                    shared={item.index < SHARED}
                    style={{ height: item.size, transform: `translateY(${item.start}px)` }}
                  />
                );
              })}
            </div>
          </LayoutGroup>
        </div>
      )}
      {menu}
      {editing !== null && <EditPersonById id={editing} onClose={() => setEditing(null)} />}
      {merging !== null && <MergeDialog id={merging} onClose={() => setMerging(null)} />}
      {categorising !== null && (
        <PersonCategories id={categorising} onClose={() => setCategorising(null)} />
      )}
      {adding && group && (
        <CategoryPeople id={group.id} name={group.name} onClose={() => setAdding(false)} />
      )}
      {editingGroup && group && (
        <CategoryEditor editing={group} onClose={() => setEditingGroup(false)} />
      )}
    </section>
  );
}

const SHARED = 36;
const WALL_LIMIT = 300;

function TravellingStamp({
  person,
  size,
  shared,
}: {
  person: PersonSummary;
  size: number;
  shared: boolean;
}) {
  const { tier } = useMotionGovernor();
  const stamp = (
    <Stamp
      name={person.name}
      ink={person.ink}
      kind={person.kind}
      size={size}
      favourite={person.vip}
    />
  );
  if (!shared || tier !== 'full') return stamp;
  return (
    <motion.span
      layoutId={`stamp-${person.id}`}
      transition={settleSpring}
      className="block shrink-0"
      style={{ width: size, height: size }}
    >
      {stamp}
    </motion.span>
  );
}

function Wall({
  people,
  selectedId,
  nowTs,
  onOpen,
  menuFor,
}: {
  people: PersonSummary[];
  selectedId?: number;
  nowTs: number;
  onOpen: (person: PersonSummary) => void;
  menuFor: (person: PersonSummary) => (e: React.MouseEvent) => void;
}) {
  const grid = useRef<HTMLDivElement>(null);
  const shown = people.slice(0, WALL_LIMIT);
  const selectedIndex = shown.findIndex((p) => p.id === selectedId);

  function onKeyDown(event: React.KeyboardEvent) {
    const columns = Math.max(
      1,
      getComputedStyle(grid.current ?? document.body).gridTemplateColumns.split(' ').length,
    );
    const at = Math.max(selectedIndex, 0);
    let next = -1;
    if (event.key === 'ArrowRight') next = Math.min(shown.length - 1, at + 1);
    else if (event.key === 'ArrowLeft') next = Math.max(0, at - 1);
    else if (event.key === 'ArrowDown') next = Math.min(shown.length - 1, at + columns);
    else if (event.key === 'ArrowUp') next = Math.max(0, at - columns);
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = shown.length - 1;
    const target = next >= 0 ? shown[next] : undefined;
    if (!target) return;
    event.preventDefault();
    onOpen(target);
  }

  return (
    <div className="scroll-y min-h-0 flex-1 px-3 pb-4 outline-none">
      <div
        ref={grid}
        role="listbox"
        aria-label="People"
        tabIndex={0}
        onKeyDown={onKeyDown}
        className="grid gap-1 outline-none focus-visible:outline-none"
        style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(82px, 1fr))' }}
      >
        {shown.map((p, i) => (
          <button
            key={p.id}
            type="button"
            role="option"
            aria-selected={p.id === selectedId}
            tabIndex={-1}
            onClick={() => onOpen(p)}
            onContextMenu={menuFor(p)}
            title={p.last_at ? `${p.name}, ${formatListTime(p.last_at, nowTs)}` : p.name}
            className={cn(
              'relative flex flex-col items-center gap-1.5 rounded-sheet px-1 pt-3 pb-2 text-center transition-colors',
              p.id === selectedId ? 'bg-blue-wash' : 'hover:bg-hover',
            )}
          >
            <span className="relative">
              <TravellingStamp person={p} size={56} shared={i < SHARED} />
              {p.unread > 0 && (
                <span
                  className="tabular absolute -top-1.5 -right-2 grid h-5 min-w-5 place-items-center rounded-full px-1.5 text-meta font-semibold text-on-stamp"
                  style={{ background: stampColor(p.ink) }}
                >
                  {p.unread}
                </span>
              )}
              {p.pinned && (
                <PushPin
                  size={12}
                  weight="fill"
                  className="absolute -bottom-1 -left-2 text-text-2"
                  aria-label="Pinned"
                />
              )}
            </span>
            <span
              dir="auto"
              className={cn(
                'line-clamp-2 w-full text-ui leading-tight',
                p.unread > 0 ? 'font-semibold' : 'font-medium',
              )}
            >
              {p.name}
            </span>
          </button>
        ))}
      </div>
      {people.length > WALL_LIMIT && (
        <p className="px-1 pt-3 text-meta text-text-2">
          Showing the {WALL_LIMIT} most recent. Search, or switch to the list, to see everyone.
        </p>
      )}
    </div>
  );
}

export function PersonRow({
  person,
  selected,
  nowTs,
  onOpen,
  onContext,
  shared = false,
  style,
}: {
  person: PersonSummary;
  selected: boolean;
  nowTs: number;
  onOpen: () => void;
  onContext?: (e: React.MouseEvent) => void;
  shared?: boolean;
  style: React.CSSProperties;
}) {
  const unread = person.unread > 0;
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      tabIndex={-1}
      onClick={onOpen}
      onContextMenu={onContext}
      style={style}
      className={cn(
        'absolute top-0 left-0 flex w-full items-center gap-3 px-4 text-left transition-colors',
        selected ? 'bg-blue-wash' : 'hover:bg-hover',
      )}
    >
      {selected && <span className="absolute inset-y-2.5 left-0 w-[3px] rounded-r-full bg-blue" />}
      <TravellingStamp person={person} size={40} shared={shared} />
      <span className="min-w-0 flex-1">
        <span className="flex items-baseline gap-2">
          <span
            dir="auto"
            className={cn('truncate text-body', unread ? 'font-semibold' : 'font-medium')}
          >
            {person.name}
          </span>
          {person.pinned && (
            <PushPin
              size={12}
              weight="fill"
              className="shrink-0 self-center text-text-2"
              aria-label="Pinned"
            />
          )}
          <time className="tabular ml-auto shrink-0 font-stretch-semi-condensed text-meta text-text-2">
            {person.last_at ? formatListTime(person.last_at, nowTs) : ''}
          </time>
        </span>
        <span className="mt-0.5 flex items-center gap-2">
          <span dir="auto" className={cn('truncate text-ui', unread ? 'text-text' : 'text-text-2')}>
            {person.last_subject || person.title || ''}
          </span>
          {unread && (
            <span
              className="tabular ml-auto grid h-5 min-w-5 shrink-0 place-items-center rounded-full px-1.5 text-meta font-semibold text-on-stamp"
              style={{ background: stampColor(person.ink) }}
            >
              {person.unread}
            </span>
          )}
        </span>
      </span>
    </button>
  );
}

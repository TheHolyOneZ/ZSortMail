import { PencilSimple, Plus, Star, UsersThree } from '@phosphor-icons/react';
import { useState } from 'react';
import type { GroupInfo } from '@/lib/bindings';
import { plural } from '@/lib/format';
import { useT } from '@/lib/i18n';
import { stampColor } from '@/lib/ink';
import { useGroups, usePeopleIn } from '@/lib/queries';
import { useUi } from '@/lib/store';
import { CategoryBadge } from './CategoryBadge';
import { CategoryEditor } from './CategoryEditor';

function Tile({
  title,
  detail,
  onOpen,
  mark,
  onEdit,
  editLabel,
}: {
  title: string;
  detail: string;
  onOpen: () => void;
  mark: React.ReactNode;
  onEdit?: () => void;
  editLabel?: string;
}) {
  return (
    <li className="relative">
      <button
        type="button"
        onClick={onOpen}
        className="flex w-full items-center gap-3 rounded-control border border-rule-strong bg-sheet p-3 pe-12 text-start transition-colors hover:bg-hover"
      >
        {mark}
        <span className="min-w-0 flex-1">
          <span dir="auto" className="block truncate text-ui font-semibold">
            {title}
          </span>
          <span dir="auto" className="line-clamp-2 text-meta text-text-2">
            {detail}
          </span>
        </span>
      </button>
      {onEdit && (
        <button
          type="button"
          aria-label={editLabel}
          title={editLabel}
          onClick={onEdit}
          className="absolute end-2 top-1/2 grid size-[36px] -translate-y-1/2 place-items-center rounded-control text-text-2 hover:bg-hover hover:text-text"
        >
          <PencilSimple size={16} />
        </button>
      )}
    </li>
  );
}

export function CategoryGallery() {
  const t = useT();
  const setCategory = useUi((s) => s.setPeopleCategory);
  const { data: groups = [] } = useGroups();
  const { data: everyone = [] } = usePeopleIn('everyone', 'all', '');
  const favourites = everyone.filter((p) => p.vip);
  const [editing, setEditing] = useState<GroupInfo | 'new' | null>(null);

  return (
    <section
      className="scroll-y flex h-full min-w-0 flex-col bg-pane px-4 py-4"
      aria-label="People"
    >
      <h2 className="text-name font-semibold">{t('categories.title')}</h2>
      <p className="mt-1 text-ui text-text-2">{t('categories.intro')}</p>
      <ul className="mt-4 flex flex-col gap-2" aria-label="Categories">
        <Tile
          title={t('categories.everyone')}
          detail={`${plural(everyone.length, 'person', 'people')}. Everybody you have mail with.`}
          onOpen={() => setCategory('everyone')}
          mark={
            <span
              aria-hidden="true"
              className="grid size-[44px] shrink-0 place-items-center rounded-full bg-text text-sheet"
            >
              <UsersThree size={22} weight="fill" />
            </span>
          }
        />
        <Tile
          title={t('categories.favourites')}
          detail={
            favourites.length === 0
              ? 'Star someone to keep them here.'
              : `${plural(favourites.length, 'person', 'people')}`
          }
          onOpen={() => setCategory('favourites')}
          mark={
            <span
              aria-hidden="true"
              className="grid size-[44px] shrink-0 place-items-center rounded-full"
              style={{ background: '#b7791f' }}
            >
              <Star size={22} weight="fill" color="#fdfcf9" />
            </span>
          }
        />
        {groups.map((g) => (
          <Tile
            key={g.id}
            title={g.name}
            detail={g.description || `${plural(g.members, 'person', 'people')}`}
            onOpen={() => setCategory(`group:${g.id}`)}
            mark={<CategoryBadge icon={g.icon} ink={g.ink} image={g.image} />}
            onEdit={() => setEditing(g)}
            editLabel={`Edit the category ${g.name}`}
          />
        ))}
        <li>
          <button
            type="button"
            onClick={() => setEditing('new')}
            className="flex min-h-[68px] w-full items-center justify-center gap-2 rounded-control border border-dashed border-rule-strong text-ui font-medium text-text-2 transition-colors hover:border-blue hover:text-text"
          >
            <Plus size={16} style={{ color: stampColor(4) }} />
            {t('categories.new')}
          </button>
        </li>
      </ul>
      {editing !== null && (
        <CategoryEditor
          editing={editing === 'new' ? null : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </section>
  );
}

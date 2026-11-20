import {
  Funnel,
  GearSix,
  HourglassMedium,
  NotePencil,
  PencilSimpleLine,
  Repeat,
  SidebarSimple,
  Sun,
  Tray,
  UserPlus,
  UsersThree,
} from '@phosphor-icons/react';
import { Link, useRouterState } from '@tanstack/react-router';
import { motion } from 'motion/react';
import { type ComponentType, useState } from 'react';
import { cn } from '@/lib/cn';
import { compose } from '@/lib/composer';
import { useT } from '@/lib/i18n';
import { useAppState, useGroups } from '@/lib/queries';
import { cleanSidebar, type SidebarTab, visibleTabs } from '@/lib/sidebar';
import { PANE_LIMITS, useUi } from '@/lib/store';
import type { Key } from '@/locales/en';
import { settleSpring } from '@/motion/tokens';
import { Kbd } from '@/ui/controls';
import { Splitter } from '@/ui/Splitter';
import { CategoryBadge } from '../features/people/CategoryBadge';
import { ProfileSwitcher } from '../features/profiles/ProfileSwitcher';

type RailPath =
  | '/today'
  | '/people'
  | '/river'
  | '/new-senders'
  | '/waiting'
  | '/routines'
  | '/drafts'
  | '/rules'
  | '/settings';

interface RailItem {
  to: RailPath;
  label: Key;
  icon: ComponentType<{ size?: number; weight?: 'regular' | 'fill' }>;
}

const ITEMS: Record<SidebarTab, RailItem> = {
  '/today': { to: '/today', label: 'nav.today', icon: Sun },
  '/people': { to: '/people', label: 'nav.people', icon: UsersThree },
  '/river': { to: '/river', label: 'nav.mail', icon: Tray },
  '/new-senders': { to: '/new-senders', label: 'nav.new_senders', icon: UserPlus },
  '/waiting': { to: '/waiting', label: 'nav.waiting', icon: HourglassMedium },
  '/routines': { to: '/routines', label: 'nav.routines', icon: Repeat },
  '/drafts': { to: '/drafts', label: 'nav.drafts', icon: NotePencil },
  '/rules': { to: '/rules', label: 'nav.rules', icon: Funnel },
};

const SETTINGS: RailItem = { to: '/settings', label: 'nav.settings', icon: GearSix };

export function Rail() {
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const t = useT();
  const counts = useAppState().data?.counts;
  const open = useUi((s) => s.railOpen);
  const setOpen = useUi((s) => s.setRailOpen);
  const width = useUi((s) => s.paneWidths.rail);
  const setWidth = useUi((s) => s.setPaneWidth);
  const resetWidth = useUi((s) => s.resetPaneWidth);
  const [dragging, setDragging] = useState(false);
  const layout = useUi((s) => s.sidebar);
  const tabs = visibleTabs(cleanSidebar(layout));
  const setCategory = useUi((s) => s.setPeopleCategory);
  const pinned = (useGroups().data ?? []).filter((g) => g.sidebar);

  return (
    <nav
      aria-label="Main"
      data-open={open}
      style={{ width: open ? width : 56 }}
      className={cn(
        'relative flex shrink-0 flex-col border-r border-rule bg-desk py-3 motion-reduce:transition-none',
        !dragging && 'transition-[width] duration-200 ease-out',
        open ? 'px-3' : 'items-center',
      )}
    >
      {open && (
        <Splitter
          label="Resize the sidebar"
          value={width}
          min={PANE_LIMITS.rail.min}
          max={PANE_LIMITS.rail.max}
          onChange={(w) => setWidth('rail', w)}
          onDefault={() => resetWidth('rail')}
          onDragging={setDragging}
          style={{ right: -4 }}
        />
      )}
      <div
        className={cn('mb-4 flex items-center', open ? 'justify-between ps-1' : 'justify-center')}
      >
        <span className="flex items-center gap-2">
          <img src="/icon.png" alt="ZSortMail" width={28} height={28} className="size-7 shrink-0" />
          {open && <span className="text-body font-semibold tracking-[-0.01em]">ZSortMail</span>}
        </span>
      </div>
      <ProfileSwitcher open={open} />
      <button
        type="button"
        onClick={() => void compose()}
        title="Write a new message (C)"
        aria-label={t('nav.write')}
        className={cn(
          'mb-3 flex h-10 items-center rounded-control bg-blue text-on-blue transition-[filter] hover:brightness-110 active:brightness-95',
          open ? 'gap-3 px-3' : 'w-10 justify-center',
        )}
      >
        <PencilSimpleLine size={20} weight="bold" />
        {open && (
          <span className="min-w-0 flex-1 truncate text-start text-ui font-semibold">
            {t('nav.write')}
          </span>
        )}
        {open && <Kbd>C</Kbd>}
      </button>
      <div className={cn('flex flex-1 flex-col gap-1', !open && 'items-center')}>
        {tabs.map((tab) => (
          <RailLink
            key={tab}
            item={ITEMS[tab]}
            open={open}
            active={pathname.startsWith(tab)}
            badge={tab === '/new-senders' ? counts?.new_senders : undefined}
          />
        ))}
        {pinned.length > 0 && (
          <>
            {open ? (
              <div className="mt-3 mb-1 ps-3 text-meta font-medium text-text-2">
                {t('categories.sidebar')}
              </div>
            ) : (
              <hr className="my-2 h-px w-6 border-0 bg-rule" />
            )}
            {pinned.map((g) => (
              <Link
                key={g.id}
                to="/people"
                title={g.name}
                aria-label={`Category ${g.name}`}
                onClick={() => setCategory(`group:${g.id}`)}
                className={cn(
                  'flex h-10 items-center rounded-control text-text-2 transition-colors hover:bg-hover hover:text-text',
                  open ? 'gap-3 px-3' : 'w-10 justify-center',
                )}
              >
                <CategoryBadge icon={g.icon} ink={g.ink} image={g.image} size={22} />
                {open && (
                  <span className="min-w-0 flex-1 truncate text-ui font-medium">{g.name}</span>
                )}
              </Link>
            ))}
          </>
        )}
      </div>
      <RailLink item={SETTINGS} open={open} active={pathname.startsWith(SETTINGS.to)} />
      <button
        type="button"
        aria-label={open ? t('nav.collapse_label') : t('nav.expand_label')}
        aria-expanded={open}
        title={open ? 'Collapse the sidebar (Ctrl+B)' : 'Expand the sidebar (Ctrl+B)'}
        onClick={() => setOpen(!open)}
        className={cn(
          'mt-1 flex h-10 items-center gap-3 rounded-control text-text-2 transition-colors hover:bg-hover hover:text-text',
          open ? 'px-3' : 'w-10 justify-center',
        )}
      >
        <SidebarSimple size={22} weight={open ? 'fill' : 'regular'} />
        {open && <span className="text-ui font-medium">{t('nav.collapse')}</span>}
      </button>
    </nav>
  );
}

function RailLink({
  item,
  active,
  badge,
  open,
}: {
  item: RailItem;
  active: boolean;
  badge?: number;
  open: boolean;
}) {
  const Icon = item.icon;
  const label = useT()(item.label);
  return (
    <Link
      to={item.to}
      aria-label={label}
      title={label}
      aria-current={active ? 'page' : undefined}
      className={cn(
        'relative flex h-10 items-center rounded-control transition-colors',
        open ? 'gap-3 px-3' : 'w-10 justify-center',
        active ? 'text-text' : 'text-text-2 hover:bg-hover hover:text-text',
        open && active && 'bg-blue-wash',
      )}
    >
      {active && (
        <motion.span
          layoutId="rail-active"
          transition={settleSpring}
          className={cn(
            'absolute inset-y-2 w-[3px] rounded-r-full bg-blue',
            open ? '-left-3' : '-left-[8px]',
          )}
        />
      )}
      <Icon size={22} weight={active ? 'fill' : 'regular'} />
      {open && <span className="min-w-0 flex-1 truncate text-ui font-medium">{label}</span>}
      {badge ? (
        <span
          className={cn(
            'grid min-w-[16px] place-items-center rounded-full bg-red px-1 text-[10px] font-semibold leading-4 text-on-stamp tabular-nums',
            !open && 'absolute top-1 right-0.5',
          )}
        >
          {badge > 99 ? '99+' : badge}
        </span>
      ) : null}
    </Link>
  );
}

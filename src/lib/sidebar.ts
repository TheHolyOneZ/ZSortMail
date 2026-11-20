export const SIDEBAR_TABS = [
  '/today',
  '/people',
  '/river',
  '/new-senders',
  '/waiting',
  '/routines',
  '/drafts',
  '/rules',
] as const;

export type SidebarTab = (typeof SIDEBAR_TABS)[number];

export interface SidebarLayout {
  order: SidebarTab[];
  hidden: SidebarTab[];
}

export const DEFAULT_SIDEBAR: SidebarLayout = { order: [...SIDEBAR_TABS], hidden: [] };

const known = (t: unknown): t is SidebarTab => SIDEBAR_TABS.includes(t as SidebarTab);

export function cleanSidebar(saved: unknown): SidebarLayout {
  const raw = (saved ?? {}) as Partial<Record<keyof SidebarLayout, unknown>>;
  const order = Array.isArray(raw.order) ? raw.order.filter(known) : [];
  const unique = [...new Set(order)];
  for (const t of SIDEBAR_TABS) if (!unique.includes(t)) unique.push(t);
  const hidden = Array.isArray(raw.hidden) ? [...new Set(raw.hidden.filter(known))] : [];
  return { order: unique, hidden };
}

export function moveTab(layout: SidebarLayout, tab: SidebarTab, by: -1 | 1): SidebarLayout {
  const i = layout.order.indexOf(tab);
  const j = i + by;
  if (i < 0 || j < 0 || j >= layout.order.length) return layout;
  const order = [...layout.order];
  [order[i], order[j]] = [order[j] as SidebarTab, order[i] as SidebarTab];
  return { ...layout, order };
}

export function setTabShown(layout: SidebarLayout, tab: SidebarTab, shown: boolean): SidebarLayout {
  const rest = layout.hidden.filter((t) => t !== tab);
  return { ...layout, hidden: shown ? rest : [...rest, tab] };
}

export function visibleTabs(layout: SidebarLayout): SidebarTab[] {
  return layout.order.filter((t) => !layout.hidden.includes(t));
}

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
  /** Label names the person tucked away in the Mail sidebar. */
  hiddenLabels: string[];
}

export const DEFAULT_SIDEBAR: SidebarLayout = {
  order: [...SIDEBAR_TABS],
  hidden: [],
  hiddenLabels: [],
};

const known = (t: unknown): t is SidebarTab => SIDEBAR_TABS.includes(t as SidebarTab);

export function cleanSidebar(saved: unknown): SidebarLayout {
  const raw = (saved ?? {}) as Partial<Record<keyof SidebarLayout, unknown>>;
  const order = Array.isArray(raw.order) ? raw.order.filter(known) : [];
  const unique = [...new Set(order)];
  for (const t of SIDEBAR_TABS) if (!unique.includes(t)) unique.push(t);
  const hidden = Array.isArray(raw.hidden) ? [...new Set(raw.hidden.filter(known))] : [];
  const hiddenLabels = Array.isArray(raw.hiddenLabels)
    ? [...new Set(raw.hiddenLabels.filter((n): n is string => typeof n === 'string'))]
    : [];
  return { order: unique, hidden, hiddenLabels };
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

/**
 * Labels Gmail keeps for itself: the old star colours, the Promotions-style
 * categories, the "[Gmail]/..." folders and the uppercase system ids. They are
 * not something a person made, so the sidebar leaves them out.
 */
const GMAIL_OWN =
  /^(\[gmail\]|\[imap\]|category_|chat$|inbox$|sent$|draft$|trash$|spam$|unread$|starred$|important$|(yellow|blue|red|orange|green|purple)_(star|bang|check|info|question|guillemet)$)/i;

export function isSystemLabel(name: string): boolean {
  return GMAIL_OWN.test(name.trim());
}

export function setLabelShown(layout: SidebarLayout, name: string, shown: boolean): SidebarLayout {
  const rest = layout.hiddenLabels.filter((n) => n !== name);
  return { ...layout, hiddenLabels: shown ? rest : [...rest, name] };
}

export function visibleLabels<T extends { name: string }>(tags: T[], layout: SidebarLayout): T[] {
  return tags.filter((t) => !isSystemLabel(t.name) && !layout.hiddenLabels.includes(t.name));
}

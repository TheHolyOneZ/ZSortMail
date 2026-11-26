import { describe, expect, it } from 'vitest';
import {
  cleanSidebar,
  DEFAULT_SIDEBAR,
  moveTab,
  SIDEBAR_TABS,
  setLabelShown,
  setTabShown,
  visibleLabels,
  visibleTabs,
} from './sidebar';

describe('sidebar layout', () => {
  it('starts with every tab', () => {
    expect(visibleTabs(DEFAULT_SIDEBAR)).toEqual([...SIDEBAR_TABS]);
  });

  it('moves a tab and stops at the ends', () => {
    const down = moveTab(DEFAULT_SIDEBAR, '/today', 1);
    expect(down.order.slice(0, 2)).toEqual(['/people', '/today']);
    expect(moveTab(DEFAULT_SIDEBAR, '/today', -1)).toBe(DEFAULT_SIDEBAR);
    const last = SIDEBAR_TABS[SIDEBAR_TABS.length - 1] as (typeof SIDEBAR_TABS)[number];
    expect(moveTab(DEFAULT_SIDEBAR, last, 1)).toBe(DEFAULT_SIDEBAR);
  });

  it('hides and shows without losing the order', () => {
    const hidden = setTabShown(moveTab(DEFAULT_SIDEBAR, '/today', 1), '/people', false);
    expect(visibleTabs(hidden)[0]).toBe('/today');
    expect(hidden.order).toContain('/people');
    expect(visibleTabs(setTabShown(hidden, '/people', true))).toContain('/people');
  });

  it('repairs a saved layout from another version', () => {
    const fixed = cleanSidebar({
      order: ['/rules', '/gone', '/rules', '/today'],
      hidden: ['/x', '/waiting'],
    });
    expect(fixed.order.slice(0, 2)).toEqual(['/rules', '/today']);
    expect([...fixed.order].sort()).toEqual([...SIDEBAR_TABS].sort());
    expect(fixed.hidden).toEqual(['/waiting']);
    expect(cleanSidebar(null)).toEqual(DEFAULT_SIDEBAR);
    expect(cleanSidebar('nonsense')).toEqual(DEFAULT_SIDEBAR);
  });

  it('hides Gmail system labels and the ones a person hides', () => {
    const tags = [
      { name: '[Gmail]/Others' },
      { name: 'YELLOW_STAR' },
      { name: 'CATEGORY_PROMOTIONS' },
      { name: 'Taxes' },
      { name: 'Trips' },
    ];
    expect(visibleLabels(tags, DEFAULT_SIDEBAR).map((t) => t.name)).toEqual(['Taxes', 'Trips']);
    const hid = setLabelShown(DEFAULT_SIDEBAR, 'Taxes', false);
    expect(visibleLabels(tags, hid).map((t) => t.name)).toEqual(['Trips']);
    expect(visibleLabels(tags, setLabelShown(hid, 'Taxes', true)).map((t) => t.name)).toEqual([
      'Taxes',
      'Trips',
    ]);
    expect(cleanSidebar({ hiddenLabels: ['A', 'A', 3] }).hiddenLabels).toEqual(['A']);
  });
});

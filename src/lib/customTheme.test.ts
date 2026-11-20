import { describe, expect, it } from 'vitest';
import {
  checkContrast,
  cssVariables,
  fromShareText,
  invalidColors,
  STARTS,
  TOKENS,
  toShareText,
} from './customTheme';

describe('the starting themes', () => {
  it('are readable', () => {
    for (const base of ['light', 'dark'] as const) {
      const failing = checkContrast(STARTS[base]).filter((c) => !c.ok);
      expect(failing, base).toEqual([]);
    }
  });
});

describe('custom themes', () => {
  it('turn into the variables the app reads, with the derived ones', () => {
    const vars = cssVariables({
      base: 'dark',
      colors: { ...STARTS.dark, desk: 'rgb(1, 2, 3)', blue: '#fff' },
    });
    expect(vars['--desk']).toBe('#010203');
    expect(vars['--blue']).toBe('#ffffff');
    expect(vars['--text-3']).toMatch(/^#[0-9a-f]{6}$/);
    expect(vars['--paper']).toBeUndefined();
    const light = cssVariables({ base: 'light', colors: STARTS.light });
    expect(light['--paper']).toBe(STARTS.light.sheet);
  });

  it('fall back to the base for a colour that is not one, instead of breaking the page', () => {
    const vars = cssVariables({ base: 'light', colors: { ...STARTS.light, text: 'banana' } });
    expect(vars['--text']).toBe(STARTS.light.text);
    expect(invalidColors({ ...STARTS.light, text: 'banana', pane: '' })).toEqual(['pane', 'text']);
  });

  it('say which pairs are unreadable', () => {
    const bad = checkContrast({ ...STARTS.light, text: '#e6eef4' });
    expect(bad.filter((c) => !c.ok).map((c) => c.label)).toContain('Text on the background');
    expect(checkContrast({ ...STARTS.light, text: 'nope' })[0]?.ok).toBe(false);
  });

  it('are shared as text and read back, with clear errors', () => {
    const theme = {
      name: 'Dusk',
      base: 'dark' as const,
      colors: { ...STARTS.dark, blue: 'rgb(255, 120, 0)' },
    };
    const text = toShareText(theme);
    const back = fromShareText(text);
    expect(typeof back).toBe('object');
    expect(back).toMatchObject({ name: 'Dusk', base: 'dark' });
    expect((back as { colors: Record<string, string> }).colors.blue).toBe('#ff7800');
    expect(fromShareText('hello')).toBe('That is not a theme. It should start with {');
    expect(fromShareText('{"a":1}')).toBe('That is not a ZSortMail theme');
    expect(fromShareText('{"zsortmailTheme":1,"colors":{"blue":"x"}}')).toContain(
      'is not a colour (Accent)',
    );
    const partial = fromShareText(
      '{"zsortmailTheme":1,"base":"light","colors":{"blue":"#123456"}}',
    );
    expect((partial as { colors: Record<string, string> }).colors.desk).toBe(STARTS.light.desk);
    expect(TOKENS.length).toBe(10);
  });
});

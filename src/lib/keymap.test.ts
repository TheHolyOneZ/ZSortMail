import { describe, expect, it } from 'vitest';
import {
  ACTIONS,
  type ActionId,
  bindingFor,
  bindingFromEvent,
  conflicts,
  DEFAULT_KEYS,
  matches,
  PRESETS,
  parseBinding,
  sameBinding,
} from './keymap';

const press = (
  key: string,
  mods: Partial<{ ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean }> = {},
) => ({
  key,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  altKey: false,
  ...mods,
});

describe('bindings', () => {
  it('match letters ignoring case, but honour Shift when it is part of the binding', () => {
    expect(matches('e', press('e'))).toBe(true);
    expect(matches('e', press('E'))).toBe(true);
    expect(matches('e', press('e', { shiftKey: true }))).toBe(false);
    expect(matches('Shift+U', press('U', { shiftKey: true }))).toBe(true);
    expect(matches('Shift+U', press('u'))).toBe(false);
    expect(matches('Ctrl+B', press('b', { ctrlKey: true }))).toBe(true);
    expect(matches('Ctrl+B', press('b', { metaKey: true }))).toBe(true);
    expect(matches('Ctrl+B', press('b'))).toBe(false);
    expect(matches('b', press('b', { ctrlKey: true }))).toBe(false);
  });

  it('let symbols through whatever Shift or Alt they need on a keyboard', () => {
    expect(matches('#', press('#', { shiftKey: true }))).toBe(true);
    expect(matches('#', press('#'))).toBe(true);
    expect(matches(']', press(']', { altKey: false }))).toBe(true);
    expect(matches('/', press('/'))).toBe(true);
  });

  it('parse and compare', () => {
    expect(parseBinding('Ctrl+Shift+K')).toEqual({ key: 'K', ctrl: true, shift: true, alt: false });
    expect(sameBinding('Shift+u', 'shift+U')).toBe(true);
    expect(sameBinding('e', 'Shift+E')).toBe(false);
  });

  it('are written from a key press', () => {
    expect(bindingFromEvent({ key: 'e' } as KeyboardEvent)).toBe('e');
    expect(bindingFromEvent({ key: 'U', shiftKey: true } as KeyboardEvent)).toBe('Shift+U');
    expect(bindingFromEvent({ key: 'b', ctrlKey: true } as KeyboardEvent)).toBe('Ctrl+b');
    expect(bindingFromEvent({ key: '#', shiftKey: true } as KeyboardEvent)).toBe('#');
    expect(bindingFromEvent({ key: 'Shift' } as KeyboardEvent)).toBeNull();
    expect(bindingFromEvent({ key: ' ' } as KeyboardEvent)).toBe('Space');
  });
});

describe('presets', () => {
  it('give every action a key, and no preset uses one key twice', () => {
    for (const preset of PRESETS) {
      for (const a of ACTIONS) expect(preset.keys[a.id], `${preset.id} ${a.id}`).toBeTruthy();
      const used = ACTIONS.map((a) => preset.keys[a.id].toLowerCase());
      expect(new Set(used).size, `${preset.id} reuses a key`).toBe(used.length);
    }
  });

  it('can be changed one key at a time, and a clash is found', () => {
    const keys = {
      ...DEFAULT_KEYS,
      overrides: { archive: 'x' } as Partial<Record<ActionId, string>>,
    };
    expect(bindingFor(keys, 'archive')).toBe('x');
    expect(bindingFor(keys, 'reply')).toBe('r');
    expect(conflicts(keys, 'archive', 'r')).toEqual(['reply']);
    expect(conflicts(keys, 'archive', 'q')).toEqual([]);
    expect(bindingFor({ preset: 'gmail', overrides: {} }, 'snooze')).toBe('b');
  });
});

import { contrast, isDark, mix, parseColor, type Rgb, toHex } from './color';

export const TOKENS = [
  { key: 'desk', css: '--desk', label: 'Background', hint: 'The window and the sidebar' },
  { key: 'pane', css: '--pane', label: 'Lists', hint: 'People, conversations and screens' },
  {
    key: 'sheet',
    css: '--sheet',
    label: 'Cards and dialogs',
    hint: 'Menus, dialogs and the composer',
  },
  { key: 'text', css: '--text', label: 'Text', hint: 'Names, subjects and headings' },
  { key: 'text2', css: '--text-2', label: 'Secondary text', hint: 'Details, dates and hints' },
  { key: 'rule', css: '--rule', label: 'Dividers', hint: 'The thin lines between things' },
  {
    key: 'ruleStrong',
    css: '--rule-strong',
    label: 'Outlines',
    hint: 'Edges of fields, chips and buttons',
  },
  { key: 'blue', css: '--blue', label: 'Accent', hint: 'Links, the main button, focus' },
  {
    key: 'onBlue',
    css: '--on-blue',
    label: 'Text on the accent',
    hint: 'Writing inside the main button',
  },
  { key: 'red', css: '--red', label: 'Warnings', hint: 'Errors, trash, unread counts' },
] as const;

export type TokenKey = (typeof TOKENS)[number]['key'];
export type Colors = Record<TokenKey, string>;

export interface CustomTheme {
  id: string;
  name: string;
  base: 'light' | 'dark';
  colors: Colors;
}

export const STARTS: Record<'light' | 'dark', Colors> = {
  light: {
    desk: '#e6eef4',
    pane: '#eff5fa',
    sheet: '#fdfcf9',
    text: '#172135',
    text2: '#4d5969',
    rule: '#d1d8df',
    ruleStrong: '#b4c0cc',
    blue: '#005798',
    onBlue: '#fdfcf9',
    red: '#d73431',
  },
  dark: {
    desk: '#0b1421',
    pane: '#131c2a',
    sheet: '#1e2734',
    text: '#e5ecf3',
    text2: '#a0acbb',
    rule: '#293342',
    ruleStrong: '#3a4658',
    blue: '#65b2f1',
    onBlue: '#0b1421',
    red: '#fd7468',
  },
};

export function cssVariables(theme: Pick<CustomTheme, 'base' | 'colors'>): Record<string, string> {
  const fallback = STARTS[theme.base];
  const rgb = (key: TokenKey): Rgb =>
    parseColor(theme.colors[key]) ?? (parseColor(fallback[key]) as Rgb);
  const vars: Record<string, string> = {};
  for (const t of TOKENS) vars[t.css] = toHex(rgb(t.key));
  vars['--text-3'] = toHex(mix(rgb('text2'), rgb('desk'), 0.38));
  vars['--scrim'] = isDark(rgb('desk')) ? 'rgb(0 0 0 / 0.55)' : 'rgb(23 33 53 / 0.38)';
  if (theme.base === 'light') {
    vars['--paper'] = toHex(rgb('sheet'));
    vars['--paper-blue'] = toHex(rgb('blue'));
    vars['--on-paper-blue'] = toHex(rgb('onBlue'));
    vars['--paper-text'] = toHex(rgb('text'));
    vars['--paper-text-2'] = toHex(rgb('text2'));
    vars['--paper-rule'] = toHex(mix(rgb('rule'), rgb('sheet'), 0.3));
  }
  return vars;
}

export const CHECKS: { text: TokenKey; on: TokenKey; label: string; need: number }[] = [
  { text: 'text', on: 'desk', label: 'Text on the background', need: 4.5 },
  { text: 'text', on: 'pane', label: 'Text on lists', need: 4.5 },
  { text: 'text', on: 'sheet', label: 'Text on cards', need: 4.5 },
  { text: 'text2', on: 'pane', label: 'Secondary text on lists', need: 4.5 },
  { text: 'text2', on: 'desk', label: 'Secondary text on the background', need: 4.5 },
  { text: 'blue', on: 'sheet', label: 'Accent on cards', need: 4.5 },
  { text: 'onBlue', on: 'blue', label: 'Text on the accent', need: 4.5 },
  { text: 'red', on: 'sheet', label: 'Warnings on cards', need: 3 },
];

export interface CheckResult {
  label: string;
  ratio: number;
  need: number;
  ok: boolean;
}

export function checkContrast(colors: Colors): CheckResult[] {
  return CHECKS.map((c) => {
    const a = parseColor(colors[c.text]);
    const b = parseColor(colors[c.on]);
    const ratio = a && b ? contrast(a, b) : 0;
    return { label: c.label, ratio, need: c.need, ok: ratio >= c.need };
  });
}

export function invalidColors(colors: Colors): TokenKey[] {
  return TOKENS.filter((t) => !parseColor(colors[t.key])).map((t) => t.key);
}

export function toShareText(theme: Pick<CustomTheme, 'name' | 'base' | 'colors'>): string {
  const colors = Object.fromEntries(
    TOKENS.map((t) => [t.key, toHex(parseColor(theme.colors[t.key]) ?? { r: 0, g: 0, b: 0 })]),
  );
  return JSON.stringify({ zsortmailTheme: 1, name: theme.name, base: theme.base, colors }, null, 2);
}

export function fromShareText(
  text: string,
): { name: string; base: 'light' | 'dark'; colors: Colors } | string {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    return 'That is not a theme. It should start with {';
  }
  if (
    typeof data !== 'object' ||
    data === null ||
    (data as { zsortmailTheme?: unknown }).zsortmailTheme !== 1
  ) {
    return 'That is not a ZSortMail theme';
  }
  const d = data as { name?: unknown; base?: unknown; colors?: Record<string, unknown> };
  const base = d.base === 'dark' ? 'dark' : 'light';
  const colors = { ...STARTS[base] };
  for (const t of TOKENS) {
    const value = d.colors?.[t.key];
    if (typeof value === 'string') {
      const parsed = parseColor(value);
      if (!parsed) return `“${value}” is not a colour (${t.label})`;
      colors[t.key] = toHex(parsed);
    }
  }
  return { name: typeof d.name === 'string' ? d.name.slice(0, 40) : 'Shared theme', base, colors };
}

export const CUSTOM_PREFIX = 'custom:';
export const isCustomId = (value: string) => value.startsWith(CUSTOM_PREFIX);

export function applyCustom(
  root: HTMLElement,
  theme: Pick<CustomTheme, 'base' | 'colors'> | null,
): void {
  const known = [
    ...TOKENS.map((t) => t.css),
    '--text-3',
    '--scrim',
    '--paper',
    '--paper-blue',
    '--on-paper-blue',
    '--paper-text',
    '--paper-text-2',
    '--paper-rule',
  ];
  for (const name of known) root.style.removeProperty(name);
  if (!theme) {
    delete root.dataset.customTheme;
    return;
  }
  for (const [name, value] of Object.entries(cssVariables(theme)))
    root.style.setProperty(name, value);
  root.dataset.customTheme = theme.base;
}

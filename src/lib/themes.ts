export interface ThemeInfo {
  id: string;
  name: string;
  description: string;
  scheme: 'light' | 'dark';
  swatch: { desk: string; pane: string; sheet: string; text: string; accent: string };
}

export const THEMES: ThemeInfo[] = [
  {
    id: 'light',
    name: 'Paper',
    description: 'The default light desk, blue-black ink on cool grey.',
    scheme: 'light',
    swatch: {
      desk: '#e6eef4',
      pane: '#eff5fa',
      sheet: '#fdfcf9',
      text: '#172135',
      accent: '#005798',
    },
  },
  {
    id: 'dark',
    name: 'Ink',
    description: 'The default dark desk.',
    scheme: 'dark',
    swatch: {
      desk: '#0b1421',
      pane: '#131c2a',
      sheet: '#1e2734',
      text: '#e5ecf3',
      accent: '#65b2f1',
    },
  },
  {
    id: 'midnight',
    name: 'Midnight',
    description: 'Near black, easy on the eyes at night.',
    scheme: 'dark',
    swatch: {
      desk: '#05070b',
      pane: '#0b0f16',
      sheet: '#121822',
      text: '#e8edf5',
      accent: '#6cb6ff',
    },
  },
  {
    id: 'graphite',
    name: 'Graphite',
    description: 'Neutral dark grey with no tint.',
    scheme: 'dark',
    swatch: {
      desk: '#151515',
      pane: '#1c1c1c',
      sheet: '#262626',
      text: '#ececec',
      accent: '#7db7ff',
    },
  },
  {
    id: 'nord',
    name: 'Nord',
    description: 'Cold slate with an ice-blue accent.',
    scheme: 'dark',
    swatch: {
      desk: '#242933',
      pane: '#2b303b',
      sheet: '#343b49',
      text: '#e5e9f0',
      accent: '#88c0d0',
    },
  },
  {
    id: 'forest',
    name: 'Forest',
    description: 'Deep green dark with a mint accent.',
    scheme: 'dark',
    swatch: {
      desk: '#0c1511',
      pane: '#111d17',
      sheet: '#182a22',
      text: '#e3efe8',
      accent: '#6fd0a0',
    },
  },
  {
    id: 'sky',
    name: 'Sky',
    description: 'A brighter, bluer light theme.',
    scheme: 'light',
    swatch: {
      desk: '#dcecf8',
      pane: '#e8f3fb',
      sheet: '#fbfdff',
      text: '#0f2236',
      accent: '#0a5fb4',
    },
  },
  {
    id: 'sepia',
    name: 'Sepia',
    description: 'Warm light theme for long reading.',
    scheme: 'light',
    swatch: {
      desk: '#ebe3d3',
      pane: '#f3ecdd',
      sheet: '#fbf6ea',
      text: '#3a2f22',
      accent: '#8a4b12',
    },
  },
  {
    id: 'rose',
    name: 'Rose',
    description: 'Soft pink light theme with a berry accent.',
    scheme: 'light',
    swatch: {
      desk: '#f4e6ea',
      pane: '#f9eff2',
      sheet: '#fffafb',
      text: '#2c1a22',
      accent: '#a1204f',
    },
  },
  {
    id: 'contrast-light',
    name: 'High contrast',
    description: 'Black on white with strong lines.',
    scheme: 'light',
    swatch: {
      desk: '#ffffff',
      pane: '#ffffff',
      sheet: '#ffffff',
      text: '#000000',
      accent: '#0000cc',
    },
  },
  {
    id: 'contrast-dark',
    name: 'High contrast dark',
    description: 'White on black with strong lines.',
    scheme: 'dark',
    swatch: {
      desk: '#000000',
      pane: '#000000',
      sheet: '#0a0a0a',
      text: '#ffffff',
      accent: '#9cc2ff',
    },
  },
];

export const THEME_IDS = THEMES.map((t) => t.id);

export type ThemePreference = string;

export function isTheme(value: unknown): value is ThemePreference {
  return (
    value === 'system' ||
    (typeof value === 'string' && (THEME_IDS.includes(value) || value.startsWith('custom:')))
  );
}

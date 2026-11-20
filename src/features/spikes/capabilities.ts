export interface Check {
  name: string;
  ok: boolean;
  use: string;
}

const css = (property: string, value?: string) =>
  typeof CSS !== 'undefined' &&
  (value === undefined ? CSS.supports(property) : CSS.supports(property, value));

export function runCapabilityChecks(): Check[] {
  return [
    { name: 'light-dark()', ok: css('color', 'light-dark(red, blue)'), use: 'every theme token' },
    {
      name: 'color-mix()',
      ok: css('color', 'color-mix(in oklab, red 40%, blue)'),
      use: 'hover, selected and unread washes',
    },
    { name: 'oklch()', ok: css('color', 'oklch(0.5 0.1 200)'), use: 'palette authoring' },
    {
      name: 'font-stretch (variable width)',
      ok: css('font-stretch', '87.5%'),
      use: 'condensed list metadata',
    },
    {
      name: 'content-visibility: auto',
      ok: css('content-visibility', 'auto'),
      use: 'skipping off-screen message rows',
    },
    { name: 'scrollbar-gutter', ok: css('scrollbar-gutter', 'stable'), use: 'stable list width' },
    {
      name: 'overscroll-behavior',
      ok: css('overscroll-behavior', 'contain'),
      use: 'pane scrolling',
    },
    { name: ':has()', ok: css('selector(:has(*))'), use: 'state styling without JS' },
    {
      name: 'container queries',
      ok: css('container-type', 'inline-size'),
      use: 'future pane layout',
    },
    { name: 'text-wrap: balance', ok: css('text-wrap', 'balance'), use: 'headings (optional)' },
    {
      name: 'backdrop-filter',
      ok: css('backdrop-filter', 'blur(4px)'),
      use: 'not used: costly on WebKitGTK',
    },
    { name: 'Web Animations API', ok: 'animate' in Element.prototype, use: 'motion engine' },
    {
      name: 'ResizeObserver',
      ok: typeof ResizeObserver !== 'undefined',
      use: 'layout mode, scroll pinning',
    },
    {
      name: 'Intl.Segmenter',
      ok: typeof Intl.Segmenter === 'function',
      use: 'initials, text splitting',
    },
    { name: 'structuredClone', ok: typeof structuredClone === 'function', use: 'state copies' },
    {
      name: 'View Transitions API',
      ok: 'startViewTransition' in document,
      use: 'not used: not reliable in every webview',
    },
  ];
}

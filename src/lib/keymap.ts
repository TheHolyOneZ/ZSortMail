export type ActionId =
  | 'compose'
  | 'search'
  | 'next_conversation'
  | 'previous_conversation'
  | 'next_person'
  | 'previous_person'
  | 'reply'
  | 'reply_all'
  | 'forward'
  | 'archive'
  | 'trash'
  | 'hide'
  | 'unread'
  | 'snooze'
  | 'sidebar';

export interface ActionInfo {
  id: ActionId;
  label: string;
  group: 'Go' | 'Write' | 'Sort' | 'Window';
}

export const ACTIONS: ActionInfo[] = [
  { id: 'next_conversation', label: 'Next conversation', group: 'Go' },
  { id: 'previous_conversation', label: 'Previous conversation', group: 'Go' },
  { id: 'next_person', label: 'Next person', group: 'Go' },
  { id: 'previous_person', label: 'Previous person', group: 'Go' },
  { id: 'search', label: 'Search and commands', group: 'Go' },
  { id: 'compose', label: 'New message', group: 'Write' },
  { id: 'reply', label: 'Reply', group: 'Write' },
  { id: 'reply_all', label: 'Reply to all', group: 'Write' },
  { id: 'forward', label: 'Forward', group: 'Write' },
  { id: 'archive', label: 'Archive', group: 'Sort' },
  { id: 'trash', label: 'Move to trash', group: 'Sort' },
  { id: 'hide', label: 'Hide conversation', group: 'Sort' },
  { id: 'unread', label: 'Mark as unread', group: 'Sort' },
  { id: 'snooze', label: 'Snooze until tomorrow morning', group: 'Sort' },
  { id: 'sidebar', label: 'Show or hide the sidebar', group: 'Window' },
];

export type PresetId = 'zsortmail' | 'gmail' | 'superhuman' | 'vim';

export const PRESETS: {
  id: PresetId;
  name: string;
  description: string;
  keys: Record<ActionId, string>;
}[] = [
  {
    id: 'zsortmail',
    name: 'ZSortMail',
    description: 'The default keys.',
    keys: {
      next_conversation: 'j',
      previous_conversation: 'k',
      next_person: ']',
      previous_person: '[',
      search: '/',
      compose: 'c',
      reply: 'r',
      reply_all: 'a',
      forward: 'f',
      archive: 'e',
      trash: '#',
      hide: 'h',
      unread: 'u',
      snooze: 's',
      sidebar: 'Ctrl+B',
    },
  },
  {
    id: 'gmail',
    name: 'Gmail',
    description: 'The keys Gmail uses, where they apply.',
    keys: {
      next_conversation: 'j',
      previous_conversation: 'k',
      next_person: 'n',
      previous_person: 'p',
      search: '/',
      compose: 'c',
      reply: 'r',
      reply_all: 'a',
      forward: 'f',
      archive: 'e',
      trash: '#',
      hide: 'Shift+H',
      unread: 'Shift+U',
      snooze: 'b',
      sidebar: 'Ctrl+B',
    },
  },
  {
    id: 'superhuman',
    name: 'Superhuman',
    description: 'Done with E, remind with H.',
    keys: {
      next_conversation: 'j',
      previous_conversation: 'k',
      next_person: ']',
      previous_person: '[',
      search: '/',
      compose: 'c',
      reply: 'r',
      reply_all: 'Shift+R',
      forward: 'f',
      archive: 'e',
      trash: '#',
      hide: 'Shift+H',
      unread: 'u',
      snooze: 'h',
      sidebar: 'Ctrl+B',
    },
  },
  {
    id: 'vim',
    name: 'Vim',
    description: 'Home-row movement, capitals for the bigger actions.',
    keys: {
      next_conversation: 'j',
      previous_conversation: 'k',
      next_person: 'l',
      previous_person: 'h',
      search: '/',
      compose: 'i',
      reply: 'r',
      reply_all: 'Shift+R',
      forward: 'f',
      archive: 'x',
      trash: 'Shift+D',
      hide: 'Shift+H',
      unread: 'u',
      snooze: 's',
      sidebar: 'Ctrl+B',
    },
  },
];

export interface Keys {
  preset: PresetId;
  overrides: Partial<Record<ActionId, string>>;
}

export const DEFAULT_KEYS: Keys = { preset: 'zsortmail', overrides: {} };

export function bindingFor(keys: Keys, id: ActionId): string {
  const preset =
    PRESETS.find((p) => p.id === keys.preset) ?? (PRESETS[0] as (typeof PRESETS)[number]);
  return keys.overrides[id] ?? preset.keys[id];
}

export function conflicts(keys: Keys, id: ActionId, binding: string): ActionId[] {
  return ACTIONS.filter((a) => a.id !== id && sameBinding(bindingFor(keys, a.id), binding)).map(
    (a) => a.id,
  );
}

interface Parsed {
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

export function parseBinding(binding: string): Parsed {
  const parts = binding.split('+');
  const key = binding.endsWith('++') || binding === '+' ? '+' : (parts[parts.length - 1] ?? '');
  const mods = new Set(parts.slice(0, -1).map((m) => m.toLowerCase()));
  return {
    key,
    ctrl: mods.has('ctrl') || mods.has('cmd') || mods.has('mod'),
    shift: mods.has('shift'),
    alt: mods.has('alt'),
  };
}

export function sameBinding(a: string, b: string): boolean {
  const x = parseBinding(a);
  const y = parseBinding(b);
  return (
    x.key.toLowerCase() === y.key.toLowerCase() &&
    x.ctrl === y.ctrl &&
    x.shift === y.shift &&
    x.alt === y.alt
  );
}

export function matches(
  binding: string,
  event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>,
): boolean {
  const b = parseBinding(binding);
  const symbol = b.key.length === 1 && !/[a-z0-9]/i.test(b.key);
  const letter = b.key.length === 1 && /[a-z]/i.test(b.key);
  if ((event.ctrlKey || event.metaKey) !== b.ctrl) return false;
  if (event.altKey !== b.alt) return false;
  if (!symbol && event.shiftKey !== b.shift) return false;
  if (letter) return event.key.toLowerCase() === b.key.toLowerCase();
  return event.key === b.key;
}

export function bindingFromEvent(event: KeyboardEvent): string | null {
  if (['Control', 'Shift', 'Alt', 'Meta', 'Dead', 'Unidentified'].includes(event.key)) return null;
  const letter = event.key.length === 1 && /[a-z]/i.test(event.key);
  const symbol = event.key.length === 1 && !letter && !/[0-9]/.test(event.key);
  const key = letter ? event.key.toLowerCase() : event.key === ' ' ? 'Space' : event.key;
  const mods = [
    event.ctrlKey || event.metaKey ? 'Ctrl' : '',
    event.altKey ? 'Alt' : '',
    event.shiftKey && !symbol ? 'Shift' : '',
  ].filter(Boolean);
  return [...mods, letter && event.shiftKey ? key.toUpperCase() : key].join('+');
}

export function showBinding(binding: string, mac = false): string {
  return mac
    ? binding.replace('Ctrl', '⌘').replace('Alt', '⌥').replace('Shift', '⇧').replaceAll('+', '')
    : binding;
}

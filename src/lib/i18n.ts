import { de } from '@/locales/de';
import { type Catalog, en, type Key } from '@/locales/en';
import { useUi } from './store';

export type Locale = 'en' | 'de';
export type LocalePreference = 'en' | 'de' | 'system';

export const LOCALES: { id: Locale; name: string }[] = [
  { id: 'en', name: 'English' },
  { id: 'de', name: 'Deutsch' },
];

const CATALOGS: Record<Locale, Catalog> = { en, de };

const RTL = new Set(['ar', 'he', 'fa', 'ur']);

export function resolveLocale(
  preference: LocalePreference,
  systemLanguages: readonly string[],
): Locale {
  if (preference !== 'system') return preference;
  for (const tag of systemLanguages) {
    const base = tag.toLowerCase().split('-')[0];
    const found = LOCALES.find((l) => l.id === base);
    if (found) return found.id;
  }
  return 'en';
}

export function direction(locale: string): 'ltr' | 'rtl' {
  return RTL.has(locale.split('-')[0] ?? '') ? 'rtl' : 'ltr';
}

export function format(template: string, params?: Record<string, string | number>): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}

export function translate(
  locale: Locale,
  key: Key,
  params?: Record<string, string | number>,
): string {
  return format(CATALOGS[locale][key] ?? en[key], params);
}

function current(): Locale {
  const preference = useUi.getState().language;
  return resolveLocale(preference, typeof navigator === 'undefined' ? [] : navigator.languages);
}

export function t(key: Key, params?: Record<string, string | number>): string {
  return translate(current(), key, params);
}

export function useT(): (key: Key, params?: Record<string, string | number>) => string {
  const preference = useUi((s) => s.language);
  const locale = resolveLocale(
    preference,
    typeof navigator === 'undefined' ? [] : navigator.languages,
  );
  return (key, params) => translate(locale, key, params);
}

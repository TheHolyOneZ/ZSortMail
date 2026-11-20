import { describe, expect, it } from 'vitest';
import { de } from '@/locales/de';
import { en } from '@/locales/en';
import { direction, format, resolveLocale, translate } from './i18n';

const markers = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe('catalogs', () => {
  it('give every language every key, with the same markers', () => {
    for (const key of Object.keys(en) as (keyof typeof en)[]) {
      expect(de[key], `de is missing ${key}`).toBeTruthy();
      expect(markers(de[key]), `de changes the markers of ${key}`).toEqual(markers(en[key]));
    }
    expect(Object.keys(de).sort()).toEqual(Object.keys(en).sort());
  });
});

describe('translate', () => {
  it('fills in values and leaves a missing one visible', () => {
    expect(translate('en', 'people.mark_all', { count: 7 })).toBe('Mark all 7 unread as read');
    expect(translate('de', 'people.mark_all', { count: 7 })).toBe(
      'Alle 7 ungelesenen als gelesen markieren',
    );
    expect(format('Hello {name}', {})).toBe('Hello {name}');
    expect(translate('de', 'nav.today')).toBe('Heute');
  });

  it('chooses a language from the preference or the computer, and falls back to English', () => {
    expect(resolveLocale('de', ['en-US'])).toBe('de');
    expect(resolveLocale('system', ['fr-FR', 'de-AT', 'en'])).toBe('de');
    expect(resolveLocale('system', ['fr-FR'])).toBe('en');
    expect(resolveLocale('system', [])).toBe('en');
  });

  it('knows which languages run right to left', () => {
    expect(direction('ar')).toBe('rtl');
    expect(direction('he-IL')).toBe('rtl');
    expect(direction('de')).toBe('ltr');
  });
});

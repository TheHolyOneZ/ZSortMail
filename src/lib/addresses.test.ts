import { describe, expect, it } from 'vitest';
import { addressLabel, looksLikeEmail, parseAddresses, sameAddress } from './addresses';

describe('parseAddresses', () => {
  it('reads bare addresses, named addresses and quoted names', () => {
    expect(parseAddresses('Maya@Northwind.example')).toEqual([
      { email: 'maya@northwind.example', name: null },
    ]);
    expect(parseAddresses('Maya Okafor <maya@northwind.example>')).toEqual([
      { email: 'maya@northwind.example', name: 'Maya Okafor' },
    ]);
    expect(parseAddresses('"Okafor, Maya" <maya@northwind.example>')).toEqual([
      { email: 'maya@northwind.example', name: 'Okafor, Maya' },
    ]);
    expect(parseAddresses('<only@angle.example>')).toEqual([
      { email: 'only@angle.example', name: null },
    ]);
  });

  it('splits several addresses on commas, semicolons and new lines, but not inside quotes', () => {
    expect(
      parseAddresses('a@x.co, "Doe, Jo" <jo@x.co>; b@x.co\n c@x.co ,').map((a) => a.email),
    ).toEqual(['a@x.co', 'jo@x.co', 'b@x.co', 'c@x.co']);
    expect(parseAddresses('')).toEqual([]);
    expect(parseAddresses(' , ; ')).toEqual([]);
  });

  it('keeps what is not an address as typed, so it can be marked', () => {
    expect(parseAddresses('half-typed')).toEqual([{ email: 'half-typed', name: null }]);
    expect(looksLikeEmail('half-typed')).toBe(false);
  });
});

describe('looksLikeEmail', () => {
  it('accepts ordinary addresses and rejects the rest', () => {
    for (const ok of ['a@b.co', 'first.last+tag@sub.example.org', ' a@b.co ']) {
      expect(looksLikeEmail(ok), ok).toBe(true);
    }
    for (const bad of ['', 'a', 'a@b', '@b.co', 'a@@b.co', 'a b@c.co', 'a@b.c o']) {
      expect(looksLikeEmail(bad), bad).toBe(false);
    }
  });
});

describe('labels and comparison', () => {
  it('prefers the name and ignores case when comparing', () => {
    expect(addressLabel({ email: 'a@b.co', name: 'Ann' })).toBe('Ann');
    expect(addressLabel({ email: 'a@b.co', name: ' ' })).toBe('a@b.co');
    expect(sameAddress({ email: 'A@B.co', name: null }, { email: 'a@b.CO', name: 'x' })).toBe(true);
  });
});

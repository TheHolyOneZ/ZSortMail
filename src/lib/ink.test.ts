import { describe, expect, it } from 'vitest';
import { INK_COUNT, initials, inkFor, stampColor } from './ink';

describe('inkFor', () => {
  it('is stable for the same address', () => {
    expect(inkFor('maya@northwind.example')).toBe(inkFor('maya@northwind.example'));
  });

  it('ignores case, so one person never changes colour', () => {
    expect(inkFor('Maya@Northwind.Example')).toBe(inkFor('maya@northwind.example'));
  });

  it('always lands in range', () => {
    for (const key of ['', 'a', '田中@example.jp', '🎉@example.com', 'x'.repeat(500)]) {
      const ink = inkFor(key);
      expect(ink).toBeGreaterThanOrEqual(0);
      expect(ink).toBeLessThan(INK_COUNT);
    }
  });

  it('spreads addresses across most of the palette', () => {
    const seen = new Set<number>();
    for (let i = 0; i < 400; i++) seen.add(inkFor(`person${i}@example.com`));
    expect(seen.size).toBe(INK_COUNT);
  });
});

describe('stampColor', () => {
  it('wraps out-of-range and negative values instead of producing a broken variable', () => {
    expect(stampColor(3)).toBe('var(--stamp-3)');
    expect(stampColor(13)).toBe('var(--stamp-3)');
    expect(stampColor(-1)).toBe('var(--stamp-9)');
  });
});

describe('initials', () => {
  it.each([
    ['Maya Okafor', 'MO'],
    ['Dad', 'D'],
    ['DHL', 'DH'],
    ['DHL Parcel', 'DP'],
    ['Amélie Dubois-Castellanos', 'AD'],
    ['田中 弘', '田'],
    ['علي حسن', 'ع'],
    ['Noa Katz (נועה כץ)', 'NK'],
    ['Dr. Bartholomew Fitzgerald-Montgomery-Wellington III', 'DI'],
    ['  ', '?'],
    ['', '?'],
  ])('%s -> %s', (name, expected) => {
    expect(initials(name)).toBe(expected);
  });

  it('keeps a leading emoji whole, including joined sequences', () => {
    expect(initials('👩‍💻 Dev')).toBe('👩‍💻');
  });

  it('uses the first letter of lowercase names, uppercased', () => {
    expect(initials('jules moreau')).toBe('JM');
  });
});

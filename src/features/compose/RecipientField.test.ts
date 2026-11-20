import { describe, expect, it } from 'vitest';
import { matches } from './RecipientField';

describe('matches', () => {
  it('finds typed text in a name or address, ignoring case and accents', () => {
    expect(matches('Maya Okafor maya.okafor@northwind.example', 'okafor')).toBe(true);
    expect(matches('Amélie Dubois a@x.co', 'AMELIE')).toBe(true);
    expect(matches('Maya Okafor maya@x.co', 'someone@elsewhere.example')).toBe(false);
    expect(matches('Maya Okafor maya@x.co', '  ')).toBe(true);
  });
});

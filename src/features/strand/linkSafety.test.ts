import { describe, expect, it } from 'vitest';
import { assessLink, describeRisk } from './linkSafety';

describe('assessLink', () => {
  it('lets an ordinary link through', () => {
    expect(assessLink('https://example.com/a?b=c', 'Read the article')).toBeNull();
    expect(assessLink('https://example.com/a', 'https://example.com/a')).toBeNull();
    expect(assessLink('https://www.example.com/', 'example.com')).toBeNull();
    expect(assessLink('https://news.example.com/x', 'example.com')).toBeNull();
  });

  it('flags text that names one site while the link goes to another', () => {
    const risk = assessLink('https://evil.example.net/login', 'https://mybank.com/login');
    expect(risk).toEqual({ kind: 'mismatch', says: 'mybank.com', goes: 'evil.example.net' });
    expect(describeRisk(risk as NonNullable<typeof risk>)).toBe(
      'The link says mybank.com but goes to evil.example.net.',
    );
    expect(assessLink('https://evil.example.net/', 'mybank.com')?.kind).toBe('mismatch');
  });

  it('does not mistake a sentence for a site', () => {
    expect(
      assessLink('https://evil.example.net/', 'Sign in to your bank. It is quick.'),
    ).toBeNull();
    expect(assessLink('https://example.com/', 'Click here')).toBeNull();
  });

  it('flags look-alike hosts, bare IP addresses and embedded credentials', () => {
    expect(assessLink('https://xn--pple-43d.com/', 'Apple')?.kind).toBe('lookalike');
    expect(assessLink('http://192.168.0.1/admin', 'Router')?.kind).toBe('address');
    expect(assessLink('https://mybank.com@evil.example.net/', 'mybank.com')?.kind).toBe(
      'credentials',
    );
  });

  it('ignores schemes it does not judge', () => {
    expect(assessLink('mailto:a@b.example', 'a@b.example')).toBeNull();
    expect(assessLink('not a url', 'x')).toBeNull();
  });

  it('judges the real destination, not what the sender capitalised', () => {
    expect(assessLink('https://Example.COM/x', 'EXAMPLE.com')).toBeNull();
  });
});

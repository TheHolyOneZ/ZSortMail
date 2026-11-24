import { describe, expect, it } from 'vitest';
import { parseMailto } from './mailto';

describe('parseMailto', () => {
  it('reads addresses, subject and body', () => {
    const m = parseMailto(
      'mailto:maya@northwind.example,dan@northwind.example?subject=Q3%20launch&body=Hi%20Maya%0A%0AThanks',
    );
    expect(m?.to.map((a) => a.email)).toEqual(['maya@northwind.example', 'dan@northwind.example']);
    expect(m?.subject).toBe('Q3 launch');
    expect(m?.body).toBe('Hi Maya\n\nThanks');
  });

  it('reads cc and bcc and a to in the query', () => {
    const m = parseMailto('MAILTO:a@b.example?cc=c@d.example&bcc=e@f.example&to=g@h.example');
    expect(m?.to.map((a) => a.email)).toEqual(['a@b.example', 'g@h.example']);
    expect(m?.cc[0]?.email).toBe('c@d.example');
    expect(m?.bcc[0]?.email).toBe('e@f.example');
  });

  it('ignores attachments, bad addresses and line breaks in the subject', () => {
    const m = parseMailto(
      'mailto:not-an-address,ok@x.example?attach=/etc/passwd&subject=Hi%0D%0ABcc:%20evil@x.example',
    );
    expect(m?.to.map((a) => a.email)).toEqual(['ok@x.example']);
    expect(m?.subject).toBe('Hi Bcc: evil@x.example');
    expect(m?.bcc).toEqual([]);
  });

  it('survives broken escapes and links that are not mailto', () => {
    expect(parseMailto('mailto:a@b.example?subject=100%')?.subject).toBe('100%');
    expect(parseMailto('https://example.com')).toBeNull();
    expect(parseMailto('mailto:')?.to).toEqual([]);
  });
});

import { describe, expect, it } from 'vitest';
import { normaliseUrl } from './link';

describe('normaliseUrl', () => {
  it('keeps web and mail addresses and adds what is missing', () => {
    expect(normaliseUrl('https://example.com/a?b=c')).toBe('https://example.com/a?b=c');
    expect(normaliseUrl(' http://example.com ')).toBe('http://example.com');
    expect(normaliseUrl('example.com')).toBe('https://example.com');
    expect(normaliseUrl('www.example.co.uk/path#top')).toBe('https://www.example.co.uk/path#top');
    expect(normaliseUrl('localhost.test:8080/x')).toBe('https://localhost.test:8080/x');
    expect(normaliseUrl('maya@northwind.example')).toBe('mailto:maya@northwind.example');
    expect(normaliseUrl('mailto:maya@northwind.example')).toBe('mailto:maya@northwind.example');
    expect(normaliseUrl('bücher.example')).toBe('https://bücher.example');
  });

  it('refuses everything that is not a web or mail address', () => {
    for (const bad of [
      '',
      '   ',
      'javascript:alert(1)',
      'JaVaScRiPt:alert(1)',
      'data:text/html,hi',
      'file:///etc/passwd',
      'ftp://example.com',
      'not a link',
      'example',
      'http://',
      'https://exam ple.com',
    ]) {
      expect(normaliseUrl(bad), bad).toBeNull();
    }
  });
});

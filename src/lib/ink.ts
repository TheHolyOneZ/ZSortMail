export const INK_COUNT = 10;

export function inkFor(key: string): number {
  let hash = 0x811c9dc5;
  for (const char of key.toLowerCase()) {
    hash ^= char.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash % INK_COUNT;
}

export function stampColor(ink: number): string {
  const index = ((Math.trunc(ink) % INK_COUNT) + INK_COUNT) % INK_COUNT;
  return `var(--stamp-${index})`;
}

const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

function graphemes(text: string): string[] {
  return Array.from(segmenter.segment(text), (s) => s.segment);
}

const CASED_SCRIPT = /^[\p{Script=Latin}\p{Script=Cyrillic}\p{Script=Greek}]/u;

export function initials(name: string): string {
  const primary = name.replace(/\s*[([{].*$/u, '').trim() || name.trim();
  const words = primary.split(/\s+/u).filter(Boolean);
  const first = words[0];
  if (!first) return '?';

  const firstLetter = graphemes(first)[0] ?? '?';
  if (!CASED_SCRIPT.test(firstLetter)) return firstLetter;

  const last = words.length > 1 ? words[words.length - 1] : undefined;
  if (last) {
    const lastLetter = graphemes(last)[0];
    if (lastLetter && CASED_SCRIPT.test(lastLetter)) {
      return (firstLetter + lastLetter).toLocaleUpperCase();
    }
  }
  if (first.length <= 3 && first === first.toLocaleUpperCase() && graphemes(first).length > 1) {
    return graphemes(first).slice(0, 2).join('');
  }
  return firstLetter.toLocaleUpperCase();
}

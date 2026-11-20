import type { Address } from '@/lib/bindings';

const EMAIL = /^[^\s@<>,;"()]+@[^\s@<>,;"()]+\.[^\s@<>,;"()]+$/;

export function looksLikeEmail(value: string): boolean {
  return EMAIL.test(value.trim());
}

function pieces(text: string): string[] {
  const out: string[] = [];
  let current = '';
  let quoted = false;
  let angle = false;
  for (const ch of text) {
    if (ch === '"') quoted = !quoted;
    else if (!quoted && ch === '<') angle = true;
    else if (!quoted && ch === '>') angle = false;
    if (!quoted && !angle && (ch === ',' || ch === ';' || ch === '\n')) {
      out.push(current);
      current = '';
    } else {
      current += ch;
    }
  }
  out.push(current);
  return out.map((p) => p.trim()).filter(Boolean);
}

export function parseAddresses(text: string): Address[] {
  return pieces(text).map((piece) => {
    const angled = /^(.*?)<\s*([^<>]*?)\s*>$/.exec(piece);
    if (angled) {
      const name = (angled[1] ?? '')
        .trim()
        .replace(/^"(.*)"$/, '$1')
        .trim();
      return { email: (angled[2] ?? '').toLowerCase(), name: name || null };
    }
    return {
      email: piece
        .replace(/^"(.*)"$/, '$1')
        .trim()
        .toLowerCase(),
      name: null,
    };
  });
}

export function addressLabel(a: Address): string {
  return a.name?.trim() || a.email;
}

export function sameAddress(a: Address, b: Address): boolean {
  return a.email.toLowerCase() === b.email.toLowerCase();
}

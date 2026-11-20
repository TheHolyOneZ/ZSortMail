export type LinkRisk =
  | { kind: 'mismatch'; says: string; goes: string }
  | { kind: 'lookalike'; host: string }
  | { kind: 'address'; host: string }
  | { kind: 'credentials' };

const LOOKS_LIKE_URL = /^(?:https?:\/\/)?(?:[\p{L}\p{N}-]+\.)+[\p{L}]{2,}(?::\d+)?(?:[/?#]\S*)?$/u;

function parse(raw: string): URL | null {
  try {
    return new URL(raw);
  } catch {
    return null;
  }
}

function bare(host: string): string {
  return host.toLowerCase().replace(/^www\./, '');
}

function sameSite(a: string, b: string): boolean {
  const x = bare(a);
  const y = bare(b);
  return x === y || x.endsWith(`.${y}`) || y.endsWith(`.${x}`);
}

export function assessLink(href: string, text: string): LinkRisk | null {
  const url = parse(href);
  if (!url || (url.protocol !== 'http:' && url.protocol !== 'https:')) return null;

  if (url.username || url.password) return { kind: 'credentials' };
  const host = url.hostname;
  if (host.split('.').some((label) => label.startsWith('xn--'))) return { kind: 'lookalike', host };
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(host) || host.startsWith('['))
    return { kind: 'address', host };

  const shown = text.trim();
  if (shown && LOOKS_LIKE_URL.test(shown)) {
    const named = parse(/^https?:\/\//i.test(shown) ? shown : `https://${shown}`);
    if (named && !sameSite(named.hostname, host)) {
      return { kind: 'mismatch', says: named.hostname, goes: host };
    }
  }
  return null;
}

export function describeRisk(risk: LinkRisk): string {
  switch (risk.kind) {
    case 'mismatch':
      return `The link says ${risk.says} but goes to ${risk.goes}.`;
    case 'lookalike':
      return `${risk.host} uses characters that can imitate another site.`;
    case 'address':
      return `This link goes to a numeric address (${risk.host}), not a named site.`;
    case 'credentials':
      return 'This link has a user name in it, a trick used to hide the real site.';
  }
}

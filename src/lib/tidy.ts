export interface Tidied {
  body: string;
  footer: string | null;
  images: number;
}

const FOOTER_STARTS: RegExp[] = [
  /^reply to this email directly/i,
  /^you are receiving this because/i,
  /^you('re| are) receiving this (email|message)/i,
  /^you received this (email|message) because/i,
  /^to unsubscribe/i,
  /^unsubscribe\b/i,
  /^manage (your )?(email |notification )?(preferences|subscriptions?|settings)/i,
  /^triage notifications/i,
  /^this (email|message) was sent to/i,
  /^if you (no longer|do not|don't) want to receive/i,
  /^view (this|it) (email )?(in your browser|on github|online)/i,
  /^update (your )?(email )?preferences/i,
  /^sent by .* via /i,
  /^message id:\s*</i,
];

const IMG_TAG = /<img\b[^>]*>/gi;
const ANGLE_URL = /<(https?:\/\/[^\s<>]+)>/g;

export function tidyText(text: string): Tidied {
  let images = 0;
  let body = text
    .replace(IMG_TAG, () => {
      images += 1;
      return '[image]';
    })
    .replace(ANGLE_URL, '$1');

  const lines = body.split('\n');
  let cut = -1;
  for (let i = 0; i < lines.length; i++) {
    const line = (lines[i] ?? '').trim();
    if (FOOTER_STARTS.some((re) => re.test(line))) {
      cut = i;
      break;
    }
  }
  if (cut < 0) return { body: body.trim(), footer: null, images };

  let end = cut;
  while (end > 0 && ['', '--'].includes((lines[end - 1] ?? '').trim())) end -= 1;
  const written = lines.slice(0, end).join('\n').trim();
  const footer = lines.slice(cut).join('\n').trim();
  if (written === '') return { body: body.trim(), footer: null, images };
  body = written;
  return { body, footer, images };
}

export function shortUrl(url: string, max = 44): string {
  let host = '';
  let rest = '';
  try {
    const u = new URL(url);
    host = u.host.replace(/^www\./, '');
    rest = `${u.pathname === '/' ? '' : u.pathname}${u.search}${u.hash}`;
  } catch {
    return url.length > max ? `${url.slice(0, max - 1)}…` : url;
  }
  const full = host + rest;
  if (full.length <= max) return full;
  const room = Math.max(8, max - host.length - 1);
  return `${host}${rest.slice(0, room)}…`;
}

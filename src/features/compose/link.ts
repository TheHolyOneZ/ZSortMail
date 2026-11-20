export function normaliseUrl(input: string): string | null {
  const url = input.trim();
  if (!url || /\s/.test(url)) return null;
  if (/^https?:\/\/[^/\s]+\.[^/\s]+/i.test(url)) return url;
  if (/^mailto:[^\s@]+@[^\s@]+\.[^\s@]+$/i.test(url)) return url;
  if (/^[^\s@/:]+@[^\s@/:]+\.[^\s@/:]+$/.test(url)) return `mailto:${url}`;
  if (/^[\p{L}\p{N}-]+(\.[\p{L}\p{N}-]+)+(:\d+)?([/?#].*)?$/u.test(url)) return `https://${url}`;
  return null;
}

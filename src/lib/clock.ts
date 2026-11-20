export function now(): number {
  if (typeof window !== 'undefined') {
    const pinned = new URLSearchParams(window.location.search).get('now');
    if (pinned) {
      const t = Date.parse(pinned);
      if (!Number.isNaN(t)) return t;
    }
  }
  return Date.now();
}

export const MINUTE = 60_000;
export const HOUR = 60 * MINUTE;
export const DAY = 24 * HOUR;

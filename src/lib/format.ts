import { DAY, HOUR, MINUTE } from './clock';

function startOfDay(ts: number): number {
  const d = new Date(ts);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

function calendarDaysBetween(from: number, to: number): number {
  return Math.round((startOfDay(to) - startOfDay(from)) / DAY);
}

export function formatListTime(ts: number, nowTs: number, locale?: string): string {
  const days = calendarDaysBetween(ts, nowTs);
  if (days <= 0) {
    return new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' }).format(ts);
  }
  if (days === 1) return 'Yesterday';
  if (days < 7) return new Intl.DateTimeFormat(locale, { weekday: 'short' }).format(ts);
  const sameYear = new Date(ts).getFullYear() === new Date(nowTs).getFullYear();
  return new Intl.DateTimeFormat(
    locale,
    sameYear
      ? { day: 'numeric', month: 'short' }
      : { day: 'numeric', month: 'short', year: 'numeric' },
  ).format(ts);
}

export function formatMessageTime(ts: number, nowTs: number, locale?: string): string {
  const days = calendarDaysBetween(ts, nowTs);
  const clock = new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit' }).format(ts);
  if (days <= 0) return `Today ${clock}`;
  if (days === 1) return `Yesterday ${clock}`;
  if (days < 7) {
    return `${new Intl.DateTimeFormat(locale, { weekday: 'short' }).format(ts)} ${clock}`;
  }
  const sameYear = new Date(ts).getFullYear() === new Date(nowTs).getFullYear();
  const day = new Intl.DateTimeFormat(
    locale,
    sameYear
      ? { day: 'numeric', month: 'short' }
      : { day: 'numeric', month: 'short', year: 'numeric' },
  ).format(ts);
  return `${day}, ${clock}`;
}

export type TimeGroup = 'Today' | 'Yesterday' | 'This week' | 'This month' | 'Earlier';

export function timeGroup(ts: number, nowTs: number): TimeGroup {
  const days = calendarDaysBetween(ts, nowTs);
  if (days <= 0) return 'Today';
  if (days === 1) return 'Yesterday';
  if (days < 7) return 'This week';
  if (days < 31) return 'This month';
  return 'Earlier';
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

export function formatSize(bytes: number): string {
  if (bytes < 1000) return `${bytes} B`;
  if (bytes < 1_000_000) return `${Math.round(bytes / 1000)} KB`;
  const mb = bytes / 1_000_000;
  return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`;
}

export function gapLabel(gapMs: number): string {
  const minutes = gapMs / MINUTE;
  if (minutes < 1) return 'moments later';
  if (gapMs < HOUR) return `+${Math.round(minutes)} min`;
  if (gapMs < 2 * DAY) return `+${Math.round(gapMs / HOUR)} h`;
  if (gapMs < 3 * DAY) return `+${Math.round(gapMs / DAY)} d`;
  const days = Math.round(gapMs / DAY);
  if (days < 14) return `${days} days later`;
  if (days < 60) return `${Math.round(days / 7)} weeks later`;
  if (days < 730) return `${Math.round(days / 30)} months later`;
  return `${Math.round(days / 365)} years later`;
}

export function spanLabel(spanMs: number): string {
  if (spanMs < DAY) return 'the same day';
  const days = Math.round(spanMs / DAY);
  if (days < 14) return plural(days, 'day');
  if (days < 60) return plural(Math.round(days / 7), 'week');
  if (days < 730) return plural(Math.round(days / 30), 'month');
  return plural(Math.round(days / 365), 'year');
}

export function isLongGap(gapMs: number): boolean {
  return gapMs >= 3 * DAY;
}

export function elapsedLabel(ms: number): string {
  if (ms < MINUTE) return 'moments';
  if (ms < HOUR) return `${Math.round(ms / MINUTE)} min`;
  if (ms < DAY) return `${Math.floor(ms / HOUR)} h`;
  if (ms < 7 * DAY) {
    const days = Math.floor(ms / DAY);
    const hours = Math.floor((ms - days * DAY) / HOUR);
    return hours > 0 ? `${days} d ${hours} h` : `${days} d`;
  }
  const weeks = Math.floor(ms / (7 * DAY));
  return weeks < 9 ? plural(weeks, 'week') : plural(Math.round(ms / (30 * DAY)), 'month');
}

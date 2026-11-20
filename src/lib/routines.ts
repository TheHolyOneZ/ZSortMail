import type { Recurrence } from '@/lib/bindings';

export const DAY_NAMES = [
  'Monday',
  'Tuesday',
  'Wednesday',
  'Thursday',
  'Friday',
  'Saturday',
  'Sunday',
] as const;

export function minutesToTime(minutes: number): string {
  return `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`;
}

export function timeToMinutes(value: string): number {
  const [h, m] = value.split(':').map(Number);
  return (h ?? 0) * 60 + (m ?? 0);
}

export function ordinal(n: number): string {
  const rest = n % 100;
  if (rest >= 11 && rest <= 13) return `${n}th`;
  switch (n % 10) {
    case 1:
      return `${n}st`;
    case 2:
      return `${n}nd`;
    case 3:
      return `${n}rd`;
    default:
      return `${n}th`;
  }
}

function list(items: string[]): string {
  if (items.length <= 1) return items[0] ?? '';
  return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;
}

export function describeRecurrence(rec: Recurrence): string {
  const at = minutesToTime(rec.minutes);
  switch (rec.kind) {
    case 'daily':
      return `Every day at ${at}`;
    case 'weekdays':
      return `Every weekday at ${at}`;
    case 'weekly': {
      const days = [...new Set(rec.days)]
        .filter((d) => d >= 1 && d <= 7)
        .sort((a, b) => a - b)
        .map((d) => DAY_NAMES[d - 1] as string);
      return `Every ${list(days.length > 0 ? days : ['Monday'])} at ${at}`;
    }
    case 'monthly':
      if (rec.day_of_month >= 31) return `On the last day of every month at ${at}`;
      if (rec.day_of_month >= 29)
        return `On the ${ordinal(rec.day_of_month)} of every month, or the last day if it is shorter, at ${at}`;
      return `On the ${ordinal(rec.day_of_month)} of every month at ${at}`;
    case 'manual':
      return 'Only when you ask';
  }
}

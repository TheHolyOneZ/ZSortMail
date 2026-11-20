import { DAY, HOUR } from '@/lib/clock';

export const GAP_MIN_PX = 12;
export const GAP_MAX_PX = 176;

export function gapPx(gapMs: number): number {
  const hours = Math.max(0, gapMs) / HOUR;
  const px = GAP_MIN_PX + 16 * Math.log2(1 + hours);
  return Math.min(GAP_MAX_PX, Math.max(GAP_MIN_PX, px));
}

export const LONG_GAP_MS = 3 * DAY;

export function gapsBetween(times: readonly number[]): number[] {
  return times.map((t, i) => (i === 0 ? 0 : Math.max(0, t - (times[i - 1] ?? t))));
}

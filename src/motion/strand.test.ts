import { describe, expect, it } from 'vitest';
import { DAY, HOUR, MINUTE } from '@/lib/clock';
import { GAP_MAX_PX, GAP_MIN_PX, gapPx, gapsBetween, LONG_GAP_MS } from './strand';

describe('gapPx', () => {
  it('never goes below the minimum, even for zero or negative gaps', () => {
    expect(gapPx(0)).toBe(GAP_MIN_PX);
    expect(gapPx(-5 * HOUR)).toBe(GAP_MIN_PX);
  });

  it('is capped so a year-long silence does not scroll forever', () => {
    expect(gapPx(365 * DAY)).toBe(GAP_MAX_PX);
  });

  it('grows with elapsed time and never shrinks', () => {
    const samples = [MINUTE, 10 * MINUTE, HOUR, 6 * HOUR, DAY, 3 * DAY, 7 * DAY, 30 * DAY];
    const heights = samples.map(gapPx);
    for (let i = 1; i < heights.length; i++) {
      expect(heights[i] as number).toBeGreaterThanOrEqual(heights[i - 1] as number);
    }
  });

  it('keeps the values the design is tuned around', () => {
    expect(Math.round(gapPx(HOUR))).toBe(28);
    expect(Math.round(gapPx(DAY))).toBe(86);
    expect(Math.round(gapPx(7 * DAY))).toBe(130);
    expect(Math.round(gapPx(21 * DAY))).toBe(156);
  });

  it('keeps a three-week gap visibly shorter than a three-month gap', () => {
    expect(gapPx(90 * DAY) - gapPx(21 * DAY)).toBeGreaterThan(10);
  });
});

describe('gapsBetween', () => {
  it('returns zero for the first message and the difference for the rest', () => {
    expect(gapsBetween([1000, 4000, 4500])).toEqual([0, 3000, 500]);
  });

  it('clamps out-of-order timestamps to zero', () => {
    expect(gapsBetween([5000, 4000])).toEqual([0, 0]);
  });

  it('handles an empty list', () => {
    expect(gapsBetween([])).toEqual([]);
  });
});

describe('LONG_GAP_MS', () => {
  it('is three days', () => {
    expect(LONG_GAP_MS).toBe(3 * DAY);
  });
});

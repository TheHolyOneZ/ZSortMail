import { describe, expect, it } from 'vitest';
import { isSmooth, percentile, summarize, worstFrames } from './stats';

describe('percentile', () => {
  it('uses the nearest rank', () => {
    const list = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    expect(percentile(list, 0.5)).toBe(5);
    expect(percentile(list, 0.95)).toBe(10);
    expect(percentile(list, 0)).toBe(1);
    expect(percentile(list, 1)).toBe(10);
  });

  it('is zero for an empty list instead of NaN', () => {
    expect(percentile([], 0.95)).toBe(0);
  });
});

describe('worstFrames', () => {
  it('returns the slowest frames with the time at which they ended', () => {
    const worst = worstFrames([16, 16, 90, 16, 40, 16], 2);
    expect(worst).toEqual([
      { atMs: 122, frameMs: 90 },
      { atMs: 178, frameMs: 40 },
    ]);
  });

  it('copes with fewer frames than asked for', () => {
    expect(worstFrames([16], 5)).toEqual([{ atMs: 16, frameMs: 16 }]);
    expect(worstFrames([], 5)).toEqual([]);
  });
});

describe('summarize', () => {
  it('reports a steady 60 Hz run as 60 fps with no hitches', () => {
    const stats = summarize(
      Array.from({ length: 120 }, () => 16.7),
      16.7,
    );
    expect(Math.round(stats.fps)).toBe(60);
    expect(stats.hitches).toBe(0);
    expect(stats.p95Ms).toBeCloseTo(16.7);
    expect(isSmooth(stats, 16.7)).toBe(true);
  });

  it('counts frames over 1.5 refresh intervals as hitches', () => {
    const deltas = [
      ...Array.from({ length: 95 }, () => 16.7),
      ...Array.from({ length: 5 }, () => 40),
    ];
    const stats = summarize(deltas, 16.7);
    expect(stats.hitches).toBe(5);
    expect(stats.maxMs).toBe(40);
  });

  it('judges a 144 Hz display by its own refresh interval', () => {
    const stats = summarize(
      Array.from({ length: 200 }, () => 6.9),
      6.9,
    );
    expect(isSmooth(stats, 6.9)).toBe(true);
    expect(
      isSmooth(
        summarize(
          Array.from({ length: 200 }, () => 16.7),
          6.9,
        ),
        6.9,
      ),
    ).toBe(false);
  });

  it('fails a run with one very long frame even when most are fine', () => {
    const deltas = [...Array.from({ length: 199 }, () => 16.7), 120];
    expect(isSmooth(summarize(deltas, 16.7), 16.7)).toBe(false);
  });

  it('handles no samples', () => {
    const stats = summarize([], 16.7);
    expect(stats.frames).toBe(0);
    expect(stats.fps).toBe(0);
  });
});

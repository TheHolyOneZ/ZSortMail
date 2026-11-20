import { describe, expect, it } from 'vitest';
import {
  FrameMonitor,
  MIN_SAMPLES,
  PROBE_SLOW_MS,
  pickTier,
  RESTORE_AFTER_MS,
  ROLLING_SLOW_MS,
  shouldRestore,
  specFor,
  type TierInputs,
} from './governor';

const base: TierInputs = {
  preference: 'auto',
  reducedMotion: false,
  startupFrameMs: 16.7,
  downgraded: false,
};

describe('pickTier', () => {
  it('plays everything on a healthy machine', () => {
    expect(pickTier(base)).toBe('full');
  });

  it('gives reduced-motion users the lite tier, not off', () => {
    expect(pickTier({ ...base, reducedMotion: true })).toBe('lite');
  });

  it('starts lite when the startup probe saw slow frames', () => {
    expect(pickTier({ ...base, startupFrameMs: PROBE_SLOW_MS + 1 })).toBe('lite');
    expect(pickTier({ ...base, startupFrameMs: PROBE_SLOW_MS })).toBe('full');
  });

  it('waits for the probe instead of guessing', () => {
    expect(pickTier({ ...base, startupFrameMs: null })).toBe('full');
  });

  it('drops to lite after the rolling monitor downgrades', () => {
    expect(pickTier({ ...base, downgraded: true })).toBe('lite');
  });

  it('lets an explicit preference override the OS and the monitors', () => {
    const stressed = { ...base, reducedMotion: true, downgraded: true, startupFrameMs: 40 };
    expect(pickTier({ ...stressed, preference: 'full' })).toBe('full');
    expect(pickTier({ ...base, preference: 'off' })).toBe('off');
    expect(pickTier({ ...base, preference: 'lite' })).toBe('lite');
  });
});

describe('specFor', () => {
  it('allows no spatial movement below full', () => {
    for (const tier of ['lite', 'off'] as const) {
      expect(specFor(tier).fold).toBe(false);
      expect(specFor(tier).flight).toBe(false);
      expect(specFor(tier).draw).toBe(false);
    }
  });

  it('keeps a quiet confirmation in lite and none in off', () => {
    expect(specFor('lite').stamp).toBe('fade');
    expect(specFor('off').stamp).toBe('none');
    expect(specFor('full').stamp).toBe('full');
  });
});

describe('FrameMonitor', () => {
  function fill(monitor: FrameMonitor, ms: number, n: number) {
    for (let i = 0; i < n; i++) monitor.push(ms);
  }

  it('does not judge until it has enough samples', () => {
    const monitor = new FrameMonitor();
    fill(monitor, 50, MIN_SAMPLES - 1);
    expect(monitor.p95()).toBeNull();
    expect(monitor.shouldDowngrade()).toBe(false);
  });

  it('downgrades when the 95th percentile is slow', () => {
    const monitor = new FrameMonitor();
    fill(monitor, 16, 80);
    fill(monitor, ROLLING_SLOW_MS + 8, 20);
    expect(monitor.shouldDowngrade()).toBe(true);
  });

  it('ignores a few isolated slow frames', () => {
    const monitor = new FrameMonitor();
    fill(monitor, 16, 110);
    fill(monitor, 80, 3);
    expect(monitor.shouldDowngrade()).toBe(false);
  });

  it('forgets old frames once the window is full', () => {
    const monitor = new FrameMonitor(40);
    fill(monitor, 60, 40);
    expect(monitor.shouldDowngrade()).toBe(true);
    fill(monitor, 16, 40);
    expect(monitor.shouldDowngrade()).toBe(false);
    expect(monitor.count).toBe(40);
  });
});

describe('shouldRestore', () => {
  it('waits the full cool-down before trying full motion again', () => {
    expect(shouldRestore(null, 1_000_000)).toBe(false);
    expect(shouldRestore(1000, 1000 + RESTORE_AFTER_MS - 1)).toBe(false);
    expect(shouldRestore(1000, 1000 + RESTORE_AFTER_MS)).toBe(true);
  });
});

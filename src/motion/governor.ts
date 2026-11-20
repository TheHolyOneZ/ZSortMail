export type MotionTier = 'full' | 'lite' | 'off';
export type MotionPreference = 'auto' | MotionTier;

export const PROBE_SLOW_MS = 24;
export const ROLLING_SLOW_MS = 22;
export const MIN_SAMPLES = 30;
export const RESTORE_AFTER_MS = 60_000;

export interface TierInputs {
  preference: MotionPreference;
  reducedMotion: boolean;
  startupFrameMs: number | null;
  downgraded: boolean;
}

export function pickTier(input: TierInputs): MotionTier {
  if (input.preference !== 'auto') return input.preference;
  if (input.reducedMotion) return 'lite';
  if (input.downgraded) return 'lite';
  if (input.startupFrameMs !== null && input.startupFrameMs > PROBE_SLOW_MS) return 'lite';
  return 'full';
}

export interface MotionSpec {
  fold: boolean;
  flight: boolean;
  stamp: 'full' | 'fade' | 'none';
  settle: 'spring' | 'fade' | 'none';
  draw: boolean;
}

const SPECS: Record<MotionTier, MotionSpec> = {
  full: { fold: true, flight: true, stamp: 'full', settle: 'spring', draw: true },
  lite: { fold: false, flight: false, stamp: 'fade', settle: 'fade', draw: false },
  off: { fold: false, flight: false, stamp: 'none', settle: 'none', draw: false },
};

export function specFor(tier: MotionTier): MotionSpec {
  return SPECS[tier];
}

export class FrameMonitor {
  private readonly samples: number[] = [];

  constructor(private readonly capacity = 120) {}

  push(frameMs: number): void {
    this.samples.push(frameMs);
    if (this.samples.length > this.capacity) this.samples.shift();
  }

  get count(): number {
    return this.samples.length;
  }

  clear(): void {
    this.samples.length = 0;
  }

  p95(): number | null {
    if (this.samples.length < MIN_SAMPLES) return null;
    const sorted = [...this.samples].sort((a, b) => a - b);
    return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))] ?? null;
  }

  shouldDowngrade(): boolean {
    const p95 = this.p95();
    return p95 !== null && p95 > ROLLING_SLOW_MS;
  }
}

export function shouldRestore(downgradedAt: number | null, nowMs: number): boolean {
  return downgradedAt !== null && nowMs - downgradedAt >= RESTORE_AFTER_MS;
}

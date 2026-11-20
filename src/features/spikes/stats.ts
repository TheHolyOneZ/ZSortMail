export function percentile(sorted: readonly number[], p: number): number {
  if (sorted.length === 0) return 0;
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1));
  return sorted[index] ?? 0;
}

export interface FrameStats {
  frames: number;
  durationMs: number;
  fps: number;
  p50Ms: number;
  p95Ms: number;
  p99Ms: number;
  maxMs: number;
  hitches: number;
}

export function summarize(deltas: readonly number[], refreshMs: number): FrameStats {
  const sorted = [...deltas].sort((a, b) => a - b);
  const total = deltas.reduce((sum, d) => sum + d, 0);
  return {
    frames: deltas.length,
    durationMs: total,
    fps: total > 0 ? (deltas.length / total) * 1000 : 0,
    p50Ms: percentile(sorted, 0.5),
    p95Ms: percentile(sorted, 0.95),
    p99Ms: percentile(sorted, 0.99),
    maxMs: sorted[sorted.length - 1] ?? 0,
    hitches: deltas.filter((d) => d > refreshMs * 1.5).length,
  };
}

export function worstFrames(
  deltas: readonly number[],
  count = 5,
): Array<{ atMs: number; frameMs: number }> {
  let elapsed = 0;
  const timeline = deltas.map((frameMs) => {
    elapsed += frameMs;
    return { atMs: Math.round(elapsed), frameMs };
  });
  return timeline.sort((a, b) => b.frameMs - a.frameMs).slice(0, count);
}

export function isSmooth(stats: FrameStats, refreshMs: number): boolean {
  return stats.p95Ms <= refreshMs * 1.5 && stats.maxMs <= refreshMs * 4;
}

export function nextFrame(): Promise<number> {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

export async function sampleFrames(
  durationMs: number,
  onFrame?: (dtMs: number) => void,
): Promise<number[]> {
  const deltas: number[] = [];
  let last = await nextFrame();
  const end = last + durationMs;
  while (last < end) {
    const t = await nextFrame();
    const dt = t - last;
    deltas.push(dt);
    onFrame?.(dt);
    last = t;
  }
  return deltas;
}

export async function measureRefreshMs(): Promise<number> {
  const deltas = await sampleFrames(600);
  const sorted = [...deltas].sort((a, b) => a - b);
  return percentile(sorted, 0.5) || 16.7;
}

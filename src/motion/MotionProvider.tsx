import { MotionConfig } from 'motion/react';
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useUi } from '@/lib/store';
import {
  FrameMonitor,
  type MotionPreference,
  type MotionSpec,
  type MotionTier,
  pickTier,
  shouldRestore,
  specFor,
} from './governor';

interface MotionContextValue {
  tier: MotionTier;
  spec: MotionSpec;
  preference: MotionPreference;
  track: (durationMs: number) => void;
}

const defaultValue: MotionContextValue = {
  tier: 'full',
  spec: specFor('full'),
  preference: 'auto',
  track: () => {},
};

const MotionContext = createContext<MotionContextValue>(defaultValue);

export function useMotionGovernor(): MotionContextValue {
  return useContext(MotionContext);
}

function probeStartup(done: (frameMs: number) => void): () => void {
  const deltas: number[] = [];
  let last = 0;
  let frame = 0;
  let id = 0;
  const step = (t: number) => {
    if (last) deltas.push(t - last);
    last = t;
    frame += 1;
    if (frame < 48) {
      id = requestAnimationFrame(step);
      return;
    }
    const usable = deltas.slice(3).sort((a, b) => a - b);
    done(usable[Math.floor(usable.length / 2)] ?? 16.7);
  };
  id = requestAnimationFrame(step);
  return () => cancelAnimationFrame(id);
}

export function MotionTierOverride({ tier, children }: { tier: MotionTier; children: ReactNode }) {
  const value = useMemo<MotionContextValue>(
    () => ({ tier, spec: specFor(tier), preference: tier, track: () => {} }),
    [tier],
  );
  return (
    <MotionContext.Provider value={value}>
      <MotionConfig reducedMotion={tier === 'full' ? 'never' : 'always'}>{children}</MotionConfig>
    </MotionContext.Provider>
  );
}

export function MotionProvider({ children }: { children: ReactNode }) {
  const preference = useUi((s) => s.motion);
  const [reducedMotion, setReducedMotion] = useState(
    () => window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  );
  const [startupFrameMs, setStartupFrameMs] = useState<number | null>(null);
  const [downgradedAt, setDowngradedAt] = useState<number | null>(null);
  const monitor = useRef(new FrameMonitor());
  const tracking = useRef(false);

  useEffect(() => {
    const query = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onChange = () => setReducedMotion(query.matches);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  }, []);

  useEffect(() => {
    let cancel = () => {};
    const timer = window.setTimeout(() => {
      cancel = probeStartup(setStartupFrameMs);
    }, 900);
    return () => {
      window.clearTimeout(timer);
      cancel();
    };
  }, []);

  const track = useCallback(
    (durationMs: number) => {
      if (tracking.current) return;
      if (shouldRestore(downgradedAt, performance.now())) {
        monitor.current.clear();
        setDowngradedAt(null);
      }
      tracking.current = true;
      const end = performance.now() + durationMs;
      let last = 0;
      const step = (t: number) => {
        if (last) monitor.current.push(t - last);
        last = t;
        if (t < end) {
          requestAnimationFrame(step);
          return;
        }
        tracking.current = false;
        if (monitor.current.shouldDowngrade()) setDowngradedAt(performance.now());
      };
      requestAnimationFrame(step);
    },
    [downgradedAt],
  );

  const tier = pickTier({
    preference,
    reducedMotion,
    startupFrameMs,
    downgraded: downgradedAt !== null,
  });

  const anim = useUi((s) => s.anim);
  const value = useMemo<MotionContextValue>(() => {
    const base = specFor(tier);
    const spec: MotionSpec = {
      ...base,
      draw: base.draw && anim.strand,
      flight: base.flight && anim.flight,
      stamp:
        anim.stamp === 'none' || base.stamp === 'none'
          ? 'none'
          : anim.stamp === 'fade'
            ? 'fade'
            : base.stamp,
    };
    return { tier, spec, preference, track };
  }, [tier, preference, track, anim.strand, anim.flight, anim.stamp]);

  return (
    <MotionContext.Provider value={value}>
      <MotionConfig
        reducedMotion={tier === 'full' ? 'never' : 'always'}
        transition={tier === 'off' ? { duration: 0 } : undefined}
      >
        {children}
      </MotionConfig>
    </MotionContext.Provider>
  );
}

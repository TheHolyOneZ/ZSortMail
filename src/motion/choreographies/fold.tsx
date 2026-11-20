import { AnimatePresence, motion, type Variants } from 'motion/react';
import { type ReactNode, useState } from 'react';
import { useUi } from '@/lib/store';
import { useMotionGovernor } from '../MotionProvider';
import { duration, ease } from '../tokens';

const SWING_DEG = 82;
const PARALLAX_PX = 16;

export function FoldStack({ level, panes }: { level: number; panes: ReactNode[] }) {
  const { spec, tier, track } = useMotionGovernor();

  const [tracked, setTracked] = useState({ level, direction: 1 });
  if (tracked.level !== level) {
    setTracked({ level, direction: level > tracked.level ? 1 : -1 });
  }
  const direction = tracked.direction;

  const timing = { duration: duration.slow * 0.8, ease: ease.fold };

  const hinge: Variants = {
    enter: (dir: number) =>
      dir > 0 ? { rotateY: SWING_DEG, x: 0 } : { rotateY: 0, x: -PARALLAX_PX },
    center: { rotateY: 0, x: 0, transition: timing },
    exit: (dir: number) => ({
      ...(dir > 0 ? { rotateY: 0, x: -PARALLAX_PX } : { rotateY: SWING_DEG, x: 0 }),
      transition: timing,
    }),
  };

  const shade: Variants = {
    enter: (dir: number) => ({ opacity: dir > 0 ? 0.42 : 0.28 }),
    center: { opacity: 0, transition: timing },
    exit: (dir: number) => ({ opacity: dir > 0 ? 0.28 : 0.42, transition: timing }),
  };

  const fade: Variants = {
    enter: { opacity: 0 },
    center: { opacity: 1, transition: { duration: 0.12 } },
    exit: { opacity: 0, transition: { delay: 0.12, duration: 0.02 } },
  };

  const move = (from: Record<string, number | string>): Variants => ({
    enter: (dir: number) => (dir > 0 ? from : { x: 0, y: 0, scale: 1, opacity: 1 }),
    center: { x: 0, y: 0, scale: 1, opacity: 1, transition: timing },
    exit: (dir: number) => ({
      ...(dir > 0 ? { x: 0, y: 0, scale: 1, opacity: 1 } : from),
      transition: timing,
    }),
  });
  const slide: Variants = {
    enter: (dir: number) => (dir > 0 ? { x: '100%' } : { x: '-18%' }),
    center: { x: 0, transition: timing },
    exit: (dir: number) => ({ ...(dir > 0 ? { x: '-18%' } : { x: '100%' }), transition: timing }),
  };
  const none: Variants = {
    enter: {},
    center: { transition: { duration: 0 } },
    exit: { transition: { duration: 0 } },
  };

  const chosen = useUi((u) => u.anim.page);
  const style = tier === 'off' ? 'none' : !spec.fold && chosen === 'fold' ? 'fade' : chosen;
  const variants: Variants = {
    fold: hinge,
    slide,
    rise: move({ y: 28, opacity: 0 }),
    zoom: move({ scale: 0.94, opacity: 0 }),
    fade,
    none,
  }[style];
  const folds = style === 'fold';

  return (
    <div className="relative h-full overflow-hidden" style={{ perspective: 1400 }}>
      <AnimatePresence initial={false} custom={direction} mode="popLayout">
        <motion.div
          key={level}
          custom={direction}
          variants={variants}
          initial="enter"
          animate="center"
          exit="exit"
          onAnimationStart={() => track(400)}
          className="absolute inset-0 bg-pane"
          style={{ transformOrigin: 'left center', zIndex: level }}
        >
          {panes[level]}
          {folds && (
            <motion.div
              variants={shade}
              aria-hidden="true"
              className="pointer-events-none absolute inset-0 bg-text"
            />
          )}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

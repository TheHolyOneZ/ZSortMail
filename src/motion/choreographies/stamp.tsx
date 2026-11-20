import { motion } from 'motion/react';
import { useMotionGovernor } from '../MotionProvider';
import { duration, ease } from '../tokens';

export function Postmark({ label, onDone }: { label: string; onDone?: () => void }) {
  const { spec, track } = useMotionGovernor();
  if (spec.stamp === 'none') return null;
  const full = spec.stamp === 'full';

  return (
    <motion.svg
      viewBox="0 0 120 120"
      width={76}
      height={76}
      aria-hidden="true"
      className="pointer-events-none"
      style={{ color: 'var(--red)' }}
      initial={full ? { scale: 1.35, rotate: -9, opacity: 0 } : { opacity: 0 }}
      animate={full ? { scale: 1, rotate: -5, opacity: [0, 1, 1, 0] } : { opacity: [0, 1, 1, 0] }}
      transition={{
        duration: 1.2,
        times: [0, 0.14, 0.72, 1],
        ease: full ? ease.stamp : 'linear',
      }}
      onAnimationStart={() => track(500)}
      onAnimationComplete={onDone}
    >
      <motion.circle
        cx="60"
        cy="60"
        r="52"
        fill="none"
        stroke="currentColor"
        strokeWidth="4"
        initial={full ? { pathLength: 0 } : false}
        animate={{ pathLength: 1 }}
        transition={{ duration: duration.slow, ease: ease.fold }}
      />
      <circle cx="60" cy="60" r="44" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <text
        x="60"
        y="60"
        dy="0.34em"
        textAnchor="middle"
        fill="currentColor"
        fontSize="22"
        fontWeight="650"
        fontFamily="var(--font-ui)"
      >
        {label}
      </text>
    </motion.svg>
  );
}

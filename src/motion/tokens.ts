export const duration = {
  instant: 0.09,
  quick: 0.16,
  base: 0.24,
  slow: 0.42,
  ceremony: 0.7,
} as const;

export const ease = {
  settle: [0.22, 1, 0.36, 1],
  stamp: [0.34, 1.56, 0.64, 1],
  fold: [0.65, 0, 0.35, 1],
} as const satisfies Record<string, readonly [number, number, number, number]>;

export const settleSpring = {
  type: 'spring',
  stiffness: 520,
  damping: 38,
  mass: 1,
} as const;

import { Star } from '@phosphor-icons/react';
import { useId } from 'react';
import type { PersonKind } from '@/lib/bindings';
import { initials, stampColor } from '@/lib/ink';

export function perforatedSquarePath(teeth: number, size = 100, biteRatio = 0.26): string {
  const pitch = size / teeth;
  const r = pitch * biteRatio;
  const n = (v: number) => Number(v.toFixed(2));
  const arc = (x: number, y: number) => `A${n(r)} ${n(r)} 0 0 0 ${n(x)} ${n(y)}`;
  const centre = (i: number) => (i + 0.5) * pitch;

  let d = 'M0 0';
  for (let i = 0; i < teeth; i++) {
    const c = centre(i);
    d += ` L${n(c - r)} 0 ${arc(c + r, 0)}`;
  }
  d += ` L${size} 0`;
  for (let i = 0; i < teeth; i++) {
    const c = centre(i);
    d += ` L${size} ${n(c - r)} ${arc(size, c + r)}`;
  }
  d += ` L${size} ${size}`;
  for (let i = 0; i < teeth; i++) {
    const c = size - centre(i);
    d += ` L${n(c + r)} ${size} ${arc(c - r, size)}`;
  }
  d += ` L0 ${size}`;
  for (let i = 0; i < teeth; i++) {
    const c = size - centre(i);
    d += ` L0 ${n(c + r)} ${arc(0, c - r)}`;
  }
  return `${d} Z`;
}

const pathCache = new Map<number, string>();
function pathFor(teeth: number): string {
  let path = pathCache.get(teeth);
  if (!path) {
    path = perforatedSquarePath(teeth);
    pathCache.set(teeth, path);
  }
  return path;
}

function teethFor(size: number): number {
  if (size <= 32) return 5;
  if (size <= 56) return 7;
  return 9;
}

const WAVES = [24, 78, 90].map((y) => `M-4 ${y} q12 -8 24 0 t24 0 t24 0 t24 0 t24 0`).join(' ');

function glyphSize(letters: string): number {
  const count = Array.from(letters).length;
  return count <= 1 ? 46 : 36;
}

export interface StampProps {
  name: string;
  ink: number;
  kind?: PersonKind;
  size?: number;
  className?: string;
  favourite?: boolean;
}

export function Stamp({ favourite = false, ...props }: StampProps) {
  if (!favourite) return <StampMark {...props} />;
  const badge = Math.max(12, Math.round(props.size ? props.size * 0.4 : 14));
  return (
    <span
      className="relative inline-block shrink-0"
      style={{ width: props.size ?? 36, height: props.size ?? 36 }}
      data-favourite="true"
    >
      <StampMark {...props} />
      <span
        className="absolute -end-1 -top-1 grid place-items-center rounded-full bg-sheet text-[#b7791f] shadow-[0_0_0_1.5px_var(--sheet)]"
        style={{ width: badge, height: badge }}
        title="Favourite"
        role="img"
        aria-label="Favourite"
      >
        <Star size={badge - 4} weight="fill" />
      </span>
    </span>
  );
}

function StampMark({ name, ink, kind = 'person', size = 36, className }: StampProps) {
  const clipId = `stamp-${useId().replace(/:/g, '')}`;
  const letters = initials(name);
  const fontSize = glyphSize(letters);
  const isSelf = kind === 'self';
  const color = isSelf ? 'var(--text)' : stampColor(ink);
  const common = {
    width: size,
    height: size,
    viewBox: '0 0 100 100',
    className,
    style: { color, flexShrink: 0 },
  } as const;

  if (kind === 'service' || kind === 'list') {
    return (
      <svg {...common} aria-hidden="true" data-stamp="postmark">
        <circle cx="50" cy="50" r="45" fill="none" stroke="currentColor" strokeWidth="5" />
        <circle
          cx="50"
          cy="50"
          r="36"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeDasharray={kind === 'list' ? '3 4' : undefined}
        />
        <text
          x="50"
          y="50"
          dy="0.34em"
          textAnchor="middle"
          fill="currentColor"
          fontSize={fontSize * 0.88}
          fontWeight="650"
          fontFamily="var(--font-ui)"
        >
          {letters}
        </text>
      </svg>
    );
  }

  return (
    <svg {...common} aria-hidden="true" data-stamp="perforated">
      <path d={pathFor(teethFor(size))} fill="currentColor" />
      {size >= 56 && (
        <>
          <clipPath id={clipId}>
            <path d={pathFor(teethFor(size))} />
          </clipPath>
          <path
            d={WAVES}
            clipPath={`url(#${clipId})`}
            fill="none"
            stroke={isSelf ? 'var(--desk)' : 'var(--on-stamp)'}
            strokeOpacity="0.2"
            strokeWidth="2"
          />
        </>
      )}
      <text
        x="50"
        y="50"
        dy="0.34em"
        textAnchor="middle"
        fill={isSelf ? 'var(--desk)' : 'var(--on-stamp)'}
        fontSize={fontSize}
        fontWeight="650"
        fontFamily="var(--font-ui)"
      >
        {letters}
      </text>
    </svg>
  );
}

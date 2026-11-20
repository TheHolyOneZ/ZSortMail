import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';
import { stampColor } from '@/lib/ink';
import { categoryIcon } from './categoryIcons';

export function CategoryBadge({
  icon,
  ink,
  image,
  size = 44,
  children,
  className,
}: {
  icon: string;
  ink: number;
  image?: string | null;
  size?: number;
  children?: ReactNode;
  className?: string;
}) {
  const Icon = categoryIcon(icon);
  return (
    <span
      aria-hidden="true"
      className={cn('grid shrink-0 place-items-center overflow-hidden rounded-full', className)}
      style={{ width: size, height: size, background: image ? 'var(--desk)' : stampColor(ink) }}
    >
      {image ? (
        <img src={image} alt="" className="size-full object-cover" />
      ) : (
        (children ?? <Icon size={Math.round(size * 0.5)} weight="fill" color="var(--on-stamp)" />)
      )}
    </span>
  );
}

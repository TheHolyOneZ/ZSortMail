import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

/**
 * A top-level heading inside Settings. Larger than the setting titles under it
 * and set off by a rule, so a long page reads as sections rather than one list.
 * The first heading on a page passes `first` to skip the rule under the title.
 */
export function SectionHeading({ children, first }: { children: ReactNode; first?: boolean }) {
  return (
    <h2
      className={cn(
        'text-title font-semibold tracking-[-0.01em]',
        first ? 'mt-8' : 'mt-14 border-t border-rule-strong pt-6',
      )}
    >
      {children}
    </h2>
  );
}

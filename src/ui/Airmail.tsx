import { cn } from '@/lib/cn';

export function Airmail({ moving = false, className }: { moving?: boolean; className?: string }) {
  return <div className={cn('airmail', className)} data-moving={moving} aria-hidden="true" />;
}

import type { SVGProps } from 'react';

export function ZMark({ size = 28, ...props }: SVGProps<SVGSVGElement> & { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="currentColor"
      role="img"
      aria-label="ZSortMail"
      {...props}
    >
      <path d="M12 16h76v20H12z" />
      <path d="M88 36H62L12 64h26z" opacity="0.55" />
      <path d="M12 64h76v20H12z" />
    </svg>
  );
}

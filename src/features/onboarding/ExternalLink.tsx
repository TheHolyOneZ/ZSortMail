import { ArrowSquareOut } from '@phosphor-icons/react';
import type { ReactNode } from 'react';
import { api } from '@/lib/api';
import { isTauri } from '@/lib/tauri';
import { toastError } from '@/lib/toasts';

export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer noopener"
      onClick={(e) => {
        if (!isTauri()) return;
        e.preventDefault();
        void api.openLink(href).catch(toastError);
      }}
      className="inline-flex items-center gap-1 font-medium text-blue underline underline-offset-2"
    >
      {children}
      <ArrowSquareOut size={13} aria-hidden="true" />
    </a>
  );
}

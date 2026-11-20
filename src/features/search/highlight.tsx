import type { ReactNode } from 'react';

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

export function highlight(text: string, query: string): ReactNode {
  const words = query
    .split(/\s+/)
    .map((w) => w.trim())
    .filter((w) => w.length > 0);
  if (words.length === 0) return text;
  const pattern = new RegExp(`(${words.map(escapeRegExp).join('|')})`, 'giu');
  let offset = 0;
  return text.split(pattern).map((part, i) => {
    const key = offset;
    offset += part.length;
    return i % 2 === 1 ? (
      <mark
        key={key}
        className="rounded-[2px] bg-transparent font-semibold text-text underline decoration-blue decoration-2 underline-offset-2"
      >
        {part}
      </mark>
    ) : (
      part
    );
  });
}

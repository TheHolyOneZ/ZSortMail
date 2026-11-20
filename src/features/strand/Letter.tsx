import { Fragment, type ReactNode, useState } from 'react';
import { useUi } from '@/lib/store';
import { shortUrl, tidyText } from '@/lib/tidy';

const URL_PATTERN = /(https?:\/\/[^\s<>"')]+)/g;

function linkify(line: string, shorten = false): ReactNode[] {
  return line.split(URL_PATTERN).map((part, i) =>
    i % 2 === 1 ? (
      // biome-ignore lint/suspicious/noArrayIndexKey: static text; the parts never reorder.
      <a key={`${i}-${part}`} href={part} target="_blank" rel="noreferrer noopener" title={part}>
        {shorten && part.length > 48 ? shortUrl(part) : part}
      </a>
    ) : (
      part
    ),
  );
}

function Block({ text, shorten }: { text: string; shorten: boolean }) {
  const lines = text.split('\n');
  if (lines.length > 0 && lines.every((line) => line.startsWith('- '))) {
    return (
      <ul className="mb-[0.9em] list-disc ps-5 last:mb-0">
        {lines.map((line) => (
          <li key={line}>{linkify(line.slice(2), shorten)}</li>
        ))}
      </ul>
    );
  }
  return (
    <p>
      {lines.map((line, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: static text; lines never reorder.
        <Fragment key={i}>
          {i > 0 && <br />}
          {linkify(line, shorten)}
        </Fragment>
      ))}
    </p>
  );
}

export function Letter({ text }: { text: string }) {
  const tidy = useUi((s) => s.tidyLetters);
  const [showFooter, setShowFooter] = useState(false);
  const t = tidy ? tidyText(text) : { body: text, footer: null, images: 0 };
  const blocks = t.body.split(/\n{2,}/).filter((b) => b.trim() !== '');
  return (
    <div className="letter" dir="auto">
      {blocks.map((block, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: static text; blocks never reorder.
        <Block key={i} text={block} shorten={tidy} />
      ))}
      {t.footer && (
        <div className="mt-3 border-t border-paper-rule pt-2">
          <button
            type="button"
            aria-expanded={showFooter}
            onClick={() => setShowFooter((v) => !v)}
            className="font-ui text-meta font-medium text-paper-text-2 hover:text-paper-text"
          >
            {showFooter ? 'Hide the automatic footer' : 'Show the automatic footer'}
          </button>
          {showFooter && (
            <div className="mt-2 font-ui text-ui text-paper-text-2">
              {t.footer.split(/\n{2,}/).map((block, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: static text; blocks never reorder.
                <Block key={i} text={block} shorten={tidy} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

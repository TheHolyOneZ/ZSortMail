import { Plus, Tag as TagIcon, X } from '@phosphor-icons/react';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import { useMailSearch } from '@/lib/mailSearch';
import { useConversationTags, useTags } from '@/lib/queries';
import { toastError } from '@/lib/toasts';

export function TagBar({ conversationId }: { conversationId: number }) {
  const navigate = useNavigate();
  const { data: tags = [] } = useConversationTags(conversationId);
  const { data: known = [] } = useTags();
  const [adding, setAdding] = useState(false);
  const [text, setText] = useState('');
  const search = useMailSearch((s) => s.set);
  const suggestions = known
    .filter((k) => k.local && !tags.some((t) => t.id === k.id))
    .filter((k) => k.name.toLowerCase().includes(text.trim().toLowerCase()))
    .slice(0, 5);

  const open = (name: string) => {
    search(`label:${/\s/.test(name) ? `"${name}"` : name}`);
    void navigate({ to: '/river' });
  };

  async function add(name: string) {
    const value = name.trim();
    if (!value) return;
    setText('');
    setAdding(false);
    await act(() => api.tagConversation(conversationId, value));
  }

  return (
    <div className="mt-2 flex flex-wrap items-center gap-1.5">
      {tags.map((t) => (
        <span
          key={t.id}
          className="inline-flex items-center rounded-full border border-rule-strong text-meta"
          title={t.local ? 'Your tag' : 'A Gmail label'}
        >
          <button
            type="button"
            onClick={() => open(t.name)}
            className="inline-flex items-center gap-1 rounded-s-full py-0.5 ps-2 pe-1.5 hover:bg-hover"
          >
            <TagIcon size={11} weight={t.local ? 'fill' : 'regular'} className="text-text-2" />
            {t.name}
          </button>
          {t.local && (
            <button
              type="button"
              aria-label={`Remove the tag ${t.name}`}
              onClick={() =>
                void act(() => api.untagConversation(conversationId, t.id)).catch(toastError)
              }
              className="grid size-5 place-items-center rounded-e-full text-text-2 hover:bg-hover hover:text-text"
            >
              <X size={9} weight="bold" />
            </button>
          )}
        </span>
      ))}
      {adding ? (
        <form
          className="relative"
          onSubmit={(e) => {
            e.preventDefault();
            void add(text);
          }}
        >
          <input
            aria-label="New tag"
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                e.stopPropagation();
                setAdding(false);
                setText('');
              }
            }}
            onBlur={() => !text.trim() && setAdding(false)}
            placeholder="Tag name"
            // biome-ignore lint/a11y/noAutofocus: opened by the button that was just pressed.
            autoFocus
            maxLength={40}
            className="h-6 w-32 rounded-full border border-blue bg-sheet px-2.5 text-meta outline-none"
          />
          {suggestions.length > 0 && text.trim() !== '' && (
            <ul className="absolute top-full left-0 z-20 mt-1 min-w-36 rounded-control bg-sheet py-1 shadow-lift">
              {suggestions.map((s) => (
                <li key={s.id}>
                  <button
                    type="button"
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => void add(s.name)}
                    className="block w-full px-3 py-1 text-left text-ui hover:bg-hover"
                  >
                    {s.name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </form>
      ) : (
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="inline-flex h-6 items-center gap-1 rounded-full border border-dashed border-rule-strong px-2 text-meta text-text-2 transition-colors hover:border-blue hover:text-text"
        >
          <Plus size={10} weight="bold" />
          Tag
        </button>
      )}
    </div>
  );
}

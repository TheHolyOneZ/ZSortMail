import { X } from '@phosphor-icons/react';
import { type ReactNode, useEffect, useId, useRef, useState } from 'react';
import { addressLabel, looksLikeEmail, parseAddresses, sameAddress } from '@/lib/addresses';
import type { Address, GroupInfo, RecipientSuggestion } from '@/lib/bindings';
import { cn } from '@/lib/cn';
import { categoryAddresses } from '@/lib/composer';
import { inkFor, stampColor } from '@/lib/ink';
import { useGroups, useRecipients } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { Stamp } from '@/ui/Stamp';
import { CategoryBadge } from '../people/CategoryBadge';

const fold = (text: string) => text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().trim();

export function matches(candidate: string, typed: string): boolean {
  return fold(candidate).includes(fold(typed));
}

type Suggestion =
  | { kind: 'category'; key: string; group: GroupInfo }
  | { kind: 'person'; key: string; person: RecipientSuggestion };

export function RecipientField({
  label,
  value,
  onChange,
  autoFocus,
  trailing,
  tone = 'paper',
}: {
  label: string;
  value: Address[];
  onChange: (next: Address[]) => void;
  autoFocus?: boolean;
  trailing?: ReactNode;
  tone?: 'paper' | 'desk';
}) {
  const t =
    tone === 'paper'
      ? {
          text: 'text-paper-text',
          soft: 'text-paper-text-2',
          rule: 'border-paper-rule',
          chip: 'bg-paper-rule/40',
          hover: 'hover:bg-paper-rule hover:text-paper-text',
        }
      : {
          text: 'text-text',
          soft: 'text-text-2',
          rule: 'border-rule',
          chip: 'bg-hover',
          hover: 'hover:bg-press hover:text-text',
        };
  const [text, setText] = useState('');
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const listId = useId();
  const { data: found = [] } = useRecipients(text);
  const { data: groups = [] } = useGroups();
  const people = found.filter(
    (s) =>
      !value.some((v) => v.email === s.email.toLowerCase()) &&
      matches(`${s.name} ${s.email}`, text),
  );
  const categories = text.trim()
    ? groups.filter((g) => g.members > 0 && matches(g.name, text)).slice(0, 3)
    : [];
  const suggestions: Suggestion[] = [
    ...categories.map((g) => ({ kind: 'category' as const, key: `category:${g.id}`, group: g })),
    ...people.map((s) => ({ kind: 'person' as const, key: s.email, person: s })),
  ];
  const shown = open && text.trim().length > 0 && suggestions.length > 0;
  const current = Math.min(active, Math.max(suggestions.length - 1, 0));

  useEffect(() => {
    if (autoFocus) input.current?.focus();
  }, [autoFocus]);

  function add(addresses: Address[]) {
    const fresh = addresses.filter((a) => a.email && !value.some((v) => sameAddress(v, a)));
    if (fresh.length > 0) onChange([...value, ...fresh]);
    setText('');
    setActive(0);
  }

  function pick(s: Suggestion) {
    if (s.kind === 'person') {
      add([{ email: s.person.email.toLowerCase(), name: s.person.name }]);
      return;
    }
    categoryAddresses(s.group.id).then(add, toastError);
  }

  function commit() {
    if (text.trim()) add(parseAddresses(text));
  }

  return (
    <div className={cn('relative flex items-start gap-2 border-b px-4 py-1.5', t.rule)}>
      <label htmlFor={listId + label} className={cn('w-14 shrink-0 pt-1.5 text-meta', t.soft)}>
        {label}
      </label>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
        {value.map((a) => {
          const valid = looksLikeEmail(a.email);
          return (
            <span
              key={a.email}
              title={valid ? a.email : `“${a.email}” is not an email address`}
              style={valid ? { borderInlineStartColor: stampColor(inkFor(a.email)) } : undefined}
              className={cn(
                'inline-flex h-6 max-w-full items-center gap-1 rounded-[4px] border-s-[3px] ps-1.5 pe-0.5 text-ui',
                t.chip,
                !valid && 'border-red text-red',
              )}
            >
              <span dir="auto" className="truncate">
                {addressLabel(a)}
              </span>
              <button
                type="button"
                aria-label={`Remove ${addressLabel(a)}`}
                onClick={() => onChange(value.filter((v) => v.email !== a.email))}
                className={cn('grid size-5 place-items-center rounded-[4px]', t.soft, t.hover)}
              >
                <X size={11} weight="bold" />
              </button>
            </span>
          );
        })}
        <input
          ref={input}
          id={listId + label}
          role="combobox"
          aria-expanded={shown}
          aria-controls={`${listId}-list`}
          aria-activedescendant={shown ? `${listId}-${current}` : undefined}
          aria-autocomplete="list"
          value={text}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => {
            const next = e.target.value;
            if (/[,;]$/.test(next)) {
              add(parseAddresses(next));
              return;
            }
            setText(next);
            setOpen(true);
            setActive(0);
          }}
          onPaste={(e) => {
            const pasted = e.clipboardData.getData('text');
            if (/[,;\n]/.test(pasted)) {
              e.preventDefault();
              add(parseAddresses(pasted));
            }
          }}
          onBlur={() => {
            commit();
            setOpen(false);
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown' && shown) {
              e.preventDefault();
              setActive(Math.min(current + 1, suggestions.length - 1));
            } else if (e.key === 'ArrowUp' && shown) {
              e.preventDefault();
              setActive(Math.max(current - 1, 0));
            } else if (e.key === 'Enter' || (e.key === 'Tab' && text.trim())) {
              const chosen = shown ? suggestions[current] : undefined;
              if (chosen) {
                e.preventDefault();
                pick(chosen);
              } else if (text.trim()) {
                e.preventDefault();
                commit();
              }
            } else if (e.key === 'Backspace' && text === '' && value.length > 0) {
              onChange(value.slice(0, -1));
            } else if (e.key === 'Escape' && shown) {
              e.stopPropagation();
              setOpen(false);
            }
          }}
          className={cn(
            '!h-6 !min-h-0 min-w-[8ch] flex-1 !border-0 !bg-transparent !px-0 text-ui outline-none',
            t.text,
          )}
        />
      </div>
      {trailing}
      {shown && (
        <div
          id={`${listId}-list`}
          role="listbox"
          aria-label={`${label} suggestions`}
          className="absolute inset-x-3 top-full z-10 mt-1 max-h-60 overflow-auto rounded-pop border border-paper-rule bg-paper py-1 text-paper-text shadow-pop"
        >
          {suggestions.map((s, i) => (
            <div
              key={s.key}
              id={`${listId}-${i}`}
              role="option"
              aria-selected={i === current}
              tabIndex={-1}
              onPointerDown={(e) => {
                e.preventDefault();
                pick(s);
                input.current?.focus();
              }}
              className={cn(
                'flex cursor-pointer items-center gap-3 px-3 py-1.5',
                i === current ? 'bg-paper-rule/70' : 'hover:bg-paper-rule/40',
              )}
            >
              {s.kind === 'category' ? (
                <CategoryBadge
                  icon={s.group.icon}
                  ink={s.group.ink}
                  image={s.group.image}
                  size={24}
                />
              ) : (
                <Stamp name={s.person.name} ink={s.person.ink} kind="person" size={24} />
              )}
              <span className="min-w-0 flex-1">
                <span dir="auto" className="block truncate text-ui font-medium">
                  {s.kind === 'category' ? s.group.name : s.person.name}
                </span>
                <span className="block truncate text-meta text-paper-text-2">
                  {s.kind === 'category'
                    ? `Category, ${s.group.members === 1 ? '1 person' : `${s.group.members} people`}`
                    : s.person.email}
                </span>
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

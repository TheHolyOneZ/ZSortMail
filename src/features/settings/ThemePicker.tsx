import { Check, PencilSimple, Plus, Trash } from '@phosphor-icons/react';
import { useState } from 'react';
import { cn } from '@/lib/cn';
import { CUSTOM_PREFIX, type CustomTheme } from '@/lib/customTheme';
import { useUi } from '@/lib/store';
import { THEMES, type ThemeInfo, type ThemePreference } from '@/lib/themes';
import { ThemeEditor } from './ThemeEditor';

function Preview({ t }: { t: ThemeInfo['swatch'] }) {
  return (
    <span
      aria-hidden="true"
      className="relative block h-16 w-full overflow-hidden rounded-[6px] border border-black/10"
      style={{ background: t.desk }}
    >
      <span
        className="absolute inset-y-0 left-0 w-3"
        style={{ background: t.desk, borderRight: `1px solid ${t.text}22` }}
      />
      <span className="absolute inset-y-0 left-3 w-1/3" style={{ background: t.pane }}>
        <span
          className="mx-1.5 mt-2 block h-1.5 w-3/4 rounded-full"
          style={{ background: t.text, opacity: 0.8 }}
        />
        <span
          className="mx-1.5 mt-1.5 block h-1 w-1/2 rounded-full"
          style={{ background: t.text, opacity: 0.35 }}
        />
        <span
          className="mx-1.5 mt-2 block h-1.5 w-2/3 rounded-full"
          style={{ background: t.text, opacity: 0.8 }}
        />
      </span>
      <span
        className="absolute top-2 right-2 bottom-2 left-[calc(0.75rem+36%)] rounded-[3px]"
        style={{ background: t.sheet }}
      >
        <span
          className="mx-1.5 mt-1.5 block h-1.5 w-1/2 rounded-full"
          style={{ background: t.text, opacity: 0.8 }}
        />
        <span
          className="mx-1.5 mt-1 block h-1 w-4/5 rounded-full"
          style={{ background: t.text, opacity: 0.3 }}
        />
        <span
          className="absolute right-1.5 bottom-1.5 h-2 w-5 rounded-[2px]"
          style={{ background: t.accent }}
        />
      </span>
    </span>
  );
}

export function ThemePicker({
  value,
  onChange,
}: {
  value: ThemePreference;
  onChange: (theme: ThemePreference) => void;
}) {
  const custom = useUi((u) => u.customThemes);
  const remove = useUi((u) => u.deleteCustomTheme);
  const [editing, setEditing] = useState<CustomTheme | null | 'new'>(null);
  const own = custom.map((c) => ({
    id: `${CUSTOM_PREFIX}${c.id}`,
    name: c.name,
    description: 'A theme you made.',
    swatch: {
      desk: c.colors.desk,
      pane: c.colors.pane,
      sheet: c.colors.sheet,
      text: c.colors.text,
      accent: c.colors.blue,
    },
    theme: c,
  }));
  const cards: {
    id: ThemePreference;
    name: string;
    description: string;
    swatch: ThemeInfo['swatch'];
    theme?: CustomTheme;
  }[] = [
    {
      id: 'system',
      name: 'System',
      description: 'Paper or Ink, following your computer.',
      swatch: THEMES[0]?.swatch as ThemeInfo['swatch'],
    },
    ...THEMES,
    ...own,
  ];
  return (
    <fieldset className="mt-3">
      <legend className="sr-only">Theme</legend>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
        {cards.map((c) => {
          const on = value === c.id;
          return (
            <label
              key={c.id}
              title={c.description}
              className={cn(
                'relative flex cursor-pointer flex-col gap-2 rounded-control border p-2 transition-colors',
                on ? 'border-blue bg-blue-wash' : 'border-rule-strong hover:bg-hover',
              )}
            >
              <input
                type="radio"
                name="theme"
                value={c.id}
                checked={on}
                onChange={() => onChange(c.id)}
                className="peer sr-only"
              />
              <span className="pointer-events-none absolute inset-0 rounded-control peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-blue" />
              <Preview t={c.swatch} />
              <span className="flex items-center justify-between gap-2 px-0.5">
                <span className="truncate text-ui font-medium">{c.name}</span>
                {on && <Check size={14} weight="bold" className="shrink-0 text-blue" />}
              </span>
              {c.theme && (
                <span className="relative z-10 flex gap-1 px-0.5">
                  <button
                    type="button"
                    aria-label={`Edit the theme ${c.name}`}
                    onClick={(e) => {
                      e.preventDefault();
                      setEditing(c.theme ?? null);
                    }}
                    className="grid size-6 place-items-center rounded-[4px] text-text-2 hover:bg-hover hover:text-text"
                  >
                    <PencilSimple size={13} />
                  </button>
                  <button
                    type="button"
                    aria-label={`Delete the theme ${c.name}`}
                    onClick={(e) => {
                      e.preventDefault();
                      if (c.theme) remove(c.theme.id);
                    }}
                    className="grid size-6 place-items-center rounded-[4px] text-text-2 hover:bg-hover hover:text-red"
                  >
                    <Trash size={13} />
                  </button>
                </span>
              )}
            </label>
          );
        })}
        <button
          type="button"
          onClick={() => setEditing('new')}
          className="flex min-h-[112px] flex-col items-center justify-center gap-1.5 rounded-control border border-dashed border-rule-strong text-ui font-medium text-text-2 transition-colors hover:border-blue hover:text-text"
        >
          <Plus size={18} />
          Create a theme
        </button>
      </div>
      {editing !== null && (
        <ThemeEditor
          editing={editing === 'new' ? null : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </fieldset>
  );
}

import { Check, WarningCircle } from '@phosphor-icons/react';
import { useEffect, useMemo, useState } from 'react';
import { cn } from '@/lib/cn';
import { parseColor, toHex } from '@/lib/color';
import {
  type Colors,
  CUSTOM_PREFIX,
  type CustomTheme,
  checkContrast,
  fromShareText,
  invalidColors,
  STARTS,
  TOKENS,
  type TokenKey,
  toShareText,
} from '@/lib/customTheme';
import { useUi } from '@/lib/store';
import { toast } from '@/lib/toasts';
import { Button, Segmented } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Field, Modal, Section } from '@/ui/Modal';

function currentColors(): Colors {
  const style = getComputedStyle(document.documentElement);
  const read = (css: string, fallback: string) => {
    const parsed = parseColor(style.getPropertyValue(css));
    return parsed ? toHex(parsed) : fallback;
  };
  const dark =
    document.documentElement.style.colorScheme === 'dark' ||
    matchMedia('(prefers-color-scheme: dark)').matches;
  const base = STARTS[dark ? 'dark' : 'light'];
  return Object.fromEntries(TOKENS.map((t) => [t.key, read(t.css, base[t.key])])) as Colors;
}

function Row({
  token,
  value,
  onChange,
}: {
  token: (typeof TOKENS)[number];
  value: string;
  onChange: (v: string) => void;
}) {
  const parsed = parseColor(value);
  return (
    <div className="grid grid-cols-[1fr_auto] items-center gap-x-3 gap-y-1 py-1.5 sm:grid-cols-[minmax(0,1fr)_auto_190px]">
      <div className="min-w-0">
        <div className="text-ui font-medium">{token.label}</div>
        <div className="truncate text-meta text-text-2">{token.hint}</div>
      </div>
      <label className="relative size-9 shrink-0 cursor-pointer overflow-hidden rounded-control border border-rule-strong">
        <span className="sr-only">Pick {token.label}</span>
        <input
          type="color"
          value={parsed ? toHex(parsed) : '#000000'}
          onChange={(e) => onChange(e.target.value)}
          className="absolute -inset-2 size-[calc(100%+16px)] cursor-pointer border-0 bg-transparent p-0"
        />
      </label>
      <input
        aria-label={`${token.label} as hex or rgb`}
        aria-invalid={!parsed}
        value={value}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => onChange(e.target.value)}
        onBlur={() => parsed && onChange(toHex(parsed))}
        className={cn('col-span-2 font-mono text-ui sm:col-span-1', !parsed && '!border-red')}
        placeholder="#1a2b3c or rgb(26, 43, 60)"
      />
    </div>
  );
}

export function ThemeEditor({
  editing,
  onClose,
}: {
  editing: CustomTheme | null;
  onClose: () => void;
}) {
  const saveTheme = useUi((s) => s.saveCustomTheme);
  const setTheme = useUi((s) => s.setTheme);
  const setDraft = useUi((s) => s.setDraftTheme);
  const [name, setName] = useState(editing?.name ?? 'My theme');
  const [base, setBase] = useState<'light' | 'dark'>(editing?.base ?? 'dark');
  const [colors, setColors] = useState<Colors>(editing?.colors ?? { ...STARTS.dark });
  const [share, setShare] = useState('');
  const [problem, setProblem] = useState<string | null>(null);

  const bad = useMemo(() => invalidColors(colors), [colors]);
  const checks = useMemo(() => checkContrast(colors), [colors]);
  const unreadable = checks.filter((c) => !c.ok);

  useEffect(() => {
    setDraft({ base, colors });
  }, [base, colors, setDraft]);
  useEffect(() => () => setDraft(null), [setDraft]);

  const set = (key: TokenKey) => (value: string) => setColors((c) => ({ ...c, [key]: value }));

  function start(from: 'light' | 'dark' | 'current') {
    if (from === 'current') setColors(currentColors());
    else {
      setBase(from);
      setColors({ ...STARTS[from] });
    }
  }

  function save() {
    if (bad.length > 0) return;
    const id = editing?.id ?? `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
    const normalised = Object.fromEntries(
      TOKENS.map((t) => [t.key, toHex(parseColor(colors[t.key]) ?? { r: 0, g: 0, b: 0 })]),
    ) as Colors;
    saveTheme({ id, name: name.trim() || 'My theme', base, colors: normalised });
    setDraft(null);
    setTheme(`${CUSTOM_PREFIX}${id}`);
    toast(`Theme “${name.trim() || 'My theme'}” saved.`);
    onClose();
  }

  return (
    <Dialog
      open
      onClose={onClose}
      label={editing ? `Edit ${editing.name}` : 'New theme'}
      className="[--dialog-w:680px]"
    >
      <Modal
        title={editing ? `Edit ${editing.name}` : 'New theme'}
        description="Change a colour and the whole app follows at once. Type a hex code like #1a2b3c or an rgb value like rgb(26, 43, 60)."
        onClose={onClose}
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
        footer={
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button type="submit" variant="primary" disabled={bad.length > 0 || !name.trim()}>
              Save theme
            </Button>
          </>
        }
      >
        <div className="grid gap-3 sm:grid-cols-[1fr_auto]">
          <Field label="Name">
            <input value={name} maxLength={40} onChange={(e) => setName(e.target.value)} />
          </Field>
          <div>
            <span className="block text-ui font-medium">It is</span>
            <Segmented
              className="mt-1"
              label="Light or dark"
              value={base}
              onChange={setBase}
              options={[
                { value: 'light', label: 'Light' },
                { value: 'dark', label: 'Dark' },
              ]}
            />
          </div>
        </div>
        <p className="mt-1 text-meta text-text-2">
          Light or dark sets the scrollbars and pickers, and keeps mail on paper that suits it.
        </p>

        <Section title="Start from">
          <div className="flex flex-wrap gap-2">
            <Button onClick={() => start('light')}>Paper</Button>
            <Button onClick={() => start('dark')}>Ink</Button>
            <Button onClick={() => start('current')}>What I see now</Button>
          </div>
        </Section>

        <Section title="Colours">
          {TOKENS.map((t) => (
            <Row key={t.key} token={t} value={colors[t.key]} onChange={set(t.key)} />
          ))}
          {bad.length > 0 && (
            <p role="alert" className="mt-2 text-ui text-red">
              {bad.map((k) => TOKENS.find((t) => t.key === k)?.label).join(', ')}: not a colour. Use
              hex like #1a2b3c or rgb(26, 43, 60).
            </p>
          )}
        </Section>

        <Section
          title="Readability"
          hint="Text should have a contrast of at least 4.5 to 1 against what it sits on. You can still save a theme that does not."
        >
          <ul className="grid gap-1 sm:grid-cols-2">
            {checks.map((c) => (
              <li key={c.label} className="flex items-center gap-2 text-ui">
                {c.ok ? (
                  <Check size={14} weight="bold" className="shrink-0 text-blue" />
                ) : (
                  <WarningCircle size={14} weight="fill" className="shrink-0 text-red" />
                )}
                <span className={cn('min-w-0 flex-1 truncate', !c.ok && 'text-red')}>
                  {c.label}
                </span>
                <span className="tabular text-meta text-text-2">
                  {c.ratio ? `${c.ratio.toFixed(1)} : 1` : '–'}
                </span>
              </li>
            ))}
          </ul>
          {unreadable.length > 0 && (
            <p className="mt-2 text-meta text-text-2">
              {unreadable.length === 1
                ? 'One combination is'
                : `${unreadable.length} combinations are`}{' '}
              hard to read.
            </p>
          )}
        </Section>

        <Section
          title="Share"
          hint="Copy this text to keep the theme or send it to someone, or paste a theme here to use it."
        >
          <textarea
            aria-label="Theme as text"
            className="font-mono !text-meta"
            rows={4}
            value={share || toShareText({ name, base, colors })}
            spellCheck={false}
            onChange={(e) => {
              setShare(e.target.value);
              setProblem(null);
            }}
          />
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <Button
              onClick={() => {
                void navigator.clipboard
                  ?.writeText(share || toShareText({ name, base, colors }))
                  .then(
                    () => toast('Theme copied.'),
                    () => toast('Could not copy. Select the text and use Ctrl+C.'),
                  );
              }}
            >
              Copy
            </Button>
            <Button
              disabled={!share.trim()}
              onClick={() => {
                const read = fromShareText(share);
                if (typeof read === 'string') {
                  setProblem(read);
                  return;
                }
                setName(read.name);
                setBase(read.base);
                setColors(read.colors);
                setShare('');
                setProblem(null);
              }}
            >
              Use this text
            </Button>
            {problem && (
              <span role="alert" className="text-ui text-red">
                {problem}
              </span>
            )}
          </div>
        </Section>
      </Modal>
    </Dialog>
  );
}

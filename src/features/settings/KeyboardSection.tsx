import { useEffect, useState } from 'react';
import { useT } from '@/lib/i18n';
import {
  ACTIONS,
  type ActionId,
  bindingFor,
  bindingFromEvent,
  conflicts,
  PRESETS,
  type PresetId,
  showBinding,
} from '@/lib/keymap';
import { useUi } from '@/lib/store';
import { toast } from '@/lib/toasts';
import { Button, Kbd } from '@/ui/controls';
import { Select } from '@/ui/Select';

const mac = typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform);
const GROUPS = ['Go', 'Write', 'Sort', 'Window'] as const;

export function KeyboardSection() {
  const t = useT();
  const keys = useUi((s) => s.keys);
  const setKeys = useUi((s) => s.setKeys);
  const [listening, setListening] = useState<ActionId | null>(null);

  useEffect(() => {
    if (!listening) return;
    function onKey(e: KeyboardEvent) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Escape') {
        setListening(null);
        return;
      }
      const binding = bindingFromEvent(e);
      if (!binding || !listening) return;
      const clash = conflicts(keys, listening, binding);
      if (clash.length > 0) {
        const name = ACTIONS.find((a) => a.id === clash[0])?.label ?? 'another action';
        toast(`${showBinding(binding, mac)} already does “${name}”. Pick another key.`);
        return;
      }
      setKeys({ overrides: { ...keys.overrides, [listening]: binding } });
      setListening(null);
    }
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [listening, keys, setKeys]);

  const preset = PRESETS.find((p) => p.id === keys.preset);
  const changed = Object.keys(keys.overrides).length;

  return (
    <>
      <h2 className="mt-8 text-name font-semibold">{t('settings.keyboard')}</h2>
      <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 border-b border-rule py-4">
        <div className="max-w-[48ch]">
          <h3 className="text-body font-medium">Keys</h3>
          <p className="mt-0.5 text-ui text-text-2">
            {preset?.description} Pick a set, then change any key below.
          </p>
        </div>
        <Select<PresetId>
          label="Keys"
          className="w-48"
          align="end"
          value={keys.preset}
          onChange={(p) => setKeys({ preset: p, overrides: {} })}
          options={PRESETS.map((p) => ({ value: p.id, label: p.name, hint: p.description }))}
        />
      </div>
      {GROUPS.map((group) => (
        <section key={group} aria-label={`${group} keys`} className="mt-4">
          <h3 className="text-meta font-medium text-text-2">{group}</h3>
          <ul className="mt-1 border-t border-rule">
            {ACTIONS.filter((a) => a.group === group).map((a) => {
              const own = a.id in keys.overrides;
              return (
                <li
                  key={a.id}
                  className="flex items-center justify-between gap-4 border-b border-rule py-2"
                >
                  <span className="text-ui">{a.label}</span>
                  <span className="flex items-center gap-2">
                    {own && (
                      <button
                        type="button"
                        onClick={() => {
                          const { [a.id]: _gone, ...rest } = keys.overrides;
                          setKeys({ overrides: rest });
                        }}
                        className="text-meta text-text-2 underline underline-offset-2 hover:text-text"
                      >
                        Reset
                      </button>
                    )}
                    {listening === a.id ? (
                      <span
                        role="status"
                        className="rounded-control border border-blue px-2 py-1 text-meta text-blue"
                      >
                        Press the new key
                      </span>
                    ) : (
                      <Kbd>{showBinding(bindingFor(keys, a.id), mac)}</Kbd>
                    )}
                    <Button
                      onClick={() => setListening(listening === a.id ? null : a.id)}
                      aria-label={`Change the key for ${a.label}`}
                    >
                      {listening === a.id ? 'Cancel' : 'Change'}
                    </Button>
                  </span>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
      {changed > 0 && (
        <div className="py-4">
          <Button onClick={() => setKeys({ overrides: {} })}>
            Back to the {preset?.name} keys
          </Button>
        </div>
      )}
      <p className="mt-2 text-meta text-text-2">
        Ctrl+K opens search and commands, Ctrl+Z undoes, and Delete moves a conversation to the
        trash, whatever the keys above.
      </p>
    </>
  );
}

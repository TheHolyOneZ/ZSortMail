import { useState } from 'react';
import { api } from '@/lib/api';
import type { Rule, RuleAction } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { formatListTime, plural } from '@/lib/format';
import { usePreview } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Choice, Field, Modal, Section } from '@/ui/Modal';

type Simple = Exclude<RuleAction['type'], 'label'>;

export const ACTION_LABELS: Record<Simple | 'label', { label: string; hint: string }> = {
  archive: { label: 'Archive', hint: 'Take it out of the inbox. It stays under the person.' },
  mark_read: { label: 'Mark as read', hint: '' },
  star: { label: 'Star it', hint: '' },
  hide: { label: 'Hide it', hint: 'Out of lists, search and notifications.' },
  trash: { label: 'Move to trash', hint: 'Nothing is deleted for good.' },
  label: { label: 'Add a label', hint: 'A tag you can search for with label:' },
  allow: { label: 'Let the sender into People', hint: 'Skips the New senders deck.' },
  block: { label: 'Block the sender', hint: 'Their mail goes to Spam.' },
  mute: { label: 'Mute the sender', hint: 'No notifications from them.' },
  notify: { label: 'Always tell me', hint: 'Notify even if the sender is not a favourite.' },
};

const ORDER: (Simple | 'label')[] = [
  'archive',
  'mark_read',
  'star',
  'hide',
  'trash',
  'label',
  'allow',
  'block',
  'mute',
  'notify',
];

const GROUPS: { title: string; hint: string; types: (Simple | 'label')[] }[] = [
  {
    title: 'Tidy the mail',
    hint: 'What happens to the message itself.',
    types: ['archive', 'mark_read', 'star', 'label', 'hide', 'trash'],
  },
  {
    title: 'Deal with the sender',
    hint: 'What happens to everything from them.',
    types: ['allow', 'mute', 'block'],
  },
  { title: 'Tell me', hint: 'Whether it should interrupt you.', types: ['notify'] },
];

export function describeActions(actions: RuleAction[]): string {
  return actions
    .map((a) =>
      a.type === 'label' ? `label “${a.name}”` : ACTION_LABELS[a.type].label.toLowerCase(),
    )
    .join(', ');
}

export function RuleDialog({
  open,
  onClose,
  editing,
}: {
  open: boolean;
  onClose: () => void;
  editing?: Rule;
}) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      label={editing ? 'Edit rule' : 'New rule'}
      className="[--dialog-w:600px]"
    >
      <RuleForm onClose={onClose} editing={editing} />
    </Dialog>
  );
}

function RuleForm({ onClose, editing }: { onClose: () => void; editing?: Rule }) {
  const [name, setName] = useState(editing?.name ?? '');
  const [query, setQuery] = useState(editing?.query ?? '');
  const [chosen, setChosen] = useState<Set<Simple | 'label'>>(
    () => new Set(editing?.actions.map((a) => a.type) ?? []),
  );
  const [labelName, setLabelName] = useState(() => {
    const l = editing?.actions.find((a) => a.type === 'label');
    return l && l.type === 'label' ? l.name : '';
  });
  const [stop, setStop] = useState(editing?.stop ?? false);
  const [saving, setSaving] = useState(false);
  const preview = usePreview(query);
  const found = preview.data;

  function toggle(type: Simple | 'label') {
    setChosen((prev) => {
      const next = new Set(prev);
      if (!next.delete(type)) next.add(type);
      return next;
    });
  }

  async function save() {
    setSaving(true);
    try {
      const actions: RuleAction[] = ORDER.filter((t) => chosen.has(t)).map((t) =>
        t === 'label' ? { type: 'label', name: labelName.trim() } : ({ type: t } as RuleAction),
      );
      await api.saveRule({
        id: editing?.id ?? null,
        name,
        enabled: editing?.enabled ?? true,
        query: query.trim(),
        actions,
        stop,
      });
      onClose();
    } catch (e) {
      toastError(e);
      setSaving(false);
    }
  }

  return (
    <Modal
      title={editing ? 'Edit rule' : 'New rule'}
      description="When new mail matches a search, do something with it. See what it would catch before you save."
      onClose={onClose}
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            type="submit"
            variant="primary"
            disabled={saving || !query.trim() || chosen.size === 0}
          >
            Save rule
          </Button>
        </>
      }
    >
      <Field label="Name">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Optional. The search is used if you leave it empty."
          dir="auto"
        />
      </Field>

      <Section title="When mail matches">
        <input
          aria-label="When mail matches"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="from:shop.example subject:receipt"
          // biome-ignore lint/a11y/noAutofocus: the dialog exists for this field.
          autoFocus={!editing}
          autoComplete="off"
          spellCheck={false}
          dir="auto"
        />
        <div className="mt-2 min-h-[56px] text-ui text-text-2" aria-live="polite">
          {query.trim() === '' ? (
            'Words, or operators like from:, subject:, has:attachment, category:newsletter, newer_than:7d.'
          ) : found === undefined ? (
            'Checking'
          ) : found.count === 0 ? (
            'No mail here matches. It will still catch mail that arrives later.'
          ) : (
            <>
              This would catch {plural(found.count, 'message')} here. The newest:
              <ul className="mt-1">
                {found.samples.slice(0, 3).map((s) => (
                  <li key={s.message_id} dir="auto" className="truncate">
                    {s.from.is_me ? 'You' : s.from.name}: {s.subject},{' '}
                    {formatListTime(s.sent_at, now())}
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      </Section>

      {GROUPS.map((group) => (
        <Section key={group.title} title={group.title} hint={group.hint}>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            {group.types.map((type) => (
              <Choice
                key={type}
                checked={chosen.has(type)}
                onChange={() => toggle(type)}
                title={ACTION_LABELS[type].label}
                hint={
                  type === 'label' && chosen.has('label') ? undefined : ACTION_LABELS[type].hint
                }
              >
                {type === 'label' && chosen.has('label') && (
                  <input
                    aria-label="Label name"
                    className="mt-1"
                    value={labelName}
                    onChange={(e) => setLabelName(e.target.value)}
                    placeholder="Receipts"
                  />
                )}
              </Choice>
            ))}
          </div>
        </Section>
      ))}

      <Section title="Order">
        <Choice
          checked={stop}
          onChange={setStop}
          title="Stop here"
          hint="Rules below this one do not run for mail it catches."
        />
      </Section>
    </Modal>
  );
}

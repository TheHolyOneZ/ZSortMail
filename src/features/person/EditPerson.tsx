import { useEffect, useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { PersonDetail } from '@/lib/bindings';
import { stampColor } from '@/lib/ink';
import { useGroups, usePerson, usePersonGroups } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Field, Modal, Section } from '@/ui/Modal';

const INK_NAMES = [
  'Vermilion',
  'Copper',
  'Ochre',
  'Moss',
  'Viridian',
  'Teal',
  'Cerulean',
  'Aubergine',
  'Plum',
  'Rose',
];

export function EditPersonById({ id, onClose }: { id: number; onClose: () => void }) {
  const { data } = usePerson(id);
  if (!data) return null;
  return <EditPerson person={data} open onClose={onClose} />;
}

export function EditPerson({
  person,
  open,
  onClose,
}: {
  person: PersonDetail;
  open: boolean;
  onClose: () => void;
}) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      label={`Edit ${person.summary.name}`}
      className="[--dialog-w:560px]"
    >
      <EditForm person={person} onClose={onClose} />
    </Dialog>
  );
}

function EditForm({ person, onClose }: { person: PersonDetail; onClose: () => void }) {
  const [name, setName] = useState(person.summary.name);
  const [title, setTitle] = useState(person.summary.title ?? '');
  const [organization, setOrganization] = useState(person.organization ?? '');
  const [description, setDescription] = useState(person.description ?? '');
  const [ink, setInk] = useState(person.summary.ink);
  const [saving, setSaving] = useState(false);
  const { data: groups = [] } = useGroups();
  const { data: memberOf } = usePersonGroups(person.summary.id);
  const [chosen, setChosen] = useState<number[] | null>(null);
  const selected = chosen ?? memberOf ?? [];
  useEffect(() => {
    if (chosen === null && memberOf) setChosen(memberOf);
  }, [memberOf, chosen]);

  async function save() {
    setSaving(true);
    try {
      await api.editPerson(person.summary.id, { name, title, organization, description, ink });
      if (chosen) await api.setPersonGroups(person.summary.id, chosen);
      onClose();
    } catch (e) {
      toastError(e);
      setSaving(false);
    }
  }

  return (
    <Modal
      title={`Edit ${person.summary.name}`}
      description="Name, role and colour show wherever this person appears. Their email addresses come from their mail."
      onClose={onClose}
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={saving || !name.trim()}>
            Save
          </Button>
        </>
      }
    >
      <Section title="Who">
        <Field label="Name">
          <input value={name} onChange={(e) => setName(e.target.value)} required dir="auto" />
        </Field>
        <div className="mt-3 grid grid-cols-2 gap-3">
          <Field label="Role">
            <input value={title} onChange={(e) => setTitle(e.target.value)} dir="auto" />
          </Field>
          <Field label="Organisation">
            <input
              value={organization}
              onChange={(e) => setOrganization(e.target.value)}
              dir="auto"
            />
          </Field>
        </div>
        <Field label="About" className="mt-3">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            dir="auto"
          />
        </Field>
      </Section>

      <Section title="Email addresses" hint="Mail from any of these is kept under this person.">
        <ul className="flex flex-col gap-1">
          {person.addresses.map((a) => (
            <li key={a} className="flex items-center gap-2 rounded-control bg-hover ps-3 text-ui">
              <span dir="ltr" className="min-w-0 flex-1 select-text truncate py-1.5">
                {a}
              </span>
              {person.addresses.length > 1 && (
                <Button
                  aria-label={`Split ${a} into its own person`}
                  title="Give this address a person of its own. You can undo it."
                  onClick={() => {
                    onClose();
                    void act(() => api.splitPerson(person.summary.id, a));
                  }}
                >
                  Split off
                </Button>
              )}
            </li>
          ))}
        </ul>
      </Section>

      <Section title="Colour" hint="Their stamp, and the edge of their unread messages.">
        <div className="flex flex-wrap gap-2.5">
          {INK_NAMES.map((inkName, i) => (
            <label key={inkName} className="relative cursor-pointer" title={inkName}>
              <input
                type="radio"
                name="ink"
                value={i}
                checked={ink === i}
                onChange={() => setInk(i)}
                className="peer sr-only"
              />
              <span
                className="block size-8 rounded-sheet ring-offset-2 ring-offset-sheet peer-checked:ring-2 peer-checked:ring-text peer-focus-visible:ring-2 peer-focus-visible:ring-blue"
                style={{ background: stampColor(i) }}
              />
              <span className="sr-only">{inkName}</span>
            </label>
          ))}
        </div>
      </Section>

      {groups.length > 0 && (
        <Section title="Groups" hint="Use groups in searches, rules and Focus mode.">
          <div className="flex flex-wrap gap-2">
            {groups.map((g) => {
              const on = selected.includes(g.id);
              return (
                <label
                  key={g.id}
                  className="inline-flex cursor-pointer items-center gap-2 rounded-full border border-rule-strong px-3 py-1 text-ui has-[:checked]:border-text has-[:checked]:bg-text has-[:checked]:text-desk has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-blue"
                >
                  <input
                    type="checkbox"
                    className="sr-only"
                    checked={on}
                    onChange={() =>
                      setChosen(on ? selected.filter((id) => id !== g.id) : [...selected, g.id])
                    }
                  />
                  {g.name}
                </label>
              );
            })}
          </div>
        </Section>
      )}
    </Modal>
  );
}

import { useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import { usePeople, usePerson } from '@/lib/queries';
import { Button, SearchField } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Modal } from '@/ui/Modal';
import { Stamp } from '@/ui/Stamp';

export function MergeDialog({ id, onClose }: { id: number; onClose: () => void }) {
  const { data: person } = usePerson(id);
  const [query, setQuery] = useState('');
  const [target, setTarget] = useState<number | null>(null);
  const { data: people = [] } = usePeople('all', query);
  if (!person) return null;
  const name = person.summary.name;
  const choices = people.filter((p) => p.id !== id && p.kind === 'person').slice(0, 40);
  const picked = choices.find((p) => p.id === target) ?? people.find((p) => p.id === target);

  return (
    <Dialog open onClose={onClose} label={`Merge ${name}`} className="[--dialog-w:520px]">
      <Modal
        title={`Merge ${name} into someone`}
        description={`Pick the person ${name} really is. Everything of theirs moves there, and you can undo it.`}
        onClose={onClose}
        footer={
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button
              variant="primary"
              disabled={!picked}
              onClick={() => {
                if (!picked) return;
                onClose();
                void act(() => api.mergePeople(picked.id, id));
              }}
            >
              {picked ? `Merge into ${picked.name}` : 'Merge'}
            </Button>
          </>
        }
      >
        <SearchField label="Search people" value={query} onChange={setQuery} />
        <ul
          className="mt-3 max-h-72 overflow-auto rounded-control border border-rule"
          aria-label="People to merge into"
        >
          {choices.map((p) => (
            <li key={p.id}>
              <label className="flex cursor-pointer items-center gap-3 px-3 py-2 hover:bg-hover has-[:checked]:bg-blue-wash">
                <input
                  type="radio"
                  name="merge-target"
                  checked={target === p.id}
                  onChange={() => setTarget(p.id)}
                />
                <Stamp name={p.name} ink={p.ink} kind={p.kind} size={28} />
                <span className="min-w-0 flex-1">
                  <span dir="auto" className="block truncate text-ui font-medium">
                    {p.name}
                  </span>
                  <span dir="auto" className="block truncate text-meta text-text-2">
                    {p.last_subject}
                  </span>
                </span>
              </label>
            </li>
          ))}
          {choices.length === 0 && (
            <li className="px-3 py-4 text-ui text-text-2">No one matches.</li>
          )}
        </ul>
        {picked && (
          <p className="mt-3 text-ui text-text-2">
            {name}'s addresses, conversations, files, notes and groups move to {picked.name}. Their
            name stops appearing in People.
          </p>
        )}
      </Modal>
    </Dialog>
  );
}

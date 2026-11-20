import { api } from '@/lib/api';
import { useGroups, usePerson, usePersonGroups } from '@/lib/queries';
import { toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Choice, Modal } from '@/ui/Modal';

export function PersonCategories({ id, onClose }: { id: number; onClose: () => void }) {
  const { data: person } = usePerson(id);
  const { data: groups = [] } = useGroups();
  const { data: mine = [] } = usePersonGroups(id);
  if (!person) return null;
  const name = person.summary.name;
  const set = (group: number, on: boolean) =>
    void api
      .setPersonGroups(id, on ? [...mine, group] : mine.filter((g) => g !== group))
      .catch(toastError);
  return (
    <Dialog open onClose={onClose} label={`Categories for ${name}`}>
      <Modal
        title={`Categories for ${name}`}
        description="A person can be in as many categories as you like. Make new ones from People."
        onClose={onClose}
        footer={
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        }
      >
        {groups.length === 0 ? (
          <p className="text-ui text-text-2">
            You have no categories yet. Open People, then New category.
          </p>
        ) : (
          <div className="flex flex-col gap-2">
            {groups.map((g) => (
              <Choice
                key={g.id}
                title={g.name}
                hint={g.description || undefined}
                checked={mine.includes(g.id)}
                onChange={(on) => set(g.id, on)}
              />
            ))}
          </div>
        )}
      </Modal>
    </Dialog>
  );
}

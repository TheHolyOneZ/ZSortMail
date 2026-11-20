import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Modal } from '@/ui/Modal';
import { Members } from './CategoryEditor';

export function CategoryPeople({
  id,
  name,
  onClose,
}: {
  id: number;
  name: string;
  onClose: () => void;
}) {
  return (
    <Dialog open onClose={onClose} label={`People in ${name}`}>
      <Modal
        title={`People in ${name}`}
        description="Search someone you already write to, or type an email address and the name to show for them here."
        onClose={onClose}
        footer={
          <Button variant="primary" onClick={onClose}>
            Done
          </Button>
        }
      >
        <Members id={id} />
      </Modal>
    </Dialog>
  );
}

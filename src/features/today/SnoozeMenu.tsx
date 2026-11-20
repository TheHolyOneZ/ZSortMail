import { Clock } from '@phosphor-icons/react';
import { useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import { now } from '@/lib/clock';
import { snoozeChoices } from '@/lib/snooze';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Menu } from '@/ui/Menu';
import { Modal } from '@/ui/Modal';

function clock(at: number): string {
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'short',
    hour: '2-digit',
    minute: '2-digit',
  }).format(at);
}

function localInputValue(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function SnoozeMenu({
  conversationId,
  className,
  big = false,
}: {
  conversationId: number;
  className?: string;
  big?: boolean;
}) {
  const [picking, setPicking] = useState(false);
  const [value, setValue] = useState(() => localInputValue(now() + 86_400_000));
  const choices = snoozeChoices(now());

  return (
    <>
      <Menu
        className={className}
        label="Snooze"
        trigger={<Clock size={big ? 20 : 18} />}
        triggerClassName={big ? 'size-[40px]' : undefined}
        items={[
          ...choices.map((c) => ({
            label: c.label,
            hint: clock(c.at),
            onSelect: () => void act(() => api.snoozeConversation(conversationId, c.at)),
          })),
          { label: 'Pick a time', separated: true, onSelect: () => setPicking(true) },
        ]}
      />
      <Dialog open={picking} onClose={() => setPicking(false)} label="Snooze until">
        <Modal
          title="Snooze until"
          description="It leaves your lists and comes back, unread, at this time."
          onClose={() => setPicking(false)}
          onSubmit={(e) => {
            e.preventDefault();
            const at = new Date(value).getTime();
            if (Number.isFinite(at) && at > now()) {
              setPicking(false);
              void act(() => api.snoozeConversation(conversationId, at));
            }
          }}
          footer={
            <>
              <Button onClick={() => setPicking(false)}>Cancel</Button>
              <Button type="submit" variant="primary">
                Snooze
              </Button>
            </>
          }
        >
          <input
            type="datetime-local"
            aria-label="Date and time"
            value={value}
            min={localInputValue(now() + 60_000)}
            onChange={(e) => setValue(e.target.value)}
          />
        </Modal>
      </Dialog>
    </>
  );
}

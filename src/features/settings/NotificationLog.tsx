import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { api } from '@/lib/api';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { formatMessageTime } from '@/lib/format';
import { useNotifications } from '@/lib/queries';
import { Chip } from '@/ui/controls';

const KIND: Record<string, string> = {
  watch: 'Waiting for',
  favourite: 'Favourite',
  important: 'Important',
  rule: 'Rule',
  routine: 'Routine',
  mail: 'New mail',
  system: 'ZSortMail',
};

type Show = 'all' | 'notified' | 'quiet';

export function NotificationLog() {
  const { data: all = [], isSuccess } = useNotifications();
  const [show, setShow] = useState<Show>('all');
  const log = all.filter((n) => show === 'all' || (show === 'notified') === n.shown);
  const navigate = useNavigate();
  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Notification log">
      <div className="mx-auto max-w-[760px]">
        <h1 className="text-title font-semibold tracking-[-0.01em]">Notification log</h1>
        <p className="mt-1 max-w-[56ch] text-ui text-text-2">
          Every new message from someone, and whether it notified you. Messages that stayed quiet
          say why. The newest 500 are kept for up to 90 days.
        </p>
        <fieldset className="mt-4 flex flex-wrap gap-2 border-0 p-0">
          <legend className="sr-only">Show</legend>
          <Chip active={show === 'all'} count={all.length} onClick={() => setShow('all')}>
            Everything
          </Chip>
          <Chip
            active={show === 'notified'}
            count={all.filter((n) => n.shown).length}
            onClick={() => setShow('notified')}
          >
            Notified
          </Chip>
          <Chip
            active={show === 'quiet'}
            count={all.filter((n) => !n.shown).length}
            onClick={() => setShow('quiet')}
          >
            Not notified
          </Chip>
        </fieldset>
        {isSuccess && log.length === 0 && (
          <p className="mt-10 text-ui text-text-2">
            {all.length === 0
              ? 'Nothing yet. New mail from people shows up here as it arrives.'
              : 'Nothing in this view.'}
          </p>
        )}
        <ul className="mt-6 border-t border-rule">
          {log.map((n) => (
            <li key={n.id} className="border-b border-rule">
              <button
                type="button"
                disabled={n.conversation_id === null || n.person_id === null}
                onClick={() => {
                  void api.openNotification(n.id).catch(() => {});
                  if (n.person_id !== null && n.conversation_id !== null) {
                    void navigate({
                      to: '/people/$personId/c/$conversationId',
                      params: {
                        personId: String(n.person_id),
                        conversationId: String(n.conversation_id),
                      },
                    });
                  }
                }}
                className="flex w-full items-start gap-4 py-3 text-left hover:bg-hover"
              >
                <span className="min-w-0 flex-1">
                  <span className="flex items-baseline gap-2">
                    <span dir="auto" className="truncate text-body font-medium">
                      {n.title}
                    </span>
                    <span className="shrink-0 text-meta text-text-2">{KIND[n.kind] ?? n.kind}</span>
                  </span>
                  <span dir="auto" className="block truncate text-ui text-text-2">
                    {n.body.split('\n')[0]}
                  </span>
                  <span
                    className={cn(
                      'mt-0.5 block text-meta',
                      n.shown ? 'text-text-2' : 'font-medium text-text',
                    )}
                  >
                    {n.shown ? 'Notified. ' : 'Not notified. '}
                    {n.reason}
                  </span>
                </span>
                <time className="tabular shrink-0 text-meta text-text-2">
                  {formatMessageTime(n.at, now())}
                </time>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

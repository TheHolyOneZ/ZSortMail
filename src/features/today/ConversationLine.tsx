import { useNavigate } from '@tanstack/react-router';
import type { ReactNode } from 'react';
import type { ConversationRow } from '@/lib/bindings';
import { cn } from '@/lib/cn';
import { writeToPerson } from '@/lib/composer';
import { conversationMenu } from '@/lib/menus';
import { useContextMenu } from '@/ui/ContextMenu';
import { Stamp } from '@/ui/Stamp';

export function ConversationLine({
  row,
  meta,
  actions,
  leading,
  selected = false,
  onOpen,
}: {
  row: ConversationRow;
  meta: ReactNode;
  actions?: ReactNode;
  leading?: ReactNode;
  selected?: boolean;
  onOpen?: () => void;
}) {
  const navigate = useNavigate();
  const { onContextMenu, menu } = useContextMenu();
  const c = row.conversation;
  const unread = c.unread > 0;
  const personId = row.person_id;
  const open = () => {
    if (onOpen) return onOpen();
    if (personId === null) return;
    void navigate({
      to: '/people/$personId/c/$conversationId',
      params: { personId: String(personId), conversationId: String(c.id) },
    });
  };

  return (
    <li
      aria-current={selected ? 'true' : undefined}
      className={cn(
        'group flex items-center gap-3 border-b border-rule',
        selected && 'bg-blue-wash',
      )}
      onContextMenu={onContextMenu(() =>
        conversationMenu(c, {
          open,
          writeTo:
            personId === null
              ? undefined
              : {
                  label: `Write a new message to ${row.person_name.split(' ')[0]}`,
                  run: () => void writeToPerson(personId, row.person_name),
                },
        }),
      )}
    >
      {leading}
      <button
        type="button"
        disabled={personId === null && !onOpen}
        onClick={open}
        className="flex min-w-0 flex-1 items-center gap-3 py-3 text-left transition-colors hover:bg-hover"
      >
        <Stamp
          name={row.person_name || '?'}
          ink={row.person_ink}
          kind={row.person_kind}
          size={36}
          favourite={row.person_vip}
        />
        <span className="min-w-0 flex-1">
          <span className="flex items-baseline gap-2">
            <span
              dir="auto"
              className={cn('truncate text-body', unread ? 'font-semibold' : 'font-medium')}
            >
              {row.person_name}
            </span>
            <span className="ml-auto shrink-0 text-meta text-text-2 tabular">{meta}</span>
          </span>
          <span
            dir="auto"
            className={cn('block truncate text-ui', unread ? 'text-text' : 'text-text-2')}
          >
            {c.subject}
          </span>
          <span dir="auto" className="block truncate text-meta text-text-2">
            {c.snippet}
          </span>
        </span>
      </button>
      {actions && (
        <div className="flex shrink-0 items-center gap-0.5 opacity-70 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
          {actions}
        </div>
      )}
      {menu}
    </li>
  );
}

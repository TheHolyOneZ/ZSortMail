import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import { compose, composeFor } from '@/lib/composer';
import { toast, toastError } from '@/lib/toasts';
import type { MenuItem } from '@/ui/Menu';
import type { Address, ConversationSummary, PersonSummary } from './bindings';
import { DAY, now } from './clock';

export async function copyText(text: string, what: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    toast(`Copied ${what}.`);
  } catch {
    toast(`Could not copy ${what}. Select it and use Ctrl+C.`);
  }
}

const first = (name: string) => name.split(' ')[0] ?? name;

export async function personMenu(
  person: PersonSummary,
  actions: {
    edit: () => void;
    merge?: () => void;
    categories?: () => void;
    open?: () => void;
    afterHide?: () => void;
  },
): Promise<MenuItem[]> {
  const [flags, detail] = await Promise.all([
    api.getPersonFlags(person.id).catch(() => null),
    api.getPerson(person.id).catch(() => null),
  ]);
  const email = detail?.addresses[0];
  const items: MenuItem[] = [];
  if (actions.open) items.push({ label: 'Open', onSelect: actions.open });
  items.push({
    label: `Write to ${first(person.name)}`,
    disabled: !email,
    onSelect: () => email && void compose([{ email, name: person.name }]),
  });
  if (person.unread > 0) {
    items.push({
      label: 'Mark all read',
      onSelect: () => void act(() => api.markAllRead(person.id)),
    });
  }
  items.push({
    label: person.pinned ? 'Unpin' : 'Pin to the top',
    separated: true,
    onSelect: () => api.setPersonPinned(person.id, !person.pinned).catch(toastError),
  });
  if (flags) {
    items.push(
      {
        label: flags.vip ? 'Remove from favourites' : 'Add to favourites',
        onSelect: () =>
          void act(() => api.setPersonFlags(person.id, { ...flags, vip: !flags.vip })),
      },
      {
        label: flags.muted ? 'Unmute notifications' : 'Mute notifications',
        onSelect: () =>
          void act(() => api.setPersonFlags(person.id, { ...flags, muted: !flags.muted })),
      },
    );
  }
  if (actions.categories)
    items.push({ label: 'Add to a category', separated: true, onSelect: actions.categories });
  items.push({ label: 'Edit profile', separated: !actions.categories, onSelect: actions.edit });
  if (email)
    items.push({ label: 'Copy email address', onSelect: () => void copyText(email, email) });
  items.push(
    {
      label: `Hide ${first(person.name)}`,
      separated: true,
      onSelect: () =>
        void act(() => api.hidePerson(person.id, true)).then((ok) => ok && actions.afterHide?.()),
    },
    {
      label: 'Hide for a week',
      onSelect: () =>
        void act(() => api.hidePerson(person.id, true, now() + 7 * DAY)).then(
          (ok) => ok && actions.afterHide?.(),
        ),
    },
  );
  return items;
}

export function conversationMenu(
  c: ConversationSummary,
  actions: {
    open: () => void;
    afterRemove?: () => void;
    writeTo?: { label: string; run: () => void };
  },
): MenuItem[] {
  const gone = (ok: boolean) => ok && actions.afterRemove?.();
  return [
    { label: 'Open', onSelect: actions.open },
    { label: 'Reply', hint: 'R', onSelect: () => void composeFor(c.id, 'reply') },
    { label: 'Forward', hint: 'F', onSelect: () => void composeFor(c.id, 'forward') },
    ...(actions.writeTo ? [{ label: actions.writeTo.label, onSelect: actions.writeTo.run }] : []),
    c.unread > 0
      ? {
          label: 'Mark as read',
          separated: true,
          onSelect: () => api.markConversationRead(c.id, true).catch(toastError),
        }
      : {
          label: 'Mark as unread',
          separated: true,
          onSelect: () => api.markConversationRead(c.id, false).catch(toastError),
        },
    {
      label: 'Archive',
      hint: 'E',
      onSelect: () => void act(() => api.archiveConversation(c.id, true)).then(gone),
    },
    {
      label: 'Snooze until tomorrow',
      onSelect: () => {
        const d = new Date(now() + DAY);
        d.setHours(8, 0, 0, 0);
        void act(() => api.snoozeConversation(c.id, d.getTime()));
      },
    },
    {
      label: 'Hide conversation',
      hint: 'H',
      onSelect: () => void act(() => api.hideConversation(c.id, true)).then(gone),
    },
    {
      label: 'Move to trash',
      hint: '#',
      danger: true,
      separated: true,
      onSelect: () => void act(() => api.trashConversation(c.id, true)).then(gone),
    },
  ];
}

export function addressMenu(
  a: Address,
  personId: number | null,
  open?: (id: number) => void,
): MenuItem[] {
  const items: MenuItem[] = [
    { label: `Write to ${a.name?.trim() || a.email}`, onSelect: () => void compose([a]) },
    { label: 'Copy email address', onSelect: () => void copyText(a.email, a.email) },
  ];
  if (personId !== null && open) {
    items.unshift({ label: 'Open person', onSelect: () => open(personId) });
  }
  return items;
}

import { useNavigate } from '@tanstack/react-router';
import type { MailAddress, MessageView } from '@/lib/bindings';
import { addressMenu, copyText } from '@/lib/menus';
import { useContextMenu } from '@/ui/ContextMenu';

function full(ms: number): string {
  return new Intl.DateTimeFormat(undefined, {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(ms);
}

export function MessageDetails({ message: m }: { message: MessageView }) {
  const navigate = useNavigate();
  const { onContextMenu, menu } = useContextMenu();
  const open = (personId: number) =>
    void navigate({ to: '/people/$personId', params: { personId: String(personId) } });

  const chip = (a: MailAddress) => (
    <button
      key={a.email}
      type="button"
      title={`Copy ${a.email}`}
      onClick={() => void copyText(a.email, a.email)}
      onContextMenu={onContextMenu(() =>
        addressMenu({ email: a.email, name: a.name }, a.person_id, open),
      )}
      className="rounded-[4px] px-1 text-left hover:bg-black/5"
    >
      <span dir="auto" className="font-medium">
        {a.is_me ? 'You' : a.name || a.email}
      </span>
      {(a.name || a.is_me) && (
        <span dir="ltr" className="ms-1 text-paper-text-2">
          {a.email}
        </span>
      )}
    </button>
  );

  const rows: { label: string; value: React.ReactNode }[] = [
    {
      label: 'From',
      value: m.from.is_me
        ? chip({ email: m.from_email, name: null, person_id: null, is_me: true })
        : chip({
            email: m.from_email,
            name: m.from.name,
            person_id: m.from.person_id,
            is_me: false,
          }),
    },
    {
      label: 'To',
      value:
        m.to.length > 0 ? m.to.map(chip) : <span className="text-paper-text-2">nobody listed</span>,
    },
  ];
  if (m.cc.length > 0) rows.push({ label: 'Cc', value: m.cc.map(chip) });
  rows.push({ label: 'Date', value: <span>{full(m.sent_at)}</span> });

  return (
    <dl className="mx-4 mt-1 mb-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 rounded-sheet bg-black/[0.03] px-3 py-2 text-ui">
      {rows.map(({ label, value }) => (
        <div key={label} className="contents">
          <dt className="text-paper-text-2">{label}</dt>
          <dd className="flex min-w-0 flex-wrap gap-x-1 select-text">{value}</dd>
        </div>
      ))}
      {menu}
    </dl>
  );
}

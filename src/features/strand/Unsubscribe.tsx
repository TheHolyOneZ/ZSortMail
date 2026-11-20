import { useState } from 'react';
import { act } from '@/lib/actions';
import { api } from '@/lib/api';
import type { MessageView } from '@/lib/bindings';
import { openDraft } from '@/lib/composer';
import { toast, toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Choice, Modal } from '@/ui/Modal';

function hostOf(url: string | null): string {
  if (!url) return '';
  try {
    return new URL(url).host;
  } catch {
    return '';
  }
}

export function UnsubscribeRow({ message: m, name }: { message: MessageView; name: string }) {
  const info = m.unsubscribe;
  const [asking, setAsking] = useState(false);
  const [hide, setHide] = useState(true);
  const [busy, setBusy] = useState(false);
  if (!info) return null;

  if (info.done_at !== null) {
    return (
      <p className="mt-3 text-meta text-paper-text-2">
        You unsubscribed from {name} on{' '}
        {new Intl.DateTimeFormat(undefined, {
          day: 'numeric',
          month: 'short',
          year: 'numeric',
        }).format(info.done_at)}
        .
      </p>
    );
  }

  const oneClick = info.one_click && info.https !== null;
  const host = hostOf(info.https);
  const how = oneClick
    ? `ZSortMail asks ${host} to take you off the list. No page opens, and a few more emails may arrive while they process it.`
    : info.https
      ? `ZSortMail opens ${host} so you can finish there.`
      : `ZSortMail prepares an email to ${info.mailto?.replace(/^mailto:/i, '').split('?')[0]}. You look it over and send it.`;

  async function run() {
    setBusy(true);
    try {
      const out = await api.unsubscribe(m.id);
      setAsking(false);
      if (out.type === 'done') {
        toast(`Unsubscribed from ${name}.`);
        if (hide && out.person_id !== null) {
          const personId = out.person_id;
          await act(() => api.hidePerson(personId, true));
        }
      } else if (out.type === 'open') {
        await api.openLink(out.url);
        toast(`Opened ${hostOf(out.url)} to finish unsubscribing.`);
      } else {
        openDraft(out.draft_id);
      }
    } catch (e) {
      setAsking(false);
      toastError(e);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <button
        type="button"
        onClick={() => setAsking(true)}
        className="mt-3 rounded-sheet text-meta font-medium text-paper-text-2 underline underline-offset-2 hover:text-paper-text"
      >
        Unsubscribe
      </button>
      <Dialog open={asking} onClose={() => setAsking(false)} label={`Unsubscribe from ${name}?`}>
        <Modal
          title={`Unsubscribe from ${name}?`}
          description={how}
          onClose={() => setAsking(false)}
          footer={
            <>
              <Button onClick={() => setAsking(false)}>Cancel</Button>
              <Button variant="primary" disabled={busy} onClick={() => void run()}>
                Unsubscribe
              </Button>
            </>
          }
        >
          {oneClick ? (
            <Choice
              checked={hide}
              onChange={setHide}
              title={`Also hide ${name}`}
              hint="Anything that still arrives stays out of your lists. You can bring them back from Hidden."
            />
          ) : (
            <p className="text-ui text-text-2">Nothing is sent until you press Unsubscribe.</p>
          )}
        </Modal>
      </Dialog>
    </>
  );
}

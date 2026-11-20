import { Warning } from '@phosphor-icons/react';
import { useCallback, useState } from 'react';
import { api } from '@/lib/api';
import { isTauri } from '@/lib/tauri';
import { toast, toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Modal } from '@/ui/Modal';
import { assessLink, describeRisk, type LinkRisk } from './linkSafety';

interface Pending {
  href: string;
  text: string;
  risk: LinkRisk;
}

async function openExternally(href: string) {
  try {
    if (isTauri()) await api.openLink(href);
    else window.open(href, '_blank', 'noopener,noreferrer');
  } catch (error) {
    toastError(error);
  }
}

export function useLinkGuard() {
  const [pending, setPending] = useState<Pending | null>(null);

  const request = useCallback((href: string, text: string) => {
    const risk = assessLink(href, text);
    if (risk) setPending({ href, text, risk });
    else void openExternally(href);
  }, []);

  const dialog = (
    <Dialog open={pending !== null} onClose={() => setPending(null)} label="Check this link">
      {pending && (
        <Modal
          title="Check this link before you open it"
          description={describeRisk(pending.risk)}
          onClose={() => setPending(null)}
          footer={
            <>
              <Button
                onClick={() => {
                  void navigator.clipboard
                    ?.writeText(pending.href)
                    .then(() => toast('Link copied'));
                  setPending(null);
                }}
              >
                Copy link
              </Button>
              <Button variant="quiet" onClick={() => setPending(null)}>
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  void openExternally(pending.href);
                  setPending(null);
                }}
              >
                Open anyway
              </Button>
            </>
          }
        >
          <div className="flex items-start gap-3">
            <Warning size={20} weight="fill" className="mt-0.5 shrink-0 text-red" />
            <div className="min-w-0 flex-1">
              <p className="text-meta text-text-2">It goes to</p>
              <p
                dir="ltr"
                className="mt-1 rounded-control bg-hover px-3 py-2 text-ui break-all select-text"
              >
                {pending.href}
              </p>
            </div>
          </div>
        </Modal>
      )}
    </Dialog>
  );

  return { request, dialog };
}

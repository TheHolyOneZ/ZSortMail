import { useNavigate } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import { queryClient, useAppState } from '@/lib/queries';
import { Airmail } from '@/ui/Airmail';
import { Button } from '@/ui/controls';
import { ZMark } from '@/ui/icons';
import { LinkGmail } from './LinkGmail';

export function Welcome() {
  const navigate = useNavigate();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { data: state } = useAppState();

  const linked = (state?.accounts.length ?? 0) > 0;
  useEffect(() => {
    if (linked) void navigate({ to: '/people' });
  }, [linked, navigate]);

  async function tryDemo() {
    setBusy(true);
    setError(null);
    try {
      await api.startDemo();
      await queryClient.invalidateQueries();
      await navigate({ to: '/people' });
    } catch (e) {
      setError(typeof e === 'object' && e && 'message' in e ? String(e.message) : String(e));
      setBusy(false);
    }
  }

  return (
    <main className="grid h-full place-items-center overflow-auto bg-desk px-6 py-6">
      <section
        aria-labelledby="welcome-title"
        className="w-full max-w-[900px] overflow-hidden rounded-sheet bg-paper text-paper-text shadow-lift"
      >
        <Airmail />
        <div className="grid md:grid-cols-[1.15fr_1fr]">
          <div className="px-8 pt-7 pb-7">
            <ZMark size={30} className="text-paper-text" />
            <h1
              id="welcome-title"
              className="mt-4 text-[26px] font-semibold leading-[1.12] tracking-[-0.02em]"
            >
              Every conversation, kept with the person it belongs to.
            </h1>
            <p className="mt-3 max-w-[46ch] font-read text-[15px] leading-[1.5] text-paper-text-2">
              ZSortMail files each Gmail message under whoever wrote it. Open a person to see
              everything between you: conversations in order, their files, your notes.
            </p>
            <div className="mt-5 border-t border-paper-rule pt-4">
              <Button
                variant="quiet"
                className="-ms-3 h-9 px-3 text-ui text-paper-text hover:bg-black/5"
                onClick={() => void tryDemo()}
                disabled={busy}
              >
                {busy ? 'Opening the demo' : 'Look around with demo mail first'}
              </Button>
              <p className="mt-0.5 text-ui text-paper-text-2">
                The demo has people, files, newsletters and a 300-message thread. It never goes
                online.
              </p>
            </div>
            {error && (
              <p role="alert" className="mt-3 text-ui font-medium text-[#b3261e]">
                {error}
              </p>
            )}
          </div>
          <div className="flex flex-col justify-center border-t border-paper-rule bg-black/[0.03] px-8 py-7 md:border-s md:border-t-0">
            <h2 className="text-name font-semibold">Use your own mail</h2>
            <p className="mt-1 mb-4 text-ui text-paper-text-2">
              No ZSortMail server: your mail is stored on this computer and goes only between here
              and Google.
            </p>
            <LinkGmail />
          </div>
        </div>
      </section>
    </main>
  );
}

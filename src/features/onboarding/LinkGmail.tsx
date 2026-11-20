import { useState } from 'react';
import { api } from '@/lib/api';
import { useBackend } from '@/lib/backend';
import { useGoogleSetup } from '@/lib/queries';
import { isTauri } from '@/lib/tauri';
import { Airmail } from '@/ui/Airmail';
import { Button } from '@/ui/controls';
import { GoogleGuide } from './GoogleGuide';

function messageOf(error: unknown): string {
  return typeof error === 'object' && error && 'message' in error
    ? String((error as { message: unknown }).message)
    : String(error);
}

export function LinkGmail({
  tone = 'paper',
  label = 'Link a Gmail account',
}: {
  tone?: 'paper' | 'desk';
  label?: string;
}) {
  const { data: setup } = useGoogleSetup();
  const link = useBackend((s) => s.link);
  const setLink = useBackend((s) => s.setLink);
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const muted = tone === 'paper' ? 'text-paper-text-2' : 'text-text-2';
  const waiting = link.phase === 'waiting';

  async function start() {
    setError(null);
    try {
      const url = await api.beginGmailLink();
      setAuthUrl(url);
      if (isTauri()) await api.openLink(url);
      else window.open(url, '_blank', 'noopener');
    } catch (e) {
      setError(messageOf(e));
      setLink('idle');
    }
  }

  async function cancel() {
    await api.cancelGmailLink().catch(() => {});
    setAuthUrl(null);
  }

  if (setup && !setup.configured) return <NeedsClient tone={tone} />;

  return (
    <div>
      <div className="flex flex-wrap items-center gap-3">
        <Button
          variant={tone === 'paper' ? 'paperPrimary' : 'primary'}
          className="h-10 px-5 text-body"
          disabled={waiting}
          onClick={() => void start()}
        >
          {waiting ? 'Waiting for your browser' : link.phase === 'failed' ? 'Try again' : label}
        </Button>
        {waiting && (
          <Button variant="quiet" className="h-10 px-4 text-body" onClick={() => void cancel()}>
            Cancel
          </Button>
        )}
      </div>
      {waiting && (
        <div className="mt-4 max-w-[46ch]">
          <Airmail moving />
          <p className={`mt-2 text-ui ${muted}`}>
            Finish signing in to Google in your browser. This window updates by itself.
            {authUrl && (
              <>
                {' '}
                If nothing opened,{' '}
                <a
                  href={authUrl}
                  target="_blank"
                  rel="noreferrer"
                  className="font-medium text-paper-blue underline"
                >
                  open the sign-in page
                </a>
                .
              </>
            )}
          </p>
        </div>
      )}
      {(error || link.phase === 'failed') && (
        <p role="alert" className="mt-3 max-w-[56ch] text-ui font-medium text-[#b3261e]">
          {error ?? link.message}
        </p>
      )}
      {link.phase === 'linked' && (
        <p role="status" className={`mt-3 text-ui ${muted}`}>
          {link.message}
        </p>
      )}
    </div>
  );
}

function NeedsClient({ tone }: { tone: 'paper' | 'desk' }) {
  const [guiding, setGuiding] = useState(false);
  const muted = tone === 'paper' ? 'text-paper-text-2' : 'text-text-2';
  return (
    <div className="max-w-[52ch]">
      <h3 className="text-name font-semibold">First, a free Google key</h3>
      <p className={`mt-1 text-ui ${muted}`}>
        Google lets an app read Gmail only through a key its owner makes. You make yours once, in
        about ten minutes, and it stays on this computer. We walk you through every click.
      </p>
      <Button
        variant={tone === 'paper' ? 'paperPrimary' : 'primary'}
        className="mt-3 h-10 px-5 text-body"
        onClick={() => setGuiding(true)}
      >
        Set up Google access
      </Button>
      {guiding && <GoogleGuide onClose={() => setGuiding(false)} />}
    </div>
  );
}

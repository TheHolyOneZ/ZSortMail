import { Check, Copy } from '@phosphor-icons/react';
import { type ReactNode, useState } from 'react';
import { api } from '@/lib/api';
import { cn } from '@/lib/cn';
import { clientIdProblem, readClientJson } from '@/lib/googleClient';
import { copyText } from '@/lib/menus';
import { qk, queryClient } from '@/lib/queries';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Modal } from '@/ui/Modal';
import { ExternalLink } from './ExternalLink';

const CONSOLE = 'https://console.cloud.google.com';
const SCOPES = [
  ['openid', 'openid'],
  ['email', 'email'],
  ['profile', 'profile'],
  ['gmail.modify', 'https://www.googleapis.com/auth/gmail.modify'],
] as const;

function Copyable({ label, value }: { label: string; value: string }) {
  return (
    <button
      type="button"
      onClick={() => void copyText(value, label)}
      className="inline-flex max-w-full items-center gap-1.5 rounded-control border border-rule-strong bg-desk px-2 py-1 text-start text-meta hover:bg-hover"
      title={`Copy ${label}`}
      aria-label={`Copy ${label}`}
    >
      <code className="truncate" dir="ltr">
        {value}
      </code>
      <Copy size={13} aria-hidden="true" className="shrink-0 text-text-2" />
    </button>
  );
}

interface Step {
  title: string;
  body: ReactNode;
  see: string;
}

const STEPS: Step[] = [
  {
    title: 'Make a Google Cloud project',
    body: (
      <>
        <p>
          A project is a folder in Google's developer area that holds your sign-in settings. It is
          free and no card is needed.
        </p>
        <ol className="mt-2 list-decimal space-y-1 ps-5">
          <li>
            <ExternalLink href={`${CONSOLE}/projectcreate`}>Open “New project”</ExternalLink> and
            sign in with the Google account you read mail with.
          </li>
          <li>
            Name it <Copyable label="the project name" value="ZSortMail" /> and press <b>Create</b>.
            Leave “Organization” as it is.
          </li>
        </ol>
      </>
    ),
    see: 'The project name ZSortMail shows in the bar at the top of the page.',
  },
  {
    title: 'Turn on the Gmail API',
    body: (
      <ol className="list-decimal space-y-1 ps-5">
        <li>
          <ExternalLink href={`${CONSOLE}/apis/library/gmail.googleapis.com`}>
            Open the Gmail API page
          </ExternalLink>
          . Check the project at the top is ZSortMail.
        </li>
        <li>
          Press <b>Enable</b>.
        </li>
      </ol>
    ),
    see: 'The page changes to “API enabled” with graphs.',
  },
  {
    title: 'Set up the consent screen',
    body: (
      <>
        <p>This is the screen Google shows when you allow ZSortMail in. Nobody else sees it.</p>
        <ol className="mt-2 list-decimal space-y-1 ps-5">
          <li>
            <ExternalLink href={`${CONSOLE}/auth/overview`}>Open Google Auth Platform</ExternalLink>{' '}
            and press <b>Get started</b>.
          </li>
          <li>
            App name: <Copyable label="the app name" value="ZSortMail" />. User support email:
            choose your own address. Press <b>Next</b>.
          </li>
          <li>
            Audience: choose <b>External</b>. Next.
          </li>
          <li>Contact information: your own address. Next, tick the agreement, then Create.</li>
        </ol>
      </>
    ),
    see: 'You land on an overview page for ZSortMail.',
  },
  {
    title: 'Add yourself as a test user',
    body: (
      <>
        <p>
          Until an app is checked by Google, only people on its test list can sign in. That person
          is you.
        </p>
        <ol className="mt-2 list-decimal space-y-1 ps-5">
          <li>
            <ExternalLink href={`${CONSOLE}/auth/audience`}>Open Audience</ExternalLink>.
          </li>
          <li>
            Under <b>Test users</b> press <b>Add users</b>, type the Gmail address you want to link,
            and <b>Save</b>.
          </li>
        </ol>
      </>
    ),
    see: 'Your address is listed under Test users. Without this, Google answers “access_denied”.',
  },
  {
    title: 'Allow the Gmail permission',
    body: (
      <>
        <p>
          ZSortMail asks for one Gmail permission, <code>gmail.modify</code>. It lets the app read,
          label, archive, draft and send. ZSortMail never deletes mail for good.
        </p>
        <ol className="mt-2 list-decimal space-y-1 ps-5">
          <li>
            <ExternalLink href={`${CONSOLE}/auth/scopes`}>Open Data access</ExternalLink> and press{' '}
            <b>Add or remove scopes</b>.
          </li>
          <li>
            Tick these, or paste them one by one into “Manually add scopes”, then <b>Update</b> and{' '}
            <b>Save</b>:
            <span className="mt-1.5 flex flex-wrap gap-1.5">
              {SCOPES.map(([label, value]) => (
                <Copyable key={value} label={label} value={value} />
              ))}
            </span>
          </li>
        </ol>
      </>
    ),
    see: 'gmail.modify appears under “Restricted scopes”. That label is normal.',
  },
  {
    title: 'Create the client',
    body: (
      <ol className="list-decimal space-y-1 ps-5">
        <li>
          <ExternalLink href={`${CONSOLE}/auth/clients`}>Open Clients</ExternalLink> and press{' '}
          <b>Create client</b>.
        </li>
        <li>
          Application type: <b>Desktop app</b>. This one matters. Any other type fails later with
          “redirect_uri_mismatch”.
        </li>
        <li>
          Name: <Copyable label="the client name" value="ZSortMail desktop" />. Press <b>Create</b>.
        </li>
        <li>
          A box shows the <b>Client ID</b> and <b>Client secret</b>. Keep it open, or press{' '}
          <b>Download JSON</b>. You will paste them on the next step.
        </li>
      </ol>
    ),
    see: 'A box with a long Client ID ending in .apps.googleusercontent.com.',
  },
];

function Hand({ onSaved }: { onSaved: () => void }) {
  const [id, setId] = useState('');
  const [secret, setSecret] = useState('');
  const [file, setFile] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const problem = clientIdProblem(id);
  const field =
    'mt-1 h-9 w-full rounded-control border border-rule-strong bg-sheet px-3 text-body outline-none focus-visible:outline-2 focus-visible:outline-blue';

  function fromFile(text: string) {
    setFile(text);
    const read = readClientJson(text);
    if (read) {
      setId(read.id);
      setSecret(read.secret);
      setError(null);
    }
  }

  async function save() {
    setBusy(true);
    setError(null);
    try {
      await api.saveGoogleClient(id, secret);
      await queryClient.invalidateQueries({ queryKey: qk.googleSetup });
      onSaved();
    } catch (e) {
      setError(
        typeof e === 'object' && e && 'message' in e
          ? String((e as { message: unknown }).message)
          : String(e),
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <label className="block text-ui font-medium">
        Client ID
        <input
          className={field}
          value={id}
          onChange={(e) => setId(e.target.value)}
          placeholder="123456-abc.apps.googleusercontent.com"
          autoComplete="off"
          spellCheck={false}
          dir="ltr"
        />
      </label>
      {problem && <p className="mt-1 text-meta text-red">{problem}</p>}
      <label className="mt-3 block text-ui font-medium">
        Client secret
        <input
          className={field}
          value={secret}
          onChange={(e) => setSecret(e.target.value)}
          placeholder="GOCSPX-…"
          autoComplete="off"
          spellCheck={false}
          dir="ltr"
          type="password"
        />
      </label>
      <details className="mt-3 text-ui">
        <summary className="cursor-pointer font-medium text-blue">
          Or paste the downloaded JSON file
        </summary>
        <textarea
          value={file}
          onChange={(e) => fromFile(e.target.value)}
          rows={3}
          spellCheck={false}
          dir="ltr"
          aria-label="Downloaded JSON file"
          placeholder='{"installed":{"client_id":"…","client_secret":"…"}}'
          className="mt-1.5 w-full font-mono text-meta"
        />
        {file.trim() && !readClientJson(file) && (
          <p className="text-meta text-red">That does not look like Google's client file.</p>
        )}
      </details>
      {error && (
        <p role="alert" className="mt-3 text-ui font-medium text-red">
          {error}
        </p>
      )}
      <Button
        type="submit"
        variant="primary"
        className="mt-4 h-10 px-5"
        disabled={busy || !id.trim() || !secret.trim()}
      >
        Save and continue
      </Button>
      <div className="mt-4 rounded-control bg-desk p-3 text-meta text-text-2">
        <p className="font-medium text-text">What happens next</p>
        <ul className="mt-1 list-disc space-y-1 ps-4">
          <li>These two values stay on this computer. There is no ZSortMail server.</li>
          <li>
            Your browser opens Google. If it says <b>“Google hasn't verified this app”</b>, that is
            expected for your own client: press <b>Advanced</b>, then <b>Go to ZSortMail</b>, and
            allow. On the permissions screen, keep the <b>Gmail</b> box ticked: ZSortMail cannot
            read mail without it.
          </li>
          <li>
            While the app is in “Testing”, Google asks you to sign in again about once a week.
            ZSortMail shows a “Link again” button when that happens.
          </li>
          <li>
            Stuck? <b>access_denied</b> means your address is missing from Test users (step 4).{' '}
            <b>redirect_uri_mismatch</b> means the client is not a Desktop app (step 6).
          </li>
        </ul>
      </div>
    </form>
  );
}

export function GoogleGuide({ onClose }: { onClose: () => void }) {
  const [at, setAt] = useState(0);
  const last = STEPS.length;
  const step = STEPS[at];
  return (
    <Dialog open onClose={onClose} label="Set up Google access" className="[--dialog-w:640px]">
      <Modal
        title="Set up Google access"
        description={`Step ${at + 1} of ${last + 1}. About ten minutes, free, and you only do it once.`}
        onClose={onClose}
        footer={
          <>
            <div className="me-auto flex items-center gap-1.5" aria-hidden="true">
              {Array.from({ length: last + 1 }, (_, i) => (
                <span
                  // biome-ignore lint/suspicious/noArrayIndexKey: a fixed row of dots.
                  key={i}
                  className={cn('size-2 rounded-full', i <= at ? 'bg-blue' : 'bg-rule-strong')}
                />
              ))}
            </div>
            {at > 0 && <Button onClick={() => setAt(at - 1)}>Back</Button>}
            {at < last && (
              <Button variant="primary" onClick={() => setAt(at + 1)}>
                {at === last - 1 ? 'I have the ID and secret' : 'Done, next step'}
              </Button>
            )}
          </>
        }
      >
        <h3 className="text-body font-semibold">
          {step ? `${at + 1}. ${step.title}` : `${last + 1}. Give them to ZSortMail`}
        </h3>
        <div className="mt-2 text-ui text-text-2 [&_b]:font-semibold [&_b]:text-text [&_code]:rounded [&_code]:bg-desk [&_code]:px-1">
          {step ? (
            <>
              {step.body}
              <p className="mt-3 flex items-start gap-1.5 rounded-control bg-blue-wash px-3 py-2 text-text">
                <Check size={15} weight="bold" className="mt-0.5 shrink-0 text-blue" />
                <span>
                  <b>You should see:</b> {step.see}
                </span>
              </p>
              {at === 0 && (
                <p className="mt-3 text-meta">
                  Already have a client?{' '}
                  <button
                    type="button"
                    className="font-medium text-blue underline"
                    onClick={() => setAt(last)}
                  >
                    Skip to pasting it
                  </button>
                  .
                </p>
              )}
            </>
          ) : (
            <Hand onSaved={onClose} />
          )}
        </div>
      </Modal>
    </Dialog>
  );
}

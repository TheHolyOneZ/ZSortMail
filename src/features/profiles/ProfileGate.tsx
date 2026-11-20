import { useEffect, useState } from 'react';
import { api } from '@/lib/api';
import { profileKey } from '@/lib/profiles';
import { useAppState } from '@/lib/queries';
import { useUi } from '@/lib/store';
import { toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Modal } from '@/ui/Modal';
import { Toggle } from '@/ui/Toggle';

export function ProfileGate() {
  const accounts = useAppState().data?.accounts;
  const adopt = useUi((s) => s.adoptAccounts);
  const add = useUi((s) => s.addProfile);
  const active = useUi((s) => s.profiles.active);
  const sync = useUi((s) => s.profiles.sync);
  const [pending, setPending] = useState<string[]>([]);
  const [share, setShare] = useState(false);

  const emailsKey = (accounts ?? []).map((a) => a.email).join('|');
  // biome-ignore lint/correctness/useExhaustiveDependencies: the joined address list is the dependency.
  useEffect(() => {
    if (!accounts || accounts.length === 0) return;
    setPending(adopt(accounts.map((a) => a.email)));
  }, [emailsKey]);

  const scope =
    accounts && accounts.length >= 2
      ? (accounts.find((a) => profileKey(a.email) === active) ?? accounts[0])?.id
      : undefined;
  useEffect(() => {
    if (!accounts) return;
    void api.setActiveAccount(scope ?? null).catch(toastError);
  }, [accounts, scope]);

  const email = pending[0];
  if (email === undefined) return null;
  const label = (accounts ?? []).find((a) => profileKey(a.email) === email)?.email ?? email;
  const done = (copy: boolean) => {
    add(email, { copy, sync: share || sync });
    setPending((p) => p.slice(1));
    setShare(false);
  };

  return (
    <Dialog open onClose={() => done(false)} label="New profile">
      <Modal
        title={`Set up ${label}`}
        description={`This mailbox gets its own profile. Copy the settings and themes from ${active ?? 'your current profile'}, or start with the defaults.`}
        onClose={() => done(false)}
        footer={
          <>
            <Button onClick={() => done(false)}>Start fresh</Button>
            <Button variant="primary" onClick={() => done(true)}>
              Copy my settings
            </Button>
          </>
        }
      >
        <div className="flex items-center justify-between gap-4 rounded-control border border-rule-strong p-3">
          <div>
            <div className="text-ui font-medium">Keep settings the same on every profile</div>
            <div className="text-meta text-text-2">
              A change to a theme or a shortcut then applies everywhere. You can turn this off in
              Settings.
            </div>
          </div>
          <Toggle
            label="Keep settings the same on every profile"
            checked={share}
            onChange={setShare}
          />
        </div>
      </Modal>
    </Dialog>
  );
}

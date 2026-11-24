import { CaretDown, CaretUp } from '@phosphor-icons/react';
import { Link, useNavigate } from '@tanstack/react-router';
import { useEffect, useState } from 'react';
import { accountLabel } from '@/lib/accounts';
import { api } from '@/lib/api';
import type { AccountInfo, SyncStatus } from '@/lib/bindings';
import { now } from '@/lib/clock';
import { cn } from '@/lib/cn';
import { type LocalePreference, useT } from '@/lib/i18n';
import { profileKey } from '@/lib/profiles';
import { qk, queryClient, useAppState, useSyncStatus } from '@/lib/queries';
import {
  cleanSidebar,
  DEFAULT_SIDEBAR,
  moveTab,
  type SidebarTab,
  setTabShown,
} from '@/lib/sidebar';
import { type Density, type RowActions, type StartPage, useUi } from '@/lib/store';
import { toast, toastError } from '@/lib/toasts';
import type { Key } from '@/locales/en';
import type { MotionPreference } from '@/motion/governor';
import { useMotionGovernor } from '@/motion/MotionProvider';
import { Button, IconButton, Segmented } from '@/ui/controls';
import { Dialog } from '@/ui/Dialog';
import { Modal } from '@/ui/Modal';
import { Select } from '@/ui/Select';
import { Toggle } from '@/ui/Toggle';
import { LinkGmail } from '../onboarding/LinkGmail';
import { AnimationsSection } from './AnimationsSection';
import { ContactsSection } from './ContactsSection';
import { KeyboardSection } from './KeyboardSection';
import { NotificationsSection } from './NotificationsSection';
import { SortingSection } from './SortingSection';
import { ThemePicker } from './ThemePicker';
import { WritingSection } from './WritingSection';

function Row({
  title,
  hint,
  children,
}: {
  title: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 border-b border-rule py-4">
      <div className="max-w-[48ch]">
        <h3 className="text-body font-medium">{title}</h3>
        <p className="mt-0.5 text-ui text-text-2">{hint}</p>
      </div>
      {children}
    </div>
  );
}

function ago(ms: number, nowTs: number): string {
  const minutes = Math.max(0, Math.round((nowTs - ms) / 60_000));
  if (minutes < 1) return 'just now';
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `${hours} h ago`;
  return `${Math.round(hours / 24)} days ago`;
}

function statusLine(account: AccountInfo, status: SyncStatus | undefined): string {
  if (account.provider === 'demo') return 'Sample mail on this computer. Nothing is sent anywhere.';
  if (!status) return 'Starting';
  const queued =
    status.queued > 0
      ? ` ${status.queued} change${status.queued === 1 ? '' : 's'} waiting to send.`
      : '';
  switch (status.phase) {
    case 'reading':
      return status.total > 0
        ? `Reading your mail, ${status.done.toLocaleString()} of ${status.total.toLocaleString()}.${queued}`
        : `Reading your mail.${queued}`;
    case 'catching':
      return `Catching up with new mail.${queued}`;
    case 'offline':
      return `${status.message ?? 'Gmail did not answer.'}${queued}`;
    case 'needs_link':
      return status.message ?? 'Link this account again to keep syncing.';
    default:
      return status.last_sync_at
        ? `Up to date. Checked ${ago(status.last_sync_at, now())}.${queued}`
        : `Waiting for the first sync.${queued}`;
  }
}

function AccountRow({ account, status }: { account: AccountInfo; status: SyncStatus | undefined }) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const navigate = useNavigate();
  const demo = account.provider === 'demo';

  async function remove() {
    setBusy(true);
    try {
      await api.unlinkAccount(account.id);
      await queryClient.invalidateQueries();
      toast(demo ? 'Demo mail removed.' : `${account.email} removed from ZSortMail.`);
      setConfirming(false);
      const state = await api.appState();
      if (state.accounts.length === 0) await navigate({ to: '/welcome' });
    } catch (e) {
      toastError(e);
      setBusy(false);
    }
  }

  return (
    <div className="border-b border-rule py-4">
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2">
        <div className="min-w-0">
          <h3 dir="ltr" className="truncate text-body font-medium">
            {demo ? 'Demo mailbox' : account.email}
          </h3>
          <p className="mt-0.5 text-ui text-text-2" aria-live="polite">
            {statusLine(account, status)}
          </p>
        </div>
        <div className="flex gap-2">
          {!demo && (
            <Button onClick={() => void api.syncNow().then(() => toast('Checking Gmail now.'))}>
              Sync now
            </Button>
          )}
          <Button onClick={() => setConfirming(true)}>{demo ? 'Remove demo' : 'Remove'}</Button>
        </div>
      </div>
      {status?.phase === 'needs_link' && (
        <div className="mt-3">
          <LinkGmail tone="desk" label="Link again" />
        </div>
      )}
      <Dialog open={confirming} onClose={() => setConfirming(false)} label="Remove account">
        <Modal
          title={demo ? 'Remove the demo mailbox?' : `Remove ${account.email}?`}
          description={
            demo
              ? 'The sample people and messages are deleted from this computer.'
              : 'ZSortMail deletes its copy of this mailbox, your notes and your rules for it from this computer. Your mail in Gmail is not touched, and you can link the account again later.'
          }
          onClose={() => setConfirming(false)}
          footer={
            <>
              <Button onClick={() => setConfirming(false)}>Keep it</Button>
              <Button variant="ink" disabled={busy} onClick={() => void remove()}>
                Remove
              </Button>
            </>
          }
        >
          <p className="text-ui text-text-2">
            This cannot be undone here, but nothing in Gmail changes.
          </p>
        </Modal>
      </Dialog>
    </div>
  );
}

function AccountsSection() {
  const t = useT();
  const { data: state } = useAppState();
  const { data: statuses = [] } = useSyncStatus();
  const accounts = state?.accounts ?? [];
  const hasGmail = accounts.some((a) => a.provider === 'gmail');
  void qk;
  return (
    <>
      <h2 className="mt-8 text-name font-semibold">{t('settings.accounts')}</h2>
      {accounts.length === 0 && <p className="mt-2 text-ui text-text-2">{t('set.no_mailbox')}</p>}
      {accounts.map((a) => (
        <AccountRow key={a.id} account={a} status={statuses.find((s) => s.account_id === a.id)} />
      ))}
      <div className="py-5">
        <LinkGmail
          tone="desk"
          label={hasGmail ? 'Add another Gmail account' : 'Link a Gmail account'}
        />
      </div>
    </>
  );
}

const TAB_NAMES: Record<SidebarTab, Key> = {
  '/today': 'nav.today',
  '/people': 'nav.people',
  '/river': 'nav.mail',
  '/new-senders': 'nav.new_senders',
  '/waiting': 'nav.waiting',
  '/routines': 'nav.routines',
  '/drafts': 'nav.drafts',
  '/rules': 'nav.rules',
};

function ProfilesSection() {
  const t = useT();
  const accounts = useAppState().data?.accounts ?? [];
  const profiles = useUi((s) => s.profiles);
  const switchProfile = useUi((s) => s.switchProfile);
  const setSync = useUi((s) => s.setProfileSync);
  const copy = useUi((s) => s.copyProfile);
  const [from, setFrom] = useState('');
  if (accounts.length === 0) return null;
  const name = accountLabel;
  const others = accounts.filter((a) => profileKey(a.email) !== profiles.active);
  const source = from || (others[0] ? profileKey(others[0].email) : '');
  return (
    <>
      <h2 className="mt-8 text-name font-semibold">{t('settings.profiles')}</h2>
      <p className="mt-1 text-ui text-text-2">
        Each mailbox is a profile with its own theme, density, animations, shortcuts and sidebar.
        {accounts.length < 2 && ' Link a second account to get a second profile.'}
      </p>
      <ul
        aria-label="Profiles"
        className="mt-3 divide-y divide-rule rounded-control border border-rule-strong"
      >
        {accounts.map((a) => {
          const on = profileKey(a.email) === profiles.active;
          return (
            <li key={a.id} className="flex items-center gap-3 px-3 py-2">
              <span className="min-w-0 flex-1 truncate text-ui font-medium">{name(a)}</span>
              {on ? (
                <span className="text-meta text-text-2">{t('set.in_use')}</span>
              ) : (
                <Button onClick={() => switchProfile(a.email)}>{t('set.switch_profile')}</Button>
              )}
            </li>
          );
        })}
      </ul>
      <Row title={t('set.sync_profiles')} hint={t('set.sync_hint')}>
        <Toggle label={t('set.sync_profiles')} checked={profiles.sync} onChange={setSync} />
      </Row>
      {others.length > 0 && !profiles.sync && (
        <Row title={t('set.copy_title')} hint={t('set.copy_hint')}>
          <span className="flex items-center gap-2">
            <Select
              label={t('set.copy_from')}
              className="w-52"
              align="end"
              value={source}
              onChange={setFrom}
              options={others.map((a) => ({ value: profileKey(a.email), label: name(a) }))}
            />
            <Button onClick={() => copy(source)}>{t('set.copy')}</Button>
          </span>
        </Row>
      )}
    </>
  );
}

function TagMirrorRow() {
  const t = useT();
  const [on, setOn] = useState<boolean | null>(null);
  useEffect(() => {
    void api.getMirrorTags().then(setOn).catch(toastError);
  }, []);
  return (
    <Row title={t('set.mirror_title')} hint={t('set.mirror_hint')}>
      <Toggle
        label={t('set.mirror_title')}
        checked={on === true}
        disabled={on === null}
        onChange={(next) => {
          setOn(next);
          void api.setMirrorTags(next).catch((e) => {
            setOn(!next);
            toastError(e);
          });
        }}
      />
    </Row>
  );
}

function SidebarSection() {
  const t = useT();
  const stored = useUi((s) => s.sidebar);
  const set = useUi((s) => s.setSidebar);
  const layout = cleanSidebar(stored);
  return (
    <>
      <h2 className="mt-8 text-name font-semibold">{t('settings.sidebar')}</h2>
      <p className="mt-1 text-ui text-text-2">{t('set.sidebar_intro')}</p>
      <ul
        aria-label={t('set.sidebar_tabs')}
        className="mt-3 divide-y divide-rule rounded-control border border-rule-strong"
      >
        {layout.order.map((tab, i) => {
          const name = t(TAB_NAMES[tab]);
          const shown = !layout.hidden.includes(tab);
          return (
            <li key={tab} className="flex items-center gap-3 px-3 py-1.5">
              <span className={cn('min-w-0 flex-1 text-ui', !shown && 'text-text-2')}>{name}</span>
              <IconButton
                label={`Move ${name} up`}
                disabled={i === 0}
                className="size-[36px]"
                onClick={() => set(moveTab(layout, tab, -1))}
              >
                <CaretUp size={16} />
              </IconButton>
              <IconButton
                label={`Move ${name} down`}
                disabled={i === layout.order.length - 1}
                className="size-[36px]"
                onClick={() => set(moveTab(layout, tab, 1))}
              >
                <CaretDown size={16} />
              </IconButton>
              <Toggle
                label={`Show ${name} in the sidebar`}
                checked={shown}
                onChange={(on) => set(setTabShown(layout, tab, on))}
              />
            </li>
          );
        })}
      </ul>
      <div className="mt-2">
        <Button onClick={() => set(DEFAULT_SIDEBAR)}>{t('settings.reset_sidebar')}</Button>
      </div>
    </>
  );
}

export function SettingsScreen() {
  const t = useT();
  const {
    theme,
    density,
    motion,
    setTheme,
    setDensity,
    setMotion,
    tidyLetters,
    setTidyLetters,
    categoriesFirst,
    setCategoriesFirst,
    startPage,
    setStartPage,
    rowActions,
    setRowActions,
    language,
    setLanguage,
  } = useUi();
  const { tier } = useMotionGovernor();

  return (
    <section className="scroll-y h-full bg-pane px-10 py-8" aria-label="Settings">
      <div className="mx-auto max-w-[760px]">
        <h1 className="text-title font-semibold tracking-[-0.01em]">{t('settings.title')}</h1>
        <AccountsSection />
        <SortingSection />
        <WritingSection />
        <ContactsSection />
        <NotificationsSection />
        <h2 className="mt-8 text-name font-semibold">{t('settings.appearance')}</h2>

        <Row title={t('settings.language')} hint={t('settings.language_hint')}>
          <Select<LocalePreference>
            label={t('settings.language')}
            className="w-44"
            align="end"
            value={language}
            onChange={setLanguage}
            options={[
              { value: 'en', label: 'English' },
              { value: 'de', label: 'Deutsch' },
              { value: 'system', label: 'Same as the computer', hint: 'English or Deutsch' },
            ]}
          />
        </Row>

        <div className="border-b border-rule py-4">
          <h3 className="text-body font-medium">{t('settings.theme')}</h3>
          <p className="mt-0.5 text-ui text-text-2">
            Follow the computer, or pick one. Mail itself stays on paper.
          </p>
          <ThemePicker value={theme} onChange={setTheme} />
        </div>

        <Row title={t('set.density')} hint={t('set.density_hint')}>
          <Segmented<Density>
            label={t('set.density')}
            value={density}
            onChange={setDensity}
            options={[
              { value: 'comfortable', label: t('set.comfortable') },
              { value: 'compact', label: t('set.compact') },
            ]}
          />
        </Row>

        <Row title={t('set.start_page')} hint={t('set.start_page_hint')}>
          <Segmented<StartPage>
            label={t('set.start_page')}
            value={startPage}
            onChange={setStartPage}
            options={[
              { value: 'people', label: t('nav.people') },
              { value: 'mail', label: t('nav.mail') },
              { value: 'today', label: t('nav.today') },
            ]}
          />
        </Row>

        <Row title={t('set.row_actions')} hint={t('set.row_actions_hint')}>
          <Segmented<RowActions>
            label={t('set.row_actions')}
            value={rowActions}
            onChange={setRowActions}
            options={[
              { value: 'hover', label: t('set.on_hover') },
              { value: 'always', label: t('set.always') },
            ]}
          />
        </Row>

        <Row
          title={t('set.motion')}
          hint={`Auto lowers animation on slow machines and when the system asks for less motion. Playing now: ${tier}.`}
        >
          <Segmented<MotionPreference>
            label={t('set.motion')}
            value={motion}
            onChange={setMotion}
            options={[
              { value: 'auto', label: t('set.auto') },
              { value: 'full', label: t('set.full') },
              { value: 'lite', label: t('set.lite') },
              { value: 'off', label: t('set.off') },
            ]}
          />
        </Row>

        <AnimationsSection />
        <KeyboardSection />

        <ProfilesSection />
        <SidebarSection />

        <h2 className="mt-8 text-name font-semibold">{t('settings.people')}</h2>
        <Row title={t('set.start_categories')} hint={t('set.start_categories_hint')}>
          <Toggle
            label={t('set.start_categories')}
            checked={categoriesFirst}
            onChange={setCategoriesFirst}
          />
        </Row>

        <h2 className="mt-8 text-name font-semibold">{t('set.tags')}</h2>
        <TagMirrorRow />

        <h2 className="mt-8 text-name font-semibold">{t('settings.reading')}</h2>
        <Row title={t('set.tidy')} hint={t('set.tidy_hint')}>
          <Toggle label={t('set.tidy')} checked={tidyLetters} onChange={setTidyLetters} />
        </Row>

        <p className="mt-8 text-ui text-text-2">
          Colours, type and motion are documented on the{' '}
          <Link to="/__design" className="text-blue underline underline-offset-2">
            design specimen
          </Link>
          .
        </p>
      </div>
    </section>
  );
}

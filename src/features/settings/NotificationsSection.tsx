import { Link } from '@tanstack/react-router';
import { api } from '@/lib/api';
import type { NotificationSettings } from '@/lib/bindings';
import { useT } from '@/lib/i18n';
import { qk, queryClient, useNotificationSettings } from '@/lib/queries';
import { toast, toastError } from '@/lib/toasts';
import { Button } from '@/ui/controls';
import { SectionHeading } from '@/ui/SectionHeading';
import { Toggle } from '@/ui/Toggle';

function minutesToTime(minutes: number): string {
  return `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`;
}

function timeToMinutes(value: string): number {
  const [h, m] = value.split(':').map(Number);
  return (h ?? 0) * 60 + (m ?? 0);
}

function SettingRow({
  checked,
  onChange,
  title,
  hint,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  title: string;
  hint: string;
}) {
  return (
    <div className="flex items-start justify-between gap-6 border-b border-rule py-4">
      <span className="max-w-[52ch]">
        <span className="block text-body font-medium">{title}</span>
        <span className="mt-0.5 block text-ui text-text-2">{hint}</span>
      </span>
      <Toggle checked={checked} onChange={onChange} label={title} className="mt-0.5" />
    </div>
  );
}

export function NotificationsSection() {
  const t = useT();
  const { data: settings } = useNotificationSettings();

  async function save(next: NotificationSettings) {
    queryClient.setQueryData(qk.notificationSettings, next);
    try {
      await api.saveNotificationSettings(next);
    } catch (e) {
      toastError(e);
    } finally {
      await queryClient.invalidateQueries({ queryKey: qk.notificationSettings });
    }
  }

  if (!settings) return null;
  return (
    <>
      <SectionHeading>{t('settings.notifications')}</SectionHeading>
      <SettingRow
        checked={settings.enabled}
        onChange={(enabled) => void save({ ...settings, enabled })}
        title="Notify me"
        hint="The master switch. While it is on, the things you are waiting for, favourites and rules that ask to be told can notify you. Ordinary new mail does not, unless you turn on the next switch."
      />
      <SettingRow
        checked={settings.everyone}
        onChange={(everyone) => void save({ ...settings, everyone })}
        title="Every new message from people"
        hint="Also notify when someone you already write to sends something new. New senders, newsletters, receipts and automated mail stay quiet. Off by default, so the app only interrupts you for what you asked for."
      />
      <SettingRow
        checked={settings.focus}
        onChange={(focus) => void save({ ...settings, focus })}
        title="Focus mode"
        hint="Only favourites, always-allowed people and the things you are waiting for get through."
      />
      <SettingRow
        checked={settings.important}
        onChange={(important) => void save({ ...settings, important })}
        title="Mail Gmail marks as important"
        hint="Adds to what notifies: with this on, mail that Gmail marks as important notifies you too, even from senders that would otherwise stay quiet. Off by default, because Gmail marks a lot as important."
      />
      <div className="border-b border-rule py-4">
        <div className="flex items-start justify-between gap-6">
          <span className="max-w-[52ch]">
            <span className="block text-body font-medium">Quiet hours</span>
            <span className="mt-0.5 block text-ui text-text-2">
              Hold back everything except urgent watches.
            </span>
          </span>
          <Toggle
            label="Quiet hours"
            className="mt-0.5"
            checked={settings.quiet !== null}
            onChange={(on) =>
              void save({ ...settings, quiet: on ? { from: 22 * 60, to: 7 * 60 } : null })
            }
          />
        </div>
        {settings.quiet && (
          <div className="mt-3 flex items-center gap-3 text-ui">
            <label>
              From{' '}
              <input
                type="time"
                value={minutesToTime(settings.quiet.from)}
                onChange={(e) =>
                  settings.quiet &&
                  void save({
                    ...settings,
                    quiet: { ...settings.quiet, from: timeToMinutes(e.target.value) },
                  })
                }
                className="ml-1 h-8 rounded-control bg-hover px-2 outline-none focus-visible:outline-2 focus-visible:outline-blue"
              />
            </label>
            <label>
              to{' '}
              <input
                type="time"
                value={minutesToTime(settings.quiet.to)}
                onChange={(e) =>
                  settings.quiet &&
                  void save({
                    ...settings,
                    quiet: { ...settings.quiet, to: timeToMinutes(e.target.value) },
                  })
                }
                className="ml-1 h-8 rounded-control bg-hover px-2 outline-none focus-visible:outline-2 focus-visible:outline-blue"
              />
            </label>
          </div>
        )}
      </div>
      <div className="flex items-start justify-between gap-6 border-b border-rule py-4">
        <span className="max-w-[52ch]">
          <span className="block text-body font-medium">Check that notifications work</span>
          <span className="mt-0.5 block text-ui text-text-2">
            Shows one now. If nothing appears on Windows, open Settings, System, Notifications, find
            ZSortMail and make sure it is on, and that Do not disturb is off.
          </span>
        </span>
        <Button
          className="mt-0.5"
          onClick={() =>
            void api
              .sendTestNotification()
              .then(() => toast('Test notification sent'))
              .catch(toastError)
          }
        >
          Send a test
        </Button>
      </div>
      <p className="mt-3 text-ui text-text-2">
        <Link to="/notifications" className="font-medium text-blue underline underline-offset-2">
          See every new message and whether it notified
        </Link>
        , and why.
      </p>
    </>
  );
}

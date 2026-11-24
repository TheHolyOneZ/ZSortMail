import { Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { useEffect } from 'react';
import { Composer } from '@/features/compose/Composer';
import { Palette } from '@/features/search/Palette';
import { undo } from '@/lib/actions';
import { api } from '@/lib/api';
import { useBackend } from '@/lib/backend';
import { compose, openDraft } from '@/lib/composer';
import { bindingFor, matches } from '@/lib/keymap';
import { listenForMailto } from '@/lib/mailto';
import { usePalette } from '@/lib/palette';
import { useStoreEvents } from '@/lib/queries';
import { useApplyUiPreferences, useUi } from '@/lib/store';
import { useToasts } from '@/lib/toasts';
import { Airmail } from '@/ui/Airmail';
import { Toaster } from '@/ui/Toaster';
import { ProfileGate } from '../features/profiles/ProfileGate';
import { Rail } from './Rail';
import { Frame } from './TitleBar';

export function Shell() {
  useApplyUiPreferences();
  useStoreEvents();
  useNotifications();
  useEffect(() => {
    const stop = listenForMailto();
    return () => void stop.then((off) => off());
  }, []);
  const syncing = useBackend((s) => s.sync.phase === 'reading' || s.sync.phase === 'catching');
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const target = e.target as HTMLElement | null;
      const typing =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target?.isContentEditable === true;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        usePalette.getState().toggle();
      } else if (
        (e.ctrlKey || e.metaKey) &&
        !e.shiftKey &&
        e.key.toLowerCase() === 'z' &&
        !typing
      ) {
        e.preventDefault();
        void undo();
      } else if (!typing && !document.querySelector('dialog[open]')) {
        const keys = useUi.getState().keys;
        if (matches(bindingFor(keys, 'sidebar'), e)) {
          e.preventDefault();
          const ui = useUi.getState();
          ui.setRailOpen(!ui.railOpen);
        } else if (matches(bindingFor(keys, 'search'), e)) {
          e.preventDefault();
          usePalette.getState().show();
        } else if (matches(bindingFor(keys, 'compose'), e)) {
          e.preventDefault();
          void compose();
        }
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  const onWelcome = useRouterState({ select: (s) => s.location.pathname.startsWith('/welcome') });
  if (onWelcome) {
    return (
      <Frame>
        <Outlet />
        <Toaster />
      </Frame>
    );
  }

  return (
    <Frame>
      <div
        className="grid h-full"
        style={{
          gridTemplateColumns: 'auto minmax(0, 1fr)',
          gridTemplateRows: 'minmax(0, 1fr)',
        }}
      >
        <Rail />
        <main className="min-h-0 min-w-0 bg-pane">
          <Outlet />
        </main>
        {syncing && (
          <div
            className="pointer-events-none fixed inset-x-0 top-0 z-40"
            role="status"
            aria-label="Syncing"
          >
            <Airmail moving />
          </div>
        )}
        <Composer />
        <Palette />
        <ProfileGate />
        <Toaster />
      </div>
    </Frame>
  );
}

function useNotifications(): void {
  const navigate = useNavigate();
  useEffect(
    () =>
      api.onEvent((event) => {
        if (event.type !== 'notification') return;
        const { notice } = event;
        const open =
          notice.draft_id !== null
            ? {
                label: 'Open',
                run: () => {
                  void api.openNotification(notice.id).catch(() => {});
                  openDraft(notice.draft_id as number);
                },
              }
            : notice.conversation_id !== null && notice.person_id !== null
              ? {
                  label: 'Open',
                  run: () => {
                    void api.openNotification(notice.id).catch(() => {});
                    void navigate({
                      to: '/people/$personId/c/$conversationId',
                      params: {
                        personId: String(notice.person_id),
                        conversationId: String(notice.conversation_id),
                      },
                    });
                  },
                }
              : undefined;
        useToasts
          .getState()
          .push(
            { text: `${notice.title}. ${notice.body.split('\n')[0]}`, tone: 'info', action: open },
            notice.urgent ? 0 : 10_000,
          );
      }),
    [navigate],
  );
}

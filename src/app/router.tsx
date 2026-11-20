import {
  createHashHistory,
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
} from '@tanstack/react-router';
import { lazy, Suspense } from 'react';
import { DraftsScreen } from '@/features/compose/DraftsScreen';
import { DesignSpecimen } from '@/features/design/DesignSpecimen';
import { Folio } from '@/features/folio/Folio';
import { HiddenScreen } from '@/features/hidden/HiddenScreen';
import { MailScreen } from '@/features/mail/MailScreen';
import { Welcome } from '@/features/onboarding/Welcome';
import { RoutinesScreen } from '@/features/routines/RoutinesScreen';
import { RulesScreen } from '@/features/rules/RulesScreen';
import { NewSendersScreen } from '@/features/senders/NewSendersScreen';
import { NotificationLog } from '@/features/settings/NotificationLog';
import { SettingsScreen } from '@/features/settings/SettingsScreen';
import { TodayScreen } from '@/features/today/TodayScreen';
import { WaitingScreen } from '@/features/waiting/WaitingScreen';
import { api } from '@/lib/api';
import { qk, queryClient } from '@/lib/queries';
import { Shell } from './Shell';

async function hasMailbox(): Promise<boolean> {
  try {
    const state = await queryClient.fetchQuery({
      queryKey: qk.appState,
      queryFn: () => api.appState(),
      staleTime: 0,
    });
    return state.accounts.length > 0;
  } catch {
    return true;
  }
}

const rootRoute = createRootRoute({
  component: Shell,
  beforeLoad: async ({ location }) => {
    const onWelcome = location.pathname.startsWith('/welcome');
    const dev = location.pathname.startsWith('/__');
    const linked = await hasMailbox();
    if (!linked && !onWelcome && !dev) throw redirect({ to: '/welcome' });
    if (linked && onWelcome) throw redirect({ to: '/people' });
  },
});

const welcomeRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'welcome',
  component: Welcome,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => {
    throw redirect({ to: '/people' });
  },
});

const peopleRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'people',
  component: Folio,
});
const personRoute = createRoute({ getParentRoute: () => peopleRoute, path: '$personId' });
const conversationRoute = createRoute({
  getParentRoute: () => personRoute,
  path: 'c/$conversationId',
});

const todayRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'today',
  component: TodayScreen,
});

const riverRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'river',
  component: MailScreen,
});

const newSendersRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'new-senders',
  component: NewSendersScreen,
});

const hiddenRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'hidden',
  component: HiddenScreen,
});

const draftsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'drafts',
  component: DraftsScreen,
});

const rulesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'rules',
  component: RulesScreen,
});

const notificationsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'notifications',
  component: NotificationLog,
});

const waitingRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'waiting',
  component: WaitingScreen,
});

const routinesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'routines',
  component: RoutinesScreen,
});

const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: 'settings',
  component: SettingsScreen,
});

const SpikesScreen = lazy(async () => {
  if (import.meta.env.DEV) {
    const module = await import('@/features/spikes/SpikesScreen');
    return { default: module.SpikesScreen };
  }
  return {
    default: () => <p className="p-8 text-ui text-text-2">Only available in development builds.</p>,
  };
});

const spikesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '__spikes',
  component: () => (
    <Suspense fallback={null}>
      <SpikesScreen />
    </Suspense>
  ),
});

const designRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '__design',
  component: DesignSpecimen,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  welcomeRoute,
  peopleRoute.addChildren([personRoute.addChildren([conversationRoute])]),
  todayRoute,
  riverRoute,
  newSendersRoute,
  hiddenRoute,
  draftsRoute,
  rulesRoute,
  notificationsRoute,
  waitingRoute,
  routinesRoute,
  settingsRoute,
  designRoute,
  spikesRoute,
]);

export const router = createRouter({
  routeTree,
  history: createHashHistory(),
  defaultPreload: 'intent',
});

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

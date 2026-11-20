import { keepPreviousData, QueryClient, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import type { Category, CategoryRef, CoreEvent, PeopleFilter } from '@/lib/bindings';
import { api } from './api';
import { useBackend } from './backend';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: Number.POSITIVE_INFINITY,
      gcTime: 5 * 60_000,
      retry: false,
      refetchOnWindowFocus: false,
    },
  },
});

export const qk = {
  appState: ['appState'] as const,
  people: (filter: PeopleFilter, query: string) => ['people', filter, query] as const,
  peopleIn: (category: string, filter: PeopleFilter, query: string) =>
    ['people', 'in', category, filter, query] as const,
  categoryMembers: (id: number) => ['categoryMembers', id] as const,
  person: (id: number) => ['person', id] as const,
  conversations: (personId: number) => ['conversations', personId] as const,
  conversation: (id: number, viewerPersonId?: number) =>
    ['conversation', id, viewerPersonId ?? null] as const,
  files: (personId: number) => ['files', personId] as const,
  syncStatus: ['syncStatus'] as const,
  newSenders: ['newSenders'] as const,
  rules: ['rules'] as const,
  watches: ['watches'] as const,
  today: ['today'] as const,
  inbox: (category: string | null) => ['inbox', category] as const,
  notifications: ['notifications'] as const,
  views: ['views'] as const,
  tags: ['tags'] as const,
  conversationTags: (id: number) => ['conversationTags', id] as const,
  view: (query: string) => ['view', query] as const,
  drafts: ['drafts'] as const,
  routines: ['routines'] as const,
  routineRuns: (id: number) => ['routineRuns', id] as const,
  routineVariables: ['routineVariables'] as const,
  nudges: ['nudges'] as const,
  composeSettings: ['composeSettings'] as const,
  recipients: (query: string) => ['recipients', query] as const,
  notificationSettings: ['notificationSettings'] as const,
  preview: (query: string) => ['preview', query] as const,
  hidden: ['hidden'] as const,
  groups: ['groups'] as const,
  domainRules: ['domainRules'] as const,
  personFlags: (id: number) => ['personFlags', id] as const,
  personGroups: (id: number) => ['personGroups', id] as const,
  googleSetup: ['googleSetup'] as const,
  search: (query: string) => ['search', query] as const,
};

export function useAppState() {
  return useQuery({ queryKey: qk.appState, queryFn: () => api.appState() });
}

export function usePeople(filter: PeopleFilter, query: string) {
  return useQuery({
    queryKey: qk.people(filter, query),
    queryFn: () => api.listPeople(filter, query),
    placeholderData: keepPreviousData,
  });
}

export function categoryRef(key: string): CategoryRef {
  if (key === 'favourites') return { type: 'favourites' };
  if (key.startsWith('group:')) return { type: 'group', id: Number(key.slice(6)) };
  return { type: 'everyone' };
}

export function usePeopleIn(category: string, filter: PeopleFilter, query: string) {
  return useQuery({
    queryKey: qk.peopleIn(category, filter, query),
    queryFn: () => api.listPeopleIn(categoryRef(category), filter, query),
    placeholderData: keepPreviousData,
  });
}

export function useCategoryMembers(id: number | null) {
  return useQuery({
    queryKey: qk.categoryMembers(id ?? 0),
    queryFn: () => api.listCategoryMembers(id as number),
    enabled: id !== null,
  });
}

export function useRules() {
  return useQuery({ queryKey: qk.rules, queryFn: () => api.listRules() });
}

export function useWatches() {
  return useQuery({ queryKey: qk.watches, queryFn: () => api.listWatches() });
}

export function useToday() {
  return useQuery({ queryKey: qk.today, queryFn: () => api.getToday() });
}

export function useInbox(category: Category | null, limit: number) {
  return useQuery({
    queryKey: [...qk.inbox(category), limit],
    queryFn: () => api.listInbox(category, limit, 0),
    placeholderData: keepPreviousData,
  });
}

export function useTags() {
  return useQuery({ queryKey: qk.tags, queryFn: () => api.listTags() });
}

export function useConversationTags(id: number | undefined) {
  return useQuery({
    queryKey: qk.conversationTags(id ?? -1),
    queryFn: () => api.getConversationTags(id as number),
    enabled: id !== undefined,
  });
}

export function useSavedViews() {
  return useQuery({ queryKey: qk.views, queryFn: () => api.listSavedViews() });
}

export function useView(query: string) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: qk.view(trimmed),
    queryFn: () => api.listView(trimmed),
    enabled: trimmed.length > 0,
    placeholderData: keepPreviousData,
  });
}

export function useDrafts() {
  return useQuery({ queryKey: qk.drafts, queryFn: () => api.listDrafts() });
}

export function useRoutines() {
  return useQuery({ queryKey: qk.routines, queryFn: () => api.listRoutines() });
}

export function useRoutineRuns(id: number | undefined) {
  return useQuery({
    queryKey: qk.routineRuns(id ?? -1),
    queryFn: () => api.listRoutineRuns(id as number),
    enabled: id !== undefined,
  });
}

export function useRoutineVariables() {
  return useQuery({ queryKey: qk.routineVariables, queryFn: () => api.listRoutineVariables() });
}

export function useNudges() {
  return useQuery({ queryKey: qk.nudges, queryFn: () => api.listNudges() });
}

export function useComposeSettings() {
  return useQuery({ queryKey: qk.composeSettings, queryFn: () => api.getComposeSettings() });
}

export function useRecipients(query: string) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: qk.recipients(trimmed),
    queryFn: () => api.suggestRecipients(trimmed),
    placeholderData: keepPreviousData,
  });
}

export function useNotifications() {
  return useQuery({ queryKey: qk.notifications, queryFn: () => api.listNotifications() });
}

export function useNotificationSettings() {
  return useQuery({
    queryKey: qk.notificationSettings,
    queryFn: () => api.getNotificationSettings(),
  });
}

export function usePreview(query: string) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: qk.preview(trimmed),
    queryFn: () => api.previewQuery(trimmed),
    enabled: trimmed.length > 0,
    placeholderData: keepPreviousData,
  });
}

export function useNewSenders() {
  return useQuery({ queryKey: qk.newSenders, queryFn: () => api.listNewSenders() });
}

export function useHidden() {
  return useQuery({ queryKey: qk.hidden, queryFn: () => api.listHidden() });
}

export function useGroups() {
  return useQuery({ queryKey: qk.groups, queryFn: () => api.listGroups() });
}

export function useDomainRules() {
  return useQuery({ queryKey: qk.domainRules, queryFn: () => api.listDomainRules() });
}

export function usePersonFlags(id: number | undefined) {
  return useQuery({
    queryKey: qk.personFlags(id ?? -1),
    queryFn: () => api.getPersonFlags(id as number),
    enabled: id !== undefined,
  });
}

export function usePersonGroups(id: number | undefined) {
  return useQuery({
    queryKey: qk.personGroups(id ?? -1),
    queryFn: () => api.getPersonGroups(id as number),
    enabled: id !== undefined,
  });
}

export function useSyncStatus() {
  return useQuery({ queryKey: qk.syncStatus, queryFn: () => api.syncStatus() });
}

export function useGoogleSetup() {
  return useQuery({ queryKey: qk.googleSetup, queryFn: () => api.googleSetup() });
}

export function usePerson(id: number | undefined) {
  return useQuery({
    queryKey: qk.person(id ?? -1),
    queryFn: () => api.getPerson(id as number),
    enabled: id !== undefined,
  });
}

export function useConversations(personId: number | undefined) {
  return useQuery({
    queryKey: qk.conversations(personId ?? -1),
    queryFn: () => api.listConversations(personId as number),
    enabled: personId !== undefined,
  });
}

export function useConversation(id: number | undefined, viewerPersonId?: number) {
  return useQuery({
    queryKey: qk.conversation(id ?? -1, viewerPersonId),
    queryFn: () => api.getConversation(id as number, viewerPersonId),
    enabled: id !== undefined,
  });
}

export function useFiles(personId: number | undefined) {
  return useQuery({
    queryKey: qk.files(personId ?? -1),
    queryFn: () => api.listFiles(personId as number),
    enabled: personId !== undefined,
  });
}

export function useSearch(query: string) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: qk.search(trimmed),
    queryFn: () => api.search(trimmed),
    enabled: trimmed.length > 0,
    placeholderData: keepPreviousData,
  });
}

export function rootsFor(entity: string): string[] {
  switch (entity) {
    case '*':
      return [];
    case 'people':
      return ['people', 'person', 'appState', 'search', 'newSenders', 'hidden', 'personFlags'];
    case 'conversations':
    case 'messages':
      return [
        'tags',
        'conversationTags',
        'view',
        'timeline',
        'conversation',
        'conversations',
        'people',
        'person',
        'files',
        'appState',
        'search',
        'newSenders',
        'hidden',
        'today',
        'inbox',
        'watches',
        'preview',
        'recipients',
        'nudges',
      ];
    case 'sorting':
      return [
        'view',
        'newSenders',
        'hidden',
        'appState',
        'domainRules',
        'personFlags',
        'today',
        'inbox',
      ];
    case 'rules':
      return ['rules', 'preview'];
    case 'watches':
      return ['watches', 'today'];
    case 'notifications':
      return ['notifications'];
    case 'drafts':
      return ['drafts'];
    case 'views':
      return ['views'];
    case 'tags':
      return ['tags', 'conversationTags', 'view'];
    case 'routines':
      return ['routines', 'routineRuns'];
    case 'nudges':
      return ['nudges'];
    case 'groups':
      return ['groups', 'personGroups', 'people', 'categoryMembers'];
    default:
      return [entity];
  }
}

export function applyCoreEvent(client: QueryClient, event: CoreEvent): void {
  useBackend.getState().apply(event);
  if (event.type === 'sync_progress' || event.type === 'link_state') {
    void client.invalidateQueries({ queryKey: ['syncStatus'] });
    void client.invalidateQueries({ queryKey: qk.appState });
    return;
  }
  if (event.type !== 'store_changed') return;
  const roots = rootsFor(event.entity);
  if (roots.length === 0) {
    void client.invalidateQueries();
    return;
  }
  for (const root of roots) void client.invalidateQueries({ queryKey: [root] });
}

export function useStoreEvents(): void {
  const client = useQueryClient();
  useEffect(() => api.onEvent((event) => applyCoreEvent(client, event)), [client]);
}

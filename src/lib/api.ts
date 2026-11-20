import {
  type AccountInfo,
  type Address,
  type AppState,
  type BulkAction,
  type Category,
  type CategoryInput,
  type CategoryMember,
  type CategoryRef,
  type ComposeSettings,
  type ConversationDetail,
  type ConversationRow,
  type ConversationSummary,
  type CoreEvent,
  commands,
  type DomainRule,
  type Done,
  type Draft,
  type DraftAttachment,
  type DraftInput,
  type DraftMode,
  events,
  type FileEntry,
  type GoogleSetup,
  type GroupInfo,
  type HiddenOverview,
  type ImportReport,
  type InboxPage,
  type MessageView,
  type NewSender,
  type NotificationRecord,
  type NotificationSettings,
  type Nudge,
  type PeopleFilter,
  type PersonDetail,
  type PersonEdit,
  type PersonFlags,
  type PersonSummary,
  type RecipientSuggestion,
  type Routine,
  type RoutineInput,
  type RoutineRun,
  type RoutineVariable,
  type Rule,
  type RuleInput,
  type RulePreview,
  type RunOutcome,
  type SavedView,
  type SavedWatch,
  type SearchResults,
  type SenderDecision,
  type SendTicket,
  type SyncStatus,
  type Tag,
  type TimelineEntry,
  type TodayView,
  type Unsubscribed,
  type WatchInput,
  type WatchView,
} from '@/lib/bindings';

export const api = {
  appState: (): Promise<AppState> => commands.appState(),
  startSecondDemo: (): Promise<AccountInfo> => commands.startSecondDemo(),
  startDemo: (stressPeople?: number): Promise<AccountInfo> =>
    commands.startDemo(stressPeople ?? null),

  listPeople: (filter: PeopleFilter, query: string): Promise<PersonSummary[]> =>
    commands.listPeople(filter, query),
  getPerson: (id: number): Promise<PersonDetail | null> => commands.getPerson(id),
  listConversations: (personId: number): Promise<ConversationSummary[]> =>
    commands.listConversations(personId),
  getConversation: (id: number, viewerPersonId?: number): Promise<ConversationDetail | null> =>
    commands.getConversation(id, viewerPersonId ?? null),
  listFiles: (personId: number): Promise<FileEntry[]> => commands.listFiles(personId),
  search: (query: string): Promise<SearchResults> => commands.search(query),

  markConversationRead: (id: number, read: boolean): Promise<null> =>
    commands.markConversationRead(id, read),
  setMessageStarred: (id: number, starred: boolean): Promise<null> =>
    commands.setMessageStarred(id, starred),
  setPersonPinned: (id: number, pinned: boolean): Promise<null> =>
    commands.setPersonPinned(id, pinned),
  setPersonNotes: (id: number, notes: string): Promise<null> => commands.setPersonNotes(id, notes),
  editPerson: (id: number, edit: PersonEdit): Promise<null> => commands.editPerson(id, edit),
  reply: (conversationId: number, text: string): Promise<MessageView> =>
    commands.reply(conversationId, text),

  archiveConversation: (id: number, archived: boolean): Promise<Done> =>
    commands.archiveConversation(id, archived),
  trashConversation: (id: number, trashed: boolean): Promise<Done> =>
    commands.trashConversation(id, trashed),
  hideConversation: (id: number, hidden: boolean, until?: number): Promise<Done> =>
    commands.hideConversation(id, hidden, until ?? null),
  hidePerson: (id: number, hidden: boolean, until?: number): Promise<Done> =>
    commands.hidePerson(id, hidden, until ?? null),
  decideSender: (id: number, decision: SenderDecision): Promise<Done> =>
    commands.decideSender(id, decision),
  decideDomain: (domain: string, decision: SenderDecision): Promise<Done> =>
    commands.decideDomain(domain, decision),
  getPersonFlags: (id: number): Promise<PersonFlags> => commands.getPersonFlags(id),
  setPersonFlags: (id: number, flags: PersonFlags): Promise<Done> =>
    commands.setPersonFlags(id, flags),
  undo: (): Promise<string | null> => commands.undo(),
  undoLabel: (): Promise<string | null> => commands.undoLabel(),
  listNewSenders: (): Promise<NewSender[]> => commands.listNewSenders(),
  listHidden: (): Promise<HiddenOverview> => commands.listHidden(),
  setDomainRule: (domain: string, action: 'allow' | 'hide' | 'block' | null): Promise<null> =>
    commands.setDomainRule(domain, action),
  listDomainRules: (): Promise<DomainRule[]> => commands.listDomainRules(),
  listGroups: (): Promise<GroupInfo[]> => commands.listGroups(),
  createGroup: (name: string, ink: number): Promise<number> => commands.createGroup(name, ink),
  deleteGroup: (id: number): Promise<null> => commands.deleteGroup(id),
  setPersonGroups: (personId: number, groupIds: number[]): Promise<null> =>
    commands.setPersonGroups(personId, groupIds),
  getPersonGroups: (personId: number): Promise<number[]> => commands.getPersonGroups(personId),

  listRules: (): Promise<Rule[]> => commands.listRules(),
  saveRule: (input: RuleInput): Promise<number> => commands.saveRule(input),
  deleteRule: (id: number): Promise<null> => commands.deleteRule(id),
  orderRules: (ids: number[]): Promise<null> => commands.orderRules(ids),
  applyRule: (id: number): Promise<Done> => commands.applyRule(id),
  previewQuery: (query: string): Promise<RulePreview> => commands.previewQuery(query),

  listWatches: (): Promise<WatchView[]> => commands.listWatches(),
  saveWatch: (input: WatchInput): Promise<SavedWatch> => commands.saveWatch(input),
  deleteWatch: (id: number): Promise<null> => commands.deleteWatch(id),
  setWatchActive: (id: number, active: boolean): Promise<null> =>
    commands.setWatchActive(id, active),
  markConversationSeen: (id: number): Promise<null> => commands.markConversationSeen(id),

  getToday: (): Promise<TodayView> => commands.getToday(),
  listInbox: (category: Category | null, limit: number, offset: number): Promise<InboxPage> =>
    commands.listInbox(category, limit, offset),
  snoozeConversation: (id: number, until: number | null): Promise<Done> =>
    commands.snoozeConversation(id, until),

  getNotificationSettings: (): Promise<NotificationSettings> => commands.getNotificationSettings(),
  saveNotificationSettings: (settings: NotificationSettings): Promise<null> =>
    commands.saveNotificationSettings(settings),
  listNotifications: (): Promise<NotificationRecord[]> => commands.listNotifications(),
  openNotification: (id: number): Promise<number | null> => commands.openNotification(id),
  prepareAttachment: (id: number): Promise<null> => commands.prepareAttachment(id),
  saveAttachment: (id: number): Promise<string> => commands.saveAttachment(id),

  getComposeSettings: (): Promise<ComposeSettings> => commands.getComposeSettings(),
  saveComposeSettings: (settings: ComposeSettings): Promise<null> =>
    commands.saveComposeSettings(settings),
  newDraft: (to: Address[] = []): Promise<Draft> => commands.newDraft(to),
  replyDraft: (conversationId: number, mode: DraftMode): Promise<Draft> =>
    commands.replyDraft(conversationId, mode),
  saveDraft: (input: DraftInput): Promise<number> => commands.saveDraft(input),
  getDraft: (id: number): Promise<Draft | null> => commands.getDraft(id),
  listDrafts: (): Promise<Draft[]> => commands.listDrafts(),
  deleteDraft: (id: number): Promise<null> => commands.deleteDraft(id),
  addDraftFile: (
    draftId: number,
    filename: string,
    mime: string,
    data: string,
  ): Promise<DraftAttachment> => commands.addDraftFile(draftId, filename, mime, data),
  removeDraftFile: (id: number): Promise<null> => commands.removeDraftFile(id),
  sendDraft: (id: number): Promise<SendTicket> => commands.sendDraft(id),
  unsend: (messageId: number): Promise<Draft> => commands.unsend(messageId),
  scheduleDraft: (id: number, sendAt: number | null): Promise<null> =>
    commands.scheduleDraft(id, sendAt),
  suggestRecipients: (query: string): Promise<RecipientSuggestion[]> =>
    commands.suggestRecipients(query),
  listNudges: (): Promise<Nudge[]> => commands.listNudges(),
  cancelNudge: (id: number): Promise<null> => commands.cancelNudge(id),

  getMirrorTags: (): Promise<boolean> => commands.getMirrorTags(),
  setMirrorTags: (on: boolean): Promise<null> => commands.setMirrorTags(on),
  setActiveAccount: (account: number | null): Promise<null> => commands.setActiveAccount(account),
  listPeopleIn: (
    category: CategoryRef,
    filter: PeopleFilter,
    query: string,
  ): Promise<PersonSummary[]> => commands.listPeopleIn(category, filter, query),
  saveCategory: (input: CategoryInput): Promise<number> => commands.saveCategory(input),
  listCategoryMembers: (id: number): Promise<CategoryMember[]> => commands.listCategoryMembers(id),
  addToCategory: (
    group: number,
    target: { person?: number; email?: string; name?: string },
  ): Promise<number> =>
    commands.addToCategory(group, target.person ?? null, target.email ?? null, target.name ?? null),
  setMemberAlias: (group: number, person: number, alias: string): Promise<null> =>
    commands.setMemberAlias(group, person, alias),
  removeFromCategory: (group: number, person: number): Promise<null> =>
    commands.removeFromCategory(group, person),
  orderCategories: (ids: number[]): Promise<null> => commands.orderCategories(ids),
  mergePeople: (keep: number, absorb: number): Promise<Done> => commands.mergePeople(keep, absorb),
  splitPerson: (person: number, address: string): Promise<Done> =>
    commands.splitPerson(person, address),
  importContacts: (text: string): Promise<ImportReport> => commands.importContacts(text),
  listTags: (): Promise<Tag[]> => commands.listTags(),
  getConversationTags: (id: number): Promise<Tag[]> => commands.getConversationTags(id),
  tagConversation: (id: number, name: string): Promise<Done> => commands.tagConversation(id, name),
  untagConversation: (id: number, labelId: number): Promise<Done> =>
    commands.untagConversation(id, labelId),
  listView: (query: string): Promise<ConversationRow[]> => commands.listView(query),
  listSavedViews: (): Promise<SavedView[]> => commands.listSavedViews(),
  saveView: (name: string, query: string): Promise<number> => commands.saveView(name, query),
  deleteView: (id: number): Promise<null> => commands.deleteView(id),
  bulkConversations: (ids: number[], action: BulkAction): Promise<Done> =>
    commands.bulkConversations(ids, action),
  getTimeline: (personId: number, before?: number): Promise<TimelineEntry[]> =>
    commands.getTimeline(personId, before ?? null),
  markAllRead: (personId?: number): Promise<Done> => commands.markAllRead(personId ?? null),
  unsubscribe: (messageId: number): Promise<Unsubscribed> => commands.unsubscribe(messageId),

  listRoutineVariables: (): Promise<RoutineVariable[]> => commands.listRoutineVariables(),
  previewRoutineText: (text: string): Promise<string> => commands.previewRoutineText(text),
  listRoutines: (): Promise<Routine[]> => commands.listRoutines(),
  saveRoutine: (input: RoutineInput): Promise<number> => commands.saveRoutine(input),
  deleteRoutine: (id: number): Promise<null> => commands.deleteRoutine(id),
  skipRoutine: (id: number, skip: boolean): Promise<null> => commands.skipRoutine(id, skip),
  listRoutineRuns: (id: number): Promise<RoutineRun[]> => commands.listRoutineRuns(id),
  runRoutine: (id: number): Promise<RunOutcome> => commands.runRoutine(id),

  googleSetup: (): Promise<GoogleSetup> => commands.googleSetup(),
  saveGoogleClient: (clientId: string, clientSecret: string): Promise<null> =>
    commands.saveGoogleClient(clientId, clientSecret),
  beginContactsImport: (): Promise<string> => commands.beginContactsImport(),
  beginGmailLink: (): Promise<string> => commands.beginGmailLink(),
  cancelGmailLink: (): Promise<null> => commands.cancelGmailLink(),
  unlinkAccount: (id: number): Promise<null> => commands.unlinkAccount(id),
  syncStatus: (): Promise<SyncStatus[]> => commands.syncStatus(),
  syncNow: (): Promise<null> => commands.syncNow(),
  openLink: (url: string): Promise<null> => commands.openLink(url),

  onEvent(callback: (event: CoreEvent) => void): () => void {
    let stopped = false;
    let unlisten: (() => void) | undefined;
    void events.zsmEvent
      .listen((e) => callback(e.payload))
      .then((off) => {
        if (stopped) off();
        else unlisten = off;
      });
    return () => {
      stopped = true;
      unlisten?.();
    };
  },
};

export type Api = typeof api;

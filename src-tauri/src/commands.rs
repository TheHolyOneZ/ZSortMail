use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::State;
use zsm_app::{
    AppState, BulkAction, Core, DomainRule, Done, GoogleSetup, InboxPage, NotificationSettings, SavedWatch, SyncStatus,
    TodayView, Unsubscribed,
};
use zsm_core::dto::Category;
use zsm_core::dto::{
    AccountInfo, ConversationDetail, ConversationSummary, FileEntry, GroupInfo, HiddenOverview, MessageView, NewSender,
    PeopleFilter, PersonDetail, PersonFlags, PersonSummary, SearchResults, SenderDecision,
};
use zsm_core::store::actions::PersonEdit;
use zsm_core::store::categories::{CategoryInput, CategoryMember, CategoryRef};
use zsm_core::store::contacts::ImportReport;
use zsm_core::store::drafts::{
    ComposeSettings, Draft, DraftAttachment, DraftInput, DraftMode, Nudge, RecipientSuggestion, SendTicket,
};
use zsm_core::store::ingest::Address;
use zsm_core::store::notifications::NotificationRecord;
use zsm_core::store::routines::{Routine, RoutineInput, RoutineRun, RunOutcome};
use zsm_core::store::rules::{Rule, RuleInput, RulePreview};
use zsm_core::store::tags::Tag;
use zsm_core::store::timeline::TimelineEntry;
use zsm_core::store::today::{ConversationRow, SavedView};
use zsm_core::store::watches::{WatchInput, WatchView};

use crate::error::AppError;

type Shared<'a> = State<'a, Arc<Core>>;
type Reply<T> = Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Pong {
    pub sent_at_us: i64,
    pub received_at_us: i64,
}

fn now_us() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX)).unwrap_or(0)
}

#[tauri::command]
#[specta::specta]
pub async fn app_state(core: Shared<'_>) -> Reply<AppState> {
    Ok(core.app_state()?)
}

#[tauri::command]
#[specta::specta]
pub async fn start_second_demo(core: Shared<'_>) -> Reply<AccountInfo> {
    Ok(core.start_second_demo()?)
}

#[tauri::command]
#[specta::specta]
pub async fn start_demo(core: Shared<'_>, stress_people: Option<u32>) -> Reply<AccountInfo> {
    let extra = stress_people.map_or(0, |n| n.min(100_000) as usize);
    Ok(core.start_demo(extra)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_people(core: Shared<'_>, filter: PeopleFilter, query: String) -> Reply<Vec<PersonSummary>> {
    Ok(core.people(filter, &query)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_person(core: Shared<'_>, id: i64) -> Reply<Option<PersonDetail>> {
    Ok(core.person(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_conversations(core: Shared<'_>, person_id: i64) -> Reply<Vec<ConversationSummary>> {
    Ok(core.conversations(person_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_conversation(
    core: Shared<'_>,
    id: i64,
    viewer_person_id: Option<i64>,
) -> Reply<Option<ConversationDetail>> {
    Ok(core.conversation(id, viewer_person_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_files(core: Shared<'_>, person_id: i64) -> Reply<Vec<FileEntry>> {
    Ok(core.files(person_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn search(core: Shared<'_>, query: String) -> Reply<SearchResults> {
    Ok(core.search(&query)?)
}

#[tauri::command]
#[specta::specta]
pub async fn mark_conversation_read(core: Shared<'_>, id: i64, read: bool) -> Reply<()> {
    Ok(core.set_conversation_read(id, read)?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_message_starred(core: Shared<'_>, id: i64, starred: bool) -> Reply<()> {
    Ok(core.set_message_starred(id, starred)?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_person_pinned(core: Shared<'_>, id: i64, pinned: bool) -> Reply<()> {
    Ok(core.set_person_pinned(id, pinned)?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_person_notes(core: Shared<'_>, id: i64, notes: String) -> Reply<()> {
    Ok(core.set_person_notes(id, &notes)?)
}

#[tauri::command]
#[specta::specta]
pub async fn edit_person(core: Shared<'_>, id: i64, edit: PersonEdit) -> Reply<()> {
    Ok(core.edit_person(id, &edit)?)
}

#[tauri::command]
#[specta::specta]
pub async fn reply(core: Shared<'_>, conversation_id: i64, text: String) -> Reply<MessageView> {
    Ok(core.reply(conversation_id, &text)?)
}

#[tauri::command]
#[specta::specta]
pub async fn archive_conversation(core: Shared<'_>, id: i64, archived: bool) -> Reply<Done> {
    Ok(core.archive_conversation(id, archived)?)
}

#[tauri::command]
#[specta::specta]
pub async fn trash_conversation(core: Shared<'_>, id: i64, trashed: bool) -> Reply<Done> {
    Ok(core.trash_conversation(id, trashed)?)
}

#[tauri::command]
#[specta::specta]
pub async fn hide_conversation(core: Shared<'_>, id: i64, hidden: bool, until: Option<i64>) -> Reply<Done> {
    Ok(core.hide_conversation(id, hidden, until)?)
}

#[tauri::command]
#[specta::specta]
pub async fn hide_person(core: Shared<'_>, id: i64, hidden: bool, until: Option<i64>) -> Reply<Done> {
    Ok(core.hide_person(id, hidden, until)?)
}

#[tauri::command]
#[specta::specta]
pub async fn decide_sender(core: Shared<'_>, id: i64, decision: SenderDecision) -> Reply<Done> {
    Ok(core.decide_sender(id, decision)?)
}

#[tauri::command]
#[specta::specta]
pub async fn decide_domain(core: Shared<'_>, domain: String, decision: SenderDecision) -> Reply<Done> {
    Ok(core.decide_domain(&domain, decision)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_person_flags(core: Shared<'_>, id: i64) -> Reply<PersonFlags> {
    Ok(core.person_flags(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_person_flags(core: Shared<'_>, id: i64, flags: PersonFlags) -> Reply<Done> {
    Ok(core.set_person_flags(id, &flags)?)
}

#[tauri::command]
#[specta::specta]
pub async fn undo(core: Shared<'_>) -> Reply<Option<String>> {
    Ok(core.undo()?)
}

#[tauri::command]
#[specta::specta]
pub async fn undo_label(core: Shared<'_>) -> Reply<Option<String>> {
    Ok(core.undo_label())
}

#[tauri::command]
#[specta::specta]
pub async fn list_new_senders(core: Shared<'_>) -> Reply<Vec<NewSender>> {
    Ok(core.new_senders()?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_hidden(core: Shared<'_>) -> Reply<HiddenOverview> {
    Ok(core.hidden()?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_domain_rule(core: Shared<'_>, domain: String, action: Option<String>) -> Reply<()> {
    Ok(core.set_domain_rule(&domain, action.as_deref())?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_domain_rules(core: Shared<'_>) -> Reply<Vec<DomainRule>> {
    Ok(core.domain_rules()?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_groups(core: Shared<'_>) -> Reply<Vec<GroupInfo>> {
    Ok(core.groups()?)
}

#[tauri::command]
#[specta::specta]
pub async fn create_group(core: Shared<'_>, name: String, ink: u8) -> Reply<i64> {
    Ok(core.create_group(&name, ink)?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_group(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.delete_group(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_person_groups(core: Shared<'_>, person_id: i64, group_ids: Vec<i64>) -> Reply<()> {
    Ok(core.set_person_groups(person_id, &group_ids)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_person_groups(core: Shared<'_>, person_id: i64) -> Reply<Vec<i64>> {
    Ok(core.groups_of(person_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_rules(core: Shared<'_>) -> Reply<Vec<Rule>> {
    Ok(core.rules()?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_rule(core: Shared<'_>, input: RuleInput) -> Reply<i64> {
    Ok(core.save_rule(&input)?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_rule(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.delete_rule(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn order_rules(core: Shared<'_>, ids: Vec<i64>) -> Reply<()> {
    Ok(core.order_rules(&ids)?)
}

#[tauri::command]
#[specta::specta]
pub async fn apply_rule(core: Shared<'_>, id: i64) -> Reply<Done> {
    Ok(core.apply_rule(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn preview_query(core: Shared<'_>, query: String) -> Reply<RulePreview> {
    Ok(core.preview_query(&query)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_watches(core: Shared<'_>) -> Reply<Vec<WatchView>> {
    Ok(core.watches()?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_watch(core: Shared<'_>, input: WatchInput) -> Reply<SavedWatch> {
    Ok(core.save_watch(&input)?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_watch(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.delete_watch(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_watch_active(core: Shared<'_>, id: i64, active: bool) -> Reply<()> {
    Ok(core.set_watch_active(id, active)?)
}

#[tauri::command]
#[specta::specta]
pub async fn mark_conversation_seen(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.mark_conversation_seen(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_today(core: Shared<'_>) -> Reply<TodayView> {
    Ok(core.today()?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_inbox(core: Shared<'_>, category: Option<Category>, limit: u32, offset: u32) -> Reply<InboxPage> {
    Ok(core.inbox(category, limit, offset)?)
}

#[tauri::command]
#[specta::specta]
pub async fn snooze_conversation(core: Shared<'_>, id: i64, until: Option<i64>) -> Reply<Done> {
    Ok(core.snooze_conversation(id, until)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_notification_settings(core: Shared<'_>) -> Reply<NotificationSettings> {
    Ok(core.notification_settings()?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_notification_settings(core: Shared<'_>, settings: NotificationSettings) -> Reply<()> {
    Ok(core.save_notification_settings(&settings)?)
}

#[tauri::command]
#[specta::specta]
pub async fn send_test_notification(core: Shared<'_>) -> Reply<()> {
    Ok(core.send_test_notification()?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_notifications(core: Shared<'_>) -> Reply<Vec<NotificationRecord>> {
    Ok(core.notification_log()?)
}

#[tauri::command]
#[specta::specta]
pub async fn open_notification(core: Shared<'_>, id: i64) -> Reply<Option<i64>> {
    Ok(core.open_notification(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_compose_settings(core: Shared<'_>) -> Reply<ComposeSettings> {
    Ok(core.compose_settings()?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_compose_settings(core: Shared<'_>, settings: ComposeSettings) -> Reply<()> {
    Ok(core.save_compose_settings(&settings)?)
}

#[tauri::command]
#[specta::specta]
pub async fn new_draft(core: Shared<'_>, to: Vec<Address>) -> Reply<Draft> {
    Ok(core.new_draft(to)?)
}

#[tauri::command]
#[specta::specta]
pub async fn reply_draft(core: Shared<'_>, conversation_id: i64, mode: DraftMode) -> Reply<Draft> {
    Ok(core.reply_draft(conversation_id, mode)?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_draft(core: Shared<'_>, input: DraftInput) -> Reply<i64> {
    Ok(core.save_draft(&input)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_draft(core: Shared<'_>, id: i64) -> Reply<Option<Draft>> {
    Ok(core.draft(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_drafts(core: Shared<'_>) -> Reply<Vec<Draft>> {
    Ok(core.drafts()?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_draft(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.delete_draft(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn add_draft_file(
    core: Shared<'_>,
    draft_id: i64,
    filename: String,
    mime: String,
    data: String,
) -> Reply<DraftAttachment> {
    Ok(core.add_draft_file_base64(draft_id, &filename, &mime, &data)?)
}

#[tauri::command]
#[specta::specta]
pub async fn remove_draft_file(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.remove_draft_file(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn send_draft(core: Shared<'_>, id: i64) -> Reply<SendTicket> {
    Ok(core.send_draft(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn unsend(core: Shared<'_>, message_id: i64) -> Reply<Draft> {
    Ok(core.unsend(message_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn schedule_draft(core: Shared<'_>, id: i64, send_at: Option<i64>) -> Reply<()> {
    Ok(core.schedule_draft(id, send_at)?)
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_recipients(core: Shared<'_>, query: String) -> Reply<Vec<RecipientSuggestion>> {
    Ok(core.suggest_recipients(&query)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_nudges(core: Shared<'_>) -> Reply<Vec<Nudge>> {
    Ok(core.nudges()?)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_nudge(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.cancel_nudge(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_timeline(core: Shared<'_>, person_id: i64, before: Option<i64>) -> Reply<Vec<TimelineEntry>> {
    Ok(core.timeline(person_id, before)?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_mirror_tags(core: Shared<'_>) -> Reply<bool> {
    Ok(core.mirror_tags()?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_mirror_tags(core: Shared<'_>, on: bool) -> Reply<()> {
    Ok(core.set_mirror_tags(on)?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_active_account(core: Shared<'_>, account: Option<i64>) -> Reply<()> {
    Ok(core.set_active_account(account)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_people_in(
    core: Shared<'_>,
    category: CategoryRef,
    filter: PeopleFilter,
    query: String,
) -> Reply<Vec<PersonSummary>> {
    Ok(core.people_in(category, filter, &query)?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_category(core: Shared<'_>, input: CategoryInput) -> Reply<i64> {
    Ok(core.save_category(&input)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_category_members(core: Shared<'_>, id: i64) -> Reply<Vec<CategoryMember>> {
    Ok(core.category_members(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn add_to_category(
    core: Shared<'_>,
    group: i64,
    person: Option<i64>,
    email: Option<String>,
    name: Option<String>,
) -> Reply<i64> {
    Ok(core.add_to_category(group, person, email.as_deref(), name.as_deref())?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_member_alias(core: Shared<'_>, group: i64, person: i64, alias: String) -> Reply<()> {
    Ok(core.set_member_alias(group, person, &alias)?)
}

#[tauri::command]
#[specta::specta]
pub async fn remove_from_category(core: Shared<'_>, group: i64, person: i64) -> Reply<()> {
    Ok(core.remove_from_category(group, person)?)
}

#[tauri::command]
#[specta::specta]
pub async fn order_categories(core: Shared<'_>, ids: Vec<i64>) -> Reply<()> {
    Ok(core.order_categories(&ids)?)
}

#[tauri::command]
#[specta::specta]
pub async fn merge_people(core: Shared<'_>, keep: i64, absorb: i64) -> Reply<Done> {
    Ok(core.merge_people(keep, absorb)?)
}

#[tauri::command]
#[specta::specta]
pub async fn split_person(core: Shared<'_>, person: i64, address: String) -> Reply<Done> {
    Ok(core.split_person(person, &address)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_tags(core: Shared<'_>) -> Reply<Vec<Tag>> {
    Ok(core.tags()?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_conversation_tags(core: Shared<'_>, id: i64) -> Reply<Vec<Tag>> {
    Ok(core.conversation_tags(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn tag_conversation(core: Shared<'_>, id: i64, name: String) -> Reply<Done> {
    Ok(core.tag_conversation(id, &name)?)
}

#[tauri::command]
#[specta::specta]
pub async fn untag_conversation(core: Shared<'_>, id: i64, label_id: i64) -> Reply<Done> {
    Ok(core.untag_conversation(id, label_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn import_contacts(core: Shared<'_>, text: String) -> Reply<ImportReport> {
    Ok(core.import_contacts(&text)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_view(core: Shared<'_>, query: String) -> Reply<Vec<ConversationRow>> {
    Ok(core.view_rows(&query)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_saved_views(core: Shared<'_>) -> Reply<Vec<SavedView>> {
    Ok(core.saved_views()?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_view(core: Shared<'_>, name: String, query: String) -> Reply<i64> {
    Ok(core.save_view(&name, &query)?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_view(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.delete_view(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn bulk_conversations(core: Shared<'_>, ids: Vec<i64>, action: BulkAction) -> Reply<Done> {
    Ok(core.bulk(&ids, action)?)
}

#[tauri::command]
#[specta::specta]
pub async fn mark_all_read(core: Shared<'_>, person_id: Option<i64>) -> Reply<Done> {
    Ok(core.mark_all_read(person_id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn unsubscribe(core: Shared<'_>, message_id: i64) -> Reply<Unsubscribed> {
    Ok(core.unsubscribe(message_id).await?)
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct RoutineVariable {
    pub name: String,
    pub meaning: String,
}

#[tauri::command]
#[specta::specta]
pub async fn list_routine_variables() -> Vec<RoutineVariable> {
    zsm_core::store::routines::VARIABLES
        .iter()
        .map(|(name, meaning)| RoutineVariable { name: (*name).into(), meaning: (*meaning).into() })
        .collect()
}

#[tauri::command]
#[specta::specta]
pub async fn preview_routine_text(core: Shared<'_>, text: String) -> Reply<String> {
    Ok(core.preview_routine_text(&text))
}

#[tauri::command]
#[specta::specta]
pub async fn list_routines(core: Shared<'_>) -> Reply<Vec<Routine>> {
    Ok(core.routines()?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_routine(core: Shared<'_>, input: RoutineInput) -> Reply<i64> {
    Ok(core.save_routine(&input)?)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_routine(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.delete_routine(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn skip_routine(core: Shared<'_>, id: i64, skip: bool) -> Reply<()> {
    Ok(core.skip_routine(id, skip)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_routine_runs(core: Shared<'_>, id: i64) -> Reply<Vec<RoutineRun>> {
    Ok(core.routine_runs(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn run_routine(core: Shared<'_>, id: i64) -> Reply<RunOutcome> {
    Ok(core.run_routine(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn google_setup(core: Shared<'_>) -> Reply<GoogleSetup> {
    Ok(core.google_setup()?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_google_client(core: Shared<'_>, client_id: String, client_secret: String) -> Reply<()> {
    Ok(core.save_google_client(&client_id, &client_secret)?)
}

#[tauri::command]
#[specta::specta]
pub async fn begin_contacts_import(core: Shared<'_>) -> Reply<String> {
    Ok(core.begin_contacts_import().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn begin_gmail_link(core: Shared<'_>) -> Reply<String> {
    Ok(core.begin_gmail_link().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_gmail_link(core: Shared<'_>) -> Reply<()> {
    core.cancel_gmail_link();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn unlink_account(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.unlink_account(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn sync_status(core: Shared<'_>) -> Reply<Vec<SyncStatus>> {
    Ok(core.sync_status())
}

#[tauri::command]
#[specta::specta]
pub async fn sync_now(core: Shared<'_>) -> Reply<()> {
    core.wake_sync();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_attachment(core: Shared<'_>, id: i64) -> Reply<()> {
    Ok(core.prepare_attachment(id).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn save_attachment(core: Shared<'_>, id: i64) -> Reply<String> {
    Ok(core.save_attachment(id)?)
}

#[tauri::command]
#[specta::specta]
pub async fn open_link(url: String) -> Reply<()> {
    let allowed = ["https://", "http://", "mailto:"];
    let lower = url.trim().to_ascii_lowercase();
    if !allowed.iter().any(|scheme| lower.starts_with(scheme)) {
        return Err(AppError::Invalid("That kind of link cannot be opened from a message".into()));
    }
    open_externally(url.trim()).map_err(|err| AppError::Internal(err.to_string()))
}

#[cfg(not(test))]
fn open_externally(url: &str) -> Result<(), tauri_plugin_opener::Error> {
    tauri_plugin_opener::open_url(url, None::<&str>)
}

#[cfg(test)]
fn open_externally(_url: &str) -> Result<(), tauri_plugin_opener::Error> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn ping(sent_at_us: i64) -> Pong {
    Pong { sent_at_us, received_at_us: now_us() }
}

#[tauri::command]
#[specta::specta]
pub async fn debug_emit(core: Shared<'_>, entity: String) -> Reply<()> {
    core.debug_emit(&entity);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn only_web_and_mail_links_may_be_opened() {
        for bad in
            ["file:///etc/passwd", "javascript:alert(1)", "ftp://x", "  data:text/html,hi", "zsm://localhost/msg/1", ""]
        {
            let err = open_link(bad.into()).await.unwrap_err();
            assert!(matches!(err, AppError::Invalid(_)), "{bad}");
        }
        for good in ["https://example.com", "HTTP://example.com/a?b=c", "mailto:someone@example.com", "  https://x.y "]
        {
            open_link(good.into()).await.unwrap();
        }
    }
}

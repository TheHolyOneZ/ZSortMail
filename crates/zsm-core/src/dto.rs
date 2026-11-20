use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PersonKind {
    Person,
    Service,
    List,
    #[serde(rename = "self")]
    Myself,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PersonStatus {
    Known,
    NewSender,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ConversationStatus {
    None,
    NeedsReply,
    WaitingOnThem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Person,
    Newsletter,
    Receipt,
    Automated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PeopleFilter {
    #[default]
    All,
    NeedsReply,
    WaitingOnThem,
    HasFiles,
    Pinned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PersonSummary {
    pub id: i64,
    pub name: String,
    pub kind: PersonKind,
    pub status: PersonStatus,
    pub ink: u8,
    pub title: Option<String>,
    pub last_subject: String,
    pub last_at: i64,
    pub unread: u32,
    pub pinned: bool,
    pub needs_reply: bool,
    pub waiting_on_them: bool,
    pub has_files: bool,
    pub photo_url: Option<String>,
    pub vip: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PersonDetail {
    pub summary: PersonSummary,
    pub description: Option<String>,
    pub organization: Option<String>,
    pub notes: Option<String>,
    pub whitelisted: bool,
    pub addresses: Vec<String>,
    pub conversation_count: u32,
    pub file_count: u32,
    pub first_contact_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ParticipantRef {
    pub person_id: Option<i64>,
    pub name: String,
    pub ink: u8,
    pub kind: PersonKind,
    pub is_me: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ConversationSummary {
    pub id: i64,
    pub subject: String,
    pub snippet: String,
    pub first_at: i64,
    pub last_at: i64,
    pub message_count: u32,
    pub unread: u32,
    pub has_files: bool,
    pub status: ConversationStatus,
    pub category: Category,
    pub others: Vec<ParticipantRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AttachmentView {
    pub id: i64,
    pub name: String,
    pub mime: String,
    pub size: u32,
    pub inline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UnsubscribeInfo {
    pub https: Option<String>,
    pub mailto: Option<String>,
    pub one_click: bool,
    pub done_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct MailAddress {
    pub email: String,
    pub name: Option<String>,
    pub person_id: Option<i64>,
    pub is_me: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct MessageView {
    pub id: i64,
    pub from: ParticipantRef,
    pub sent_at: i64,
    pub to_line: String,
    pub from_email: String,
    pub to: Vec<MailAddress>,
    pub cc: Vec<MailAddress>,
    pub preview: String,
    pub text: String,
    pub quoted: Option<String>,
    pub attachments: Vec<AttachmentView>,
    pub unread: bool,
    pub starred: bool,
    pub html_url: Option<String>,
    pub remote_images: u32,
    pub trackers_blocked: u32,
    pub unsubscribe: Option<UnsubscribeInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ConversationDetail {
    pub summary: ConversationSummary,
    pub participants: Vec<ParticipantRef>,
    pub messages: Vec<MessageView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct FileEntry {
    pub attachment: AttachmentView,
    pub message_id: i64,
    pub conversation_id: i64,
    pub conversation_subject: String,
    pub sent_at: i64,
    pub from: ParticipantRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct AccountInfo {
    pub id: i64,
    pub provider: String,
    pub email: String,
    pub display_name: Option<String>,
    pub last_sync_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchHit {
    pub message_id: i64,
    pub conversation_id: i64,
    pub person_id: Option<i64>,
    pub subject: String,
    pub snippet: String,
    pub from: ParticipantRef,
    pub sent_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchResults {
    pub people: Vec<PersonSummary>,
    pub messages: Vec<SearchHit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NewSender {
    pub person: PersonSummary,
    pub address: String,
    pub domain: String,
    pub subject: String,
    pub snippet: String,
    pub message_count: u32,
    pub file_count: u32,
    pub first_at: i64,
    pub bulk: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum SenderDecision {
    Allow,
    Hide,
    Block,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HiddenPerson {
    pub person: PersonSummary,
    pub address: String,
    pub blocked: bool,
    pub until: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HiddenConversation {
    pub conversation: ConversationSummary,
    pub person_id: Option<i64>,
    pub person_name: String,
    pub until: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HiddenOverview {
    pub people: Vec<HiddenPerson>,
    pub conversations: Vec<HiddenConversation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct GroupInfo {
    pub id: i64,
    pub name: String,
    pub ink: u8,
    pub members: u32,
    pub icon: String,
    pub description: String,
    pub image: Option<String>,
    pub sidebar: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PersonFlags {
    pub whitelisted: bool,
    pub vip: bool,
    pub muted: bool,
}

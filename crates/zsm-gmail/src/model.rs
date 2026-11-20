use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub email_address: String,
    #[serde(default)]
    pub messages_total: u64,
    #[serde(default)]
    pub threads_total: u64,
    pub history_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    pub id: String,
    #[serde(default)]
    pub thread_id: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageList {
    #[serde(default)]
    pub messages: Vec<MessageRef>,
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub result_size_estimate: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    pub attachment_id: Option<String>,
    #[serde(default)]
    pub size: u64,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(default)]
    pub part_id: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub headers: Vec<Header>,
    #[serde(default)]
    pub body: Body,
    #[serde(default)]
    pub parts: Vec<Part>,
}

impl Part {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)).map(|h| h.value.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessage {
    pub id: String,
    #[serde(default)]
    pub thread_id: String,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub snippet: String,
    pub history_id: Option<String>,
    pub internal_date: Option<String>,
    #[serde(default)]
    pub size_estimate: u64,
    pub payload: Option<Part>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    #[serde(default)]
    pub history: Vec<HistoryRecord>,
    pub next_page_token: Option<String>,
    pub history_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    #[serde(default)]
    pub messages_added: Vec<HistoryMessage>,
    #[serde(default)]
    pub messages_deleted: Vec<HistoryMessage>,
    #[serde(default)]
    pub labels_added: Vec<HistoryLabels>,
    #[serde(default)]
    pub labels_removed: Vec<HistoryLabels>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HistoryMessage {
    pub message: MessageRef,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryLabels {
    pub message: MessageRef,
    #[serde(default)]
    pub label_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Label {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct LabelList {
    #[serde(default)]
    pub labels: Vec<Label>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SentMessage {
    pub id: String,
    #[serde(default)]
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AttachmentBody {
    pub data: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyRequest<'a> {
    pub add_label_ids: &'a [String],
    pub remove_label_ids: &'a [String],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequest<'a> {
    pub raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<&'a str>,
}

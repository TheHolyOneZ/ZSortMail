use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Default)]
pub struct MessageSpec {
    pub from: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub date_ms: i64,
    pub labels: Vec<String>,
    pub thread: Option<String>,
    pub attachments: Vec<AttachmentSpec>,
    pub headers: Vec<(String, String)>,
    pub text_bytes: Option<(String, Vec<u8>)>,
    pub in_reply_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AttachmentSpec {
    pub filename: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Failure {
    pub status: u16,
    pub reason: String,
    pub path_contains: Option<String>,
    pub retry_after: Option<u32>,
}

#[derive(Clone)]
struct Stored {
    id: String,
    thread_id: String,
    labels: Vec<String>,
    date_ms: i64,
    snippet: String,
    headers: Vec<(String, String)>,
    text: Option<(Option<String>, Vec<u8>)>,
    html: Option<String>,
    attachments: Vec<(String, AttachmentSpec)>,
}

#[derive(Clone)]
enum HistoryKind {
    Added(String),
    Deleted(String),
    LabelsAdded(String, Vec<String>),
    LabelsRemoved(String, Vec<String>),
}

struct State_ {
    email: String,
    messages: Vec<Stored>,
    history: Vec<(u64, HistoryKind)>,
    history_id: u64,
    history_floor: u64,
    next_id: u64,
    next_thread: u64,
    failures: Vec<(Failure, u32)>,
    requests: Vec<String>,
    sent: Vec<Vec<u8>>,
    access_token: String,
    refresh_token: String,
    token_lifetime: u64,
    refresh_revoked: bool,
    codes: HashMap<String, String>,
    token_requests: u32,
    now_ms: i64,
    stale_tokens: Vec<String>,
    token_counter: u32,
    contacts: Vec<Value>,
    user_labels: Vec<(String, String)>,
    granted_scope: String,
}

type Shared = Arc<Mutex<State_>>;

pub struct FakeGoogle {
    state: Shared,
    pub addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for FakeGoogle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn lock(state: &Shared) -> MutexGuard<'_, State_> {
    state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl FakeGoogle {
    pub async fn start(email: &str) -> Self {
        let state: Shared = Arc::new(Mutex::new(State_ {
            email: email.to_string(),
            messages: Vec::new(),
            history: Vec::new(),
            history_id: 1000,
            history_floor: 0,
            next_id: 1,
            next_thread: 1,
            failures: Vec::new(),
            requests: Vec::new(),
            sent: Vec::new(),
            access_token: "access-1".into(),
            refresh_token: "refresh-1".into(),
            token_lifetime: 3600,
            refresh_revoked: false,
            codes: HashMap::new(),
            token_requests: 0,
            now_ms: 1_795_082_400_000,
            stale_tokens: Vec::new(),
            token_counter: 1,
            contacts: Vec::new(),
            user_labels: vec![("Label_1".into(), "Clients".into())],
            granted_scope: "openid email https://www.googleapis.com/auth/gmail.modify".into(),
        }));
        let app = Router::new()
            .route("/gmail/v1/users/me/profile", get(profile))
            .route("/gmail/v1/users/me/messages", get(list_messages))
            .route("/gmail/v1/users/me/messages/send", post(send_message))
            .route("/gmail/v1/users/me/messages/{id}", get(get_message))
            .route("/gmail/v1/users/me/messages/{id}/modify", post(modify_message))
            .route("/gmail/v1/users/me/messages/{id}/attachments/{att}", get(get_attachment))
            .route("/gmail/v1/users/me/threads/{id}/modify", post(modify_thread))
            .route("/gmail/v1/users/me/threads/{id}/trash", post(trash_thread))
            .route("/gmail/v1/users/me/threads/{id}/untrash", post(untrash_thread))
            .route("/gmail/v1/users/me/history", get(history))
            .route("/gmail/v1/users/me/labels", get(labels).post(create_label))
            .route("/v1/people/me/connections", get(connections))
            .route("/auth", get(auth))
            .route("/token", post(token))
            .with_state(Arc::clone(&state));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self { state, addr, task }
    }

    pub fn gmail_base(&self) -> String {
        format!("http://{}/gmail/v1/users/me", self.addr)
    }

    pub fn people_base(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    pub fn set_contacts(&self, contacts: Vec<Value>) {
        lock(&self.state).contacts = contacts;
    }

    pub fn set_granted_scope(&self, scope: &str) {
        lock(&self.state).granted_scope = scope.to_string();
    }

    pub fn auth_url(&self) -> String {
        format!("http://{}/auth", self.addr)
    }

    pub fn token_url(&self) -> String {
        format!("http://{}/token", self.addr)
    }

    pub fn access_token(&self) -> String {
        lock(&self.state).access_token.clone()
    }

    pub fn refresh_token(&self) -> String {
        lock(&self.state).refresh_token.clone()
    }

    pub fn expire_access_token(&self) {
        let mut s = lock(&self.state);
        let old = s.access_token.clone();
        s.stale_tokens.push(old);
        s.token_counter += 1;
        s.access_token = format!("access-{}", s.token_counter);
    }

    pub fn revoke_refresh_token(&self) {
        lock(&self.state).refresh_revoked = true;
    }

    pub fn set_token_lifetime(&self, seconds: u64) {
        lock(&self.state).token_lifetime = seconds;
    }

    pub fn token_requests(&self) -> u32 {
        lock(&self.state).token_requests
    }

    pub fn set_now(&self, ms: i64) {
        lock(&self.state).now_ms = ms;
    }

    pub fn history_id(&self) -> String {
        lock(&self.state).history_id.to_string()
    }

    pub fn add_message(&self, spec: MessageSpec) -> String {
        let mut s = lock(&self.state);
        let id = format!("{:x}", 0x18c0_0000_u64 + s.next_id);
        s.next_id += 1;
        let thread_id = spec.thread.clone().unwrap_or_else(|| {
            let t = format!("t{:x}", s.next_thread);
            s.next_thread += 1;
            t
        });
        let message_id = format!("<{id}@fake.example>");
        let mut headers = vec![
            ("From".to_string(), spec.from.clone()),
            ("To".to_string(), spec.to.join(", ")),
            ("Subject".to_string(), spec.subject.clone()),
            ("Message-ID".to_string(), message_id),
        ];
        if !spec.cc.is_empty() {
            headers.push(("Cc".into(), spec.cc.join(", ")));
        }
        if let Some(reply) = &spec.in_reply_to {
            headers.push(("In-Reply-To".into(), reply.clone()));
        }
        headers.extend(spec.headers.clone());
        let labels =
            if spec.labels.is_empty() { vec!["INBOX".to_string(), "UNREAD".to_string()] } else { spec.labels.clone() };
        let text =
            spec.text_bytes.map(|(cs, b)| (Some(cs), b)).or_else(|| spec.text.clone().map(|t| (None, t.into_bytes())));
        let snippet = spec.text.clone().unwrap_or_default().chars().take(100).collect::<String>().replace('\n', " ");
        let attachments = spec.attachments.into_iter().enumerate().map(|(i, a)| (format!("ANG{id}x{i}"), a)).collect();
        s.messages.push(Stored {
            id: id.clone(),
            thread_id,
            labels,
            date_ms: spec.date_ms,
            snippet,
            headers,
            text,
            html: spec.html,
            attachments,
        });
        s.history_id += 1;
        let h = s.history_id;
        s.history.push((h, HistoryKind::Added(id.clone())));
        id
    }

    pub fn change_labels(&self, id: &str, add: &[&str], remove: &[&str]) {
        let mut s = lock(&self.state);
        apply_labels(&mut s, id, add, remove);
    }

    pub fn delete_message(&self, id: &str) {
        let mut s = lock(&self.state);
        s.messages.retain(|m| m.id != id);
        s.history_id += 1;
        let h = s.history_id;
        s.history.push((h, HistoryKind::Deleted(id.to_string())));
    }

    pub fn expire_history(&self) {
        let mut s = lock(&self.state);
        s.history_floor = s.history_id;
    }

    pub fn fail_next(&self, count: u32, failure: Failure) {
        lock(&self.state).failures.push((failure, count));
    }

    pub fn clear_failures(&self) {
        lock(&self.state).failures.clear();
    }

    pub fn requests(&self) -> Vec<String> {
        lock(&self.state).requests.clone()
    }

    pub fn count_requests(&self, containing: &str) -> usize {
        lock(&self.state).requests.iter().filter(|r| r.contains(containing)).count()
    }

    pub fn sent(&self) -> Vec<Vec<u8>> {
        lock(&self.state).sent.clone()
    }

    pub fn user_label_names(&self) -> Vec<String> {
        lock(&self.state).user_labels.iter().map(|(_, n)| n.clone()).collect()
    }

    pub fn labels_of(&self, id: &str) -> Vec<String> {
        lock(&self.state).messages.iter().find(|m| m.id == id).map(|m| m.labels.clone()).unwrap_or_default()
    }

    pub fn message_count(&self) -> usize {
        lock(&self.state).messages.len()
    }
}

fn apply_labels(s: &mut State_, id: &str, add: &[&str], remove: &[&str]) {
    let Some(message) = s.messages.iter_mut().find(|m| m.id == id) else { return };
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for label in add {
        if !message.labels.iter().any(|l| l == label) {
            message.labels.push((*label).to_string());
            added.push((*label).to_string());
        }
    }
    for label in remove {
        if let Some(at) = message.labels.iter().position(|l| l == label) {
            message.labels.remove(at);
            removed.push((*label).to_string());
        }
    }
    if !added.is_empty() {
        s.history_id += 1;
        let h = s.history_id;
        s.history.push((h, HistoryKind::LabelsAdded(id.to_string(), added)));
    }
    if !removed.is_empty() {
        s.history_id += 1;
        let h = s.history_id;
        s.history.push((h, HistoryKind::LabelsRemoved(id.to_string(), removed)));
    }
}

fn gmail_error(status: u16, reason: &str, message: &str) -> Response {
    let body = json!({"error": {"code": status, "message": message, "errors": [{"message": message, "domain": "global", "reason": reason}]}});
    (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), axum::Json(body)).into_response()
}

#[allow(clippy::result_large_err)]
fn gate(state: &Shared, path: &str, headers: &HeaderMap) -> Result<(), Response> {
    let mut s = lock(state);
    s.requests.push(path.to_string());
    if let Some(at) =
        s.failures.iter().position(|(f, n)| *n > 0 && f.path_contains.as_deref().is_none_or(|p| path.contains(p)))
    {
        let (failure, remaining) = &mut s.failures[at];
        *remaining -= 1;
        let mut response = gmail_error(failure.status, &failure.reason, "injected failure");
        if let Some(seconds) = failure.retry_after {
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        return Err(response);
    }
    let presented =
        headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
    if presented != Some(s.access_token.as_str()) {
        return Err(gmail_error(401, "authError", "Invalid Credentials"));
    }
    Ok(())
}

fn rfc_headers(stored: &Stored, only: Option<&[String]>) -> Vec<Value> {
    let mut out: Vec<Value> = stored
        .headers
        .iter()
        .filter(|(n, _)| only.is_none_or(|names| names.iter().any(|w| w.eq_ignore_ascii_case(n))))
        .map(|(n, v)| json!({"name": n, "value": v}))
        .collect();
    let date = format!("ms:{}", stored.date_ms);
    if only.is_none_or(|names| names.iter().any(|w| w.eq_ignore_ascii_case("Date"))) {
        out.push(json!({"name": "Date", "value": date}));
    }
    out
}

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn payload_of(stored: &Stored, full: bool, only: Option<&[String]>) -> Value {
    let headers = rfc_headers(stored, only);
    if !full {
        return json!({"partId": "", "mimeType": "multipart/alternative", "filename": "", "headers": headers, "body": {"size": 0}});
    }
    let mut text_parts = Vec::new();
    if let Some((charset, bytes)) = &stored.text {
        let content_type = match charset {
            Some(cs) => format!("text/plain; charset={cs}"),
            None => "text/plain; charset=\"UTF-8\"".to_string(),
        };
        text_parts.push(json!({"partId": "0", "mimeType": "text/plain", "filename": "",
            "headers": [{"name": "Content-Type", "value": content_type}], "body": {"size": bytes.len(), "data": b64(bytes)}}));
    }
    if let Some(html) = &stored.html {
        text_parts.push(json!({"partId": "1", "mimeType": "text/html", "filename": "",
            "headers": [{"name": "Content-Type", "value": "text/html; charset=\"UTF-8\""}],
            "body": {"size": html.len(), "data": b64(html.as_bytes())}}));
    }
    if stored.attachments.is_empty() && text_parts.len() == 1 {
        let mut part = text_parts.remove(0);
        part["headers"].as_array_mut().expect("headers").extend(headers);
        return part;
    }
    let mut parts = Vec::new();
    if text_parts.len() > 1 {
        parts.push(json!({"partId": "0", "mimeType": "multipart/alternative", "filename": "", "headers": [], "body": {"size": 0}, "parts": text_parts}));
    } else {
        parts.extend(text_parts);
    }
    for (attachment_id, spec) in &stored.attachments {
        let mut part_headers = vec![json!({"name": "Content-Type", "value": spec.mime})];
        match &spec.content_id {
            Some(cid) => {
                part_headers.push(json!({"name": "Content-ID", "value": format!("<{cid}>")}));
                part_headers.push(json!({"name": "Content-Disposition", "value": "inline"}));
            }
            None => part_headers.push(
                json!({"name": "Content-Disposition", "value": format!("attachment; filename=\"{}\"", spec.filename)}),
            ),
        }
        parts.push(json!({"partId": "2", "mimeType": spec.mime, "filename": spec.filename, "headers": part_headers,
            "body": {"attachmentId": attachment_id, "size": spec.bytes.len()}}));
    }
    json!({"partId": "", "mimeType": "multipart/mixed", "filename": "", "headers": headers, "body": {"size": 0}, "parts": parts})
}

fn message_json(stored: &Stored, full: bool, only: Option<&[String]>) -> Value {
    json!({
        "id": stored.id,
        "threadId": stored.thread_id,
        "labelIds": stored.labels,
        "snippet": stored.snippet,
        "historyId": "1",
        "internalDate": stored.date_ms.to_string(),
        "sizeEstimate": 1024,
        "payload": payload_of(stored, full, only),
    })
}

async fn profile(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = gate(&state, "profile", &headers) {
        return r;
    }
    let s = lock(&state);
    axum::Json(json!({
        "emailAddress": s.email,
        "messagesTotal": s.messages.len(),
        "threadsTotal": s.messages.len(),
        "historyId": s.history_id.to_string(),
    }))
    .into_response()
}

async fn list_messages(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if let Err(r) = gate(&state, "messages?list", &headers) {
        return r;
    }
    let s = lock(&state);
    let include_spam_trash = q.get("includeSpamTrash").is_some_and(|v| v == "true");
    let mut matching: Vec<&Stored> = s
        .messages
        .iter()
        .filter(|m| include_spam_trash || !m.labels.iter().any(|l| l == "SPAM" || l == "TRASH"))
        .filter(|m| {
            match q
                .get("q")
                .and_then(|q| q.strip_prefix("newer_than:"))
                .and_then(|d| d.strip_suffix('d'))
                .and_then(|d| d.parse::<i64>().ok())
            {
                Some(days) => m.date_ms >= s.now_ms - days * 86_400_000,
                None => true,
            }
        })
        .collect();
    matching.sort_by(|a, b| b.date_ms.cmp(&a.date_ms).then_with(|| b.id.cmp(&a.id)));
    let max = q.get("maxResults").and_then(|v| v.parse::<usize>().ok()).unwrap_or(100).clamp(1, 500);
    let start = q.get("pageToken").and_then(|t| t.strip_prefix("p")).and_then(|t| t.parse::<usize>().ok()).unwrap_or(0);
    let page: Vec<Value> =
        matching.iter().skip(start).take(max).map(|m| json!({"id": m.id, "threadId": m.thread_id})).collect();
    let mut body = json!({"resultSizeEstimate": matching.len()});
    if !page.is_empty() {
        body["messages"] = Value::Array(page);
    }
    if start + max < matching.len() {
        body["nextPageToken"] = json!(format!("p{}", start + max));
    }
    axum::Json(body).into_response()
}

async fn get_message(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> Response {
    if let Err(r) = gate(&state, &format!("messages/{id}"), &headers) {
        return r;
    }
    let raw = raw.unwrap_or_default();
    let full = raw.contains("format=full");
    let only: Vec<String> =
        raw.split('&').filter_map(|p| p.strip_prefix("metadataHeaders=")).map(str::to_string).collect();
    let s = lock(&state);
    match s.messages.iter().find(|m| m.id == id) {
        Some(m) => axum::Json(message_json(m, full, (!only.is_empty()).then_some(&only[..]))).into_response(),
        None => gmail_error(404, "notFound", "Requested entity was not found."),
    }
}

async fn get_attachment(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path((id, att)): Path<(String, String)>,
) -> Response {
    if let Err(r) = gate(&state, &format!("messages/{id}/attachments/{att}"), &headers) {
        return r;
    }
    let s = lock(&state);
    let found = s.messages.iter().find(|m| m.id == id).and_then(|m| m.attachments.iter().find(|(a, _)| *a == att));
    match found {
        Some((_, spec)) => axum::Json(json!({"size": spec.bytes.len(), "data": b64(&spec.bytes)})).into_response(),
        None => gmail_error(404, "notFound", "Attachment not found."),
    }
}

async fn history(
    State(state): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    if let Err(r) = gate(&state, "history", &headers) {
        return r;
    }
    let s = lock(&state);
    let start: u64 = q.get("startHistoryId").and_then(|v| v.parse().ok()).unwrap_or(0);
    if start < s.history_floor {
        return gmail_error(404, "notFound", "Requested entity was not found.");
    }
    let page_start =
        q.get("pageToken").and_then(|t| t.strip_prefix('h')).and_then(|t| t.parse::<usize>().ok()).unwrap_or(0);
    let after: Vec<&(u64, HistoryKind)> = s.history.iter().filter(|(id, _)| *id > start).collect();
    let page: Vec<Value> = after
        .iter()
        .skip(page_start)
        .take(2)
        .map(|(id, kind)| {
            let message = |mid: &str| {
                let thread =
                    s.messages.iter().find(|m| m.id == mid).map_or("gone".to_string(), |m| m.thread_id.clone());
                json!({"id": mid, "threadId": thread})
            };
            match kind {
                HistoryKind::Added(m) => {
                    json!({"id": id.to_string(), "messages": [message(m)], "messagesAdded": [{"message": message(m)}]})
                }
                HistoryKind::Deleted(m) => json!({"id": id.to_string(), "messagesDeleted": [{"message": message(m)}]}),
                HistoryKind::LabelsAdded(m, l) => {
                    json!({"id": id.to_string(), "labelsAdded": [{"message": message(m), "labelIds": l}]})
                }
                HistoryKind::LabelsRemoved(m, l) => {
                    json!({"id": id.to_string(), "labelsRemoved": [{"message": message(m), "labelIds": l}]})
                }
            }
        })
        .collect();
    let mut body = json!({"historyId": s.history_id.to_string()});
    if !page.is_empty() {
        body["history"] = Value::Array(page);
    }
    if page_start + 2 < after.len() {
        body["nextPageToken"] = json!(format!("h{}", page_start + 2));
    }
    axum::Json(body).into_response()
}

async fn connections(
    State(state): State<Shared>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<HashMap<String, String>>,
) -> Response {
    if let Err(r) = gate(&state, "people", &headers) {
        return r;
    }
    let all = lock(&state).contacts.clone();
    let start: usize = q.get("pageToken").and_then(|t| t.parse().ok()).unwrap_or(0);
    let end = (start + 2).min(all.len());
    let mut body = json!({"connections": all[start.min(all.len())..end]});
    if end < all.len() {
        body["nextPageToken"] = json!(end.to_string());
    }
    axum::Json(body).into_response()
}

async fn create_label(
    State(state): State<Shared>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    if let Err(r) = gate(&state, "labels/create", &headers) {
        return r;
    }
    let name = body["name"].as_str().unwrap_or_default().to_string();
    let mut s = lock(&state);
    if s.user_labels.iter().any(|(_, n)| n.eq_ignore_ascii_case(&name)) {
        return gmail_error(409, "alreadyExists", "Label name exists or conflicts with another label name.");
    }
    let id = format!("Label_{}", s.user_labels.len() + 1);
    s.user_labels.push((id.clone(), name.clone()));
    axum::Json(json!({"id": id, "name": name, "type": "user"})).into_response()
}

async fn labels(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if let Err(r) = gate(&state, "labels", &headers) {
        return r;
    }
    let mut labels = vec![
        json!({"id": "INBOX", "name": "INBOX", "type": "system"}),
        json!({"id": "SENT", "name": "SENT", "type": "system"}),
    ];
    for (id, name) in &lock(&state).user_labels {
        labels.push(json!({"id": id, "name": name, "type": "user"}));
    }
    axum::Json(json!({ "labels": labels })).into_response()
}

fn labels_from(body: &Value, key: &str) -> Vec<String> {
    body[key].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

async fn modify_message(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    if let Err(r) = gate(&state, &format!("messages/{id}/modify"), &headers) {
        return r;
    }
    let mut s = lock(&state);
    if !s.messages.iter().any(|m| m.id == id) {
        return gmail_error(404, "notFound", "Requested entity was not found.");
    }
    let (add, remove) = (labels_from(&body, "addLabelIds"), labels_from(&body, "removeLabelIds"));
    apply_labels(
        &mut s,
        &id,
        &add.iter().map(String::as_str).collect::<Vec<_>>(),
        &remove.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    axum::Json(json!({"id": id})).into_response()
}

fn thread_ids(s: &State_, thread: &str) -> Vec<String> {
    s.messages.iter().filter(|m| m.thread_id == thread).map(|m| m.id.clone()).collect()
}

async fn modify_thread(
    State(state): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    if let Err(r) = gate(&state, &format!("threads/{id}/modify"), &headers) {
        return r;
    }
    let mut s = lock(&state);
    let ids = thread_ids(&s, &id);
    if ids.is_empty() {
        return gmail_error(404, "notFound", "Requested entity was not found.");
    }
    let (add, remove) = (labels_from(&body, "addLabelIds"), labels_from(&body, "removeLabelIds"));
    for message in ids {
        apply_labels(
            &mut s,
            &message,
            &add.iter().map(String::as_str).collect::<Vec<_>>(),
            &remove.iter().map(String::as_str).collect::<Vec<_>>(),
        );
    }
    axum::Json(json!({"id": id})).into_response()
}

async fn trash_thread(State(state): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(r) = gate(&state, &format!("threads/{id}/trash"), &headers) {
        return r;
    }
    let mut s = lock(&state);
    let ids = thread_ids(&s, &id);
    if ids.is_empty() {
        return gmail_error(404, "notFound", "Requested entity was not found.");
    }
    for message in ids {
        apply_labels(&mut s, &message, &["TRASH"], &["INBOX"]);
    }
    axum::Json(json!({"id": id})).into_response()
}

async fn untrash_thread(State(state): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(r) = gate(&state, &format!("threads/{id}/untrash"), &headers) {
        return r;
    }
    let mut s = lock(&state);
    for message in thread_ids(&s, &id) {
        apply_labels(&mut s, &message, &["INBOX"], &["TRASH"]);
    }
    axum::Json(json!({"id": id})).into_response()
}

async fn send_message(
    State(state): State<Shared>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    if let Err(r) = gate(&state, "messages/send", &headers) {
        return r;
    }
    let Some(raw) = body["raw"].as_str().and_then(|r| URL_SAFE_NO_PAD.decode(r.trim_end_matches('=')).ok()) else {
        return gmail_error(400, "invalidArgument", "Invalid raw message");
    };
    let text = String::from_utf8_lossy(&raw).to_string();
    let header_block = text.split("\r\n\r\n").next().unwrap_or_default();
    let find = |name: &str| -> String {
        header_block
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with(&format!("{}:", name.to_ascii_lowercase())))
            .map(|l| l[name.len() + 1..].trim().to_string())
            .unwrap_or_default()
    };
    let body_text = text.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
    let thread = body["threadId"].as_str().map(str::to_string);
    let email = lock(&state).email.clone();
    let id = {
        let spec = MessageSpec {
            from: email,
            to: vec![find("To")],
            subject: find("Subject"),
            text: Some(body_text),
            date_ms: lock(&state).now_ms,
            labels: vec!["SENT".into()],
            thread,
            in_reply_to: Some(find("In-Reply-To")).filter(|s| !s.is_empty()),
            ..Default::default()
        };
        let fake = FakeGoogleHandle(Arc::clone(&state));
        fake.add(spec)
    };
    let (thread_id, mut s) = {
        let s = lock(&state);
        (s.messages.iter().find(|m| m.id == id).map(|m| m.thread_id.clone()).unwrap_or_default(), s)
    };
    s.sent.push(raw);
    axum::Json(json!({"id": id, "threadId": thread_id, "labelIds": ["SENT"]})).into_response()
}

struct FakeGoogleHandle(Shared);

impl FakeGoogleHandle {
    fn add(&self, spec: MessageSpec) -> String {
        let mut s = lock(&self.0);
        let id = format!("{:x}", 0x18c0_0000_u64 + s.next_id);
        s.next_id += 1;
        let thread_id = spec.thread.clone().unwrap_or_else(|| {
            let t = format!("t{:x}", s.next_thread);
            s.next_thread += 1;
            t
        });
        let mut headers = vec![
            ("From".to_string(), spec.from.clone()),
            ("To".to_string(), spec.to.join(", ")),
            ("Subject".to_string(), spec.subject.clone()),
            ("Message-ID".to_string(), format!("<{id}@fake.example>")),
        ];
        if let Some(reply) = &spec.in_reply_to {
            headers.push(("In-Reply-To".into(), reply.clone()));
        }
        let snippet = spec.text.clone().unwrap_or_default().chars().take(100).collect();
        s.messages.push(Stored {
            id: id.clone(),
            thread_id,
            labels: spec.labels,
            date_ms: spec.date_ms,
            snippet,
            headers,
            text: spec.text.map(|t| (None, t.into_bytes())),
            html: None,
            attachments: Vec::new(),
        });
        s.history_id += 1;
        let h = s.history_id;
        s.history.push((h, HistoryKind::Added(id.clone())));
        id
    }
}

async fn auth(State(state): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Response {
    let (Some(redirect), Some(challenge)) = (q.get("redirect_uri"), q.get("code_challenge")) else {
        return (StatusCode::BAD_REQUEST, "missing redirect_uri or code_challenge").into_response();
    };
    if q.get("code_challenge_method").map(String::as_str) != Some("S256") {
        return (StatusCode::BAD_REQUEST, "needs S256").into_response();
    }
    let wants_mail = q.get("scope").is_some_and(|s| s.contains("gmail.modify"));
    if wants_mail && q.get("access_type").map(String::as_str) != Some("offline") {
        return (StatusCode::BAD_REQUEST, "mail needs offline access").into_response();
    }
    let code = {
        let mut s = lock(&state);
        let code = format!("code-{}", s.codes.len() + 1);
        s.codes.insert(code.clone(), challenge.clone());
        code
    };
    let mut target = format!("{redirect}/?code={code}");
    if let Some(state_param) = q.get("state") {
        target.push_str(&format!("&state={state_param}"));
    }
    Redirect::to(&target).into_response()
}

async fn token(State(state): State<Shared>, body: Bytes) -> Response {
    let form: HashMap<String, String> = String::from_utf8_lossy(&body)
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (url_decode(k), url_decode(v)))
        .collect();
    let mut s = lock(&state);
    s.token_requests += 1;
    let invalid = |error: &str, description: &str| {
        (StatusCode::BAD_REQUEST, axum::Json(json!({"error": error, "error_description": description}))).into_response()
    };
    match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            let Some(challenge) = form.get("code").and_then(|c| s.codes.remove(c)) else {
                return invalid("invalid_grant", "Bad Request");
            };
            let verifier = form.get("code_verifier").cloned().unwrap_or_default();
            if URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != challenge {
                return invalid("invalid_grant", "code_verifier does not match");
            }
            if form.get("client_id").is_none_or(String::is_empty)
                || form.get("client_secret").is_none_or(String::is_empty)
            {
                return invalid("invalid_client", "missing client credentials");
            }
            axum::Json(json!({
                "access_token": s.access_token, "expires_in": s.token_lifetime, "refresh_token": s.refresh_token,
                "scope": s.granted_scope, "token_type": "Bearer",
                "id_token": format!("e30.{}.sig", STANDARD.encode(format!("{{\"email\":\"{}\"}}", s.email))),
            }))
            .into_response()
        }
        Some("refresh_token") => {
            if s.refresh_revoked || form.get("refresh_token") != Some(&s.refresh_token) {
                return invalid("invalid_grant", "Token has been expired or revoked.");
            }
            axum::Json(json!({
                "access_token": s.access_token, "expires_in": s.token_lifetime,
                "scope": s.granted_scope, "token_type": "Bearer",
            }))
            .into_response()
        }
        _ => invalid("unsupported_grant_type", "unsupported"),
    }
}

fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                    out.push(byte);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

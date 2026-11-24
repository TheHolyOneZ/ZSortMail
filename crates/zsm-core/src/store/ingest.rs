use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use zsm_mime::{html_to_text, parse_unsubscribe, sanitize_html, snippet, split_quoted_html, strip_quotes};

use super::Store;
use super::conversations::recompute_conversation;
use super::messages::compress;
use super::query::load_facts;
use super::sorting::{apply_domain_rule, sync_person_visibility};
use super::watches::WatchHit;
use crate::dto::{Category, PersonKind, PersonStatus};
use crate::error::Result;
use crate::ink::ink_for;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Address {
    pub email: String,
    pub name: Option<String>,
}

impl Address {
    pub fn new(email: &str, name: Option<&str>) -> Self {
        Self { email: email.trim().to_lowercase(), name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HeaderFacts {
    pub list_id: Option<String>,
    pub list_unsubscribe: Option<String>,
    pub list_unsubscribe_post: Option<String>,
    pub precedence: Option<String>,
    pub auto_submitted: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IncomingAttachment {
    pub provider_id: Option<String>,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub content_id: Option<String>,
    pub inline: bool,
}

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub provider_id: String,
    pub thread_id: String,
    pub rfc_message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub from: Address,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: String,
    pub sent_at: i64,
    pub snippet: Option<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<IncomingAttachment>,
    pub labels: Vec<String>,
    pub unread: bool,
    pub starred: bool,
    pub headers: HeaderFacts,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct IngestContext {
    pub account_id: i64,
    pub self_addresses: Vec<String>,
    pub screener: bool,
    pub automation: bool,
}

#[derive(Debug, Clone, Default)]
pub struct IngestOutcome {
    pub message_id: i64,
    pub conversation_id: i64,
    pub inserted: bool,
    pub person_ids: Vec<i64>,
    pub new_person_ids: Vec<i64>,
    pub sender_person_id: Option<i64>,
    pub category: Option<Category>,
    pub automation: Automation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Automation {
    pub notify: bool,
    pub muted: bool,
    pub hidden: bool,
    pub rules: Vec<i64>,
    pub watch_hits: Vec<WatchHit>,
}

#[derive(Debug, Clone, Default)]
pub struct BatchOutcome {
    pub messages: Vec<IngestOutcome>,
    pub touched_people: Vec<i64>,
    pub touched_conversations: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classification {
    pub kind: PersonKind,
    pub category: Category,
}

static AUTOMATED_LOCAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(no[-_.]?reply|do[-_.]?not[-_.]?reply|donotreply|mailer[-_.]?daemon|postmaster|notifications?|alerts?|automated|bounce[s]?|system|robot|auto)([-_.+].*)?$")
        .expect("valid regex")
});

static RECEIPT_SUBJECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(receipt|invoice|your order|order (confirmation|#?\d)|payment|payslip|statement|booking confirmation|has been (shipped|delivered)|shipping|delivery)\b")
        .expect("valid regex")
});

pub fn classify(from: &Address, headers: &HeaderFacts, subject: &str) -> Classification {
    let bulk = headers
        .precedence
        .as_deref()
        .is_some_and(|p| matches!(p.trim().to_ascii_lowercase().as_str(), "bulk" | "list" | "junk"));
    if headers.list_id.is_some() || bulk || headers.list_unsubscribe.is_some() {
        return Classification { kind: PersonKind::List, category: Category::Newsletter };
    }
    let local = from.email.split('@').next().unwrap_or_default();
    let auto_submitted = headers.auto_submitted.as_deref().is_some_and(|a| !a.trim().eq_ignore_ascii_case("no"));
    if auto_submitted || AUTOMATED_LOCAL.is_match(local) {
        let category = if RECEIPT_SUBJECT.is_match(subject) { Category::Receipt } else { Category::Automated };
        return Classification { kind: PersonKind::Service, category };
    }
    Classification { kind: PersonKind::Person, category: Category::Person }
}

fn brand_for(email: &str) -> Option<&'static str> {
    let (local, domain) = email.rsplit_once('@')?;
    let domain = domain.to_ascii_lowercase();
    let local = local.to_ascii_lowercase();
    let on = |d: &str| domain == d || domain.ends_with(&format!(".{d}"));
    if on("github.com") && !local.contains('+') && ["notifications", "noreply"].contains(&local.as_str()) {
        return Some("GitHub");
    }
    if on("gitlab.com") && ["notifications", "noreply", "gitlab"].contains(&local.as_str()) {
        return Some("GitLab");
    }
    if on("linkedin.com") {
        return Some("LinkedIn");
    }
    if on("facebookmail.com") {
        return Some("Facebook");
    }
    if on("slack.com") && ["feedback", "notification", "notifications", "no-reply"].contains(&local.as_str()) {
        return Some("Slack");
    }
    None
}

fn is_relay(email: &str) -> bool {
    email.rsplit_once('@').is_some_and(|(_, d)| d.eq_ignore_ascii_case("noreply.github.com"))
}

pub(crate) fn prettify_local_part(email: &str) -> String {
    let local = email.split('@').next().unwrap_or(email);
    let words: Vec<String> = local
        .split(['.', '_', '-', '+'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
        })
        .collect();
    if words.is_empty() { email.to_string() } else { words.join(" ") }
}

fn kind_str(kind: PersonKind) -> &'static str {
    match kind {
        PersonKind::Person => "person",
        PersonKind::Service => "service",
        PersonKind::List => "list",
        PersonKind::Myself => "self",
    }
}

fn status_str(status: PersonStatus) -> &'static str {
    match status {
        PersonStatus::Known => "known",
        PersonStatus::NewSender => "new_sender",
        PersonStatus::Blocked => "blocked",
    }
}

pub(crate) fn category_str(category: Category) -> &'static str {
    match category {
        Category::Person => "person",
        Category::Newsletter => "newsletter",
        Category::Receipt => "receipt",
        Category::Automated => "automated",
    }
}

fn resolve_person(
    tx: &Transaction<'_>,
    account_id: i64,
    addr: &Address,
    kind: PersonKind,
    status_if_new: PersonStatus,
    now: i64,
) -> Result<(i64, bool)> {
    if let Some(id) = tx
        .prepare_cached("SELECT person_id FROM person_addresses WHERE account_id = ?1 AND address = ?2")?
        .query_row(params![account_id, addr.email], |r| r.get(0))
        .optional()?
    {
        return Ok((id, false));
    }
    let name = brand_for(&addr.email).map(str::to_string).unwrap_or_else(|| {
        addr.name
            .as_deref()
            .map(|n| n.trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace()))
            .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case(&addr.email))
            .map(str::to_string)
            .unwrap_or_else(|| prettify_local_part(&addr.email))
    });
    tx.prepare_cached(
        "INSERT INTO people (account_id, kind, status, display_name, ink, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![account_id, kind_str(kind), status_str(status_if_new), name, ink_for(&addr.email), now])?;
    let id = tx.last_insert_rowid();
    tx.prepare_cached(
        "INSERT INTO person_addresses (account_id, address, person_id, is_primary) VALUES (?1, ?2, ?3, 1)",
    )?
    .execute(params![account_id, addr.email, id])?;
    Ok((id, true))
}

struct Bodies {
    text: String,
    quoted: Option<String>,
    html: Option<String>,
    remote_images: u32,
    trackers_blocked: u32,
}

fn process_bodies(msg: &IncomingMessage) -> Bodies {
    let html = msg.html.as_deref().filter(|h| !h.trim().is_empty());
    let plain = msg.text.as_deref().filter(|t| !t.trim().is_empty());

    let (sanitized, remote_images, trackers_blocked) = match html {
        Some(raw) => {
            let s = sanitize_html(raw);
            (Some(s.html), s.remote_images, s.trackers_removed)
        }
        None => (None, 0, 0),
    };

    let split = if let Some(text) = plain {
        strip_quotes(text)
    } else if let Some(raw) = html {
        let (new_html, quoted_html) = split_quoted_html(raw);
        let mut split = strip_quotes(&html_to_text(&new_html));
        if split.quoted.is_none() {
            split.quoted = quoted_html.as_deref().map(html_to_text).filter(|q| !q.trim().is_empty());
        }
        split
    } else {
        zsm_mime::Split { text: String::new(), quoted: None }
    };

    Bodies { text: split.text, quoted: split.quoted, html: sanitized, remote_images, trackers_blocked }
}

const SYSTEM_LABELS: &[&str] = &["INBOX", "SENT", "DRAFT", "SPAM", "TRASH", "STARRED", "UNREAD", "IMPORTANT", "CHAT"];

fn label_kind(id: &str) -> &'static str {
    if SYSTEM_LABELS.contains(&id) || id.starts_with("CATEGORY_") { "system" } else { "user" }
}

fn addresses_json(list: &[Address]) -> String {
    serde_json::to_string(list).unwrap_or_else(|_| "[]".to_string())
}

impl Store {
    pub fn ingest(&mut self, ctx: &IngestContext, msg: &IncomingMessage, now: i64) -> Result<IngestOutcome> {
        let mut batch = self.ingest_batch(ctx, std::slice::from_ref(msg), now)?;
        Ok(batch.messages.pop().unwrap_or_default())
    }

    pub fn ingest_batch(&mut self, ctx: &IngestContext, msgs: &[IncomingMessage], now: i64) -> Result<BatchOutcome> {
        self.in_transaction(|tx| {
            let mut conversations = BTreeSet::new();
            let mut people = BTreeSet::new();
            let mut outcomes = Vec::with_capacity(msgs.len());
            for msg in msgs {
                let outcome = ingest_one(tx, ctx, msg, now)?;
                conversations.insert(outcome.conversation_id);
                people.extend(outcome.person_ids.iter().copied());
                outcomes.push(outcome);
            }
            for id in &conversations {
                recompute_conversation(tx, *id)?;
            }
            for id in &people {
                sync_person_visibility(tx, *id, now)?;
            }
            if ctx.automation {
                let mut changed = false;
                for out in outcomes.iter_mut().filter(|o| o.inserted && o.sender_person_id.is_some()) {
                    let facts = load_facts(tx, out.message_id)?;
                    let rules = Store::run_rules_in(tx, out.message_id, &facts, now)?;
                    let watch_hits = Store::evaluate_watches_in(tx, out.message_id, &facts, now)?;
                    changed |= !rules.matched.is_empty();
                    out.automation = Automation {
                        notify: rules.notify,
                        muted: rules.muted,
                        hidden: rules.hidden,
                        rules: rules.matched,
                        watch_hits,
                    };
                }
                if changed {
                    for id in &conversations {
                        recompute_conversation(tx, *id)?;
                    }
                    for id in &people {
                        sync_person_visibility(tx, *id, now)?;
                    }
                }
            }
            Ok(BatchOutcome {
                messages: outcomes,
                touched_people: people.into_iter().collect(),
                touched_conversations: conversations.into_iter().collect(),
            })
        })
    }
}

fn ingest_one(tx: &Transaction<'_>, ctx: &IngestContext, msg: &IncomingMessage, now: i64) -> Result<IngestOutcome> {
    let account_id = ctx.account_id;

    if let Some((id, conversation_id)) = tx
        .prepare_cached("SELECT id, conversation_id FROM messages WHERE account_id = ?1 AND provider_id = ?2")?
        .query_row(params![account_id, msg.provider_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .optional()?
    {
        tx.execute(
            "UPDATE messages SET unread = ?1, starred = ?2 WHERE id = ?3",
            params![msg.unread, msg.starred, id],
        )?;
        if !msg.bcc.is_empty() {
            tx.execute("UPDATE messages SET bcc_addresses = ?1 WHERE id = ?2", params![addresses_json(&msg.bcc), id])?;
        }
        let person_ids = people_of_conversation(tx, conversation_id)?;
        return Ok(IngestOutcome {
            message_id: id,
            conversation_id,
            inserted: false,
            person_ids,
            ..Default::default()
        });
    }

    let from_email = msg.from.email.to_lowercase();
    let is_from_me = ctx.self_addresses.contains(&from_email) || msg.labels.iter().any(|l| l == "SENT");
    let class = classify(&msg.from, &msg.headers, &msg.subject);

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut counterparties: Vec<(&Address, bool)> = Vec::new();
    if !is_from_me && seen.insert(from_email.clone()) {
        counterparties.push((&msg.from, true));
    }
    for addr in msg.to.iter().chain(msg.cc.iter()) {
        let email = addr.email.to_lowercase();
        if email.is_empty() || is_relay(&email) || ctx.self_addresses.contains(&email) || !seen.insert(email) {
            continue;
        }
        counterparties.push((addr, false));
    }
    counterparties.truncate(12);

    let mut person_ids = Vec::new();
    let mut new_person_ids = Vec::new();
    let mut sender_person_id = None;
    for (addr, is_sender) in counterparties {
        let (kind, status) = if is_sender {
            let status = if ctx.screener && class.kind == PersonKind::Person {
                PersonStatus::NewSender
            } else {
                PersonStatus::Known
            };
            (class.kind, status)
        } else {
            (PersonKind::Person, PersonStatus::Known)
        };
        let (id, created) = resolve_person(tx, account_id, addr, kind, status, now)?;
        if created {
            new_person_ids.push(id);
            if is_sender {
                apply_domain_rule(tx, id, &addr.email, now)?;
            }
        }
        if is_sender {
            sender_person_id = Some(id);
        }
        person_ids.push(id);
    }

    tx.prepare_cached(
        "INSERT INTO conversations (account_id, provider_id, subject, first_message_at, last_message_at, category)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5) ON CONFLICT (account_id, provider_id) DO NOTHING",
    )?
    .execute(params![account_id, msg.thread_id, msg.subject, msg.sent_at, category_str(class.category)])?;
    let conversation_id: i64 = tx
        .prepare_cached("SELECT id FROM conversations WHERE account_id = ?1 AND provider_id = ?2")?
        .query_row(params![account_id, msg.thread_id], |r| r.get(0))?;
    for person in &person_ids {
        tx.prepare_cached("INSERT OR IGNORE INTO conversation_people (conversation_id, person_id) VALUES (?1, ?2)")?
            .execute(params![conversation_id, person])?;
    }

    let bodies = process_bodies(msg);
    let preview = msg.snippet.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| snippet(&bodies.text, 160));
    let preview = snippet(&preview, 160);
    let body_state = if msg.text.is_some() || msg.html.is_some() { "full" } else { "metadata" };
    let unsubscribe = msg.headers.list_unsubscribe.as_deref().map(|h| {
        let u = parse_unsubscribe(h, msg.headers.list_unsubscribe_post.as_deref());
        serde_json::json!({ "https": u.https, "mailto": u.mailto, "one_click": u.one_click }).to_string()
    });

    let participants: String = std::iter::once(&msg.from)
        .chain(msg.to.iter())
        .chain(msg.cc.iter())
        .map(|a| format!("{} {}", a.name.as_deref().unwrap_or_default(), a.email))
        .collect::<Vec<_>>()
        .join(" ");

    tx.prepare_cached(
        "INSERT INTO messages (account_id, conversation_id, provider_id, rfc_message_id, in_reply_to, from_person_id,
            from_address, to_addresses, cc_addresses, subject, snippet, sent_at, is_from_me, unread, starred,
            size_bytes, body_state, body_text, body_quoted, body_html, remote_images, trackers_blocked, list_id,
            unsubscribe, category, participants, bcc_addresses)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
    )?
    .execute(params![
        account_id,
        conversation_id,
        msg.provider_id,
        msg.rfc_message_id,
        msg.in_reply_to,
        sender_person_id,
        from_email,
        addresses_json(&msg.to),
        addresses_json(&msg.cc),
        msg.subject,
        preview,
        msg.sent_at,
        is_from_me,
        msg.unread && !is_from_me,
        msg.starred,
        msg.size_bytes,
        body_state,
        compress(&bodies.text),
        bodies.quoted.as_deref().map(compress),
        bodies.html.as_deref().map(compress),
        bodies.remote_images,
        bodies.trackers_blocked,
        msg.headers.list_id,
        unsubscribe,
        category_str(class.category),
        participants,
        addresses_json(&msg.bcc),
    ])?;
    let message_id = tx.last_insert_rowid();

    for a in &msg.attachments {
        tx.prepare_cached(
            "INSERT INTO attachments (message_id, provider_id, filename, mime_type, size_bytes, content_id, inline)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?
        .execute(params![message_id, a.provider_id, a.filename, a.mime, a.size, a.content_id, a.inline])?;
    }

    for label in &msg.labels {
        tx.prepare_cached(
            "INSERT OR IGNORE INTO labels (account_id, provider_id, name, kind) VALUES (?1, ?2, ?2, ?3)",
        )?
        .execute(params![account_id, label, label_kind(label)])?;
        tx.prepare_cached(
            "INSERT OR IGNORE INTO message_labels (message_id, label_id)
             SELECT ?1, id FROM labels WHERE account_id = ?2 AND provider_id = ?3",
        )?
        .execute(params![message_id, account_id, label])?;
    }

    let indexed_text: String = bodies.text.chars().take(20_000).collect();
    tx.prepare_cached("INSERT INTO messages_fts (rowid, subject, body, participants) VALUES (?1, ?2, ?3, ?4)")?
        .execute(params![message_id, msg.subject, indexed_text, participants])?;

    Ok(IngestOutcome {
        message_id,
        conversation_id,
        inserted: true,
        person_ids,
        new_person_ids,
        sender_person_id,
        category: Some(class.category),
        automation: Automation::default(),
    })
}

fn people_of_conversation(tx: &Transaction<'_>, conversation_id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare_cached("SELECT person_id FROM conversation_people WHERE conversation_id = ?1")?;
    let rows = stmt.query_map([conversation_id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(email: &str) -> Address {
        Address::new(email, None)
    }

    #[test]
    fn people_are_people_by_default() {
        let c = classify(&addr("maya@northwind.example"), &HeaderFacts::default(), "Q3 launch");
        assert_eq!(c, Classification { kind: PersonKind::Person, category: Category::Person });
    }

    #[test]
    fn list_headers_make_a_newsletter() {
        let h = HeaderFacts { list_id: Some("<dispatch.weekly>".into()), ..Default::default() };
        let c = classify(&addr("hello@weeklydispatch.example"), &h, "Issue 212");
        assert_eq!(c, Classification { kind: PersonKind::List, category: Category::Newsletter });
        let bulk = HeaderFacts { precedence: Some("bulk".into()), ..Default::default() };
        assert_eq!(classify(&addr("a@b.example"), &bulk, "x").kind, PersonKind::List);
    }

    #[test]
    fn automated_senders_are_services_and_receipts_are_spotted() {
        let none = HeaderFacts::default();
        assert_eq!(
            classify(&addr("noreply@parcel.dhl.example"), &none, "Your parcel is on its way").category,
            Category::Automated
        );
        assert_eq!(classify(&addr("no-reply@shop.example"), &none, "Your order #394-2211").category, Category::Receipt);
        assert_eq!(
            classify(
                &addr("payroll@northwind.example"),
                &HeaderFacts { auto_submitted: Some("auto-generated".into()), ..Default::default() },
                "Your payslip"
            )
            .kind,
            PersonKind::Service
        );
        assert_eq!(
            classify(
                &addr("maya@x.example"),
                &HeaderFacts { auto_submitted: Some("no".into()), ..Default::default() },
                "hi"
            )
            .kind,
            PersonKind::Person
        );
    }

    #[test]
    fn services_that_rename_themselves_per_message_are_named_after_the_service() {
        assert_eq!(brand_for("notifications@github.com"), Some("GitHub"));
        assert_eq!(brand_for("Notifications@GitHub.com"), Some("GitHub"));
        assert_eq!(brand_for("jobs-noreply@linkedin.com"), Some("LinkedIn"));
        assert_eq!(brand_for("maya@github.com"), None, "a person at the same company keeps their own name");
        assert_eq!(brand_for("noreply@parcel.dhl.example"), None);
        assert!(is_relay("subscribed@noreply.github.com") && is_relay("ZDirMap@NOREPLY.github.com"));
        assert!(!is_relay("notifications@github.com"));
    }

    #[test]
    fn github_notifications_become_one_person_named_github_and_relay_addresses_are_not_people() {
        use crate::dto::PeopleFilter;
        use crate::store::rules::tests::{arrive, demo, incoming};
        let mut store = demo();
        let mut first = incoming("gh1", "notifications@github.com", "[Org/Repo] First issue", "Hi");
        first.from = Address::new("notifications@github.com", Some("Jadoooooo"));
        first.cc = vec![
            Address::new("subscribed@noreply.github.com", Some("Subscribed")),
            Address::new("Repo@noreply.github.com", Some("Zzowner/Zzrepo")),
        ];
        arrive(&mut store, &first, false);
        let mut second = incoming("gh2", "notifications@github.com", "[Org/Repo] Second issue", "Hi again");
        second.from = Address::new("notifications@github.com", Some("Somebody Else"));
        second.cc = first.cc.clone();
        arrive(&mut store, &second, false);

        let github = store.list_people(PeopleFilter::All, "GitHub").unwrap();
        assert_eq!(github.len(), 1);
        assert_eq!(github[0].name, "GitHub");
        assert!(store.list_people(PeopleFilter::All, "Subscribed").unwrap().is_empty());
        assert!(store.list_people(PeopleFilter::All, "Zzrepo").unwrap().is_empty());
        assert!(store.list_people(PeopleFilter::All, "Jadoooooo").unwrap().is_empty());
    }

    #[test]
    fn names_come_from_the_display_name_or_the_address() {
        assert_eq!(prettify_local_part("maya.okafor@northwind.example"), "Maya Okafor");
        assert_eq!(prettify_local_part("dan_whitaker+news@x.example"), "Dan Whitaker News");
        assert_eq!(Address::new("A@B.Example", Some("  ")).name, None);
    }
}

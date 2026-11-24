use jiff::tz::TimeZone;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use zsm_mime::{html_to_text, sanitize_html};

use super::Store;
use super::ingest::{Address, HeaderFacts, IncomingAttachment, IncomingMessage, IngestContext};
use super::outgoing::{rand_u128, reply_subject};
use crate::error::{Error, Result};

pub const MAX_EMAIL_BYTES: i64 = 25 * 1024 * 1024;
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum DraftMode {
    New,
    Reply,
    ReplyAll,
    Forward,
}

impl DraftMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Reply => "reply",
            Self::ReplyAll => "reply_all",
            Self::Forward => "forward",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "reply" => Self::Reply,
            "reply_all" => Self::ReplyAll,
            "forward" => Self::Forward,
            _ => Self::New,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DraftAttachment {
    pub id: i64,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Draft {
    pub id: i64,
    pub account_id: i64,
    pub conversation_id: Option<i64>,
    pub mode: DraftMode,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: String,
    pub body_html: String,
    pub attachments: Vec<DraftAttachment>,
    pub routine_id: Option<i64>,
    pub send_at: Option<i64>,
    pub nudge_days: Option<u32>,
    pub touched: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DraftInput {
    pub id: i64,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: String,
    pub body_html: String,
    pub nudge_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SendTicket {
    pub message_id: i64,
    pub conversation_id: i64,
    pub person_id: Option<i64>,
    pub undo_until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RecipientSuggestion {
    pub person_id: i64,
    pub name: String,
    pub email: String,
    pub ink: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ComposeSettings {
    pub signature: String,
    pub undo_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Nudge {
    pub id: i64,
    pub conversation_id: i64,
    pub person_id: Option<i64>,
    pub who: String,
    pub subject: String,
    pub sent_at: i64,
    pub due_at: i64,
}

fn normalised(list: &[Address]) -> Vec<Address> {
    list.iter().map(|a| Address::new(&a.email, a.name.as_deref())).filter(|a| !a.email.is_empty()).collect()
}

fn to_json(list: &[Address]) -> String {
    serde_json::to_string(&normalised(list)).unwrap_or_else(|_| "[]".into())
}

fn addrs(json: &str) -> Vec<Address> {
    serde_json::from_str(json).unwrap_or_default()
}

pub(crate) fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else { return false };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.contains(char::is_whitespace)
        && email.matches('@').count() == 1
}

pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

pub(crate) fn paragraphs(text: &str) -> String {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .map(|p| format!("<p>{}</p>", escape(p.trim_end()).replace('\n', "<br>")))
        .collect()
}

fn when(ms: i64, tz: &TimeZone) -> String {
    jiff::Timestamp::from_millisecond(ms)
        .map(|t| t.to_zoned_datetime(tz.clone()))
        .map(|z| z.strftime("%a, %b %-d, %Y at %H:%M").to_string())
        .unwrap_or_default()
}

fn label(a: &Address) -> String {
    match a.name.as_deref() {
        Some(name) => format!("{name} <{}>", a.email),
        None => a.email.clone(),
    }
}

fn prefixed(subject: &str, prefix: &str) -> String {
    let lower = subject.trim_start().to_lowercase();
    if lower.starts_with("fwd:") || lower.starts_with("fw:") {
        subject.to_string()
    } else {
        format!("{prefix} {subject}")
    }
}

impl Store {
    pub fn compose_settings(&self) -> Result<ComposeSettings> {
        Ok(ComposeSettings {
            signature: self.setting("compose.signature")?.unwrap_or_default(),
            undo_seconds: self.setting("compose.undo_seconds")?.and_then(|v| v.parse().ok()).unwrap_or(10).min(30),
        })
    }

    pub fn save_compose_settings(&self, settings: &ComposeSettings) -> Result<()> {
        if settings.undo_seconds > 30 {
            return Err(Error::Invalid("The undo window can be up to 30 seconds".into()));
        }
        self.set_setting("compose.signature", settings.signature.trim_end())?;
        self.set_setting("compose.undo_seconds", &settings.undo_seconds.to_string())
    }

    fn signature_html(&self) -> Result<String> {
        let signature = self.compose_settings()?.signature;
        if signature.trim().is_empty() {
            return Ok(String::new());
        }
        Ok(format!("<p>-- <br>{}</p>", escape(signature.trim()).replace('\n', "<br>")))
    }

    pub fn default_account(&self) -> Result<i64> {
        if let Some(id) = self.account_scope() {
            return Ok(id);
        }
        self.conn()
            .query_row("SELECT id FROM accounts ORDER BY id LIMIT 1", [], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Error::Invalid("Link a mailbox before writing".into()))
    }

    pub fn new_draft(&mut self, account_id: i64, to: &[Address], now: i64) -> Result<i64> {
        let signature = self.signature_html()?;
        let body = if signature.is_empty() { String::new() } else { format!("<p></p>{signature}") };
        self.insert_draft(account_id, None, DraftMode::New, to, &[], "", &body, now)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn insert_draft(
        &mut self,
        account_id: i64,
        conversation_id: Option<i64>,
        mode: DraftMode,
        to: &[Address],
        cc: &[Address],
        subject: &str,
        body_html: &str,
        now: i64,
    ) -> Result<i64> {
        self.conn().execute(
            "INSERT INTO drafts (account_id, conversation_id, mode, to_addresses, cc_addresses, subject, body_html,
               body_text, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                account_id,
                conversation_id,
                mode.as_str(),
                to_json(to),
                to_json(cc),
                subject,
                body_html,
                html_to_text(body_html),
                now
            ],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub(crate) fn keep_draft(&self, id: i64) -> Result<()> {
        self.conn().execute("UPDATE drafts SET touched = 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn purge_untouched_drafts(&mut self, before: i64) -> Result<Vec<String>> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn().prepare("SELECT id FROM drafts WHERE touched = 0 AND created_at < ?1")?;
            stmt.query_map([before], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
        };
        let mut paths = Vec::new();
        for id in ids {
            paths.extend(self.delete_draft(id)?);
        }
        Ok(paths)
    }

    pub fn prefill_draft(&mut self, conversation_id: i64, mode: DraftMode, tz: &TimeZone, now: i64) -> Result<i64> {
        if mode == DraftMode::New {
            return Err(Error::Invalid("Pick Reply, Reply all or Forward".into()));
        }
        let existing: Option<i64> = self
            .conn()
            .query_row(
                "SELECT id FROM drafts WHERE conversation_id = ?1 AND mode = ?2 AND routine_id IS NULL ORDER BY id DESC LIMIT 1",
                params![conversation_id, mode.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let (account_id, subject): (i64, String) = self
            .conn()
            .query_row("SELECT account_id, subject FROM conversations WHERE id = ?1", [conversation_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|_| Error::NotFound("conversation"))?;
        let me = self
            .self_addresses(account_id)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Invalid("No mailbox is linked yet".into()))?;
        let (last_id, from_me, from, to_json_text, cc_json_text, sent_at): (i64, bool, String, String, String, i64) =
            self.conn()
                .query_row(
                    "SELECT id, is_from_me, from_address, to_addresses, cc_addresses, sent_at FROM messages
                 WHERE conversation_id = ?1 ORDER BY sent_at DESC, id DESC LIMIT 1",
                    [conversation_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .map_err(|_| Error::NotFound("message"))?;
        let from_name: Option<String> = self
            .conn()
            .query_row(
                "SELECT p.display_name FROM person_addresses a JOIN people p ON p.id = a.person_id
                 WHERE a.account_id = ?1 AND a.address = ?2",
                params![account_id, from],
                |r| r.get(0),
            )
            .optional()?;
        let sender = Address::new(&from, from_name.as_deref());
        let last_to = addrs(&to_json_text);
        let last_cc = addrs(&cc_json_text);
        let others = |list: &[Address]| -> Vec<Address> {
            list.iter().filter(|a| a.email != me && a.email != sender.email).cloned().collect()
        };

        let (to, cc) = match mode {
            DraftMode::Reply => (if from_me { last_to.clone() } else { vec![sender.clone()] }, Vec::new()),
            DraftMode::ReplyAll => {
                let mut to = if from_me { last_to.clone() } else { vec![sender.clone()] };
                if !from_me {
                    to.extend(others(&last_to));
                }
                let cc: Vec<Address> =
                    others(&last_cc).into_iter().filter(|a| !to.iter().any(|t| t.email == a.email)).collect();
                (to, cc)
            }
            _ => (Vec::new(), Vec::new()),
        };

        let original_text = self.message_text(last_id)?.unwrap_or_default();
        let signature = self.signature_html()?;
        let quote = paragraphs(original_text.trim());
        let body = match mode {
            DraftMode::Forward => {
                let mut header = format!(
                    "<p>---------- Forwarded message ----------<br>From: {}<br>Date: {}<br>Subject: {}<br>To: {}",
                    escape(&label(&sender)),
                    escape(&when(sent_at, tz)),
                    escape(&subject),
                    escape(&last_to.iter().map(label).collect::<Vec<_>>().join(", "))
                );
                if !last_cc.is_empty() {
                    header.push_str(&format!(
                        "<br>Cc: {}",
                        escape(&last_cc.iter().map(label).collect::<Vec<_>>().join(", "))
                    ));
                }
                header.push_str("</p>");
                format!("<p></p>{signature}{header}{quote}")
            }
            _ => format!(
                "<p></p>{signature}<p>On {}, {} wrote:</p><blockquote>{quote}</blockquote>",
                escape(&when(sent_at, tz)),
                escape(&label(&sender))
            ),
        };
        let subject = match mode {
            DraftMode::Forward => prefixed(&subject, "Fwd:"),
            _ => reply_subject(&subject),
        };
        self.insert_draft(account_id, Some(conversation_id), mode, &to, &cc, &subject, &body, now)
    }

    pub fn save_draft(&mut self, input: &DraftInput, now: i64) -> Result<i64> {
        let changed = self.conn().execute(
            "UPDATE drafts SET to_addresses = ?2, cc_addresses = ?3, bcc_addresses = ?4, subject = ?5, body_html = ?6,
               body_text = ?7, nudge_days = ?8, touched = 1, updated_at = ?9 WHERE id = ?1",
            params![
                input.id,
                to_json(&input.to),
                to_json(&input.cc),
                to_json(&input.bcc),
                input.subject,
                input.body_html,
                html_to_text(&input.body_html),
                input.nudge_days,
                now
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("draft"));
        }
        Ok(input.id)
    }

    fn draft_attachments(&self, draft_id: i64) -> Result<Vec<DraftAttachment>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT id, filename, mime_type, size_bytes FROM draft_attachments WHERE draft_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([draft_id], |r| {
            Ok(DraftAttachment { id: r.get(0)?, filename: r.get(1)?, mime_type: r.get(2)?, size: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_draft(&self, id: i64) -> Result<Option<Draft>> {
        let row = self
            .conn()
            .query_row(
                "SELECT id, account_id, conversation_id, mode, to_addresses, cc_addresses, bcc_addresses, subject,
                        body_html, routine_id, send_at, nudge_days, updated_at, touched FROM drafts WHERE id = ?1",
                [id],
                |r| {
                    Ok(Draft {
                        id: r.get(0)?,
                        account_id: r.get(1)?,
                        conversation_id: r.get(2)?,
                        mode: DraftMode::parse(&r.get::<_, String>(3)?),
                        to: addrs(&r.get::<_, String>(4)?),
                        cc: addrs(&r.get::<_, String>(5)?),
                        bcc: addrs(&r.get::<_, String>(6)?),
                        subject: r.get(7)?,
                        body_html: r.get(8)?,
                        attachments: Vec::new(),
                        routine_id: r.get(9)?,
                        send_at: r.get(10)?,
                        nudge_days: r.get::<_, Option<i64>>(11)?.and_then(|d| u32::try_from(d).ok()),
                        updated_at: r.get(12)?,
                        touched: r.get(13)?,
                    })
                },
            )
            .optional()?;
        match row {
            Some(mut draft) => {
                draft.attachments = self.draft_attachments(id)?;
                Ok(Some(draft))
            }
            None => Ok(None),
        }
    }

    pub fn list_drafts(&self) -> Result<Vec<Draft>> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT d.id FROM drafts d WHERE d.touched = 1{} ORDER BY d.updated_at DESC, d.id DESC",
                self.in_account("d")
            ))?;
            stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
        };
        let mut out = Vec::new();
        for id in ids {
            if let Some(draft) = self.get_draft(id)? {
                out.push(draft);
            }
        }
        Ok(out)
    }

    pub fn delete_draft(&mut self, id: i64) -> Result<Vec<String>> {
        let paths: Vec<String> = {
            let mut stmt = self.conn().prepare("SELECT path FROM draft_attachments WHERE draft_id = ?1")?;
            stmt.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
        };
        if self.conn().execute("DELETE FROM drafts WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("draft"));
        }
        Ok(paths)
    }

    pub fn add_draft_attachment(
        &mut self,
        draft_id: i64,
        filename: &str,
        mime: &str,
        size: i64,
        path: &str,
    ) -> Result<DraftAttachment> {
        self.conn()
            .query_row("SELECT 1 FROM drafts WHERE id = ?1", [draft_id], |_| Ok(()))
            .optional()?
            .ok_or(Error::NotFound("draft"))?;
        let used: i64 = self.conn().query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM draft_attachments WHERE draft_id = ?1",
            [draft_id],
            |r| r.get(0),
        )?;
        if used + size > MAX_EMAIL_BYTES {
            return Err(Error::Invalid("Gmail accepts up to 25 MB of files in one email".into()));
        }
        self.conn().execute(
            "INSERT INTO draft_attachments (draft_id, filename, mime_type, size_bytes, path) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![draft_id, filename, mime, size, path],
        )?;
        let id = self.conn().last_insert_rowid();
        self.conn().execute("UPDATE drafts SET touched = 1 WHERE id = ?1", [draft_id])?;
        Ok(DraftAttachment { id, filename: filename.into(), mime_type: mime.into(), size })
    }

    pub fn remove_draft_attachment(&mut self, id: i64) -> Result<String> {
        let path: String = self
            .conn()
            .query_row("SELECT path FROM draft_attachments WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .ok_or(Error::NotFound("file"))?;
        self.conn().execute("DELETE FROM draft_attachments WHERE id = ?1", [id])?;
        Ok(path)
    }

    pub fn schedule_draft(&mut self, id: i64, send_at: Option<i64>, now: i64) -> Result<()> {
        if let Some(at) = send_at {
            if at <= now {
                return Err(Error::Invalid("Pick a time that has not passed yet".into()));
            }
            self.check_sendable(id)?;
        }
        if self.conn().execute("UPDATE drafts SET send_at = ?2 WHERE id = ?1", params![id, send_at])? == 0 {
            return Err(Error::NotFound("draft"));
        }
        Ok(())
    }

    pub fn due_drafts(&self, now: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn()
            .prepare_cached("SELECT id FROM drafts WHERE send_at IS NOT NULL AND send_at <= ?1 ORDER BY send_at, id")?;
        Ok(stmt.query_map([now], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
    }

    fn check_sendable(&self, id: i64) -> Result<Draft> {
        let draft = self.get_draft(id)?.ok_or(Error::NotFound("draft"))?;
        let recipients: Vec<&Address> = draft.to.iter().chain(draft.cc.iter()).chain(draft.bcc.iter()).collect();
        if recipients.is_empty() {
            return Err(Error::Invalid("Add someone to send this to".into()));
        }
        if let Some(bad) = recipients.iter().find(|a| !valid_email(&a.email)) {
            return Err(Error::Invalid(format!("“{}” is not an email address", bad.email)));
        }
        if html_to_text(&draft.body_html).trim().is_empty() && draft.attachments.is_empty() {
            return Err(Error::Invalid("Write something first".into()));
        }
        Ok(draft)
    }

    pub fn send_draft(&mut self, draft_id: i64, now: i64, undo_ms: i64) -> Result<SendTicket> {
        let draft = self.check_sendable(draft_id)?;
        let account_id = draft.account_id;
        let me = self
            .self_addresses(account_id)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Invalid("No mailbox is linked yet".into()))?;
        let provider: String =
            self.conn().query_row("SELECT provider FROM accounts WHERE id = ?1", [account_id], |r| r.get(0))?;

        let html = sanitize_html(&draft.body_html).html;
        let text = html_to_text(&html);

        let reply_to = match draft.mode {
            DraftMode::Reply | DraftMode::ReplyAll => draft.conversation_id,
            _ => None,
        };
        let (thread_id, in_reply_to) = match reply_to {
            Some(cid) => {
                let thread: String = self
                    .conn()
                    .query_row("SELECT provider_id FROM conversations WHERE id = ?1", [cid], |r| r.get(0))
                    .map_err(|_| Error::NotFound("conversation"))?;
                let last_rfc: Option<String> = self
                    .conn()
                    .query_row(
                        "SELECT rfc_message_id FROM messages WHERE conversation_id = ?1 AND rfc_message_id IS NOT NULL
                         ORDER BY sent_at DESC, id DESC LIMIT 1",
                        [cid],
                        |r| r.get(0),
                    )
                    .optional()?;
                (thread, last_rfc)
            }
            None => (format!("local:{:032x}", rand_u128()), None),
        };

        let unique = format!("{:032x}", rand_u128());
        let msg = IncomingMessage {
            provider_id: format!("local:{unique}"),
            thread_id,
            rfc_message_id: Some(format!("<{unique}@zsortmail.local>")),
            in_reply_to,
            from: Address::new(&me, None),
            to: normalised(&draft.to),
            cc: normalised(&draft.cc),
            bcc: normalised(&draft.bcc),
            subject: draft.subject.trim().to_string(),
            sent_at: now,
            snippet: None,
            text: Some(text),
            html: Some(html),
            attachments: draft
                .attachments
                .iter()
                .map(|a| IncomingAttachment {
                    provider_id: None,
                    filename: a.filename.clone(),
                    mime: a.mime_type.clone(),
                    size: a.size,
                    content_id: None,
                    inline: false,
                })
                .collect(),
            labels: vec!["SENT".into()],
            unread: false,
            starred: false,
            headers: HeaderFacts::default(),
            size_bytes: None,
        };
        let ctx = IngestContext { account_id, self_addresses: vec![me], screener: false, automation: false };
        let out = self.ingest(&ctx, &msg, now)?;

        self.in_transaction(|tx| {
            let ids: Vec<i64> = {
                let mut stmt = tx.prepare("SELECT id FROM attachments WHERE message_id = ?1 ORDER BY id")?;
                stmt.query_map([out.message_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
            };
            for (attachment_id, file) in ids.iter().zip(&draft.attachments) {
                let path: Option<String> = tx
                    .query_row("SELECT path FROM draft_attachments WHERE id = ?1", [file.id], |r| r.get(0))
                    .optional()?;
                tx.execute("UPDATE attachments SET cache_path = ?1 WHERE id = ?2", params![path, attachment_id])?;
            }
            if provider != "demo" {
                let bcc: Vec<String> = normalised(&draft.bcc).into_iter().map(|a| a.email).collect();
                tx.execute(
                    "INSERT INTO outbox (account_id, op, payload, next_attempt_at, created_at)
                     VALUES (?1, 'send_message', ?2, ?3, ?4)",
                    params![
                        account_id,
                        serde_json::json!({ "message_id": out.message_id, "bcc": bcc }).to_string(),
                        now + undo_ms,
                        now
                    ],
                )?;
            }
            if let Some(days) = draft.nudge_days {
                tx.execute(
                    "INSERT INTO nudges (message_id, conversation_id, due_at, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![out.message_id, out.conversation_id, now + i64::from(days) * DAY_MS, now],
                )?;
            }
            tx.execute("DELETE FROM draft_attachments WHERE draft_id = ?1", [draft_id])?;
            tx.execute("DELETE FROM drafts WHERE id = ?1", [draft_id])?;
            Ok(())
        })?;
        let person_id = draft.to.first().and_then(|a| {
            self.conn()
                .query_row(
                    "SELECT person_id FROM person_addresses WHERE account_id = ?1 AND address = ?2",
                    params![account_id, a.email.to_lowercase()],
                    |r| r.get(0),
                )
                .ok()
        });
        Ok(SendTicket {
            message_id: out.message_id,
            conversation_id: out.conversation_id,
            person_id,
            undo_until: now + undo_ms,
        })
    }

    pub fn unsend(&mut self, message_id: i64, now: i64) -> Result<i64> {
        type Row = (i64, i64, String, String, String, String, bool, i64);
        let (account_id, conversation_id, provider_id, subject, to, cc, is_from_me, sent_at): Row = self
            .conn()
            .query_row(
                "SELECT account_id, conversation_id, provider_id, subject, to_addresses, cc_addresses, is_from_me, sent_at
                 FROM messages WHERE id = ?1",
                [message_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
            )
            .map_err(|_| Error::NotFound("message"))?;
        if !is_from_me || !provider_id.starts_with("local:") {
            return Err(Error::Invalid("It has already been sent".into()));
        }
        let provider: String =
            self.conn().query_row("SELECT provider FROM accounts WHERE id = ?1", [account_id], |r| r.get(0))?;
        let mut bcc: Vec<Address> = Vec::new();
        if provider != "demo" {
            let row: Option<(i64, String, i64)> = self
                .conn()
                .query_row(
                    "SELECT id, payload, attempts FROM outbox WHERE op = 'send_message'
                       AND json_extract(payload, '$.message_id') = ?1",
                    [message_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((outbox_id, payload, attempts)) = row else {
                return Err(Error::Invalid("It has already been sent".into()));
            };
            if attempts > 0 {
                return Err(Error::Invalid("It is already on its way".into()));
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&payload) {
                bcc = v["bcc"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|e| e.as_str()).map(|e| Address::new(e, None)).collect())
                    .unwrap_or_default();
            }
            self.conn().execute("DELETE FROM outbox WHERE id = ?1", [outbox_id])?;
        }

        let html = self.message_html(message_id)?.filter(|h| !h.trim().is_empty());
        let body_html = match html {
            Some(html) => html,
            None => paragraphs(&self.message_text(message_id)?.unwrap_or_default()),
        };
        let files: Vec<(String, String, i64, Option<String>)> = {
            let mut stmt = self.conn().prepare(
                "SELECT filename, mime_type, size_bytes, cache_path FROM attachments WHERE message_id = ?1 AND inline = 0 ORDER BY id",
            )?;
            stmt.query_map([message_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<std::result::Result<_, _>>()?
        };
        let nudge_days: Option<u32> = self
            .conn()
            .query_row("SELECT due_at FROM nudges WHERE message_id = ?1", [message_id], |r| r.get::<_, i64>(0))
            .optional()?
            .and_then(|due| u32::try_from(((due - sent_at) / DAY_MS).max(1)).ok());
        let was_reply: bool = self.conn().query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |r| r.get::<_, i64>(0),
        )? > 1;
        let people: Vec<i64> = {
            let mut stmt =
                self.conn().prepare("SELECT person_id FROM conversation_people WHERE conversation_id = ?1")?;
            stmt.query_map([conversation_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
        };

        let draft_id = self.insert_draft(
            account_id,
            was_reply.then_some(conversation_id),
            if was_reply { DraftMode::Reply } else { DraftMode::New },
            &addrs(&to),
            &addrs(&cc),
            &subject,
            &body_html,
            now,
        )?;
        self.keep_draft(draft_id)?;
        if !bcc.is_empty() || nudge_days.is_some() {
            self.conn().execute(
                "UPDATE drafts SET bcc_addresses = ?2, nudge_days = ?3 WHERE id = ?1",
                params![draft_id, to_json(&bcc), nudge_days],
            )?;
        }
        for (filename, mime, size, path) in files {
            if let Some(path) = path {
                self.add_draft_attachment(draft_id, &filename, &mime, size, &path)?;
            }
        }
        self.remove_messages(account_id, &[provider_id])?;
        self.in_transaction(|tx| {
            for person in people {
                let left: i64 =
                    tx.query_row("SELECT COUNT(*) FROM conversation_people WHERE person_id = ?1", [person], |r| {
                        r.get(0)
                    })?;
                let fresh: bool = tx
                    .query_row(
                        "SELECT kind <> 'self' AND created_at >= ?2 FROM people WHERE id = ?1",
                        params![person, sent_at - 60_000],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if left == 0 && fresh {
                    tx.execute("DELETE FROM people_fts WHERE rowid = ?1", [person])?;
                    tx.execute("DELETE FROM people WHERE id = ?1", [person])?;
                }
            }
            Ok(())
        })?;
        Ok(draft_id)
    }

    pub fn suggest_recipients(&self, query: &str, limit: usize) -> Result<Vec<RecipientSuggestion>> {
        let clean: String = query.trim().chars().filter(|c| !matches!(c, '%' | '_' | '\\')).collect();
        let mut stmt = self.conn().prepare(&format!(
            "SELECT p.id, p.display_name, a.address, p.ink
             FROM person_addresses a JOIN people p ON p.id = a.person_id
             WHERE p.status = 'known' AND p.kind = 'person' AND p.hidden_at IS NULL{}
               AND (zfold(p.display_name) LIKE ?1 OR zfold(a.address) LIKE ?1)
             ORDER BY (zfold(p.display_name) LIKE ?2 OR zfold(a.address) LIKE ?2) DESC,
                      p.pinned DESC, a.is_primary DESC, p.last_message_at DESC
             LIMIT ?3",
            self.in_account("p")
        ))?;
        let folded = crate::text::fold(&clean);
        let wanted = limit.max(1);
        let rows = stmt.query_map(
            params![format!("%{folded}%"), format!("{folded}%"), i64::try_from(wanted * 8).unwrap_or(64)],
            |r| {
                Ok(RecipientSuggestion {
                    person_id: r.get(0)?,
                    name: r.get(1)?,
                    email: r.get(2)?,
                    ink: u8::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
                })
            },
        )?;
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for row in rows {
            let row = row?;
            if seen.insert(row.email.to_lowercase()) {
                out.push(row);
                if out.len() == wanted {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub fn resolve_nudges(&mut self, now: i64) -> Result<Vec<Nudge>> {
        let waiting = self.waiting_nudges()?;
        let mut fired = Vec::new();
        for nudge in waiting {
            let replied: bool = self.conn().query_row(
                "SELECT EXISTS (SELECT 1 FROM messages WHERE conversation_id = ?1 AND is_from_me = 0 AND sent_at > ?2)",
                params![nudge.conversation_id, nudge.sent_at],
                |r| r.get(0),
            )?;
            if replied {
                self.conn().execute("UPDATE nudges SET state = 'replied' WHERE id = ?1", [nudge.id])?;
            } else if nudge.due_at <= now {
                self.conn().execute("UPDATE nudges SET state = 'fired' WHERE id = ?1", [nudge.id])?;
                fired.push(nudge);
            }
        }
        Ok(fired)
    }

    pub fn waiting_nudges(&self) -> Result<Vec<Nudge>> {
        struct Raw {
            id: i64,
            conversation_id: i64,
            subject: String,
            to: String,
            sent_at: i64,
            due_at: i64,
            account_id: i64,
        }
        let raws: Vec<Raw> = {
            let mut stmt = self.conn().prepare_cached(
                "SELECT n.id, n.conversation_id, m.subject, m.to_addresses, m.sent_at, n.due_at, m.account_id
                 FROM nudges n JOIN messages m ON m.id = n.message_id WHERE n.state = 'waiting' ORDER BY n.due_at, n.id",
            )?;
            stmt.query_map([], |r| {
                Ok(Raw {
                    id: r.get(0)?,
                    conversation_id: r.get(1)?,
                    subject: r.get(2)?,
                    to: r.get(3)?,
                    sent_at: r.get(4)?,
                    due_at: r.get(5)?,
                    account_id: r.get(6)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?
        };
        let mut out = Vec::new();
        for raw in raws {
            let to = addrs(&raw.to);
            let first = to.first();
            let name: Option<String> = match first {
                Some(a) => self
                    .conn()
                    .query_row(
                        "SELECT p.display_name FROM person_addresses a JOIN people p ON p.id = a.person_id
                         WHERE a.account_id = ?1 AND a.address = ?2",
                        params![raw.account_id, a.email],
                        |r| r.get(0),
                    )
                    .optional()?,
                None => None,
            };
            let person_id: Option<i64> = match first {
                Some(a) => self
                    .conn()
                    .query_row(
                        "SELECT person_id FROM person_addresses WHERE account_id = ?1 AND address = ?2",
                        params![raw.account_id, a.email],
                        |r| r.get(0),
                    )
                    .optional()?,
                None => None,
            };
            let first_name = name
                .or_else(|| first.and_then(|a| a.name.clone()))
                .or_else(|| first.map(|a| a.email.clone()))
                .unwrap_or_default();
            let who = match to.len() {
                0 | 1 => first_name,
                2 => format!("{first_name} and 1 other"),
                n => format!("{first_name} and {} others", n - 1),
            };
            out.push(Nudge {
                id: raw.id,
                conversation_id: raw.conversation_id,
                person_id,
                who,
                subject: raw.subject,
                sent_at: raw.sent_at,
                due_at: raw.due_at,
            });
        }
        Ok(out)
    }

    pub fn cancel_nudge(&mut self, id: i64) -> Result<()> {
        if self.conn().execute("UPDATE nudges SET state = 'cancelled' WHERE id = ?1 AND state = 'waiting'", [id])? == 0
        {
            return Err(Error::NotFound("reminder"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::PeopleFilter;
    use crate::store::rules::tests::{NOW, arrive, demo, incoming};

    fn a(email: &str) -> Address {
        Address::new(email, None)
    }

    fn start(store: &mut Store, to: &[&str], subject: &str, html: &str) -> i64 {
        let id = store.new_draft(1, &to.iter().map(|e| a(e)).collect::<Vec<_>>(), NOW).unwrap();
        store
            .save_draft(
                &DraftInput {
                    id,
                    to: to.iter().map(|e| a(e)).collect(),
                    cc: vec![],
                    bcc: vec![],
                    subject: subject.into(),
                    body_html: html.into(),
                    nudge_days: None,
                },
                NOW,
            )
            .unwrap();
        id
    }

    fn real_account() -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.upsert_account("gmail", "me@example.com", None, NOW).unwrap();
        let mut first = incoming("g1", "friend@example.com", "Plans", "Saturday?");
        first.to = vec![a("me@example.com")];
        arrive(&mut store, &first, false);
        (store, account)
    }

    #[test]
    fn drafts_save_as_they_are_typed_and_come_back_whole() {
        let mut store = demo();
        let id = start(&mut store, &["half-typed"], "Lunch", "<p>Are you free <b>Thursday</b>?</p>");
        let draft = store.get_draft(id).unwrap().unwrap();
        assert_eq!(draft.to[0].email, "half-typed", "a half-written address is kept as typed");
        store
            .save_draft(
                &DraftInput {
                    id,
                    to: vec![Address::new("Maya.Okafor@northwind.example ", Some("Maya"))],
                    cc: vec![],
                    bcc: vec![a("boss@example.com")],
                    subject: "Lunch Thursday".into(),
                    body_html: "<p>Free Thursday?</p>".into(),
                    nudge_days: Some(3),
                },
                NOW + 5,
            )
            .unwrap();
        let draft = store.get_draft(id).unwrap().unwrap();
        assert_eq!(draft.subject, "Lunch Thursday");
        assert_eq!(
            (draft.to[0].email.as_str(), draft.to[0].name.as_deref()),
            ("maya.okafor@northwind.example", Some("Maya"))
        );
        assert_eq!((draft.bcc.len(), draft.nudge_days, draft.updated_at), (1, Some(3), NOW + 5));
        assert_eq!(store.list_drafts().unwrap().len(), 1);
        let missing = DraftInput {
            id: 999,
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: String::new(),
            body_html: String::new(),
            nudge_days: None,
        };
        assert!(store.save_draft(&missing, NOW).is_err());
        store.delete_draft(id).unwrap();
        assert!(store.get_draft(id).unwrap().is_none() && store.delete_draft(id).is_err());
    }

    #[test]
    fn a_signature_is_added_to_new_drafts() {
        let store = demo();
        assert_eq!(store.compose_settings().unwrap(), ComposeSettings { signature: String::new(), undo_seconds: 10 });
        store
            .save_compose_settings(&ComposeSettings { signature: "Zo\nZSortMail <3".into(), undo_seconds: 5 })
            .unwrap();
        assert_eq!(store.signature_html().unwrap(), "<p>-- <br>Zo<br>ZSortMail &lt;3</p>");
        let mut store = store;
        let id = store.new_draft(1, &[], NOW).unwrap();
        assert!(store.get_draft(id).unwrap().unwrap().body_html.contains("ZSortMail &lt;3"));
        assert!(store.save_compose_settings(&ComposeSettings { signature: String::new(), undo_seconds: 99 }).is_err());
    }

    #[test]
    fn sending_checks_the_draft_and_stores_the_message_in_a_conversation_of_its_own() {
        let mut store = demo();
        let empty = start(&mut store, &[], "Hi", "<p>Hello</p>");
        assert_eq!(store.send_draft(empty, NOW, 10_000).unwrap_err().to_string(), "Add someone to send this to");
        let bad = start(&mut store, &["not an address"], "Hi", "<p>Hello</p>");
        assert!(store.send_draft(bad, NOW, 10_000).unwrap_err().to_string().contains("is not an email address"));
        let blank = start(&mut store, &["a@b.example"], "Hi", "<p>  </p>");
        assert_eq!(store.send_draft(blank, NOW, 10_000).unwrap_err().to_string(), "Write something first");

        let ok = start(
            &mut store,
            &["new.person@elsewhere.example"],
            "Hello there",
            "<p>Nice to <b>meet</b> you<script>alert(1)</script></p>",
        );
        let ticket = store.send_draft(ok, NOW, 10_000).unwrap();
        assert_eq!(ticket.undo_until, NOW + 10_000);
        assert!(store.get_draft(ok).unwrap().is_none(), "the draft became a message");

        let person = store.list_people(PeopleFilter::All, "new.person").unwrap();
        assert_eq!(person.len(), 1);
        assert_eq!(ticket.person_id, Some(person[0].id));
        let convo = store.list_conversations(person[0].id).unwrap().remove(0);
        assert_eq!(convo.subject, "Hello there");
        let detail = store.get_conversation(convo.id, Some(person[0].id)).unwrap().unwrap();
        assert!(detail.messages[0].from.is_me);
        assert_eq!(detail.messages[0].text, "Nice to meet you");
        let html = store.message_html(ticket.message_id).unwrap().unwrap();
        assert!(html.contains("<b>meet</b>") && !html.contains("script"), "what is sent is the sanitized form: {html}");
    }

    #[test]
    fn a_sent_message_remembers_who_it_was_blind_copied_to() {
        let mut store = demo();
        let id = store.new_draft(1, &[a("to.person@elsewhere.example")], NOW).unwrap();
        store
            .save_draft(
                &DraftInput {
                    id,
                    to: vec![a("to.person@elsewhere.example")],
                    cc: vec![],
                    bcc: vec![a("Quiet@Elsewhere.example")],
                    subject: "Quietly".into(),
                    body_html: "<p>Hello</p>".into(),
                    nudge_days: None,
                },
                NOW,
            )
            .unwrap();
        let ticket = store.send_draft(id, NOW, 10_000).unwrap();
        let person = store.list_people(PeopleFilter::All, "to.person").unwrap().remove(0);
        let convo = store.list_conversations(person.id).unwrap().remove(0);
        let detail = store.get_conversation(convo.id, Some(person.id)).unwrap().unwrap();
        let sent = detail.messages.iter().find(|m| m.id == ticket.message_id).unwrap();
        assert_eq!(sent.to.len(), 1);
        assert_eq!(sent.bcc.iter().map(|b| b.email.as_str()).collect::<Vec<_>>(), ["quiet@elsewhere.example"]);
    }

    #[test]
    fn a_reply_goes_into_its_conversation_with_the_subject_and_threading_of_a_reply() {
        let mut store = demo();
        let maya = store.list_people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        let convo =
            store.list_conversations(maya).unwrap().into_iter().find(|c| c.subject == "Coffee Thursday?").unwrap();
        let before = convo.message_count;
        let id = store.prefill_draft(convo.id, DraftMode::Reply, &TimeZone::UTC, NOW).unwrap();
        let draft = store.get_draft(id).unwrap().unwrap();
        assert_eq!(draft.subject, "Re: Coffee Thursday?");
        assert_eq!(draft.to[0].email, "maya.okafor@northwind.example");
        assert!(draft.body_html.contains("<blockquote>") && draft.body_html.contains("wrote:"), "{}", draft.body_html);
        assert_eq!(
            store.prefill_draft(convo.id, DraftMode::Reply, &TimeZone::UTC, NOW).unwrap(),
            id,
            "asking again resumes it"
        );

        store
            .save_draft(
                &DraftInput {
                    id,
                    to: draft.to.clone(),
                    cc: vec![],
                    bcc: vec![],
                    subject: draft.subject.clone(),
                    body_html: "<p>9:30 is perfect.</p>".into(),
                    nudge_days: None,
                },
                NOW,
            )
            .unwrap();
        let ticket = store.send_draft(id, NOW, 0).unwrap();
        assert_eq!(ticket.conversation_id, convo.id);
        let detail = store.get_conversation(convo.id, Some(maya)).unwrap().unwrap();
        assert_eq!(detail.messages.len(), before as usize + 1);
        let loaded = store.load_outgoing(ticket.message_id).unwrap();
        assert_eq!(loaded.subject, "Re: Coffee Thursday?");
        assert!(loaded.in_reply_to.is_some(), "it answers the newest message");
        assert!(loaded.html.as_deref().is_some_and(|h| h.contains("9:30 is perfect.")));
    }

    #[test]
    fn reply_all_keeps_everyone_but_you_and_forward_starts_with_nobody() {
        let mut store = demo();
        let mut group = incoming("grp1", "alice@team.example", "Offsite plan", "Thursday works.\n\nAlice");
        group.to = vec![a("you@zsortmail.demo"), a("bob@team.example")];
        group.cc = vec![a("carol@team.example"), a("you@zsortmail.demo")];
        let out = arrive(&mut store, &group, false);

        let all = store.prefill_draft(out.conversation_id, DraftMode::ReplyAll, &TimeZone::UTC, NOW).unwrap();
        let draft = store.get_draft(all).unwrap().unwrap();
        let emails = |l: &[Address]| l.iter().map(|x| x.email.clone()).collect::<Vec<_>>();
        assert_eq!(emails(&draft.to), ["alice@team.example", "bob@team.example"]);
        assert_eq!(emails(&draft.cc), ["carol@team.example"]);

        let one = store.prefill_draft(out.conversation_id, DraftMode::Reply, &TimeZone::UTC, NOW).unwrap();
        assert_eq!(emails(&store.get_draft(one).unwrap().unwrap().to), ["alice@team.example"]);

        let fwd = store.prefill_draft(out.conversation_id, DraftMode::Forward, &TimeZone::UTC, NOW).unwrap();
        let draft = store.get_draft(fwd).unwrap().unwrap();
        assert_eq!(draft.subject, "Fwd: Offsite plan");
        assert!(draft.to.is_empty() && draft.cc.is_empty());
        assert!(
            draft.body_html.contains("Forwarded message") && draft.body_html.contains("Thursday works."),
            "{}",
            draft.body_html
        );
        assert!(draft.body_html.contains("alice@team.example"), "the original sender is named");
        assert!(store.prefill_draft(out.conversation_id, DraftMode::New, &TimeZone::UTC, NOW).is_err());
    }

    #[test]
    fn a_linked_account_queues_the_send_after_the_undo_window_and_unsend_takes_it_back() {
        let (mut store, account) = real_account();
        let before: i64 =
            store.conn().query_row("SELECT COUNT(*) FROM people WHERE kind <> 'self'", [], |r| r.get(0)).unwrap();
        let id = store.new_draft(account, &[], NOW).unwrap();
        store
            .save_draft(
                &DraftInput {
                    id,
                    to: vec![a("stranger@example.com")],
                    cc: vec![],
                    bcc: vec![a("boss@example.com")],
                    subject: "Intro".into(),
                    body_html: "<p>Hello, I am new.</p>".into(),
                    nudge_days: Some(2),
                },
                NOW,
            )
            .unwrap();
        let ticket = store.send_draft(id, NOW, 10_000).unwrap();
        assert!(store.outbox_peek(NOW + 5_000, 10).unwrap().is_empty(), "not before the window has passed");
        let due = store.outbox_peek(NOW + 10_000, 10).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].payload["bcc"], serde_json::json!(["boss@example.com"]));
        assert_eq!(store.waiting_nudges().unwrap().len(), 1);

        let back = store.unsend(ticket.message_id, NOW + 3_000).unwrap();
        assert!(store.outbox_peek(NOW + 60_000, 10).unwrap().is_empty());
        assert!(store.waiting_nudges().unwrap().is_empty());
        let draft = store.get_draft(back).unwrap().unwrap();
        assert_eq!(
            (draft.subject.as_str(), draft.to[0].email.as_str(), draft.bcc[0].email.as_str()),
            ("Intro", "stranger@example.com", "boss@example.com")
        );
        assert_eq!(draft.nudge_days, Some(2), "the reminder comes back with the draft");
        assert!(draft.body_html.contains("Hello, I am new."));
        let after: i64 =
            store.conn().query_row("SELECT COUNT(*) FROM people WHERE kind <> 'self'", [], |r| r.get(0)).unwrap();
        assert_eq!(after, before, "nobody is left behind");

        let again = store.send_draft(back, NOW + 4_000, 0).unwrap();
        store.conn().execute("UPDATE outbox SET attempts = 1", []).unwrap();
        assert!(store.unsend(again.message_id, NOW + 5_000).unwrap_err().to_string().contains("on its way"));
        store.conn().execute("DELETE FROM outbox", []).unwrap();
        assert!(store.unsend(again.message_id, NOW + 5_000).unwrap_err().to_string().contains("already been sent"));
    }

    #[test]
    fn the_demo_mailbox_can_always_take_a_message_back() {
        let mut store = demo();
        let id = start(&mut store, &["someone@elsewhere.example"], "Test", "<p>Body</p>");
        let ticket = store.send_draft(id, NOW, 10_000).unwrap();
        let back = store.unsend(ticket.message_id, NOW + 1).unwrap();
        assert!(store.get_draft(back).unwrap().is_some());
        assert!(store.list_people(PeopleFilter::All, "elsewhere").unwrap().is_empty());
    }

    #[test]
    fn files_travel_from_the_draft_to_the_sent_message_and_back() {
        let mut store = demo();
        let id = start(&mut store, &["files@elsewhere.example"], "With a file", "<p>See attached</p>");
        let added =
            store.add_draft_attachment(id, "plan.pdf", "application/pdf", 1234, "/tmp/zsm-test/plan.pdf").unwrap();
        assert_eq!(store.get_draft(id).unwrap().unwrap().attachments, vec![added]);
        let ticket = store.send_draft(id, NOW, 10_000).unwrap();
        let record: (String, i64) = store
            .conn()
            .query_row(
                "SELECT cache_path, size_bytes FROM attachments WHERE message_id = ?1",
                [ticket.message_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(record, ("/tmp/zsm-test/plan.pdf".to_string(), 1234));
        let loaded = store.load_outgoing(ticket.message_id).unwrap();
        assert_eq!(
            loaded.files,
            vec![crate::store::outgoing::OutgoingFile {
                filename: "plan.pdf".into(),
                mime: "application/pdf".into(),
                path: "/tmp/zsm-test/plan.pdf".into()
            }]
        );
        let back = store.unsend(ticket.message_id, NOW + 1).unwrap();
        assert_eq!(store.get_draft(back).unwrap().unwrap().attachments[0].filename, "plan.pdf");

        let only = start(&mut store, &["files@elsewhere.example"], "", "");
        store.add_draft_attachment(only, "a.txt", "text/plain", 3, "/tmp/zsm-test/a.txt").unwrap();
        assert!(store.send_draft(only, NOW, 0).is_ok(), "a file alone is a message");
        let more = start(&mut store, &["files@elsewhere.example"], "", "");
        assert!(
            store
                .add_draft_attachment(
                    more,
                    "big.bin",
                    "application/octet-stream",
                    MAX_EMAIL_BYTES + 1,
                    "/tmp/zsm-test/big.bin"
                )
                .unwrap_err()
                .to_string()
                .contains("25 MB")
        );
        let g = store.add_draft_attachment(more, "b.txt", "text/plain", 3, "/tmp/zsm-test/b.txt").unwrap();
        assert_eq!(store.remove_draft_attachment(g.id).unwrap(), "/tmp/zsm-test/b.txt");
        assert!(store.remove_draft_attachment(g.id).is_err());
    }

    #[test]
    fn a_draft_can_be_scheduled_and_comes_due() {
        let mut store = demo();
        let id = start(&mut store, &["later@elsewhere.example"], "Later", "<p>Tomorrow</p>");
        assert!(store.schedule_draft(id, Some(NOW - 1), NOW).unwrap_err().to_string().contains("has not passed"));
        let empty = start(&mut store, &[], "x", "<p>x</p>");
        assert!(store.schedule_draft(empty, Some(NOW + 1000), NOW).is_err(), "only a sendable draft can be scheduled");
        store.schedule_draft(id, Some(NOW + 60_000), NOW).unwrap();
        assert_eq!(store.get_draft(id).unwrap().unwrap().send_at, Some(NOW + 60_000));
        assert!(store.due_drafts(NOW + 59_999).unwrap().is_empty());
        assert_eq!(store.due_drafts(NOW + 60_000).unwrap(), vec![id]);
        store.schedule_draft(id, None, NOW).unwrap();
        assert!(store.due_drafts(NOW + 120_000).unwrap().is_empty());
    }

    #[test]
    fn recipients_are_suggested_by_name_or_address_with_prefix_matches_first() {
        let store = demo();
        let found = store.suggest_recipients("maya", 8).unwrap();
        assert_eq!(found[0].name, "Maya Okafor");
        assert!(found[0].email.contains('@'));
        assert!(
            store.suggest_recipients("OKAFOR", 8).unwrap().iter().any(|s| s.name == "Maya Okafor"),
            "any part of the name, any case"
        );
        assert!(
            store.suggest_recipients("amelie", 8).unwrap().iter().any(|s| s.name.starts_with("Amélie")),
            "accents do not matter"
        );
        assert!(store.suggest_recipients("zzzz-nobody", 8).unwrap().is_empty());
        assert!(store.suggest_recipients("100%_\\", 8).unwrap().is_empty(), "wildcards are not special");
        assert_eq!(store.suggest_recipients("", 3).unwrap().len(), 3, "nothing typed offers recent people");
        assert!(
            !store.suggest_recipients("", 50).unwrap().iter().any(|s| s.email.contains("noreply")),
            "services are not offered"
        );
    }

    #[test]
    fn a_reminder_fires_only_when_nobody_replied_and_ends_quietly_when_someone_did() {
        let (mut store, account) = real_account();
        let write = |store: &mut Store, to: &str, at: i64| {
            let id = store.new_draft(account, &[], at).unwrap();
            store
                .save_draft(
                    &DraftInput {
                        id,
                        to: vec![a(to)],
                        cc: vec![],
                        bcc: vec![],
                        subject: format!("Question for {to}"),
                        body_html: "<p>Can you check?</p>".into(),
                        nudge_days: Some(2),
                    },
                    at,
                )
                .unwrap();
            store.send_draft(id, at, 0).unwrap()
        };
        let quiet = write(&mut store, "quiet@example.com", NOW);
        let chatty = write(&mut store, "chatty@example.com", NOW);
        assert_eq!(store.waiting_nudges().unwrap().len(), 2);
        assert_eq!(store.waiting_nudges().unwrap()[0].who, "Quiet", "the person is named, not the address");

        let mut reply = incoming("r1", "chatty@example.com", "Re: Question for chatty@example.com", "Yes!");
        reply.to = vec![a("me@example.com")];
        reply.thread_id = store
            .conn()
            .query_row("SELECT provider_id FROM conversations WHERE id = ?1", [chatty.conversation_id], |r| r.get(0))
            .unwrap();
        reply.sent_at = NOW + DAY_MS;
        arrive(&mut store, &reply, false);

        assert!(store.resolve_nudges(NOW + DAY_MS).unwrap().is_empty(), "not due yet");
        assert_eq!(store.waiting_nudges().unwrap().len(), 1, "the answered one ended quietly");
        let fired = store.resolve_nudges(NOW + 2 * DAY_MS).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!((fired[0].conversation_id, fired[0].who.as_str()), (quiet.conversation_id, "Quiet"));
        assert!(store.waiting_nudges().unwrap().is_empty());
        assert!(store.resolve_nudges(NOW + 9 * DAY_MS).unwrap().is_empty(), "a reminder fires once");
        assert!(store.cancel_nudge(1).is_err());
    }

    #[test]
    fn email_addresses_are_checked_sensibly() {
        for good in ["a@b.co", "first.last+tag@sub.example.org"] {
            assert!(valid_email(good), "{good}");
        }
        for bad in ["", "a", "a@b", "@b.co", "a@@b.co", "a b@c.co", "a@.co", "a@b."] {
            assert!(!valid_email(bad), "{bad}");
        }
    }
}

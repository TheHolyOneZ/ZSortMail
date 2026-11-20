use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};

use super::Store;
use super::conversations::me;
use super::ingest::Address;
use super::people::parse_kind;
use crate::dto::{AttachmentView, FileEntry, MailAddress, MessageView, ParticipantRef, PersonKind, UnsubscribeInfo};
use crate::error::Result;
use crate::ink::ink_for;

pub(crate) fn compress(text: &str) -> Vec<u8> {
    zstd::encode_all(text.as_bytes(), 3).unwrap_or_default()
}

pub(crate) fn decompress(bytes: &[u8]) -> String {
    zstd::decode_all(bytes).ok().and_then(|raw| String::from_utf8(raw).ok()).unwrap_or_default()
}

fn first_name(name: &str) -> String {
    name.split_whitespace().next().unwrap_or(name).to_string()
}

fn to_line(recipients: &[Address], self_addresses: &[String]) -> String {
    let mut names: Vec<String> = Vec::new();
    let mut you = false;
    for r in recipients {
        if self_addresses.iter().any(|a| *a == r.email.to_lowercase()) {
            you = true;
        } else {
            names.push(first_name(r.name.as_deref().unwrap_or_else(|| r.email.split('@').next().unwrap_or(&r.email))));
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if you {
        parts.push("you".into());
    }
    parts.extend(names);
    match parts.len() {
        0 => "to nobody".into(),
        1 => format!("to {}", parts[0]),
        n => format!("to {} and {}", parts[..n - 1].join(", "), parts[n - 1]),
    }
}

#[derive(Debug, Clone)]
pub struct AttachmentRecord {
    pub id: i64,
    pub message_id: i64,
    pub account_id: i64,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    pub content_id: Option<String>,
    pub inline: bool,
    pub provider_id: Option<String>,
    pub cache_path: Option<String>,
}

pub(crate) fn message_views(store: &Store, conversation_id: i64) -> Result<Vec<MessageView>> {
    let conn = store.conn();
    let account_id: i64 = conn
        .query_row("SELECT account_id FROM conversations WHERE id = ?1", [conversation_id], |r| r.get(0))
        .optional()?
        .unwrap_or(0);
    let self_addresses = store.self_addresses(account_id)?;

    let mut known: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT lower(pa.address), pa.person_id FROM person_addresses pa
             WHERE pa.account_id = ?1 AND pa.person_id IN
               (SELECT person_id FROM conversation_people WHERE conversation_id = ?2)",
        )?;
        for row in
            stmt.query_map(params![account_id, conversation_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        {
            let (address, person) = row?;
            known.insert(address, person);
        }
    }

    let mut attachments: HashMap<i64, Vec<AttachmentView>> = HashMap::new();
    {
        let mut stmt = conn.prepare_cached(
            "SELECT a.id, a.message_id, a.filename, a.mime_type, a.size_bytes, a.inline FROM attachments a
             JOIN messages m ON m.id = a.message_id WHERE m.conversation_id = ?1 ORDER BY a.id",
        )?;
        let rows = stmt.query_map([conversation_id], |r| {
            Ok((
                r.get::<_, i64>(1)?,
                AttachmentView {
                    id: r.get(0)?,
                    name: r.get(2)?,
                    mime: r.get(3)?,
                    size: u32::try_from(r.get::<_, i64>(4)?).unwrap_or(u32::MAX),
                    inline: r.get(5)?,
                },
            ))
        })?;
        for row in rows {
            let (message_id, view) = row?;
            attachments.entry(message_id).or_default().push(view);
        }
    }

    let mut stmt = conn.prepare_cached(
        "SELECT m.id, m.from_person_id, p.display_name, p.ink, p.kind, m.is_from_me, m.sent_at, m.to_addresses,
                m.snippet, m.body_text, m.body_quoted, m.body_html IS NOT NULL, m.unread, m.starred,
                m.remote_images, m.trackers_blocked, m.unsubscribe, m.from_address, m.cc_addresses
         FROM messages m LEFT JOIN people p ON p.id = m.from_person_id
         WHERE m.conversation_id = ?1 AND m.hidden_at IS NULL ORDER BY m.sent_at, m.id",
    )?;
    let rows = stmt.query_map([conversation_id], |r| {
        let id: i64 = r.get(0)?;
        let from_person: Option<i64> = r.get(1)?;
        let is_from_me: bool = r.get(5)?;
        let from_address: String = r.get(17)?;
        let from = if is_from_me {
            me()
        } else {
            ParticipantRef {
                person_id: from_person,
                name: r
                    .get::<_, Option<String>>(2)?
                    .unwrap_or_else(|| from_address.split('@').next().unwrap_or(&from_address).to_string()),
                ink: r.get::<_, Option<i64>>(3)?.map_or_else(|| ink_for(&from_address), |n| n as u8),
                kind: r.get::<_, Option<String>>(4)?.map_or(PersonKind::Person, |k| parse_kind(&k)),
                is_me: false,
            }
        };
        let recipients: Vec<Address> = serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default();
        let copied: Vec<Address> = serde_json::from_str(&r.get::<_, String>(18)?).unwrap_or_default();
        let label = |a: &Address| MailAddress {
            email: a.email.clone(),
            name: a.name.clone(),
            person_id: known.get(&a.email.to_lowercase()).copied(),
            is_me: self_addresses.iter().any(|m| m == &a.email.to_lowercase()),
        };
        let preview: String = r.get(8)?;
        let text = r
            .get::<_, Option<Vec<u8>>>(9)?
            .map(|b| decompress(&b))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| preview.clone());
        let unsubscribe = r
            .get::<_, Option<String>>(16)?
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .map(|v| UnsubscribeInfo {
                https: v["https"].as_str().map(str::to_string),
                mailto: v["mailto"].as_str().map(str::to_string),
                one_click: v["one_click"].as_bool().unwrap_or(false),
                done_at: store
                    .setting(&format!("unsubscribed.{from_address}"))
                    .ok()
                    .flatten()
                    .and_then(|t| t.parse().ok()),
            })
            .filter(|u| u.https.is_some() || u.mailto.is_some());
        Ok(MessageView {
            id,
            from,
            sent_at: r.get(6)?,
            to_line: to_line(&recipients, &self_addresses),
            from_email: from_address.clone(),
            to: recipients.iter().map(label).collect(),
            cc: copied.iter().map(label).collect(),
            preview,
            text,
            quoted: r.get::<_, Option<Vec<u8>>>(10)?.map(|b| decompress(&b)).filter(|q| !q.is_empty()),
            attachments: Vec::new(),
            unread: r.get(12)?,
            starred: r.get(13)?,
            html_url: r.get::<_, bool>(11)?.then(|| format!("msg/{id}")),
            remote_images: u32::try_from(r.get::<_, i64>(14)?).unwrap_or(0),
            trackers_blocked: u32::try_from(r.get::<_, i64>(15)?).unwrap_or(0),
            unsubscribe,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        let mut view = row?;
        view.attachments = attachments.remove(&view.id).unwrap_or_default();
        out.push(view);
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubscribeTarget {
    pub info: UnsubscribeInfo,
    pub from_address: String,
    pub from_name: String,
    pub person_id: Option<i64>,
    pub provider: String,
}

impl Store {
    pub fn unsubscribe_target(&self, message_id: i64) -> Result<Option<UnsubscribeTarget>> {
        let row = self
            .conn
            .query_row(
                "SELECT m.unsubscribe, m.from_address, COALESCE(p.display_name, m.from_address), m.from_person_id, a.provider
                 FROM messages m JOIN accounts a ON a.id = m.account_id LEFT JOIN people p ON p.id = m.from_person_id
                 WHERE m.id = ?1",
                [message_id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<i64>>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((json, from_address, from_name, person_id, provider)) = row else { return Ok(None) };
        let info = json
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .map(|v| UnsubscribeInfo {
                https: v["https"].as_str().map(str::to_string),
                mailto: v["mailto"].as_str().map(str::to_string),
                one_click: v["one_click"].as_bool().unwrap_or(false),
                done_at: self
                    .setting(&format!("unsubscribed.{from_address}"))
                    .ok()
                    .flatten()
                    .and_then(|t| t.parse().ok()),
            })
            .filter(|u| u.https.is_some() || u.mailto.is_some());
        Ok(info.map(|info| UnsubscribeTarget { info, from_address, from_name, person_id, provider }))
    }

    pub fn mark_unsubscribed(&self, address: &str, now: i64) -> Result<()> {
        self.set_setting(&format!("unsubscribed.{address}"), &now.to_string())
    }

    pub fn message_html(&self, id: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT body_html FROM messages WHERE id = ?1", [id], |r| r.get::<_, Option<Vec<u8>>>(0))
            .optional()?
            .flatten()
            .map(|b| decompress(&b)))
    }

    pub fn message_text(&self, id: i64) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT body_text FROM messages WHERE id = ?1", [id], |r| r.get::<_, Option<Vec<u8>>>(0))
            .optional()?
            .flatten()
            .map(|b| decompress(&b)))
    }

    pub fn attachment(&self, id: i64) -> Result<Option<AttachmentRecord>> {
        Ok(self
            .conn
            .query_row(
                "SELECT a.id, a.message_id, m.account_id, a.filename, a.mime_type, a.size_bytes, a.content_id,
                        a.inline, a.provider_id, a.cache_path
                 FROM attachments a JOIN messages m ON m.id = a.message_id WHERE a.id = ?1",
                [id],
                |r| {
                    Ok(AttachmentRecord {
                        id: r.get(0)?,
                        message_id: r.get(1)?,
                        account_id: r.get(2)?,
                        filename: r.get(3)?,
                        mime: r.get(4)?,
                        size: r.get(5)?,
                        content_id: r.get(6)?,
                        inline: r.get(7)?,
                        provider_id: r.get(8)?,
                        cache_path: r.get(9)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn attachment_by_content_id(&self, message_id: i64, cid: &str) -> Result<Option<AttachmentRecord>> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM attachments WHERE message_id = ?1 AND content_id = ?2",
                params![message_id, cid],
                |r| r.get(0),
            )
            .optional()?;
        match id {
            Some(id) => self.attachment(id),
            None => Ok(None),
        }
    }

    pub fn list_files(&self, person_id: i64) -> Result<Vec<FileEntry>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT a.id, a.filename, a.mime_type, a.size_bytes, a.inline, m.id, c.id, c.subject, m.sent_at,
                    m.is_from_me, m.from_person_id, p.display_name, p.ink, p.kind, m.from_address
             FROM conversation_people cp
             JOIN conversations c ON c.id = cp.conversation_id AND c.hidden_at IS NULL AND c.trashed = 0
             JOIN messages m ON m.conversation_id = c.id AND m.hidden_at IS NULL
             JOIN attachments a ON a.message_id = m.id AND a.inline = 0
             LEFT JOIN people p ON p.id = m.from_person_id
             WHERE cp.person_id = ?1 ORDER BY m.sent_at DESC, a.id",
        )?;
        let rows = stmt.query_map([person_id], |r| {
            let is_from_me: bool = r.get(9)?;
            let from_address: String = r.get(14)?;
            let from = if is_from_me {
                me()
            } else {
                ParticipantRef {
                    person_id: r.get(10)?,
                    name: r
                        .get::<_, Option<String>>(11)?
                        .unwrap_or_else(|| from_address.split('@').next().unwrap_or(&from_address).to_string()),
                    ink: r.get::<_, Option<i64>>(12)?.map_or_else(|| ink_for(&from_address), |n| n as u8),
                    kind: r.get::<_, Option<String>>(13)?.map_or(PersonKind::Person, |k| parse_kind(&k)),
                    is_me: false,
                }
            };
            Ok(FileEntry {
                attachment: AttachmentView {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    mime: r.get(2)?,
                    size: u32::try_from(r.get::<_, i64>(3)?).unwrap_or(u32::MAX),
                    inline: r.get(4)?,
                },
                message_id: r.get(5)?,
                conversation_id: r.get(6)?,
                conversation_subject: r.get(7)?,
                sent_at: r.get(8)?,
                from,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(email: &str, name: Option<&str>) -> Address {
        Address::new(email, name)
    }

    #[test]
    fn compression_round_trips_text_in_any_script() {
        for text in ["", "plain", "日本語のテキスト 🎉", &"x".repeat(100_000)] {
            assert_eq!(decompress(&compress(text)), text);
        }
    }

    #[test]
    fn recipients_read_naturally() {
        let me = vec!["me@example.com".to_string()];
        assert_eq!(to_line(&[a("me@example.com", None)], &me), "to you");
        assert_eq!(to_line(&[a("maya@x.example", Some("Maya Okafor"))], &me), "to Maya");
        assert_eq!(
            to_line(
                &[
                    a("me@example.com", None),
                    a("maya@x.example", Some("Maya Okafor")),
                    a("dan@x.example", Some("Dan W"))
                ],
                &me
            ),
            "to you, Maya and Dan"
        );
        assert_eq!(to_line(&[], &me), "to nobody");
    }
}

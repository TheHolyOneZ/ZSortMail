use rusqlite::params;

use super::Store;
use super::ingest::{Address, HeaderFacts, IncomingMessage, IngestContext, IngestOutcome};
use super::messages::decompress;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMessage {
    pub message_id: i64,
    pub account_id: i64,
    pub from: Address,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub bcc: Vec<Address>,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
    pub rfc_message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub thread_id: String,
    pub files: Vec<OutgoingFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingFile {
    pub filename: String,
    pub mime: String,
    pub path: String,
}

fn with_quote(text: String, quoted: Option<String>) -> String {
    let Some(quoted) = quoted.filter(|q| !q.trim().is_empty()) else { return text };
    let quoted: Vec<String> =
        quoted.lines().map(|line| if line.trim().is_empty() { ">".to_string() } else { format!("> {line}") }).collect();
    format!("{}\n\n{}", text.trim_end(), quoted.join("\n"))
}

pub fn reply_subject(subject: &str) -> String {
    let trimmed = subject.trim_start();
    if trimmed.len() >= 3 && trimmed.is_char_boundary(3) && trimmed[..3].eq_ignore_ascii_case("re:") {
        subject.to_string()
    } else {
        format!("Re: {subject}")
    }
}

impl Store {
    pub fn store_reply(&mut self, conversation_id: i64, text: &str, now: i64) -> Result<IngestOutcome> {
        let text = text.trim();
        if text.is_empty() {
            return Err(Error::Invalid("Write something first".into()));
        }
        let (account_id, thread_id, subject, provider): (i64, String, String, String) = self
            .conn()
            .query_row(
                "SELECT c.account_id, c.provider_id, c.subject, a.provider
                 FROM conversations c JOIN accounts a ON a.id = c.account_id WHERE c.id = ?1",
                [conversation_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|_| Error::NotFound("conversation"))?;
        let me = self
            .self_addresses(account_id)?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Invalid("No mailbox is linked yet".into()))?;

        let (last_from_me, last_from, last_to, last_rfc): (bool, String, String, String) = self
            .conn()
            .query_row(
                "SELECT is_from_me, from_address, to_addresses, COALESCE(rfc_message_id, '') FROM messages
                 WHERE conversation_id = ?1 ORDER BY sent_at DESC, id DESC LIMIT 1",
                [conversation_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|_| Error::NotFound("message"))?;
        let to: Vec<Address> = if last_from_me {
            serde_json::from_str(&last_to).unwrap_or_default()
        } else {
            let name: Option<String> = self
                .conn()
                .query_row(
                    "SELECT p.display_name FROM person_addresses a JOIN people p ON p.id = a.person_id
                     WHERE a.account_id = ?1 AND a.address = ?2",
                    params![account_id, last_from],
                    |r| r.get(0),
                )
                .ok();
            vec![Address::new(&last_from, name.as_deref())]
        };

        let unique = format!("{:032x}", rand_u128());
        let msg = IncomingMessage {
            provider_id: format!("local:{unique}"),
            thread_id,
            rfc_message_id: Some(format!("<{unique}@zsortmail.local>")),
            in_reply_to: Some(last_rfc).filter(|id| !id.is_empty()),
            from: Address::new(&me, None),
            to,
            cc: Vec::new(),
            subject: reply_subject(&subject),
            sent_at: now,
            snippet: None,
            text: Some(text.to_string()),
            html: None,
            attachments: Vec::new(),
            labels: vec!["SENT".into()],
            unread: false,
            starred: false,
            headers: HeaderFacts::default(),
            size_bytes: None,
        };
        let ctx = IngestContext { account_id, self_addresses: vec![me], screener: false, automation: false };
        let out = self.ingest(&ctx, &msg, now)?;
        if provider != "demo" {
            self.conn().execute(
                "INSERT INTO outbox (account_id, op, payload, next_attempt_at, created_at)
                 VALUES (?1, 'send_message', ?2, ?3, ?3)",
                params![account_id, serde_json::json!({ "message_id": out.message_id }).to_string(), now],
            )?;
        }
        Ok(out)
    }

    pub fn load_outgoing(&self, message_id: i64) -> Result<OutgoingMessage> {
        struct Raw {
            account_id: i64,
            conversation_id: i64,
            from: String,
            to: String,
            cc: String,
            subject: String,
            text: Option<Vec<u8>>,
            quoted: Option<Vec<u8>>,
            rfc: Option<String>,
            reply_to: Option<String>,
        }
        let raw = self
            .conn()
            .query_row(
                "SELECT account_id, conversation_id, from_address, to_addresses, cc_addresses, subject, body_text,
                        body_quoted, rfc_message_id, in_reply_to FROM messages WHERE id = ?1",
                [message_id],
                |r| {
                    Ok(Raw {
                        account_id: r.get(0)?,
                        conversation_id: r.get(1)?,
                        from: r.get(2)?,
                        to: r.get(3)?,
                        cc: r.get(4)?,
                        subject: r.get(5)?,
                        text: r.get(6)?,
                        quoted: r.get(7)?,
                        rfc: r.get(8)?,
                        reply_to: r.get(9)?,
                    })
                },
            )
            .map_err(|_| Error::NotFound("message"))?;
        let Raw { account_id, conversation_id, from, to, cc, subject, text, quoted, rfc, reply_to } = raw;
        let thread_id: String =
            self.conn()
                .query_row("SELECT provider_id FROM conversations WHERE id = ?1", [conversation_id], |r| r.get(0))?;
        let mut stmt = self.conn().prepare_cached(
            "SELECT rfc_message_id FROM messages WHERE conversation_id = ?1 AND rfc_message_id IS NOT NULL
             AND id <> ?2 AND sent_at <= (SELECT sent_at FROM messages WHERE id = ?2) ORDER BY sent_at, id",
        )?;
        let references = stmt
            .query_map(params![conversation_id, message_id], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let name: Option<String> = self
            .conn()
            .query_row("SELECT display_name FROM accounts WHERE id = ?1", [account_id], |r| {
                r.get::<_, Option<String>>(0)
            })
            .ok()
            .flatten();
        let html = self.message_html(message_id)?.filter(|h| !h.trim().is_empty());
        let files = {
            let mut stmt = self.conn().prepare_cached(
                "SELECT filename, mime_type, cache_path FROM attachments
                 WHERE message_id = ?1 AND inline = 0 AND cache_path IS NOT NULL ORDER BY id",
            )?;
            stmt.query_map([message_id], |r| {
                Ok(OutgoingFile { filename: r.get(0)?, mime: r.get(1)?, path: r.get(2)? })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(OutgoingMessage {
            message_id,
            account_id,
            from: Address::new(&from, name.as_deref()),
            to: serde_json::from_str(&to).unwrap_or_default(),
            cc: serde_json::from_str(&cc).unwrap_or_default(),
            bcc: Vec::new(),
            subject,
            text: with_quote(text.map(|b| decompress(&b)).unwrap_or_default(), quoted.map(|b| decompress(&b))),
            html,
            rfc_message_id: rfc,
            in_reply_to: reply_to,
            references,
            thread_id,
            files,
        })
    }

    pub fn mark_sent(&mut self, message_id: i64, server_message_id: &str, server_thread_id: &str) -> Result<()> {
        self.in_transaction(|tx| {
            let (account_id, conversation_id): (i64, i64) = tx
                .query_row("SELECT account_id, conversation_id FROM messages WHERE id = ?1", [message_id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .map_err(|_| Error::NotFound("message"))?;
            let twin: Option<i64> = tx
                .query_row(
                    "SELECT id FROM messages WHERE account_id = ?1 AND provider_id = ?2 AND id <> ?3",
                    params![account_id, server_message_id, message_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(twin) = twin {
                tx.execute("DELETE FROM messages_fts WHERE rowid = ?1", [message_id])?;
                tx.execute("DELETE FROM messages WHERE id = ?1", [message_id])?;
                let _ = twin;
                super::conversations::recompute_conversation(tx, conversation_id)?;
                return Ok(());
            }
            tx.execute("UPDATE messages SET provider_id = ?1 WHERE id = ?2", params![server_message_id, message_id])?;
            let local_thread: bool = tx.query_row(
                "SELECT provider_id LIKE 'local:%' FROM conversations WHERE id = ?1",
                [conversation_id],
                |r| r.get(0),
            )?;
            if local_thread {
                let taken: bool = tx
                    .query_row(
                        "SELECT 1 FROM conversations WHERE account_id = ?1 AND provider_id = ?2 AND id <> ?3",
                        params![account_id, server_thread_id, conversation_id],
                        |_| Ok(true),
                    )
                    .unwrap_or(false);
                if !taken {
                    tx.execute(
                        "UPDATE conversations SET provider_id = ?1 WHERE id = ?2",
                        params![server_thread_id, conversation_id],
                    )?;
                }
            }
            Ok(())
        })
    }

    pub fn counts(&self) -> Result<(u32, u32)> {
        let new_senders: i64 = self.conn().query_row(
            &format!(
                "SELECT COUNT(*) FROM people p WHERE p.status = 'new_sender' AND p.hidden_at IS NULL{}",
                self.in_account("p")
            ),
            [],
            |r| r.get(0),
        )?;
        let unread: i64 = self.conn().query_row(
            &format!(
                "SELECT COALESCE(SUM(p.unread_count), 0) FROM people p
                 WHERE p.status = 'known' AND p.kind <> 'self' AND p.hidden_at IS NULL{}",
                self.in_account("p")
            ),
            [],
            |r| r.get(0),
        )?;
        Ok((u32::try_from(new_senders).unwrap_or(0), u32::try_from(unread).unwrap_or(0)))
    }
}

pub(crate) fn rand_u128() -> u128 {
    let mut bytes = [0u8; 16];
    use std::hash::{BuildHasher, Hasher};
    for (i, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_usize(i);
        chunk.copy_from_slice(&h.finish().to_le_bytes());
    }
    u128::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn re_is_added_once() {
        assert_eq!(reply_subject("Coffee"), "Re: Coffee");
        assert_eq!(reply_subject("Re: Coffee"), "Re: Coffee");
        assert_eq!(reply_subject("RE: Coffee"), "RE: Coffee");
        assert_eq!(reply_subject("Restaurant tonight"), "Re: Restaurant tonight");
        assert_eq!(reply_subject("日本語"), "Re: 日本語");
        assert_eq!(reply_subject(""), "Re: ");
    }

    #[test]
    fn a_stored_reply_can_be_loaded_for_sending_and_adopts_the_servers_ids() {
        use crate::demo::{DemoOptions, seed_demo};
        let mut store = Store::open_in_memory().unwrap();
        let tz = jiff::tz::TimeZone::UTC;
        seed_demo(&mut store, &DemoOptions { now_ms: 1_795_082_400_000, stress_people: 0, tz }).unwrap();
        let maya = store.list_people(crate::dto::PeopleFilter::All, "Okafor").unwrap()[0].id;
        let convo =
            store.list_conversations(maya).unwrap().into_iter().find(|c| c.subject == "Coffee Thursday?").unwrap();
        let out = store.store_reply(convo.id, "See you at 9:30.", 1_795_082_400_000).unwrap();

        let loaded = store.load_outgoing(out.message_id).unwrap();
        assert_eq!(loaded.text, "See you at 9:30.");
        assert_eq!(loaded.subject, "Re: Coffee Thursday?");
        assert_eq!(loaded.to.len(), 1);
        assert_eq!(loaded.to[0].email, "maya.okafor@northwind.example");
        assert_eq!(loaded.from.email, "you@zsortmail.demo");
        assert!(loaded.references.len() >= 2, "the thread so far: {:?}", loaded.references);
        assert!(loaded.rfc_message_id.as_deref().is_some_and(|id| id.ends_with("@zsortmail.local>")));

        store.mark_sent(out.message_id, "18c0ffee", "thread-9").unwrap();
        let provider: String = store
            .conn()
            .query_row("SELECT provider_id FROM messages WHERE id = ?1", [out.message_id], |r| r.get(0))
            .unwrap();
        assert_eq!(provider, "18c0ffee");
        assert!(store.load_outgoing(999_999).is_err());
    }

    #[test]
    fn local_ids_do_not_repeat() {
        let a = rand_u128();
        let b = rand_u128();
        assert_ne!(a, b);
    }
}

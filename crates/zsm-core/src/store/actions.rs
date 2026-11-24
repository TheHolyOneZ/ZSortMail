use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::conversations::recompute_conversation;
use super::people::refresh_person;
use crate::error::{Error, Result};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Touched {
    pub people: Vec<i64>,
    pub conversations: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PersonEdit {
    pub name: String,
    pub title: Option<String>,
    pub organization: Option<String>,
    pub description: Option<String>,
    pub ink: u8,
}

fn blank_to_none(text: Option<&str>) -> Option<String> {
    text.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string)
}

pub(crate) fn enqueue(
    tx: &Transaction<'_>,
    account_id: i64,
    op: &str,
    payload: &serde_json::Value,
    now: i64,
) -> Result<()> {
    let provider: Option<String> =
        tx.query_row("SELECT provider FROM accounts WHERE id = ?1", [account_id], |r| r.get(0)).optional()?;
    if provider.as_deref().is_none_or(|p| p == "demo") {
        return Ok(());
    }
    tx.execute(
        "INSERT INTO outbox (account_id, op, payload, next_attempt_at, created_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![account_id, op, payload.to_string(), now],
    )?;
    Ok(())
}

fn people_of(tx: &Transaction<'_>, conversation_id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare_cached("SELECT person_id FROM conversation_people WHERE conversation_id = ?1")?;
    let rows = stmt.query_map([conversation_id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

impl Store {
    pub fn unread_conversation_ids(&self, person: Option<i64>) -> Result<Vec<i64>> {
        let mut stmt = self.conn().prepare(
            "SELECT DISTINCT c.id FROM conversations c
             JOIN messages m ON m.conversation_id = c.id AND m.unread = 1
             JOIN conversation_people cp ON cp.conversation_id = c.id
             JOIN people p ON p.id = cp.person_id
             WHERE c.hidden_at IS NULL AND c.trashed = 0 AND p.status = 'known' AND p.hidden_at IS NULL
               AND (?1 IS NULL OR cp.person_id = ?1)
             ORDER BY c.id",
        )?;
        Ok(stmt.query_map([person], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
    }

    pub fn unread_message_ids(&self, conversation_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn().prepare("SELECT id FROM messages WHERE conversation_id = ?1 AND unread = 1")?;
        Ok(stmt.query_map([conversation_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_conversation_read(&mut self, conversation_id: i64, read: bool, now: i64) -> Result<Touched> {
        self.in_transaction(|tx| {
            let (account_id, thread_id): (i64, String) = tx
                .query_row("SELECT account_id, provider_id FROM conversations WHERE id = ?1", [conversation_id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?
                .ok_or(Error::NotFound("conversation"))?;
            if read {
                tx.execute(
                    "UPDATE messages SET unread = 0 WHERE conversation_id = ?1 AND unread = 1",
                    [conversation_id],
                )?;
            } else {
                tx.execute(
                    "UPDATE messages SET unread = 1 WHERE id = (
                       SELECT id FROM messages WHERE conversation_id = ?1 AND is_from_me = 0
                       ORDER BY sent_at DESC, id DESC LIMIT 1)",
                    [conversation_id],
                )?;
            }
            recompute_conversation(tx, conversation_id)?;
            let people = people_of(tx, conversation_id)?;
            for id in &people {
                refresh_person(tx, *id)?;
            }
            let (add, remove) = if read { (vec![], vec!["UNREAD"]) } else { (vec!["UNREAD"], vec![]) };
            enqueue(
                tx,
                account_id,
                "thread_modify",
                &serde_json::json!({ "thread_id": thread_id, "add": add, "remove": remove }),
                now,
            )?;
            Ok(Touched { people, conversations: vec![conversation_id] })
        })
    }

    pub fn set_message_starred(&mut self, message_id: i64, starred: bool, now: i64) -> Result<Touched> {
        self.in_transaction(|tx| {
            let (account_id, conversation_id, provider_id): (i64, i64, String) = tx
                .query_row(
                    "SELECT account_id, conversation_id, provider_id FROM messages WHERE id = ?1",
                    [message_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?
                .ok_or(Error::NotFound("message"))?;
            tx.execute("UPDATE messages SET starred = ?1 WHERE id = ?2", params![starred, message_id])?;
            let (add, remove) = if starred { (vec!["STARRED"], vec![]) } else { (vec![], vec!["STARRED"]) };
            enqueue(
                tx,
                account_id,
                "message_modify",
                &serde_json::json!({ "message_id": provider_id, "add": add, "remove": remove }),
                now,
            )?;
            Ok(Touched { people: people_of(tx, conversation_id)?, conversations: vec![conversation_id] })
        })
    }

    pub fn set_person_pinned(&mut self, person_id: i64, pinned: bool) -> Result<Touched> {
        let changed = self.conn.execute("UPDATE people SET pinned = ?1 WHERE id = ?2", params![pinned, person_id])?;
        if changed == 0 {
            return Err(Error::NotFound("person"));
        }
        Ok(Touched { people: vec![person_id], conversations: vec![] })
    }

    pub fn set_person_notes(&mut self, person_id: i64, notes: &str) -> Result<Touched> {
        let notes = blank_to_none(Some(notes));
        let changed = self.conn.execute("UPDATE people SET notes = ?1 WHERE id = ?2", params![notes, person_id])?;
        if changed == 0 {
            return Err(Error::NotFound("person"));
        }
        Ok(Touched { people: vec![person_id], conversations: vec![] })
    }

    pub fn edit_person(&mut self, person_id: i64, edit: &PersonEdit) -> Result<Touched> {
        let name = edit.name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("A person needs a name".into()));
        }
        if edit.ink > 9 {
            return Err(Error::Invalid("Colour must be between 0 and 9".into()));
        }
        self.in_transaction(|tx| {
            let changed = tx.execute(
                "UPDATE people SET display_name = ?1, title = ?2, organization = ?3, description = ?4, ink = ?5
                 WHERE id = ?6",
                params![
                    name,
                    blank_to_none(edit.title.as_deref()),
                    blank_to_none(edit.organization.as_deref()),
                    blank_to_none(edit.description.as_deref()),
                    edit.ink,
                    person_id
                ],
            )?;
            if changed == 0 {
                return Err(Error::NotFound("person"));
            }
            refresh_person(tx, person_id)?;
            Ok(Touched { people: vec![person_id], conversations: vec![] })
        })
    }
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;

    use super::*;
    use crate::demo::{DemoOptions, seed_demo};
    use crate::dto::PeopleFilter;

    const NOW: i64 = 1_795_082_400_000;

    fn demo() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        seed_demo(&mut store, &DemoOptions { now_ms: NOW, stress_people: 0, tz: TimeZone::UTC }).unwrap();
        store
    }

    fn maya(store: &Store) -> i64 {
        store.list_people(PeopleFilter::All, "Okafor").unwrap()[0].id
    }

    #[test]
    fn reading_a_conversation_clears_its_unread_count_and_the_persons_badge() {
        let mut store = demo();
        let maya = maya(&store);
        assert_eq!(store.person_summary(maya).unwrap().unwrap().unread, 1);
        let convo = store.list_conversations(maya).unwrap().into_iter().find(|c| c.unread > 0).unwrap();
        let touched = store.set_conversation_read(convo.id, true, NOW).unwrap();
        assert_eq!(touched.conversations, vec![convo.id]);
        assert!(touched.people.contains(&maya));
        assert_eq!(store.person_summary(maya).unwrap().unwrap().unread, 0);
        assert_eq!(store.conversation_summary(convo.id, Some(maya)).unwrap().unwrap().unread, 0);
    }

    #[test]
    fn marking_unread_only_flags_the_newest_incoming_message() {
        let mut store = demo();
        let maya = maya(&store);
        let convo = store.list_conversations(maya).unwrap().into_iter().find(|c| c.subject == "Invoice 2291").unwrap();
        assert_eq!(convo.unread, 0);
        store.set_conversation_read(convo.id, false, NOW).unwrap();
        let detail = store.get_conversation(convo.id, Some(maya)).unwrap().unwrap();
        let unread: Vec<bool> = detail.messages.iter().map(|m| m.unread).collect();
        assert_eq!(unread, [false, false, true], "only Maya's latest message, not mine");
    }

    #[test]
    fn the_demo_account_has_no_server_so_nothing_is_queued() {
        let mut store = demo();
        let maya = maya(&store);
        let convo = store.list_conversations(maya).unwrap().remove(0);
        store.set_conversation_read(convo.id, true, NOW).unwrap();
        let queued: i64 = store.conn().query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get(0)).unwrap();
        assert_eq!(queued, 0);
    }

    #[test]
    fn a_linked_account_queues_the_change_for_the_server() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.upsert_account("gmail", "me@example.com", None, NOW).unwrap();
        let ctx = crate::store::ingest::IngestContext {
            account_id: account,
            self_addresses: vec!["me@example.com".into()],
            screener: false,
            automation: false,
        };
        let msg = crate::store::ingest::IncomingMessage {
            provider_id: "m1".into(),
            thread_id: "t1".into(),
            rfc_message_id: None,
            in_reply_to: None,
            from: crate::store::ingest::Address::new("a@b.example", Some("A B")),
            to: vec![crate::store::ingest::Address::new("me@example.com", None)],
            cc: vec![],
            bcc: Vec::new(),
            subject: "Hello".into(),
            sent_at: NOW - 1000,
            snippet: None,
            text: Some("hi".into()),
            html: None,
            attachments: vec![],
            labels: vec!["INBOX".into(), "UNREAD".into()],
            unread: true,
            starred: false,
            headers: Default::default(),
            size_bytes: None,
        };
        let out = store.ingest(&ctx, &msg, NOW).unwrap();
        store.set_conversation_read(out.conversation_id, true, NOW).unwrap();
        let (op, payload): (String, String) =
            store.conn().query_row("SELECT op, payload FROM outbox", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(op, "thread_modify");
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(payload["thread_id"], "t1");
        assert_eq!(payload["remove"], serde_json::json!(["UNREAD"]));
    }

    #[test]
    fn pinning_and_notes_and_edits_are_remembered_and_searchable() {
        let mut store = demo();
        let rasmus = store.list_people(PeopleFilter::All, "Lindqvist").unwrap()[0].id;
        assert!(!store.person_summary(rasmus).unwrap().unwrap().pinned);
        store.set_person_pinned(rasmus, true).unwrap();
        assert_eq!(store.list_people(PeopleFilter::All, "").unwrap().iter().take(3).filter(|p| p.pinned).count(), 3);
        store.set_person_notes(rasmus, "  Prefers email before 10.  ").unwrap();
        assert_eq!(store.get_person(rasmus).unwrap().unwrap().notes.as_deref(), Some("Prefers email before 10."));

        store
            .edit_person(
                rasmus,
                &PersonEdit {
                    name: "Rasmus L.".into(),
                    title: Some("Tax adviser".into()),
                    organization: Some(String::new()),
                    description: None,
                    ink: 7,
                },
            )
            .unwrap();
        let detail = store.get_person(rasmus).unwrap().unwrap();
        assert_eq!(detail.summary.name, "Rasmus L.");
        assert_eq!(detail.summary.ink, 7);
        assert_eq!(detail.organization, None, "blank clears a field");
        assert_eq!(store.list_people(PeopleFilter::All, "adviser").unwrap().len(), 1, "the new title is searchable");
        assert!(store.edit_person(rasmus, &PersonEdit { name: "  ".into(), ..sample_edit() }).is_err());
        assert!(store.edit_person(9999, &sample_edit()).is_err());
    }

    fn sample_edit() -> PersonEdit {
        PersonEdit { name: "X".into(), title: None, organization: None, description: None, ink: 0 }
    }

    #[test]
    fn starring_a_message_is_reflected_in_the_conversation() {
        let mut store = demo();
        let maya = maya(&store);
        let convo = store.list_conversations(maya).unwrap().remove(0);
        let first = store.get_conversation(convo.id, Some(maya)).unwrap().unwrap().messages[0].id;
        store.set_message_starred(first, true, NOW).unwrap();
        let again = store.get_conversation(convo.id, Some(maya)).unwrap().unwrap();
        assert!(again.messages[0].starred);
        assert!(store.set_message_starred(123_456, true, NOW).is_err());
    }
}

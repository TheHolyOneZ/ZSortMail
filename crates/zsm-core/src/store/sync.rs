use rusqlite::{OptionalExtension, params};

use super::Store;
use super::actions::Touched;
use super::conversations::recompute_conversation;
use super::people::refresh_person;
use crate::error::Result;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncState {
    pub history_id: Option<String>,
    pub backfill_cursor: Option<String>,
    pub backfill_done: bool,
    pub last_sync_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelChange {
    pub provider_id: String,
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentOrigin {
    pub account_id: i64,
    pub message_provider_id: String,
    pub attachment_provider_id: String,
    pub filename: String,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxItem {
    pub id: i64,
    pub account_id: i64,
    pub op: String,
    pub payload: serde_json::Value,
    pub attempts: u32,
}

fn addresses_of(item: &OutboxItem, conn: &rusqlite::Connection) -> Result<Option<Vec<String>>> {
    let emails_in = |json: &str| -> Vec<String> {
        serde_json::from_str::<Vec<serde_json::Value>>(json)
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.get("email").and_then(|e| e.as_str()).or_else(|| v.as_str()))
            .map(str::to_lowercase)
            .collect()
    };
    let mut out: Vec<String> = Vec::new();
    let take = |from: String, to: String, cc: String, out: &mut Vec<String>| {
        out.push(from.to_lowercase());
        out.extend(emails_in(&to));
        out.extend(emails_in(&cc));
    };
    let rows_sql = match item.op.as_str() {
        "thread_modify" | "thread_trash" | "thread_untrash" | "tag_thread" => {
            let thread = item.payload["thread_id"].as_str().unwrap_or_default().to_string();
            (
                "SELECT m.from_address, m.to_addresses, m.cc_addresses FROM messages m JOIN conversations c ON c.id = m.conversation_id WHERE c.account_id = ?1 AND c.provider_id = ?2",
                thread,
            )
        }
        "message_modify" => {
            let id = item.payload["message_id"].as_str().unwrap_or_default().to_string();
            (
                "SELECT m.from_address, m.to_addresses, m.cc_addresses FROM messages m WHERE m.account_id = ?1 AND m.provider_id = ?2",
                id,
            )
        }
        "send_message" => {
            let id = item.payload["message_id"].as_i64().unwrap_or_default().to_string();
            for bcc in item.payload["bcc"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                out.push(bcc.to_lowercase());
            }
            (
                "SELECT m.from_address, m.to_addresses, m.cc_addresses FROM messages m WHERE m.account_id = ?1 AND m.id = CAST(?2 AS INTEGER)",
                id,
            )
        }
        _ => return Ok(None),
    };
    let mut stmt = conn.prepare(rows_sql.0)?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map(rusqlite::params![item.account_id, rows_sql.1], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?;
    if rows.is_empty() {
        return Ok(None);
    }
    for (from, to, cc) in rows {
        take(from, to, cc, &mut out);
    }
    Ok(Some(out))
}

impl Store {
    pub fn outbox_within(&self, item: &OutboxItem, allowed: &[String]) -> Result<bool> {
        Ok(match addresses_of(item, self.conn())? {
            Some(found) => !found.is_empty() && found.iter().all(|a| allowed.iter().any(|ok| ok == a)),
            None => false,
        })
    }

    pub fn sync_state(&self, account_id: i64) -> Result<SyncState> {
        Ok(self
            .conn()
            .query_row(
                "SELECT history_id, backfill_cursor, backfill_done, last_sync_at FROM sync_state WHERE account_id = ?1",
                [account_id],
                |r| {
                    Ok(SyncState {
                        history_id: r.get(0)?,
                        backfill_cursor: r.get(1)?,
                        backfill_done: r.get(2)?,
                        last_sync_at: r.get(3)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }

    pub fn save_sync_state(&self, account_id: i64, state: &SyncState) -> Result<()> {
        self.conn().execute(
            "INSERT INTO sync_state (account_id, history_id, backfill_cursor, backfill_done, last_sync_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(account_id) DO UPDATE SET history_id = excluded.history_id,
               backfill_cursor = excluded.backfill_cursor, backfill_done = excluded.backfill_done,
               last_sync_at = excluded.last_sync_at",
            params![account_id, state.history_id, state.backfill_cursor, state.backfill_done, state.last_sync_at],
        )?;
        Ok(())
    }

    pub fn known_provider_ids(&self, account_id: i64, ids: &[String]) -> Result<Vec<String>> {
        let mut stmt =
            self.conn().prepare_cached("SELECT 1 FROM messages WHERE account_id = ?1 AND provider_id = ?2")?;
        let mut known = Vec::new();
        for id in ids {
            if stmt.exists(params![account_id, id])? {
                known.push(id.clone());
            }
        }
        Ok(known)
    }

    pub fn name_labels(&self, account_id: i64, labels: &[(String, String)]) -> Result<usize> {
        let mut changed = 0;
        for (id, name) in labels {
            changed += self.conn().execute(
                "UPDATE labels SET name = ?3 WHERE account_id = ?1 AND provider_id = ?2 AND kind = 'user'
                   AND provider_id NOT LIKE 'zsm:%' AND name <> ?3",
                params![account_id, id, name.trim()],
            )?;
        }
        Ok(changed)
    }

    pub fn apply_label_changes(&mut self, account_id: i64, changes: &[LabelChange]) -> Result<Touched> {
        self.in_transaction(|tx| {
            let mut conversations = std::collections::BTreeSet::new();
            for change in changes {
                let Some((message_id, conversation_id)) = tx
                    .query_row(
                        "SELECT id, conversation_id FROM messages WHERE account_id = ?1 AND provider_id = ?2",
                        params![account_id, change.provider_id],
                        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
                    )
                    .optional()?
                else {
                    continue;
                };
                conversations.insert(conversation_id);
                for label in &change.add {
                    tx.execute(
                        "INSERT OR IGNORE INTO labels (account_id, provider_id, name, kind) VALUES (?1, ?2, ?2, ?3)",
                        params![account_id, label, if is_system_label(label) { "system" } else { "user" }],
                    )?;
                    tx.execute(
                        "INSERT OR IGNORE INTO message_labels (message_id, label_id)
                         SELECT ?1, id FROM labels WHERE account_id = ?2 AND provider_id = ?3",
                        params![message_id, account_id, label],
                    )?;
                }
                for label in &change.remove {
                    tx.execute(
                        "DELETE FROM message_labels WHERE message_id = ?1
                           AND label_id IN (SELECT id FROM labels WHERE account_id = ?2 AND provider_id = ?3)",
                        params![message_id, account_id, label],
                    )?;
                }
                if change.add.iter().any(|l| l == "UNREAD") {
                    tx.execute("UPDATE messages SET unread = 1 WHERE id = ?1 AND is_from_me = 0", [message_id])?;
                }
                if change.remove.iter().any(|l| l == "UNREAD") {
                    tx.execute("UPDATE messages SET unread = 0 WHERE id = ?1", [message_id])?;
                }
                if change.add.iter().any(|l| l == "STARRED") {
                    tx.execute("UPDATE messages SET starred = 1 WHERE id = ?1", [message_id])?;
                }
                if change.remove.iter().any(|l| l == "STARRED") {
                    tx.execute("UPDATE messages SET starred = 0 WHERE id = ?1", [message_id])?;
                }
            }
            touch(tx, conversations)
        })
    }

    pub fn remove_messages(&mut self, account_id: i64, provider_ids: &[String]) -> Result<Touched> {
        self.in_transaction(|tx| {
            let mut conversations = std::collections::BTreeSet::new();
            for provider_id in provider_ids {
                let found: Option<(i64, i64)> = tx
                    .query_row(
                        "SELECT id, conversation_id FROM messages WHERE account_id = ?1 AND provider_id = ?2",
                        params![account_id, provider_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                let Some((message_id, conversation_id)) = found else { continue };
                tx.execute("DELETE FROM messages_fts WHERE rowid = ?1", [message_id])?;
                tx.execute("DELETE FROM messages WHERE id = ?1", [message_id])?;
                conversations.insert(conversation_id);
            }
            for id in &conversations {
                let left: i64 =
                    tx.query_row("SELECT COUNT(*) FROM messages WHERE conversation_id = ?1", [id], |r| r.get(0))?;
                if left == 0 {
                    let people = people_of(tx, *id)?;
                    tx.execute("DELETE FROM conversations WHERE id = ?1", [id])?;
                    for person in people {
                        refresh_person(tx, person)?;
                    }
                }
            }
            let remaining: std::collections::BTreeSet<i64> = conversations
                .into_iter()
                .filter(|id| {
                    tx.query_row("SELECT 1 FROM conversations WHERE id = ?1", [id], |_| Ok(()))
                        .optional()
                        .ok()
                        .flatten()
                        .is_some()
                })
                .collect();
            touch(tx, remaining)
        })
    }

    pub fn outbox_peek(&self, now: i64, limit: usize) -> Result<Vec<OutboxItem>> {
        self.outbox_select(None, now, limit)
    }

    fn outbox_select(&self, account_id: Option<i64>, now: i64, limit: usize) -> Result<Vec<OutboxItem>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT id, account_id, op, payload, attempts FROM outbox
             WHERE next_attempt_at <= ?1 AND attempts < 12 AND (?3 IS NULL OR account_id = ?3)
             ORDER BY id LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![now, i64::try_from(limit).unwrap_or(50), account_id], |r| {
            let payload: String = r.get(3)?;
            Ok(OutboxItem {
                id: r.get(0)?,
                account_id: r.get(1)?,
                op: r.get(2)?,
                payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
                attempts: u32::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn outbox_due(&self, account_id: i64, now: i64, limit: usize) -> Result<Vec<OutboxItem>> {
        let items = self.outbox_select(Some(account_id), now, limit)?;
        for item in &items {
            self.conn()
                .execute("UPDATE outbox SET next_attempt_at = ?2 WHERE id = ?1", params![item.id, now + 120_000])?;
        }
        Ok(items)
    }

    pub fn outbox_done(&self, id: i64) -> Result<()> {
        self.conn().execute("DELETE FROM outbox WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn outbox_failed(&self, id: i64, error: &str, retry_at: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE outbox SET attempts = attempts + 1, last_error = ?2, next_attempt_at = ?3 WHERE id = ?1",
            params![id, error, retry_at],
        )?;
        Ok(())
    }

    pub fn outbox_dead(&self, id: i64, error: &str) -> Result<()> {
        self.conn().execute("UPDATE outbox SET attempts = 99, last_error = ?2 WHERE id = ?1", params![id, error])?;
        Ok(())
    }

    pub fn outbox_len(&self) -> Result<u32> {
        let n: i64 = self.conn().query_row("SELECT COUNT(*) FROM outbox WHERE attempts < 12", [], |r| r.get(0))?;
        Ok(u32::try_from(n).unwrap_or(0))
    }
}

fn is_system_label(id: &str) -> bool {
    matches!(id, "INBOX" | "SENT" | "DRAFT" | "SPAM" | "TRASH" | "STARRED" | "UNREAD" | "IMPORTANT" | "CHAT")
        || id.starts_with("CATEGORY_")
}

fn people_of(tx: &rusqlite::Transaction<'_>, conversation_id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare_cached("SELECT person_id FROM conversation_people WHERE conversation_id = ?1")?;
    let rows = stmt.query_map([conversation_id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

fn touch(tx: &rusqlite::Transaction<'_>, conversations: std::collections::BTreeSet<i64>) -> Result<Touched> {
    let mut people = std::collections::BTreeSet::new();
    for id in &conversations {
        recompute_conversation(tx, *id)?;
        people.extend(people_of(tx, *id)?);
    }
    for id in &people {
        refresh_person(tx, *id)?;
    }
    Ok(Touched { people: people.into_iter().collect(), conversations: conversations.into_iter().collect() })
}

impl Store {
    pub fn delete_account(&mut self, account_id: i64) -> Result<()> {
        self.in_transaction(|tx| {
            tx.execute(
                "DELETE FROM messages_fts WHERE rowid IN (SELECT id FROM messages WHERE account_id = ?1)",
                [account_id],
            )?;
            tx.execute(
                "DELETE FROM people_fts WHERE rowid IN (SELECT id FROM people WHERE account_id = ?1)",
                [account_id],
            )?;
            tx.execute("DELETE FROM accounts WHERE id = ?1", [account_id])?;
            tx.execute("DELETE FROM settings WHERE key LIKE ?1", [format!("%.{account_id}")])?;
            Ok(())
        })
    }

    pub fn attachment_origin(&self, attachment_id: i64) -> Result<Option<AttachmentOrigin>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT m.account_id, m.provider_id, COALESCE(a.provider_id, ''), a.filename, a.size_bytes
                 FROM attachments a JOIN messages m ON m.id = a.message_id WHERE a.id = ?1",
                [attachment_id],
                |r| {
                    Ok(AttachmentOrigin {
                        account_id: r.get(0)?,
                        message_provider_id: r.get(1)?,
                        attachment_provider_id: r.get(2)?,
                        filename: r.get(3)?,
                        size: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn set_attachment_cache(&self, attachment_id: i64, path: &str) -> Result<()> {
        self.conn().execute("UPDATE attachments SET cache_path = ?1 WHERE id = ?2", params![path, attachment_id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ingest::{Address, IncomingMessage, IngestContext};

    const NOW: i64 = 1_795_082_400_000;

    fn mailbox() -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.upsert_account("gmail", "me@example.com", None, NOW).unwrap();
        let ctx = IngestContext {
            account_id: account,
            self_addresses: vec!["me@example.com".into()],
            screener: false,
            automation: false,
        };
        for (n, labels) in [("m1", vec!["INBOX", "UNREAD"]), ("m2", vec!["INBOX"])] {
            let msg = IncomingMessage {
                provider_id: n.into(),
                thread_id: "t1".into(),
                rfc_message_id: None,
                in_reply_to: None,
                from: Address::new("a@b.example", Some("Ann Bee")),
                to: vec![Address::new("me@example.com", None)],
                cc: vec![],
                subject: "Hello".into(),
                sent_at: NOW - 5000,
                snippet: None,
                text: Some("hello there".into()),
                html: None,
                attachments: vec![],
                labels: labels.iter().map(|s| (*s).to_string()).collect(),
                unread: labels.contains(&"UNREAD"),
                starred: false,
                headers: Default::default(),
                size_bytes: None,
            };
            store.ingest(&ctx, &msg, NOW).unwrap();
        }
        (store, account)
    }

    #[test]
    fn sync_state_round_trips_and_defaults_to_nothing_synced() {
        let (store, account) = mailbox();
        assert_eq!(store.sync_state(account).unwrap(), SyncState::default());
        let state = SyncState {
            history_id: Some("42".into()),
            backfill_cursor: Some("tok".into()),
            backfill_done: false,
            last_sync_at: Some(NOW),
        };
        store.save_sync_state(account, &state).unwrap();
        assert_eq!(store.sync_state(account).unwrap(), state);
        store.save_sync_state(account, &SyncState { backfill_done: true, ..state.clone() }).unwrap();
        assert!(store.sync_state(account).unwrap().backfill_done);
    }

    #[test]
    fn label_changes_from_the_server_update_flags_and_aggregates() {
        let (mut store, account) = mailbox();
        let person = store.list_people(crate::dto::PeopleFilter::All, "Ann").unwrap()[0].id;
        assert_eq!(store.person_summary(person).unwrap().unwrap().unread, 1);

        let touched = store
            .apply_label_changes(
                account,
                &[
                    LabelChange {
                        provider_id: "m1".into(),
                        add: vec![],
                        remove: vec!["UNREAD".into(), "INBOX".into()],
                    },
                    LabelChange {
                        provider_id: "m2".into(),
                        add: vec!["STARRED".into(), "Label_7".into()],
                        remove: vec![],
                    },
                    LabelChange { provider_id: "never-seen".into(), add: vec!["UNREAD".into()], remove: vec![] },
                ],
            )
            .unwrap();
        assert_eq!(touched.people, vec![person]);
        assert_eq!(store.person_summary(person).unwrap().unwrap().unread, 0);
        let convo = store.list_conversations(person).unwrap().remove(0);
        let detail = store.get_conversation(convo.id, Some(person)).unwrap().unwrap();
        assert!(!detail.messages[0].unread && detail.messages[1].starred);
        let in_inbox: bool = store
            .conn()
            .query_row("SELECT in_inbox FROM conversations WHERE id = ?1", [convo.id], |r| r.get(0))
            .unwrap();
        assert!(in_inbox, "m2 is still in the inbox");

        store
            .apply_label_changes(
                account,
                &[
                    LabelChange { provider_id: "m1".into(), add: vec!["TRASH".into()], remove: vec![] },
                    LabelChange { provider_id: "m2".into(), add: vec!["TRASH".into()], remove: vec!["INBOX".into()] },
                ],
            )
            .unwrap();
        assert!(store.list_conversations(person).unwrap().is_empty());
        assert!(
            store.list_people(crate::dto::PeopleFilter::All, "Ann").unwrap().is_empty(),
            "no live conversation, no row"
        );
    }

    #[test]
    fn deleted_messages_leave_nothing_behind() {
        let (mut store, account) = mailbox();
        let person = store.list_people(crate::dto::PeopleFilter::All, "Ann").unwrap()[0].id;
        store.remove_messages(account, &["m1".into(), "ghost".into()]).unwrap();
        let convo = store.list_conversations(person).unwrap().remove(0);
        assert_eq!(convo.message_count, 1);
        assert!(
            store.search("hello", NOW, 5).unwrap().messages.len() == 1,
            "the deleted message left the search index"
        );
        let touched = store.remove_messages(account, &["m2".into()]).unwrap();
        assert!(touched.conversations.is_empty(), "the empty conversation itself is gone");
        assert!(store.list_conversations(person).unwrap().is_empty());
        let rows: i64 = store.conn().query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn the_outbox_retries_with_backoff_and_gives_up_on_dead_items() {
        let (store, account) = mailbox();
        for op in ["a", "b", "c"] {
            store
                .conn()
                .execute(
                    "INSERT INTO outbox (account_id, op, payload, next_attempt_at, created_at) VALUES (?1, ?2, '{\"x\":1}', ?3, ?3)",
                    params![account, op, NOW],
                )
                .unwrap();
        }
        let due = store.outbox_peek(NOW, 10).unwrap();
        assert_eq!(due.iter().map(|i| i.op.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(due[0].payload["x"], 1);

        store.outbox_done(due[0].id).unwrap();
        store.outbox_failed(due[1].id, "offline", NOW + 60_000).unwrap();
        store.outbox_dead(due[2].id, "bad request").unwrap();
        assert!(store.outbox_peek(NOW, 10).unwrap().is_empty(), "b waits, c is dead");
        let later = store.outbox_peek(NOW + 61_000, 10).unwrap();
        assert_eq!(later.len(), 1);
        assert_eq!((later[0].op.as_str(), later[0].attempts), ("b", 1));
        assert_eq!(store.outbox_len().unwrap(), 1);
    }

    #[test]
    fn known_ids_are_reported() {
        let (store, account) = mailbox();
        let known = store.known_provider_ids(account, &["m1".into(), "zzz".into(), "m2".into()]).unwrap();
        assert_eq!(known, ["m1", "m2"]);
    }
}

#[cfg(test)]
mod allow_list_tests {
    use super::*;
    use crate::dto::PeopleFilter;
    use crate::store::rules::tests::{NOW, demo};
    use rusqlite::params;

    fn item(account: i64, op: &str, payload: serde_json::Value) -> OutboxItem {
        OutboxItem { id: 1, account_id: account, op: op.into(), payload, attempts: 0 }
    }

    #[test]
    fn the_allow_list_only_lets_through_changes_to_mail_among_the_listed_addresses() {
        let store = demo();
        let account = store.accounts().unwrap()[0].id;
        let maya = store.list_people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        let convo =
            store.list_conversations(maya).unwrap().into_iter().find(|c| c.subject == "Q3 launch checklist").unwrap();
        let thread: String = store
            .conn()
            .query_row("SELECT provider_id FROM conversations WHERE id = ?1", [convo.id], |r| r.get(0))
            .unwrap();
        let thread_op = |op: &str| item(account, op, serde_json::json!({ "thread_id": thread }));
        let list = |a: &[&str]| a.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();

        let all = list(&["me@example.com", "maya.okafor@northwind.example", "dan.whitaker@northwind.example"]);
        let me = store.accounts().unwrap()[0].email.to_lowercase();
        let mut everyone = all.clone();
        everyone.push(me);
        for op in ["thread_modify", "thread_trash", "thread_untrash", "tag_thread"] {
            assert!(store.outbox_within(&thread_op(op), &everyone).unwrap(), "{op}: everyone listed");
            assert!(
                !store.outbox_within(&thread_op(op), &list(&["maya.okafor@northwind.example"])).unwrap(),
                "{op}: others on it"
            );
        }
        assert!(
            !store
                .outbox_within(&item(account, "thread_modify", serde_json::json!({"thread_id": "nope"})), &everyone)
                .unwrap()
        );
        assert!(!store.outbox_within(&thread_op("something_new"), &everyone).unwrap());
        assert!(
            !store
                .outbox_within(&item(account, "send_message", serde_json::json!({"message_id": 999_999})), &everyone)
                .unwrap()
        );
        assert!(
            !store
                .outbox_within(&item(account + 1, "thread_modify", serde_json::json!({"thread_id": thread})), &everyone)
                .unwrap()
        );
    }

    #[test]
    fn each_account_only_gets_its_own_changes_and_a_claimed_change_is_not_given_out_twice() {
        let store = demo();
        let first = store.accounts().unwrap()[0].id;
        let second = store.upsert_account("gmail", "two@example.test", None, NOW).unwrap();
        for (account, thread) in [(first, "a"), (second, "b"), (first, "c")] {
            store
                .conn()
                .execute(
                    "INSERT INTO outbox (account_id, op, payload, next_attempt_at, created_at) VALUES (?1, 'thread_modify', ?2, 0, 0)",
                    params![account, serde_json::json!({ "thread_id": thread }).to_string()],
                )
                .unwrap();
        }
        let mine = store.outbox_due(first, NOW, 10).unwrap();
        assert_eq!(mine.iter().map(|i| i.payload["thread_id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["a", "c"]);
        assert!(mine.iter().all(|i| i.account_id == first), "never another account's change");
        assert!(store.outbox_due(first, NOW, 10).unwrap().is_empty(), "claimed, so not given out again");
        let theirs = store.outbox_due(second, NOW, 10).unwrap();
        assert_eq!(theirs.len(), 1);
        assert_eq!(theirs[0].account_id, second);
        assert_eq!(store.outbox_due(first, NOW + 121_000, 10).unwrap().len(), 2);
    }
}

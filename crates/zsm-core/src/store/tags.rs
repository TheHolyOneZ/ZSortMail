use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::actions::Touched;
use super::sorting::{UndoOp, finish};
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub local: bool,
    pub conversations: u32,
}

pub(crate) fn mirror_on(tx: &Transaction<'_>) -> Result<bool> {
    let value: Option<String> =
        tx.query_row("SELECT value FROM settings WHERE key = 'mirror_tags'", [], |r| r.get(0)).optional()?;
    Ok(value.as_deref() == Some("1"))
}

pub(crate) fn mirror(
    tx: &Transaction<'_>,
    account: i64,
    thread_id: &str,
    name: &str,
    add: bool,
    now: i64,
) -> Result<()> {
    if thread_id.starts_with("local:") || !mirror_on(tx)? {
        return Ok(());
    }
    super::actions::enqueue(
        tx,
        account,
        "tag_thread",
        &serde_json::json!({ "thread_id": thread_id, "name": name, "add": add }),
        now,
    )
}

pub(crate) fn tag_id(name: &str) -> String {
    format!("zsm:{}", name.trim())
}

fn message_ids(tx: &Transaction<'_>, conversation_id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare("SELECT id FROM messages WHERE conversation_id = ?1")?;
    Ok(stmt.query_map([conversation_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
}

pub(crate) fn add_tag(
    tx: &Transaction<'_>,
    account: i64,
    provider_id: &str,
    name: &str,
    messages: &[i64],
) -> Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO labels (account_id, provider_id, name, kind) VALUES (?1, ?2, ?3, 'user')",
        params![account, provider_id, name.trim()],
    )?;
    for id in messages {
        tx.execute(
            "INSERT OR IGNORE INTO message_labels (message_id, label_id)
             SELECT ?1, id FROM labels WHERE account_id = ?2 AND provider_id = ?3",
            params![id, account, provider_id],
        )?;
    }
    Ok(())
}

impl Store {
    pub fn conversation_tags(&self, conversation_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT COALESCE(MIN(CASE WHEN l.provider_id LIKE 'zsm:%' THEN l.id END), MIN(l.id)), MIN(l.name),
                    MAX(l.provider_id LIKE 'zsm:%')
             FROM labels l
             WHERE l.kind = 'user' AND EXISTS (
               SELECT 1 FROM message_labels ml JOIN messages m ON m.id = ml.message_id
               WHERE ml.label_id = l.id AND m.conversation_id = ?1)
             GROUP BY l.name COLLATE NOCASE
             ORDER BY MIN(l.name) COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([conversation_id], |r| {
            Ok(Tag { id: r.get(0)?, name: r.get(1)?, local: r.get(2)?, conversations: 1 })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT COALESCE(MIN(CASE WHEN l.provider_id LIKE 'zsm:%' THEN l.id END), MIN(l.id)), MIN(l.name),
                    MAX(l.provider_id LIKE 'zsm:%'),
                    COUNT(DISTINCT m.conversation_id)
             FROM labels l
             JOIN message_labels ml ON ml.label_id = l.id
             JOIN messages m ON m.id = ml.message_id
             WHERE l.kind = 'user'
             GROUP BY l.name COLLATE NOCASE ORDER BY COUNT(DISTINCT m.conversation_id) DESC, MIN(l.name) COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                local: r.get(2)?,
                conversations: u32::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn tag_conversation(&mut self, conversation_id: i64, name: &str, now: i64) -> Result<(Touched, UndoOp)> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("Type a name for the tag".into()));
        }
        if name.chars().count() > 40 {
            return Err(Error::Invalid("Tag names can be up to 40 characters".into()));
        }
        self.in_transaction(|tx| {
            let account: i64 = tx
                .query_row("SELECT account_id FROM conversations WHERE id = ?1", [conversation_id], |r| r.get(0))
                .optional()?
                .ok_or(Error::NotFound("conversation"))?;
            let messages = message_ids(tx, conversation_id)?;
            let fresh: Vec<i64> = {
                let mut out = Vec::new();
                for id in &messages {
                    let has: bool = tx.query_row(
                        "SELECT EXISTS (SELECT 1 FROM message_labels ml JOIN labels l ON l.id = ml.label_id
                                        WHERE ml.message_id = ?1 AND l.account_id = ?2 AND l.provider_id = ?3)",
                        params![id, account, tag_id(name)],
                        |r| r.get(0),
                    )?;
                    if !has {
                        out.push(*id);
                    }
                }
                out
            };
            add_tag(tx, account, &tag_id(name), name, &messages)?;
            if !fresh.is_empty() {
                let thread = super::sorting::mailbox(tx, conversation_id)?.thread_id;
                mirror(tx, account, &thread, name, true, now)?;
            }
            let touched = finish(tx, &[conversation_id], &[])?;
            Ok((touched, UndoOp::Untag { tag: tag_id(name), message_ids: fresh }))
        })
    }

    pub fn untag_conversation(&mut self, conversation_id: i64, label_id: i64, now: i64) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let (provider_id, name, account): (String, String, i64) = tx
                .query_row("SELECT provider_id, name, account_id FROM labels WHERE id = ?1", [label_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?
                .ok_or(Error::NotFound("tag"))?;
            if !provider_id.starts_with("zsm:") {
                return Err(Error::Invalid("That label belongs to Gmail. Change it there".into()));
            }
            let had: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT ml.message_id FROM message_labels ml JOIN messages m ON m.id = ml.message_id
                     WHERE ml.label_id = ?1 AND m.conversation_id = ?2",
                )?;
                stmt.query_map(params![label_id, conversation_id], |r| r.get(0))?
                    .collect::<std::result::Result<_, _>>()?
            };
            tx.execute(
                "DELETE FROM message_labels WHERE label_id = ?1 AND message_id IN
                   (SELECT id FROM messages WHERE conversation_id = ?2)",
                params![label_id, conversation_id],
            )?;
            let thread = super::sorting::mailbox(tx, conversation_id)?.thread_id;
            mirror(tx, account, &thread, &name, false, now)?;
            let touched = finish(tx, &[conversation_id], &[])?;
            Ok((touched, UndoOp::Retag { account, tag: provider_id, name, message_ids: had }))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::store::rules::tests::{NOW, demo};

    #[test]
    fn a_tag_goes_on_a_conversation_is_searchable_and_comes_off_again() {
        let mut store = demo();
        let convo = store.inbox(None, NOW, 10, 0).unwrap()[0].conversation.id;
        assert!(store.tag_conversation(convo, "  ", NOW).is_err());
        assert!(store.tag_conversation(convo, &"x".repeat(41), NOW).is_err());

        let (_, undo) = store.tag_conversation(convo, "Taxes 2026", NOW).unwrap();
        let tags = store.conversation_tags(convo).unwrap();
        assert_eq!(tags.len(), 1);
        assert!(tags[0].local && tags[0].name == "Taxes 2026");
        let all = store.list_tags().unwrap();
        assert_eq!(all.iter().find(|t| t.name == "Taxes 2026").unwrap().conversations, 1);
        let found = store.view_rows("label:\"Taxes 2026\"", NOW, 20).unwrap();
        assert_eq!(found.iter().map(|r| r.conversation.id).collect::<Vec<_>>(), vec![convo]);

        let (_, again) = store.tag_conversation(convo, "Taxes 2026", NOW).unwrap();
        store.apply_undo(&again, NOW).unwrap();
        assert_eq!(store.conversation_tags(convo).unwrap().len(), 1, "the second tag had nothing new to undo");
        store.apply_undo(&undo, NOW).unwrap();
        assert!(store.conversation_tags(convo).unwrap().is_empty());

        store.tag_conversation(convo, "Keep", NOW).unwrap();
        let id = store.conversation_tags(convo).unwrap()[0].id;
        let (_, back) = store.untag_conversation(convo, id, NOW).unwrap();
        assert!(store.conversation_tags(convo).unwrap().is_empty());
        store.apply_undo(&back, NOW).unwrap();
        assert_eq!(store.conversation_tags(convo).unwrap()[0].name, "Keep");
    }
}

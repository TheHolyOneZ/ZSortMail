use std::collections::BTreeSet;

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::actions::Touched;
use super::people::refresh_person;
use super::sorting::{UndoOp, finish};
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeSnapshot {
    pub row: String,
    pub addresses: Vec<(String, bool)>,
    pub conversations: Vec<i64>,
    pub added_to_keep: Vec<i64>,
    pub messages: Vec<i64>,
    pub groups: Vec<i64>,
    pub groups_added_to_keep: Vec<i64>,
    pub keep_notes: Option<String>,
    pub keep_flags: (bool, bool, bool, bool),
}

fn columns(tx: &Transaction<'_>) -> Result<Vec<String>> {
    let mut stmt = tx.prepare("SELECT name FROM pragma_table_info('people')")?;
    Ok(stmt.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
}

fn row_json(tx: &Transaction<'_>, id: i64) -> Result<String> {
    let cols = columns(tx)?;
    let pairs: Vec<String> = cols.iter().map(|c| format!("'{c}', \"{c}\"")).collect();
    Ok(tx.query_row(&format!("SELECT json_object({}) FROM people WHERE id = ?1", pairs.join(", ")), [id], |r| {
        r.get(0)
    })?)
}

fn restore_row(tx: &Transaction<'_>, json: &str) -> Result<()> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| Error::Invalid("Cannot undo this merge".into()))?;
    let Some(object) = value.as_object() else { return Err(Error::Invalid("Cannot undo this merge".into())) };
    let names: Vec<&String> = object.keys().collect();
    let values: Vec<Value> = object
        .values()
        .map(|v| match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Integer(i64::from(*b)),
            serde_json::Value::Number(n) => {
                n.as_i64().map(Value::Integer).unwrap_or_else(|| Value::Real(n.as_f64().unwrap_or(0.0)))
            }
            serde_json::Value::String(s) => Value::Text(s.clone()),
            other => Value::Text(other.to_string()),
        })
        .collect();
    let sql = format!(
        "INSERT INTO people ({}) VALUES ({})",
        names.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", "),
        (1..=names.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ")
    );
    tx.execute(&sql, rusqlite::params_from_iter(values))?;
    Ok(())
}

fn ids(tx: &Transaction<'_>, sql: &str, id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare(sql)?;
    Ok(stmt.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
}

impl Store {
    pub fn split_address(&mut self, person_id: i64, address: &str, now: i64) -> Result<(i64, Touched, UndoOp)> {
        let address = address.trim().to_lowercase();
        self.in_transaction(|tx| {
            let (account, kind, status): (i64, String, String) = tx
                .query_row("SELECT account_id, kind, status FROM people WHERE id = ?1", [person_id], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?
                .ok_or(Error::NotFound("person"))?;
            if kind == "self" {
                return Err(Error::Invalid("You cannot split yourself".into()));
            }
            if status != "known" {
                return Err(Error::Invalid("Let this person in from New senders first".into()));
            }
            let count: i64 = tx.query_row("SELECT COUNT(*) FROM person_addresses WHERE person_id = ?1", [person_id], |r| r.get(0))?;
            let was_primary: bool = tx
                .query_row(
                    "SELECT is_primary FROM person_addresses WHERE person_id = ?1 AND address = ?2",
                    params![person_id, address],
                    |r| r.get(0),
                )
                .optional()?
                .ok_or_else(|| Error::Invalid("That address does not belong to this person".into()))?;
            if count < 2 {
                return Err(Error::Invalid("A person with one address has nothing to split".into()));
            }
            let name = super::ingest::prettify_local_part(&address);
            tx.execute(
                "INSERT INTO people (account_id, kind, status, display_name, ink, created_at) VALUES (?1, ?2, 'known', ?3, ?4, ?5)",
                params![account, kind, name, crate::ink::ink_for(&address), now],
            )?;
            let created = tx.last_insert_rowid();
            tx.execute(
                "UPDATE person_addresses SET person_id = ?1, is_primary = 1 WHERE person_id = ?2 AND address = ?3",
                params![created, person_id, address],
            )?;
            if was_primary {
                tx.execute(
                    "UPDATE person_addresses SET is_primary = 1 WHERE rowid = (SELECT rowid FROM person_addresses WHERE person_id = ?1 LIMIT 1)",
                    [person_id],
                )?;
            }
            let messages: Vec<i64> = {
                let mut stmt = tx.prepare("SELECT id FROM messages WHERE from_person_id = ?1 AND from_address = ?2")?;
                stmt.query_map(params![person_id, address], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
            };
            for id in &messages {
                tx.execute("UPDATE messages SET from_person_id = ?1 WHERE id = ?2", params![created, id])?;
            }
            tx.execute(
                "INSERT OR IGNORE INTO conversation_people (conversation_id, person_id)
                 SELECT DISTINCT conversation_id, ?1 FROM messages WHERE from_person_id = ?1",
                [created],
            )?;
            let conversations = ids(tx, "SELECT conversation_id FROM conversation_people WHERE person_id = ?1", created)?;
            refresh_person(tx, created)?;
            let touched = finish(tx, &conversations, &[person_id, created])?;
            Ok((created, touched, UndoOp::Rejoin { keep: person_id, created, address, was_primary, messages }))
        })
    }

    pub fn merge_people(&mut self, keep: i64, absorb: i64) -> Result<(Touched, UndoOp)> {
        if keep == absorb {
            return Err(Error::Invalid("Pick two different people".into()));
        }
        self.in_transaction(|tx| {
            let info = |id: i64| -> Result<(i64, String, String)> {
                tx.query_row("SELECT account_id, kind, status FROM people WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .optional()?
                    .ok_or(Error::NotFound("person"))
            };
            let (ka, kk, ks) = info(keep)?;
            let (aa, ak, as_) = info(absorb)?;
            if ka != aa {
                return Err(Error::Invalid("These two are in different mailboxes".into()));
            }
            if kk == "self" || ak == "self" {
                return Err(Error::Invalid("You cannot merge yourself with someone".into()));
            }
            if ks != "known" || as_ != "known" {
                return Err(Error::Invalid("Let both people in from New senders first".into()));
            }

            let keep_state: (Option<String>, bool, bool, bool, bool) = tx.query_row(
                "SELECT notes, pinned, vip, muted, whitelisted FROM people WHERE id = ?1",
                [keep],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )?;
            let absorbed_notes: Option<String> = tx.query_row("SELECT notes FROM people WHERE id = ?1", [absorb], |r| r.get(0))?;

            let keep_convs: BTreeSet<i64> =
                ids(tx, "SELECT conversation_id FROM conversation_people WHERE person_id = ?1", keep)?.into_iter().collect();
            let conversations = ids(tx, "SELECT conversation_id FROM conversation_people WHERE person_id = ?1", absorb)?;
            let added_to_keep: Vec<i64> = conversations.iter().copied().filter(|c| !keep_convs.contains(c)).collect();
            let keep_groups: BTreeSet<i64> = ids(tx, "SELECT group_id FROM person_groups WHERE person_id = ?1", keep)?.into_iter().collect();
            let groups = ids(tx, "SELECT group_id FROM person_groups WHERE person_id = ?1", absorb)?;
            let groups_added_to_keep: Vec<i64> = groups.iter().copied().filter(|g| !keep_groups.contains(g)).collect();
            let messages = ids(tx, "SELECT id FROM messages WHERE from_person_id = ?1", absorb)?;
            let addresses: Vec<(String, bool)> = {
                let mut stmt = tx.prepare("SELECT address, is_primary FROM person_addresses WHERE person_id = ?1")?;
                stmt.query_map([absorb], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?
            };
            let snapshot = MergeSnapshot {
                row: row_json(tx, absorb)?,
                addresses,
                conversations: conversations.clone(),
                added_to_keep,
                messages,
                groups,
                groups_added_to_keep,
                keep_notes: keep_state.0.clone(),
                keep_flags: (keep_state.1, keep_state.2, keep_state.3, keep_state.4),
            };

            tx.execute("UPDATE person_addresses SET person_id = ?1, is_primary = 0 WHERE person_id = ?2", params![keep, absorb])?;
            tx.execute("UPDATE messages SET from_person_id = ?1 WHERE from_person_id = ?2", params![keep, absorb])?;
            tx.execute(
                "INSERT OR IGNORE INTO conversation_people (conversation_id, person_id)
                 SELECT conversation_id, ?1 FROM conversation_people WHERE person_id = ?2",
                params![keep, absorb],
            )?;
            tx.execute("DELETE FROM conversation_people WHERE person_id = ?1", [absorb])?;
            tx.execute("INSERT OR IGNORE INTO person_groups (person_id, group_id) SELECT ?1, group_id FROM person_groups WHERE person_id = ?2", params![keep, absorb])?;
            let notes = match (keep_state.0.as_deref().filter(|n| !n.trim().is_empty()), absorbed_notes.as_deref().filter(|n| !n.trim().is_empty())) {
                (Some(a), Some(b)) => Some(format!("{a}\n\n{b}")),
                (Some(a), None) => Some(a.to_string()),
                (None, Some(b)) => Some(b.to_string()),
                (None, None) => None,
            };
            tx.execute(
                "UPDATE people SET notes = ?2,
                   pinned = pinned OR (SELECT pinned FROM people WHERE id = ?3),
                   vip = vip OR (SELECT vip FROM people WHERE id = ?3),
                   whitelisted = whitelisted OR (SELECT whitelisted FROM people WHERE id = ?3),
                   muted = muted AND (SELECT muted FROM people WHERE id = ?3)
                 WHERE id = ?1",
                params![keep, notes, absorb],
            )?;
            tx.execute("DELETE FROM people_fts WHERE rowid = ?1", [absorb])?;
            tx.execute("DELETE FROM people WHERE id = ?1", [absorb])?;
            refresh_person(tx, keep)?;
            let touched = finish(tx, &conversations, &[keep])?;
            Ok((touched, UndoOp::Unmerge { keep, absorbed: absorb, snapshot: Box::new(snapshot) }))
        })
    }
}

pub(crate) fn rejoin(
    tx: &Transaction<'_>,
    keep: i64,
    created: i64,
    address: &str,
    was_primary: bool,
    messages: &[i64],
) -> Result<Touched> {
    tx.execute(
        "UPDATE person_addresses SET person_id = ?1, is_primary = ?2 WHERE address = ?3 AND person_id = ?4",
        params![keep, was_primary, address, created],
    )?;
    for id in messages {
        tx.execute("UPDATE messages SET from_person_id = ?1 WHERE id = ?2", params![keep, id])?;
    }
    let conversations = ids(tx, "SELECT conversation_id FROM conversation_people WHERE person_id = ?1", created)?;
    tx.execute("DELETE FROM conversation_people WHERE person_id = ?1", [created])?;
    tx.execute("DELETE FROM people_fts WHERE rowid = ?1", [created])?;
    tx.execute("DELETE FROM people WHERE id = ?1", [created])?;
    refresh_person(tx, keep)?;
    finish(tx, &conversations, &[keep])
}

pub(crate) fn unmerge(tx: &Transaction<'_>, keep: i64, absorbed: i64, s: &MergeSnapshot) -> Result<Touched> {
    restore_row(tx, &s.row)?;
    for (address, primary) in &s.addresses {
        tx.execute(
            "UPDATE person_addresses SET person_id = ?1, is_primary = ?2 WHERE address = ?3 AND person_id = ?4",
            params![absorbed, primary, address, keep],
        )?;
    }
    for id in &s.messages {
        tx.execute("UPDATE messages SET from_person_id = ?1 WHERE id = ?2", params![absorbed, id])?;
    }
    for c in &s.conversations {
        tx.execute(
            "INSERT OR IGNORE INTO conversation_people (conversation_id, person_id) VALUES (?1, ?2)",
            params![c, absorbed],
        )?;
    }
    for c in &s.added_to_keep {
        tx.execute("DELETE FROM conversation_people WHERE conversation_id = ?1 AND person_id = ?2", params![c, keep])?;
    }
    for g in &s.groups {
        tx.execute("INSERT OR IGNORE INTO person_groups (person_id, group_id) VALUES (?1, ?2)", params![absorbed, g])?;
    }
    for g in &s.groups_added_to_keep {
        tx.execute("DELETE FROM person_groups WHERE person_id = ?1 AND group_id = ?2", params![keep, g])?;
    }
    let (pinned, vip, muted, whitelisted) = s.keep_flags;
    tx.execute(
        "UPDATE people SET notes = ?2, pinned = ?3, vip = ?4, muted = ?5, whitelisted = ?6 WHERE id = ?1",
        params![keep, s.keep_notes, pinned, vip, muted, whitelisted],
    )?;
    refresh_person(tx, absorbed)?;
    finish(tx, &s.conversations, &[keep, absorbed])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::PeopleFilter;
    use crate::store::rules::tests::{NOW, arrive, demo, incoming};

    fn person(store: &Store, name: &str) -> i64 {
        store.list_people(PeopleFilter::All, name).unwrap()[0].id
    }

    #[test]
    fn two_addresses_of_one_person_merge_into_one_and_come_apart_again() {
        let mut store = demo();
        let mut second = incoming("maya-home", "maya.home@gmail.example", "Weekend plans", "Are you around?");
        second.from = crate::store::ingest::Address::new("maya.home@gmail.example", Some("Maya (home)"));
        arrive(&mut store, &second, false);
        let keep = person(&store, "Okafor");
        let absorb = person(&store, "Maya (home)");
        assert_ne!(keep, absorb);
        let before = store.get_person(keep).unwrap().unwrap();
        let conversations_before = store.list_conversations(keep).unwrap().len();

        let (touched, undo) = store.merge_people(keep, absorb).unwrap();
        assert!(touched.people.contains(&keep));
        assert!(store.get_person(absorb).unwrap().is_none(), "the absorbed person is gone");
        let after = store.get_person(keep).unwrap().unwrap();
        assert!(after.addresses.contains(&"maya.home@gmail.example".to_string()));
        assert_eq!(after.addresses.len(), before.addresses.len() + 1);
        assert_eq!(store.list_conversations(keep).unwrap().len(), conversations_before + 1);
        let found = store.list_people(PeopleFilter::All, "maya.home").unwrap();
        assert_eq!(
            found.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![keep],
            "search finds the second address under the kept person"
        );
        let weekend =
            store.list_conversations(keep).unwrap().into_iter().find(|c| c.subject == "Weekend plans").unwrap();
        let detail = store.get_conversation(weekend.id, Some(keep)).unwrap().unwrap();
        assert_eq!(detail.messages[0].from.person_id, Some(keep));

        store.apply_undo(&undo, NOW).unwrap();
        let restored = store.get_person(absorb).unwrap().expect("back again");
        assert_eq!(restored.summary.name, "Maya (home)");
        assert_eq!(restored.addresses, vec!["maya.home@gmail.example".to_string()]);
        assert_eq!(store.get_person(keep).unwrap().unwrap().addresses.len(), before.addresses.len());
        assert_eq!(store.list_conversations(keep).unwrap().len(), conversations_before);
        assert_eq!(store.list_conversations(absorb).unwrap().len(), 1);
        let found = store.list_people(PeopleFilter::All, "maya.home").unwrap();
        assert_eq!(found.iter().map(|p| p.id).collect::<Vec<_>>(), vec![absorb], "search follows back");
    }

    #[test]
    fn one_address_can_be_split_off_to_a_person_of_its_own_and_rejoined() {
        let mut store = demo();
        let mut second = incoming("maya-home2", "maya.home@gmail.example", "Weekend plans", "Are you around?");
        second.from = crate::store::ingest::Address::new("maya.home@gmail.example", Some("Maya (home)"));
        arrive(&mut store, &second, false);
        let keep = person(&store, "Okafor");
        let absorb = person(&store, "Maya (home)");
        store.merge_people(keep, absorb).unwrap();
        let joined = store.get_person(keep).unwrap().unwrap();
        let conversations = store.list_conversations(keep).unwrap().len();

        let (created, touched, undo) = store.split_address(keep, "Maya.Home@gmail.example", NOW).unwrap();
        assert!(touched.people.contains(&created) && touched.people.contains(&keep));
        assert_eq!(store.get_person(keep).unwrap().unwrap().addresses.len(), joined.addresses.len() - 1);
        let new = store.get_person(created).unwrap().unwrap();
        assert_eq!(new.addresses, vec!["maya.home@gmail.example".to_string()]);
        assert_eq!(store.list_conversations(created).unwrap().len(), 1, "their mail went with the address");
        assert!(store.list_people(PeopleFilter::All, "maya.home").unwrap().iter().any(|p| p.id == created));

        store.apply_undo(&undo, NOW).unwrap();
        assert!(store.get_person(created).unwrap().is_none());
        assert_eq!(store.get_person(keep).unwrap().unwrap().addresses.len(), joined.addresses.len());
        assert_eq!(store.list_conversations(keep).unwrap().len(), conversations);

        assert!(store.split_address(keep, "stranger@example.test", NOW).is_err(), "not their address");
        let dad = person(&store, "Dad");
        let only = store.get_person(dad).unwrap().unwrap().addresses[0].clone();
        assert!(store.split_address(dad, &only, NOW).is_err(), "one address has nothing to split");
        assert!(store.split_address(9_999, &only, NOW).is_err());
    }

    #[test]
    fn a_merge_keeps_notes_and_flags_and_refuses_what_makes_no_sense() {
        let mut store = demo();
        let maya = person(&store, "Okafor");
        let dad = person(&store, "Dad");
        store.set_person_notes(maya, "Likes oat milk.").unwrap();
        store.set_person_notes(dad, "Birthday in May.").unwrap();
        let (_, undo) = store.merge_people(maya, dad).unwrap();
        let notes = store.get_person(maya).unwrap().unwrap().notes.unwrap();
        assert!(notes.contains("Likes oat milk.") && notes.contains("Birthday in May."), "{notes}");
        store.apply_undo(&undo, NOW).unwrap();
        assert_eq!(store.get_person(maya).unwrap().unwrap().notes.as_deref(), Some("Likes oat milk."));
        assert_eq!(store.get_person(dad).unwrap().unwrap().notes.as_deref(), Some("Birthday in May."));

        assert_eq!(store.merge_people(maya, maya).unwrap_err().to_string(), "Pick two different people");
        assert!(store.merge_people(maya, 999_999).is_err());
        let waiting = store.list_new_senders().unwrap()[0].person.id;
        assert!(store.merge_people(maya, waiting).unwrap_err().to_string().contains("New senders"));
    }
}

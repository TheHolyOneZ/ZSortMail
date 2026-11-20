use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::actions::{Touched, enqueue};
use super::conversations::recompute_conversation;
use super::people::{SUMMARY_COLUMNS, refresh_person, summary_from_row};
use crate::dto::{GroupInfo, HiddenConversation, HiddenOverview, HiddenPerson, NewSender, PersonFlags, SenderDecision};
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonState {
    pub status: String,
    pub hidden_at: Option<i64>,
    pub hidden_until: Option<i64>,
    pub whitelisted: bool,
    pub vip: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UndoOp {
    Mailbox { conversation: i64, in_inbox: bool, trashed: bool },
    Hidden { conversation: i64, hidden_at: Option<i64>, until: Option<i64>, reason: Option<String> },
    Person { person: i64, state: PersonState },
    Read { conversation: i64, unread_ids: Vec<i64> },
    Snooze { conversation: i64, until: Option<i64> },
    Star { message_ids: Vec<i64>, starred: bool },
    Untag { tag: String, message_ids: Vec<i64> },
    Domain { domain: String, action: Option<String> },
    Retag { account: i64, tag: String, name: String, message_ids: Vec<i64> },
    Unmerge { keep: i64, absorbed: i64, snapshot: Box<super::merge::MergeSnapshot> },
    Rejoin { keep: i64, created: i64, address: String, was_primary: bool, messages: Vec<i64> },
    Many(Vec<UndoOp>),
}

pub(crate) fn person_state(tx: &Transaction<'_>, id: i64) -> Result<PersonState> {
    tx.query_row(
        "SELECT status, hidden_at, hidden_until, whitelisted, vip, muted FROM people WHERE id = ?1",
        [id],
        |r| {
            Ok(PersonState {
                status: r.get(0)?,
                hidden_at: r.get(1)?,
                hidden_until: r.get(2)?,
                whitelisted: r.get(3)?,
                vip: r.get(4)?,
                muted: r.get(5)?,
            })
        },
    )
    .optional()?
    .ok_or(Error::NotFound("person"))
}

pub(crate) fn write_person_state(tx: &Transaction<'_>, id: i64, s: &PersonState) -> Result<()> {
    tx.execute(
        "UPDATE people SET status = ?2, hidden_at = ?3, hidden_until = ?4, whitelisted = ?5, vip = ?6, muted = ?7
         WHERE id = ?1",
        params![id, s.status, s.hidden_at, s.hidden_until, s.whitelisted, s.vip, s.muted],
    )?;
    Ok(())
}

pub(crate) struct Mailbox {
    pub account_id: i64,
    pub thread_id: String,
    pub in_inbox: bool,
    pub trashed: bool,
}

pub(crate) fn mailbox(tx: &Transaction<'_>, conversation_id: i64) -> Result<Mailbox> {
    tx.query_row(
        "SELECT account_id, provider_id, in_inbox, trashed FROM conversations WHERE id = ?1",
        [conversation_id],
        |r| Ok(Mailbox { account_id: r.get(0)?, thread_id: r.get(1)?, in_inbox: r.get(2)?, trashed: r.get(3)? }),
    )
    .optional()?
    .ok_or(Error::NotFound("conversation"))
}

pub(crate) fn set_label(
    tx: &Transaction<'_>,
    account_id: i64,
    conversation_id: i64,
    label: &str,
    present: bool,
) -> Result<()> {
    if present {
        tx.execute(
            "INSERT OR IGNORE INTO labels (account_id, provider_id, name, kind) VALUES (?1, ?2, ?2, 'system')",
            params![account_id, label],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO message_labels (message_id, label_id)
             SELECT m.id, l.id FROM messages m JOIN labels l ON l.account_id = ?1 AND l.provider_id = ?3
             WHERE m.conversation_id = ?2",
            params![account_id, conversation_id, label],
        )?;
    } else {
        tx.execute(
            "DELETE FROM message_labels WHERE label_id IN (SELECT id FROM labels WHERE account_id = ?1 AND provider_id = ?3)
               AND message_id IN (SELECT id FROM messages WHERE conversation_id = ?2)",
            params![account_id, conversation_id, label],
        )?;
    }
    Ok(())
}

fn people_of(tx: &Transaction<'_>, conversation_id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare_cached("SELECT person_id FROM conversation_people WHERE conversation_id = ?1")?;
    let rows = stmt.query_map([conversation_id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub(crate) fn finish(tx: &Transaction<'_>, conversations: &[i64], extra_people: &[i64]) -> Result<Touched> {
    let mut people: std::collections::BTreeSet<i64> = extra_people.iter().copied().collect();
    for id in conversations {
        recompute_conversation(tx, *id)?;
        people.extend(people_of(tx, *id)?);
    }
    for id in &people {
        refresh_person(tx, *id)?;
    }
    Ok(Touched { people: people.into_iter().collect(), conversations: conversations.to_vec() })
}

pub(crate) fn archive_in(tx: &Transaction<'_>, id: i64, archived: bool, now: i64) -> Result<()> {
    let mb = mailbox(tx, id)?;
    set_label(tx, mb.account_id, id, "INBOX", !archived)?;
    let (add, remove) = if archived { (vec![], vec!["INBOX"]) } else { (vec!["INBOX"], vec![]) };
    enqueue(
        tx,
        mb.account_id,
        "thread_modify",
        &serde_json::json!({ "thread_id": mb.thread_id, "add": add, "remove": remove }),
        now,
    )
}

pub(crate) fn trash_in(tx: &Transaction<'_>, id: i64, trashed: bool, now: i64) -> Result<()> {
    let mb = mailbox(tx, id)?;
    set_label(tx, mb.account_id, id, "TRASH", trashed)?;
    set_label(tx, mb.account_id, id, "INBOX", !trashed)?;
    let op = if trashed { "thread_trash" } else { "thread_untrash" };
    enqueue(tx, mb.account_id, op, &serde_json::json!({ "thread_id": mb.thread_id }), now)
}

pub(crate) fn spam_in(tx: &Transaction<'_>, id: i64, spam: bool, now: i64) -> Result<()> {
    let mb = mailbox(tx, id)?;
    set_label(tx, mb.account_id, id, "SPAM", spam)?;
    set_label(tx, mb.account_id, id, "INBOX", !spam)?;
    let (add, remove) = if spam { (vec!["SPAM"], vec!["INBOX"]) } else { (vec!["INBOX"], vec!["SPAM"]) };
    enqueue(
        tx,
        mb.account_id,
        "thread_modify",
        &serde_json::json!({ "thread_id": mb.thread_id, "add": add, "remove": remove }),
        now,
    )
}

pub(crate) fn sole_conversations(tx: &Transaction<'_>, person_id: i64) -> Result<Vec<i64>> {
    let mut stmt = tx.prepare_cached(
        "SELECT cp.conversation_id FROM conversation_people cp
         WHERE cp.person_id = ?1
           AND (SELECT COUNT(*) FROM conversation_people x WHERE x.conversation_id = cp.conversation_id) = 1",
    )?;
    let rows = stmt.query_map([person_id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

pub(crate) fn sync_person_visibility(tx: &Transaction<'_>, person_id: i64, now: i64) -> Result<()> {
    let state = person_state(tx, person_id)?;
    let reason = format!("person:{person_id}");
    let conversations = sole_conversations(tx, person_id)?;
    let hide = state.hidden_at.is_some() || state.status == "blocked";
    for id in conversations {
        if hide {
            tx.execute(
                "UPDATE conversations SET hidden_at = ?2, hidden_reason = ?3, hidden_until = ?4
                 WHERE id = ?1 AND hidden_at IS NULL",
                params![id, state.hidden_at.unwrap_or(now), reason, state.hidden_until],
            )?;
            if state.status == "blocked" {
                let in_spam: bool = tx.query_row(
                    "SELECT EXISTS (SELECT 1 FROM message_labels ml JOIN labels l ON l.id = ml.label_id
                                        JOIN messages m ON m.id = ml.message_id
                                        WHERE m.conversation_id = ?1 AND l.provider_id = 'SPAM')",
                    [id],
                    |r| r.get(0),
                )?;
                if !in_spam {
                    spam_in(tx, id, true, now)?;
                }
            }
        } else {
            tx.execute(
                "UPDATE conversations SET hidden_at = NULL, hidden_reason = NULL, hidden_until = NULL
                 WHERE id = ?1 AND hidden_reason = ?2",
                params![id, reason],
            )?;
        }
        recompute_conversation(tx, id)?;
    }
    refresh_person(tx, person_id)?;
    Ok(())
}

pub(crate) fn decide_in(tx: &Transaction<'_>, person_id: i64, decision: SenderDecision, now: i64) -> Result<UndoOp> {
    let before = person_state(tx, person_id)?;
    let mut next = before.clone();
    match decision {
        SenderDecision::Allow => {
            next.status = "known".into();
            next.hidden_at = None;
            next.hidden_until = None;
        }
        SenderDecision::Hide => {
            next.status = "known".into();
            next.hidden_at = Some(now);
        }
        SenderDecision::Block => {
            next.status = "blocked".into();
            next.hidden_at = None;
        }
    }
    write_person_state(tx, person_id, &next)?;
    sync_person_visibility(tx, person_id, now)?;
    Ok(UndoOp::Person { person: person_id, state: before })
}

fn domain_of(email: &str) -> &str {
    email.rsplit_once('@').map_or("", |(_, d)| d)
}

pub(crate) fn apply_domain_rule(tx: &Transaction<'_>, person_id: i64, email: &str, now: i64) -> Result<()> {
    let domain = domain_of(email);
    if domain.is_empty() {
        return Ok(());
    }
    let action: Option<String> =
        tx.query_row("SELECT action FROM domain_rules WHERE domain = ?1", [domain], |r| r.get(0)).optional()?;
    match action.as_deref() {
        Some("allow") => {
            tx.execute("UPDATE people SET status = 'known' WHERE id = ?1 AND status = 'new_sender'", [person_id])?;
        }
        Some("hide") => {
            tx.execute(
                "UPDATE people SET status = 'known', hidden_at = ?2 WHERE id = ?1 AND status = 'new_sender'",
                params![person_id, now],
            )?;
        }
        Some("block") => {
            tx.execute("UPDATE people SET status = 'blocked' WHERE id = ?1 AND status = 'new_sender'", [person_id])?;
        }
        _ => {}
    }
    Ok(())
}

impl Store {
    pub fn set_archived(&mut self, conversation_id: i64, archived: bool, now: i64) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let before = mailbox(tx, conversation_id)?;
            archive_in(tx, conversation_id, archived, now)?;
            let touched = finish(tx, &[conversation_id], &[])?;
            Ok((
                touched,
                UndoOp::Mailbox { conversation: conversation_id, in_inbox: before.in_inbox, trashed: before.trashed },
            ))
        })
    }

    pub fn set_trashed(&mut self, conversation_id: i64, trashed: bool, now: i64) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let before = mailbox(tx, conversation_id)?;
            trash_in(tx, conversation_id, trashed, now)?;
            let touched = finish(tx, &[conversation_id], &[])?;
            Ok((
                touched,
                UndoOp::Mailbox { conversation: conversation_id, in_inbox: before.in_inbox, trashed: before.trashed },
            ))
        })
    }

    pub fn set_conversation_hidden(
        &mut self,
        conversation_id: i64,
        hidden: bool,
        until: Option<i64>,
        now: i64,
    ) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let before: (Option<i64>, Option<i64>, Option<String>) = tx
                .query_row(
                    "SELECT hidden_at, hidden_until, hidden_reason FROM conversations WHERE id = ?1",
                    [conversation_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?
                .ok_or(Error::NotFound("conversation"))?;
            if hidden {
                tx.execute(
                    "UPDATE conversations SET hidden_at = ?2, hidden_until = ?3, hidden_reason = 'manual' WHERE id = ?1",
                    params![conversation_id, now, until],
                )?;
            } else {
                tx.execute(
                    "UPDATE conversations SET hidden_at = NULL, hidden_until = NULL, hidden_reason = NULL WHERE id = ?1",
                    [conversation_id],
                )?;
            }
            let touched = finish(tx, &[conversation_id], &[])?;
            Ok((touched, UndoOp::Hidden { conversation: conversation_id, hidden_at: before.0, until: before.1, reason: before.2 }))
        })
    }

    pub fn decide_sender(&mut self, person_id: i64, decision: SenderDecision, now: i64) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let undo = decide_in(tx, person_id, decision, now)?;
            Ok((Touched { people: vec![person_id], conversations: sole_conversations(tx, person_id)? }, undo))
        })
    }

    pub fn decide_domain(
        &mut self,
        domain: &str,
        decision: SenderDecision,
        now: i64,
    ) -> Result<(Touched, UndoOp, usize)> {
        let domain = domain.trim().trim_start_matches('@').to_lowercase();
        let action = match decision {
            SenderDecision::Allow => "allow",
            SenderDecision::Hide => "hide",
            SenderDecision::Block => "block",
        };
        let previous: Option<String> = self
            .conn()
            .query_row("SELECT action FROM domain_rules WHERE domain = ?1", [&domain], |r| r.get(0))
            .optional()?;
        self.set_domain_rule(&domain, Some(action), now)?;
        self.in_transaction(|tx| {
            let waiting: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT p.id, a.address FROM people p JOIN person_addresses a ON a.person_id = p.id
                     WHERE p.status = 'new_sender' AND a.is_primary = 1",
                )?;
                let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|(_, address)| domain_of(address).eq_ignore_ascii_case(&domain))
                    .map(|(id, _)| id)
                    .collect()
            };
            let mut ops = vec![UndoOp::Domain { domain: domain.clone(), action: previous }];
            let mut conversations = Vec::new();
            for id in &waiting {
                ops.push(decide_in(tx, *id, decision, now)?);
                conversations.extend(sole_conversations(tx, *id)?);
            }
            Ok((Touched { people: waiting.clone(), conversations }, UndoOp::Many(ops), waiting.len()))
        })
    }

    pub fn set_person_hidden(
        &mut self,
        person_id: i64,
        hidden: bool,
        until: Option<i64>,
        now: i64,
    ) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let before = person_state(tx, person_id)?;
            let mut next = before.clone();
            next.hidden_at = hidden.then_some(now);
            next.hidden_until = if hidden { until } else { None };
            if !hidden && next.status == "blocked" {
                next.status = "known".into();
            }
            write_person_state(tx, person_id, &next)?;
            sync_person_visibility(tx, person_id, now)?;
            Ok((
                Touched { people: vec![person_id], conversations: sole_conversations(tx, person_id)? },
                UndoOp::Person { person: person_id, state: before },
            ))
        })
    }

    pub fn set_person_flags(&mut self, person_id: i64, flags: &PersonFlags) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let before = person_state(tx, person_id)?;
            let next =
                PersonState { whitelisted: flags.whitelisted, vip: flags.vip, muted: flags.muted, ..before.clone() };
            write_person_state(tx, person_id, &next)?;
            Ok((
                Touched { people: vec![person_id], conversations: vec![] },
                UndoOp::Person { person: person_id, state: before },
            ))
        })
    }

    pub fn person_flags(&self, person_id: i64) -> Result<PersonFlags> {
        self.conn()
            .query_row("SELECT whitelisted, vip, muted FROM people WHERE id = ?1", [person_id], |r| {
                Ok(PersonFlags { whitelisted: r.get(0)?, vip: r.get(1)?, muted: r.get(2)? })
            })
            .optional()?
            .ok_or(Error::NotFound("person"))
    }

    pub fn apply_undo(&mut self, op: &UndoOp, now: i64) -> Result<Touched> {
        self.in_transaction(|tx| apply_undo_in(tx, op, now))
    }

    pub fn list_new_senders(&self) -> Result<Vec<NewSender>> {
        let sql = format!(
            "SELECT {SUMMARY_COLUMNS},
               COALESCE((SELECT address FROM person_addresses WHERE person_id = p.id ORDER BY is_primary DESC LIMIT 1), ''),
               COALESCE((SELECT m.subject FROM messages m WHERE m.from_person_id = p.id ORDER BY m.sent_at LIMIT 1), ''),
               COALESCE((SELECT m.snippet FROM messages m WHERE m.from_person_id = p.id ORDER BY m.sent_at LIMIT 1), ''),
               (SELECT COUNT(*) FROM messages m WHERE m.from_person_id = p.id),
               (SELECT COUNT(*) FROM attachments a JOIN messages m ON m.id = a.message_id
                  WHERE m.from_person_id = p.id AND a.inline = 0),
               COALESCE((SELECT MIN(m.sent_at) FROM messages m WHERE m.from_person_id = p.id), 0),
               (p.kind = 'list' OR EXISTS (SELECT 1 FROM messages m WHERE m.from_person_id = p.id AND m.list_id IS NOT NULL))
             FROM people p WHERE p.status = 'new_sender' AND p.hidden_at IS NULL{}
             ORDER BY COALESCE(p.last_message_at, 0) DESC, p.id",
            self.in_account("p")
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            let address: String = r.get(15)?;
            Ok(NewSender {
                person: summary_from_row(r)?,
                domain: domain_of(&address).to_string(),
                address,
                subject: r.get(16)?,
                snippet: r.get(17)?,
                message_count: u32::try_from(r.get::<_, i64>(18)?).unwrap_or(0),
                file_count: u32::try_from(r.get::<_, i64>(19)?).unwrap_or(0),
                first_at: r.get(20)?,
                bulk: r.get(21)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn list_hidden(&self) -> Result<HiddenOverview> {
        let sql = format!(
            "SELECT {SUMMARY_COLUMNS},
               COALESCE((SELECT address FROM person_addresses WHERE person_id = p.id ORDER BY is_primary DESC LIMIT 1), ''),
               p.hidden_until,
               COALESCE((SELECT MAX(m.sent_at) FROM messages m WHERE m.from_person_id = p.id), 0)
             FROM people p WHERE (p.hidden_at IS NOT NULL OR p.status = 'blocked') AND p.kind <> 'self'{}
             ORDER BY p.display_name COLLATE NOCASE",
            self.in_account("p")
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let people = stmt
            .query_map([], |r| {
                let mut person = summary_from_row(r)?;
                person.last_at = r.get(17)?;
                let blocked = person.status == crate::dto::PersonStatus::Blocked;
                Ok(HiddenPerson { person, address: r.get(15)?, blocked, until: r.get(16)? })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut stmt = self.conn().prepare(
            "SELECT c.id, c.hidden_until FROM conversations c
             WHERE c.hidden_at IS NOT NULL AND COALESCE(c.hidden_reason, 'manual') = 'manual'
             ORDER BY c.last_message_at DESC",
        )?;
        let ids: Vec<(i64, Option<i64>)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        let mut conversations = Vec::new();
        for (id, until) in ids {
            let person_id = self.primary_person_of(id)?;
            if let Some(summary) = self.conversation_summary(id, person_id)? {
                let person_name = match person_id {
                    Some(pid) => self.person_summary(pid)?.map(|p| p.name).unwrap_or_default(),
                    None => String::new(),
                };
                conversations.push(HiddenConversation { conversation: summary, person_id, person_name, until });
            }
        }
        Ok(HiddenOverview { people, conversations })
    }

    pub fn lift_expired_hides(&mut self, now: i64) -> Result<Touched> {
        self.in_transaction(|tx| {
            let people: Vec<i64> = {
                let mut stmt = tx.prepare("SELECT id FROM people WHERE hidden_until IS NOT NULL AND hidden_until <= ?1")?;
                stmt.query_map([now], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
            };
            for id in &people {
                tx.execute("UPDATE people SET hidden_at = NULL, hidden_until = NULL WHERE id = ?1", [id])?;
                sync_person_visibility(tx, *id, now)?;
            }
            let conversations: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT id FROM conversations WHERE hidden_until IS NOT NULL AND hidden_until <= ?1
                       AND COALESCE(hidden_reason, 'manual') = 'manual'",
                )?;
                stmt.query_map([now], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
            };
            for id in &conversations {
                tx.execute(
                    "UPDATE conversations SET hidden_at = NULL, hidden_until = NULL, hidden_reason = NULL WHERE id = ?1",
                    [id],
                )?;
            }
            finish(tx, &conversations, &people)
        })
    }

    pub fn set_domain_rule(&mut self, domain: &str, action: Option<&str>, now: i64) -> Result<()> {
        let domain = domain.trim().trim_start_matches('@').to_lowercase();
        if domain.is_empty() || !domain.contains('.') || domain.contains(char::is_whitespace) {
            return Err(Error::Invalid("Enter a domain like example.com".into()));
        }
        match action {
            Some(a) if matches!(a, "allow" | "hide" | "block") => {
                self.conn().execute(
                    "INSERT INTO domain_rules (domain, action, created_at) VALUES (?1, ?2, ?3)
                     ON CONFLICT(domain) DO UPDATE SET action = excluded.action",
                    params![domain, a, now],
                )?;
            }
            Some(_) => return Err(Error::Invalid("A domain rule must allow, hide or block".into())),
            None => {
                self.conn().execute("DELETE FROM domain_rules WHERE domain = ?1", [domain])?;
            }
        }
        Ok(())
    }

    pub fn domain_rules(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn().prepare("SELECT domain, action FROM domain_rules ORDER BY domain")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn create_group(&mut self, name: &str, ink: u8, now: i64) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("A group needs a name".into()));
        }
        let exists: bool =
            self.conn().query_row("SELECT EXISTS (SELECT 1 FROM groups WHERE name = ?1)", [name], |r| r.get(0))?;
        if exists {
            return Err(Error::Invalid(format!("There is already a group called {name}")));
        }
        self.conn().execute(
            "INSERT INTO groups (name, ink, created_at) VALUES (?1, ?2, ?3)",
            params![name, ink.min(9), now],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn delete_group(&mut self, id: i64) -> Result<()> {
        let changed = self.conn().execute("DELETE FROM groups WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(Error::NotFound("group"));
        }
        Ok(())
    }

    pub fn list_groups(&self) -> Result<Vec<GroupInfo>> {
        let mut stmt = self.conn().prepare(
            "SELECT g.id, g.name, g.ink, (SELECT COUNT(*) FROM person_groups pg WHERE pg.group_id = g.id),
                    g.icon, g.description, g.image, g.sidebar
             FROM groups g ORDER BY g.position, g.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(GroupInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                ink: r.get::<_, i64>(2)? as u8,
                members: u32::try_from(r.get::<_, i64>(3)?).unwrap_or(0),
                icon: r.get(4)?,
                description: r.get(5)?,
                image: r.get(6)?,
                sidebar: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_person_groups(&mut self, person_id: i64, group_ids: &[i64]) -> Result<Touched> {
        self.in_transaction(|tx| {
            tx.query_row("SELECT 1 FROM people WHERE id = ?1", [person_id], |_| Ok(()))
                .optional()?
                .ok_or(Error::NotFound("person"))?;
            tx.execute("DELETE FROM person_groups WHERE person_id = ?1", [person_id])?;
            for id in group_ids {
                tx.execute(
                    "INSERT OR IGNORE INTO person_groups (person_id, group_id) SELECT ?1, id FROM groups WHERE id = ?2",
                    params![person_id, id],
                )?;
            }
            Ok(Touched { people: vec![person_id], conversations: vec![] })
        })
    }

    pub fn groups_of(&self, person_id: i64) -> Result<Vec<i64>> {
        let mut stmt =
            self.conn().prepare("SELECT group_id FROM person_groups WHERE person_id = ?1 ORDER BY group_id")?;
        let rows = stmt.query_map([person_id], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

fn mirror_threads(tx: &Transaction<'_>, messages: &[i64], tag: &str, add: bool, now: i64) -> Result<()> {
    let name = tag.strip_prefix("zsm:").unwrap_or(tag);
    let mut seen = std::collections::BTreeSet::new();
    for id in messages {
        let row: Option<(i64, String)> = tx
            .query_row(
                "SELECT c.account_id, c.provider_id FROM messages m JOIN conversations c ON c.id = m.conversation_id WHERE m.id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((account, thread)) = row
            && seen.insert((account, thread.clone()))
        {
            super::tags::mirror(tx, account, &thread, name, add, now)?;
        }
    }
    Ok(())
}

fn apply_undo_in(tx: &Transaction<'_>, op: &UndoOp, now: i64) -> Result<Touched> {
    match op {
        UndoOp::Mailbox { conversation, in_inbox, trashed } => {
            let now_state = mailbox(tx, *conversation)?;
            if now_state.trashed != *trashed {
                trash_in(tx, *conversation, *trashed, now)?;
            }
            if !*trashed && mailbox(tx, *conversation)?.in_inbox != *in_inbox {
                archive_in(tx, *conversation, !*in_inbox, now)?;
            }
            finish(tx, &[*conversation], &[])
        }
        UndoOp::Hidden { conversation, hidden_at, until, reason } => {
            tx.execute(
                "UPDATE conversations SET hidden_at = ?2, hidden_until = ?3, hidden_reason = ?4 WHERE id = ?1",
                params![conversation, hidden_at, until, reason],
            )?;
            finish(tx, &[*conversation], &[])
        }
        UndoOp::Person { person, state } => {
            let current = person_state(tx, *person)?;
            write_person_state(tx, *person, state)?;
            if current.status == "blocked" && state.status != "blocked" {
                for id in sole_conversations(tx, *person)? {
                    spam_in(tx, id, false, now)?;
                }
            }
            sync_person_visibility(tx, *person, now)?;
            let conversations = sole_conversations(tx, *person)?;
            finish(tx, &conversations, &[*person])
        }
        UndoOp::Read { conversation, unread_ids } => {
            tx.execute("UPDATE messages SET unread = 0 WHERE conversation_id = ?1", [conversation])?;
            for id in unread_ids {
                tx.execute("UPDATE messages SET unread = 1 WHERE id = ?1", [id])?;
            }
            finish(tx, &[*conversation], &[])
        }
        UndoOp::Snooze { conversation, until } => {
            tx.execute("UPDATE conversations SET snoozed_until = ?2 WHERE id = ?1", params![conversation, until])?;
            finish(tx, &[*conversation], &[])
        }
        UndoOp::Star { message_ids, starred } => {
            let mut conversations = std::collections::BTreeSet::new();
            for id in message_ids {
                tx.execute("UPDATE messages SET starred = ?2 WHERE id = ?1", params![id, starred])?;
                if let Some(c) = tx
                    .query_row("SELECT conversation_id FROM messages WHERE id = ?1", [id], |r| r.get::<_, i64>(0))
                    .optional()?
                {
                    conversations.insert(c);
                }
            }
            let conversations: Vec<i64> = conversations.into_iter().collect();
            finish(tx, &conversations, &[])
        }
        UndoOp::Untag { tag, message_ids } => {
            mirror_threads(tx, message_ids, tag, false, now)?;
            for id in message_ids {
                tx.execute(
                    "DELETE FROM message_labels WHERE message_id = ?1
                       AND label_id IN (SELECT id FROM labels WHERE provider_id = ?2)",
                    params![id, tag],
                )?;
            }
            Ok(Touched::default())
        }
        UndoOp::Domain { domain, action } => {
            match action {
                Some(action) => {
                    tx.execute(
                        "INSERT INTO domain_rules (domain, action, created_at) VALUES (?1, ?2, ?3)
                         ON CONFLICT(domain) DO UPDATE SET action = excluded.action",
                        params![domain, action, now],
                    )?;
                }
                None => {
                    tx.execute("DELETE FROM domain_rules WHERE domain = ?1", [domain])?;
                }
            }
            Ok(Touched::default())
        }
        UndoOp::Retag { account, tag, name, message_ids } => {
            super::tags::add_tag(tx, *account, tag, name, message_ids)?;
            mirror_threads(tx, message_ids, tag, true, now)?;
            Ok(Touched::default())
        }
        UndoOp::Unmerge { keep, absorbed, snapshot } => super::merge::unmerge(tx, *keep, *absorbed, snapshot),
        UndoOp::Rejoin { keep, created, address, was_primary, messages } => {
            super::merge::rejoin(tx, *keep, *created, address, *was_primary, messages)
        }
        UndoOp::Many(ops) => {
            let mut touched = Touched::default();
            for op in ops.iter().rev() {
                let t = apply_undo_in(tx, op, now)?;
                touched.people.extend(t.people);
                touched.conversations.extend(t.conversations);
            }
            touched.people.sort_unstable();
            touched.people.dedup();
            touched.conversations.sort_unstable();
            touched.conversations.dedup();
            Ok(touched)
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;

    use super::*;
    use crate::demo::{DemoOptions, seed_demo};
    use crate::dto::PeopleFilter;
    use crate::store::ingest::{Address, IncomingMessage, IngestContext};

    const NOW: i64 = 1_795_082_400_000;
    const DAY: i64 = 86_400_000;

    fn demo() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        seed_demo(&mut store, &DemoOptions { now_ms: NOW, stress_people: 0, tz: TimeZone::UTC }).unwrap();
        store
    }

    fn person(store: &Store, name: &str) -> i64 {
        store
            .conn()
            .query_row("SELECT id FROM people WHERE display_name LIKE ?1", [format!("{name}%")], |r| r.get(0))
            .unwrap()
    }

    fn visible_people(store: &Store) -> Vec<String> {
        store.list_people(PeopleFilter::All, "").unwrap().into_iter().map(|p| p.name).collect()
    }

    fn convo_of(store: &Store, person: i64, subject: &str) -> i64 {
        store.list_conversations(person).unwrap().into_iter().find(|c| c.subject == subject).unwrap().id
    }

    #[test]
    fn archiving_takes_a_conversation_out_of_the_inbox_and_undo_puts_it_back() {
        let mut store = demo();
        let maya = person(&store, "Maya");
        let convo = convo_of(&store, maya, "Coffee Thursday?");
        let inbox = |s: &Store| -> bool {
            s.conn().query_row("SELECT in_inbox FROM conversations WHERE id = ?1", [convo], |r| r.get(0)).unwrap()
        };
        assert!(inbox(&store));

        let (touched, undo) = store.set_archived(convo, true, NOW).unwrap();
        assert_eq!(touched.conversations, vec![convo]);
        assert!(!inbox(&store));
        assert_eq!(store.list_conversations(maya).unwrap().len(), 4, "archived mail is still under the person");

        store.apply_undo(&undo, NOW).unwrap();
        assert!(inbox(&store));
    }

    #[test]
    fn trashing_hides_a_conversation_everywhere_and_undo_restores_the_inbox_state() {
        let mut store = demo();
        let rasmus = person(&store, "Rasmus");
        let convo = convo_of(&store, rasmus, "Tax return questions");
        let before = store.list_conversations(rasmus).unwrap().len();
        let (_, undo) = store.set_trashed(convo, true, NOW).unwrap();
        assert_eq!(store.list_conversations(rasmus).unwrap().len(), before - 1);
        assert!(store.search("floor plan", NOW, 5).unwrap().messages.is_empty(), "trashed mail is not searchable");
        assert!(
            store.count_matching(&crate::query::parse("is:trashed"), NOW, super::super::query::Scope::Visible).unwrap()
                > 0
        );

        store.apply_undo(&undo, NOW).unwrap();
        assert_eq!(store.list_conversations(rasmus).unwrap().len(), before);
        let in_inbox: bool =
            store.conn().query_row("SELECT in_inbox FROM conversations WHERE id = ?1", [convo], |r| r.get(0)).unwrap();
        assert!(
            in_inbox
                && !store
                    .conn()
                    .query_row::<bool, _, _>("SELECT trashed FROM conversations WHERE id = ?1", [convo], |r| r.get(0))
                    .unwrap()
        );
    }

    #[test]
    fn trashing_and_archiving_queue_the_right_changes_for_a_linked_account() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.upsert_account("gmail", "me@example.com", None, NOW).unwrap();
        let ctx = IngestContext {
            account_id: account,
            self_addresses: vec!["me@example.com".into()],
            screener: false,
            automation: false,
        };
        let msg = IncomingMessage {
            provider_id: "m1".into(),
            thread_id: "t1".into(),
            rfc_message_id: None,
            in_reply_to: None,
            from: Address::new("a@b.example", Some("Ann Bee")),
            to: vec![Address::new("me@example.com", None)],
            cc: vec![],
            subject: "Hello".into(),
            sent_at: NOW,
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
        let convo = store.ingest(&ctx, &msg, NOW).unwrap().conversation_id;
        store.set_archived(convo, true, NOW).unwrap();
        store.set_trashed(convo, true, NOW).unwrap();
        store.set_trashed(convo, false, NOW).unwrap();
        let ops: Vec<(String, String)> = store
            .outbox_peek(NOW, 10)
            .unwrap()
            .into_iter()
            .map(|i| (i.op, i.payload["thread_id"].as_str().unwrap_or_default().to_string()))
            .collect();
        assert_eq!(
            ops,
            [
                ("thread_modify".to_string(), "t1".to_string()),
                ("thread_trash".into(), "t1".into()),
                ("thread_untrash".into(), "t1".into())
            ]
        );
    }

    #[test]
    fn a_hidden_conversation_leaves_lists_and_search_and_shows_under_hidden() {
        let mut store = demo();
        let maya = person(&store, "Maya");
        let convo = convo_of(&store, maya, "Invoice 2291");
        let (_, undo) = store.set_conversation_hidden(convo, true, Some(NOW + 7 * DAY), NOW).unwrap();
        assert!(store.list_conversations(maya).unwrap().iter().all(|c| c.id != convo));
        assert!(store.search("PO number", NOW, 5).unwrap().messages.is_empty());
        let hidden = store.list_hidden().unwrap();
        assert_eq!(hidden.conversations.len(), 1);
        assert_eq!(hidden.conversations[0].conversation.subject, "Invoice 2291");
        assert_eq!(hidden.conversations[0].until, Some(NOW + 7 * DAY));

        store.lift_expired_hides(NOW + DAY).unwrap();
        assert_eq!(store.list_hidden().unwrap().conversations.len(), 1, "not yet");
        store.lift_expired_hides(NOW + 8 * DAY).unwrap();
        assert!(store.list_hidden().unwrap().conversations.is_empty());
        assert!(store.list_conversations(maya).unwrap().iter().any(|c| c.id == convo));

        store.set_conversation_hidden(convo, true, None, NOW).unwrap();
        store.apply_undo(&undo, NOW).unwrap();
        assert!(store.list_hidden().unwrap().conversations.is_empty());
    }

    #[test]
    fn the_new_senders_deck_allows_hides_and_blocks_and_all_of_it_can_be_undone() {
        let mut store = demo();
        let waiting = store.list_new_senders().unwrap();
        assert_eq!(waiting.len(), 5);
        let priya = waiting.iter().find(|n| n.person.name == "Priya Raman").unwrap();
        assert_eq!(priya.domain, "parley.example");
        assert!(priya.subject.contains("Intro") && priya.snippet.contains("translation tool"));
        assert_eq!(priya.message_count, 1);

        let (_, undo_allow) = store.decide_sender(priya.person.id, SenderDecision::Allow, NOW).unwrap();
        assert!(visible_people(&store).contains(&"Priya Raman".to_string()));
        assert_eq!(store.list_new_senders().unwrap().len(), 4);
        store.apply_undo(&undo_allow, NOW).unwrap();
        assert!(!visible_people(&store).contains(&"Priya Raman".to_string()));
        assert_eq!(store.list_new_senders().unwrap().len(), 5);

        let marcus = person(&store, "Marcus");
        let (_, undo_hide) = store.decide_sender(marcus, SenderDecision::Hide, NOW).unwrap();
        assert!(!visible_people(&store).contains(&"Marcus Lee".to_string()));
        assert_eq!(store.list_new_senders().unwrap().len(), 4);
        assert!(store.list_hidden().unwrap().people.iter().any(|h| h.person.name == "Marcus Lee" && !h.blocked));
        assert!(store.search("tasting box", NOW, 5).unwrap().messages.is_empty());
        store.apply_undo(&undo_hide, NOW).unwrap();
        assert_eq!(store.list_new_senders().unwrap().len(), 5);
        assert!(!store.search("tasting box", NOW, 5).unwrap().messages.is_empty(), "visible again");

        let ioana = person(&store, "Ioana");
        let (touched, undo_block) = store.decide_sender(ioana, SenderDecision::Block, NOW).unwrap();
        assert!(touched.people.contains(&ioana));
        let hidden = store.list_hidden().unwrap();
        assert!(hidden.people.iter().any(|h| h.person.name == "Ioana Popescu" && h.blocked));
        assert_eq!(store.list_new_senders().unwrap().len(), 4);
        store.apply_undo(&undo_block, NOW).unwrap();
        assert_eq!(store.list_new_senders().unwrap().len(), 5);
        assert!(store.list_hidden().unwrap().people.is_empty());
    }

    #[test]
    fn blocking_a_sender_on_a_linked_account_sends_their_mail_to_spam_and_undo_restores_it() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.upsert_account("gmail", "me@example.com", None, NOW).unwrap();
        let ctx = IngestContext {
            account_id: account,
            self_addresses: vec!["me@example.com".into()],
            screener: true,
            automation: false,
        };
        let mk = |id: &str, thread: &str, at: i64| IncomingMessage {
            provider_id: id.into(),
            thread_id: thread.into(),
            rfc_message_id: None,
            in_reply_to: None,
            from: Address::new("seller@shop.example", Some("Seller")),
            to: vec![Address::new("me@example.com", None)],
            cc: vec![],
            subject: "Buy now".into(),
            sent_at: at,
            snippet: None,
            text: Some("cheap".into()),
            html: None,
            attachments: vec![],
            labels: vec!["INBOX".into(), "UNREAD".into()],
            unread: true,
            starred: false,
            headers: Default::default(),
            size_bytes: None,
        };
        let first = store.ingest(&ctx, &mk("m1", "t1", NOW - 1000), NOW).unwrap();
        let seller = first.sender_person_id.unwrap();
        let (_, undo) = store.decide_sender(seller, SenderDecision::Block, NOW).unwrap();
        let queued: Vec<serde_json::Value> =
            store.outbox_peek(NOW, 10).unwrap().into_iter().map(|i| i.payload).collect();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0]["add"], serde_json::json!(["SPAM"]));
        assert_eq!(queued[0]["remove"], serde_json::json!(["INBOX"]));

        store.ingest(&ctx, &mk("m2", "t2", NOW), NOW).unwrap();
        let again = store.outbox_peek(NOW, 10).unwrap();
        assert_eq!(again.len(), 2);
        assert!(store.list_people(PeopleFilter::All, "Seller").unwrap().is_empty());
        let visible: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM conversations WHERE hidden_at IS NULL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(visible, 0, "blocked mail is out of sight");

        store.apply_undo(&undo, NOW).unwrap();
        let last = store.outbox_peek(NOW, 10).unwrap();
        assert!(last.iter().any(|i| i.payload["remove"] == serde_json::json!(["SPAM"])), "and comes back out of Spam");
    }

    #[test]
    fn mail_from_a_hidden_person_stays_hidden_and_a_group_thread_does_not() {
        let mut store = demo();
        let dad = person(&store, "Dad");
        let manual = convo_of(&store, dad, "dinner sat?");
        store.set_person_hidden(dad, true, None, NOW).unwrap();
        assert!(!visible_people(&store).contains(&"Dad".to_string()));
        let ctx = IngestContext {
            account_id: 1,
            self_addresses: vec!["you@zsortmail.demo".into()],
            screener: false,
            automation: false,
        };
        let msg = IncomingMessage {
            provider_id: "new-dad".into(),
            thread_id: "dad-new".into(),
            rfc_message_id: None,
            in_reply_to: None,
            from: Address::new("pete.harlan@mail.example", Some("Dad")),
            to: vec![Address::new("you@zsortmail.demo", None)],
            cc: vec![],
            subject: "Saturday plans".into(),
            sent_at: NOW,
            snippet: None,
            text: Some("Mum says seven.".into()),
            html: None,
            attachments: vec![],
            labels: vec!["INBOX".into()],
            unread: true,
            starred: false,
            headers: Default::default(),
            size_bytes: None,
        };
        let out = store.ingest(&ctx, &msg, NOW).unwrap();
        let hidden: Option<i64> = store
            .conn()
            .query_row("SELECT hidden_at FROM conversations WHERE id = ?1", [out.conversation_id], |r| r.get(0))
            .unwrap();
        assert!(hidden.is_some(), "the new conversation is hidden along with the person");
        assert!(store.search("Mum says seven", NOW, 5).unwrap().messages.is_empty());

        store.set_conversation_hidden(manual, true, None, NOW).unwrap();
        store.set_person_hidden(dad, false, None, NOW).unwrap();
        let after = store.list_conversations(dad).unwrap();
        assert!(after.iter().any(|c| c.subject == "Saturday plans"));
        assert!(after.iter().all(|c| c.subject != "dinner sat?"), "hidden by hand, so still hidden");
    }

    #[test]
    fn domain_rules_decide_new_senders_before_the_screener_asks() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.upsert_account("demo", "me@example.com", None, NOW).unwrap();
        store.set_domain_rule("@Shop.Example", Some("block"), NOW).unwrap();
        store.set_domain_rule("friends.example", Some("allow"), NOW).unwrap();
        store.set_domain_rule("noise.example", Some("hide"), NOW).unwrap();
        assert_eq!(store.domain_rules().unwrap().len(), 3);
        assert!(store.set_domain_rule("nodot", Some("block"), NOW).is_err());
        assert!(store.set_domain_rule("a.example", Some("explode"), NOW).is_err());

        let ctx = IngestContext {
            account_id: account,
            self_addresses: vec!["me@example.com".into()],
            screener: true,
            automation: false,
        };
        for (i, from) in
            ["a@shop.example", "b@friends.example", "c@noise.example", "d@unknown.example"].iter().enumerate()
        {
            let msg = IncomingMessage {
                provider_id: format!("m{i}"),
                thread_id: format!("t{i}"),
                rfc_message_id: None,
                in_reply_to: None,
                from: Address::new(from, None),
                to: vec![Address::new("me@example.com", None)],
                cc: vec![],
                subject: "Hello".into(),
                sent_at: NOW,
                snippet: None,
                text: Some("hi".into()),
                html: None,
                attachments: vec![],
                labels: vec!["INBOX".into()],
                unread: true,
                starred: false,
                headers: Default::default(),
                size_bytes: None,
            };
            store.ingest(&ctx, &msg, NOW).unwrap();
        }
        let status = |email: &str| -> (String, bool) {
            store
                .conn()
                .query_row(
                    "SELECT p.status, p.hidden_at IS NOT NULL FROM people p JOIN person_addresses a ON a.person_id = p.id WHERE a.address = ?1",
                    [email],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap()
        };
        assert_eq!(status("a@shop.example").0, "blocked");
        assert_eq!(status("b@friends.example"), ("known".into(), false));
        assert_eq!(status("c@noise.example"), ("known".into(), true));
        assert_eq!(status("d@unknown.example").0, "new_sender", "no rule, so the screener decides");
        assert_eq!(store.list_new_senders().unwrap().len(), 1);

        store.set_domain_rule("shop.example", None, NOW).unwrap();
        assert_eq!(store.domain_rules().unwrap().len(), 2);
    }

    #[test]
    fn a_whole_domain_can_be_decided_at_once_and_undone_together() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.upsert_account("demo", "me@example.com", None, NOW).unwrap();
        let ctx = IngestContext {
            account_id: account,
            self_addresses: vec!["me@example.com".into()],
            screener: true,
            automation: false,
        };
        for (i, from) in ["a@shop.example", "b@shop.example", "c@other.example"].iter().enumerate() {
            let msg = IncomingMessage {
                provider_id: format!("m{i}"),
                thread_id: format!("t{i}"),
                rfc_message_id: None,
                in_reply_to: None,
                from: Address::new(from, None),
                to: vec![Address::new("me@example.com", None)],
                cc: vec![],
                subject: "Hello".into(),
                sent_at: NOW,
                snippet: None,
                text: Some("hi".into()),
                html: None,
                attachments: vec![],
                labels: vec!["INBOX".into()],
                unread: true,
                starred: false,
                headers: Default::default(),
                size_bytes: None,
            };
            store.ingest(&ctx, &msg, NOW).unwrap();
        }
        assert_eq!(store.list_new_senders().unwrap().len(), 3);
        let (touched, undo, waiting) = store.decide_domain("Shop.Example", SenderDecision::Block, NOW).unwrap();
        assert_eq!(waiting, 2);
        assert_eq!(touched.people.len(), 2);
        assert_eq!(store.list_new_senders().unwrap().len(), 1, "only the other domain still waits");
        assert_eq!(store.domain_rules().unwrap(), vec![("shop.example".to_string(), "block".to_string())]);

        store.apply_undo(&undo, NOW).unwrap();
        assert_eq!(store.list_new_senders().unwrap().len(), 3);
        assert!(store.domain_rules().unwrap().is_empty(), "the rule goes with it");
    }

    #[test]
    fn flags_and_groups() {
        let mut store = demo();
        let maya = person(&store, "Maya");
        let (_, undo) =
            store.set_person_flags(maya, &PersonFlags { whitelisted: true, vip: true, muted: false }).unwrap();
        assert_eq!(store.person_flags(maya).unwrap(), PersonFlags { whitelisted: true, vip: true, muted: false });
        store.apply_undo(&undo, NOW).unwrap();
        assert_eq!(store.person_flags(maya).unwrap(), PersonFlags { whitelisted: false, vip: false, muted: false });

        let work = store.create_group(" Work ", 3, NOW).unwrap();
        let family = store.create_group("Family", 1, NOW).unwrap();
        assert!(store.create_group("work", 0, NOW).is_err(), "names are unique ignoring case");
        assert!(store.create_group("  ", 0, NOW).is_err());
        let dan = person(&store, "Dan");
        store.set_person_groups(maya, &[work]).unwrap();
        store.set_person_groups(dan, &[work, family]).unwrap();
        let groups = store.list_groups().unwrap();
        assert_eq!(
            groups.iter().map(|g| (g.name.as_str(), g.members)).collect::<Vec<_>>(),
            [("Family", 1), ("Work", 2)]
        );
        assert_eq!(store.groups_of(dan).unwrap(), vec![work, family]);
        store.delete_group(family).unwrap();
        assert_eq!(store.groups_of(dan).unwrap(), vec![work], "deleting a group leaves the people");
        assert!(store.delete_group(family).is_err());
        assert!(store.set_person_groups(999_999, &[work]).is_err());
    }

    #[test]
    fn undoing_a_batch_undoes_every_part_in_reverse() {
        let mut store = demo();
        let maya = person(&store, "Maya");
        let a = convo_of(&store, maya, "Coffee Thursday?");
        let b = convo_of(&store, maya, "Invoice 2291");
        let (_, ua) = store.set_archived(a, true, NOW).unwrap();
        let (_, ub) = store.set_trashed(b, true, NOW).unwrap();
        store.apply_undo(&UndoOp::Many(vec![ua, ub]), NOW).unwrap();
        let rows: Vec<(bool, bool)> = [a, b]
            .iter()
            .map(|id| {
                store
                    .conn()
                    .query_row("SELECT in_inbox, trashed FROM conversations WHERE id = ?1", [id], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .unwrap()
            })
            .collect();
        assert_eq!(rows, [(true, false), (true, false)]);
    }
}

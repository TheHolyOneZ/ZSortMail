use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::actions::{Touched, enqueue};
use super::query::Scope;
use super::sorting::{UndoOp, archive_in, decide_in, finish, mailbox, person_state, trash_in};
use crate::dto::{SearchHit, SenderDecision};
use crate::error::{Error, Result};
use crate::query::{Query, parse};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleAction {
    Hide,
    Archive,
    Trash,
    MarkRead,
    Star,
    Label { name: String },
    Allow,
    Block,
    Mute,
    Notify,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Rule {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub position: i64,
    pub query: String,
    pub actions: Vec<RuleAction>,
    pub stop: bool,
    pub hit_count: u32,
    pub last_hit_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RuleInput {
    pub id: Option<i64>,
    pub name: String,
    pub enabled: bool,
    pub query: String,
    pub actions: Vec<RuleAction>,
    pub stop: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RulePreview {
    pub count: u32,
    pub samples: Vec<SearchHit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleOutcome {
    pub matched: Vec<i64>,
    pub notify: bool,
    pub muted: bool,
    pub hidden: bool,
}

fn rule_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Rule> {
    let actions: String = r.get(5)?;
    Ok(Rule {
        id: r.get(0)?,
        name: r.get(1)?,
        enabled: r.get(2)?,
        position: r.get(3)?,
        query: r.get(4)?,
        actions: serde_json::from_str(&actions).unwrap_or_default(),
        stop: r.get(6)?,
        hit_count: u32::try_from(r.get::<_, i64>(7)?).unwrap_or(0),
        last_hit_at: r.get(8)?,
    })
}

const RULE_COLUMNS: &str = "id, name, enabled, position, query, actions, stop, hit_count, last_hit_at";

fn tag_for(name: &str) -> String {
    format!("zsm:{}", name.trim())
}

#[allow(clippy::too_many_arguments)]
fn apply_action(
    tx: &Transaction<'_>,
    action: &RuleAction,
    rule_id: i64,
    message_id: i64,
    conversation_id: i64,
    sender: Option<i64>,
    whole_conversation: bool,
    now: i64,
    outcome: &mut RuleOutcome,
) -> Result<()> {
    let messages: Vec<i64> = if whole_conversation {
        let mut stmt = tx.prepare_cached("SELECT id FROM messages WHERE conversation_id = ?1")?;
        stmt.query_map([conversation_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
    } else {
        vec![message_id]
    };
    match action {
        RuleAction::Hide => {
            tx.execute(
                "UPDATE conversations SET hidden_at = COALESCE(hidden_at, ?2), hidden_reason = COALESCE(hidden_reason, ?3)
                 WHERE id = ?1",
                params![conversation_id, now, format!("rule:{rule_id}")],
            )?;
            outcome.hidden = true;
        }
        RuleAction::Archive => archive_in(tx, conversation_id, true, now)?,
        RuleAction::Trash => trash_in(tx, conversation_id, true, now)?,
        RuleAction::MarkRead => {
            for id in &messages {
                tx.execute("UPDATE messages SET unread = 0 WHERE id = ?1", [id])?;
            }
            let mb = mailbox(tx, conversation_id)?;
            let thread = mb.thread_id;
            if whole_conversation {
                enqueue(
                    tx,
                    mb.account_id,
                    "thread_modify",
                    &serde_json::json!({ "thread_id": thread, "add": [], "remove": ["UNREAD"] }),
                    now,
                )?;
            } else if let Some(provider) = tx
                .query_row("SELECT provider_id FROM messages WHERE id = ?1", [message_id], |r| r.get::<_, String>(0))
                .optional()?
            {
                enqueue(
                    tx,
                    mb.account_id,
                    "message_modify",
                    &serde_json::json!({ "message_id": provider, "add": [], "remove": ["UNREAD"] }),
                    now,
                )?;
            }
        }
        RuleAction::Star => {
            let mb = mailbox(tx, conversation_id)?;
            for id in &messages {
                tx.execute("UPDATE messages SET starred = 1 WHERE id = ?1", [id])?;
                if let Some(provider) = tx
                    .query_row("SELECT provider_id FROM messages WHERE id = ?1", [id], |r| r.get::<_, String>(0))
                    .optional()?
                {
                    enqueue(
                        tx,
                        mb.account_id,
                        "message_modify",
                        &serde_json::json!({ "message_id": provider, "add": ["STARRED"], "remove": [] }),
                        now,
                    )?;
                }
            }
        }
        RuleAction::Label { name } => {
            let tag = tag_for(name);
            let account: i64 =
                tx.query_row("SELECT account_id FROM conversations WHERE id = ?1", [conversation_id], |r| r.get(0))?;
            tx.execute(
                "INSERT OR IGNORE INTO labels (account_id, provider_id, name, kind) VALUES (?1, ?2, ?3, 'user')",
                params![account, tag, name.trim()],
            )?;
            for id in &messages {
                tx.execute(
                    "INSERT OR IGNORE INTO message_labels (message_id, label_id)
                     SELECT ?1, id FROM labels WHERE account_id = ?2 AND provider_id = ?3",
                    params![id, account, tag],
                )?;
            }
        }
        RuleAction::Allow | RuleAction::Block => {
            if let Some(person) = sender {
                let status: String = tx.query_row("SELECT status FROM people WHERE id = ?1", [person], |r| r.get(0))?;
                if status == "new_sender" {
                    let decision =
                        if matches!(action, RuleAction::Allow) { SenderDecision::Allow } else { SenderDecision::Block };
                    decide_in(tx, person, decision, now)?;
                }
            }
        }
        RuleAction::Mute => {
            if let Some(person) = sender {
                tx.execute("UPDATE people SET muted = 1 WHERE id = ?1", [person])?;
                outcome.muted = true;
            }
        }
        RuleAction::Notify => outcome.notify = true,
    }
    Ok(())
}

impl Store {
    pub fn list_rules(&self) -> Result<Vec<Rule>> {
        let mut stmt = self.conn().prepare(&format!("SELECT {RULE_COLUMNS} FROM rules ORDER BY position, id"))?;
        let rows = stmt.query_map([], rule_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn save_rule(&mut self, input: &RuleInput, now: i64) -> Result<i64> {
        if parse(&input.query).is_empty() {
            return Err(Error::Invalid(
                "A rule needs something to look for. An empty search would match everything.".into(),
            ));
        }
        if input.actions.is_empty() {
            return Err(Error::Invalid("Choose at least one thing for the rule to do".into()));
        }
        if let Some(RuleAction::Label { name }) =
            input.actions.iter().find(|a| matches!(a, RuleAction::Label { name } if name.trim().is_empty()))
        {
            let _ = name;
            return Err(Error::Invalid("A label needs a name".into()));
        }
        let name =
            if input.name.trim().is_empty() { input.query.trim().to_string() } else { input.name.trim().to_string() };
        let actions = serde_json::to_string(&input.actions).map_err(|e| Error::Invalid(e.to_string()))?;
        match input.id {
            Some(id) => {
                let changed = self.conn().execute(
                    "UPDATE rules SET name = ?2, enabled = ?3, query = ?4, actions = ?5, stop = ?6 WHERE id = ?1",
                    params![id, name, input.enabled, input.query.trim(), actions, input.stop],
                )?;
                if changed == 0 {
                    return Err(Error::NotFound("rule"));
                }
                Ok(id)
            }
            None => {
                let position: i64 =
                    self.conn().query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM rules", [], |r| r.get(0))?;
                self.conn().execute(
                    "INSERT INTO rules (name, enabled, position, query, actions, stop, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![name, input.enabled, position, input.query.trim(), actions, input.stop, now],
                )?;
                Ok(self.conn().last_insert_rowid())
            }
        }
    }

    pub fn delete_rule(&mut self, id: i64) -> Result<()> {
        if self.conn().execute("DELETE FROM rules WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("rule"));
        }
        Ok(())
    }

    pub fn order_rules(&mut self, ids: &[i64]) -> Result<()> {
        self.in_transaction(|tx| {
            for (i, id) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE rules SET position = ?2 WHERE id = ?1",
                    params![id, i64::try_from(i).unwrap_or(0) + 1],
                )?;
            }
            Ok(())
        })
    }

    pub fn preview_query(&self, text: &str, now: i64) -> Result<RulePreview> {
        let query = parse(text);
        if query.is_empty() {
            return Ok(RulePreview { count: 0, samples: Vec::new() });
        }
        let count = self.count_matching(&query, now, Scope::Visible)?;
        let mut samples = Vec::new();
        for id in self.matching_message_ids(&query, now, Scope::Visible, 5)? {
            if let Some(hit) = self.search_hit(id)? {
                samples.push(hit);
            }
        }
        Ok(RulePreview { count, samples })
    }

    pub(crate) fn run_rules_in(
        tx: &Transaction<'_>,
        message_id: i64,
        facts: &crate::query::MessageFacts,
        now: i64,
    ) -> Result<RuleOutcome> {
        let mut outcome = RuleOutcome::default();
        let (conversation_id, sender): (i64, Option<i64>) =
            tx.query_row("SELECT conversation_id, from_person_id FROM messages WHERE id = ?1", [message_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if let Some(person) = sender
            && tx.query_row("SELECT whitelisted FROM people WHERE id = ?1", [person], |r| r.get::<_, bool>(0))?
        {
            return Ok(outcome);
        }
        let rules: Vec<Rule> = {
            let mut stmt = tx
                .prepare_cached(&format!("SELECT {RULE_COLUMNS} FROM rules WHERE enabled = 1 ORDER BY position, id"))?;
            stmt.query_map([], rule_from_row)?.collect::<std::result::Result<_, _>>()?
        };
        for rule in rules {
            let query = parse(&rule.query);
            if query.is_empty() || !query.matches(facts, now) {
                continue;
            }
            for action in &rule.actions {
                apply_action(tx, action, rule.id, message_id, conversation_id, sender, false, now, &mut outcome)?;
            }
            tx.execute(
                "UPDATE rules SET hit_count = hit_count + 1, last_hit_at = ?2 WHERE id = ?1",
                params![rule.id, now],
            )?;
            outcome.matched.push(rule.id);
            if rule.stop {
                break;
            }
        }
        Ok(outcome)
    }

    pub fn apply_rule_to_existing(&mut self, rule_id: i64, now: i64) -> Result<(Touched, UndoOp, u32)> {
        let rule = self
            .conn()
            .query_row(&format!("SELECT {RULE_COLUMNS} FROM rules WHERE id = ?1"), [rule_id], rule_from_row)
            .optional()?
            .ok_or(Error::NotFound("rule"))?;
        let query: Query = parse(&rule.query);
        let message_ids = self.matching_message_ids(&query, now, Scope::Visible, 100_000)?;
        self.in_transaction(|tx| {
            let mut undo: Vec<UndoOp> = Vec::new();
            let mut conversations = std::collections::BTreeSet::new();
            let mut people = std::collections::BTreeSet::new();
            let mut seen_conversation = std::collections::BTreeSet::new();
            let mut seen_person = std::collections::BTreeSet::new();
            let mut starred: Vec<i64> = Vec::new();
            let mut tagged: Vec<i64> = Vec::new();
            let mut sink = RuleOutcome::default();
            for message_id in message_ids {
                let (conversation_id, sender): (i64, Option<i64>) = tx.query_row(
                    "SELECT conversation_id, from_person_id FROM messages WHERE id = ?1",
                    [message_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                if seen_conversation.insert(conversation_id) {
                    let mb = mailbox(tx, conversation_id)?;
                    let hidden: (Option<i64>, Option<i64>, Option<String>) = tx.query_row(
                        "SELECT hidden_at, hidden_until, hidden_reason FROM conversations WHERE id = ?1",
                        [conversation_id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )?;
                    let unread: Vec<i64> = {
                        let mut stmt =
                            tx.prepare("SELECT id FROM messages WHERE conversation_id = ?1 AND unread = 1")?;
                        stmt.query_map([conversation_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
                    };
                    undo.push(UndoOp::Mailbox {
                        conversation: conversation_id,
                        in_inbox: mb.in_inbox,
                        trashed: mb.trashed,
                    });
                    undo.push(UndoOp::Hidden {
                        conversation: conversation_id,
                        hidden_at: hidden.0,
                        until: hidden.1,
                        reason: hidden.2,
                    });
                    undo.push(UndoOp::Read { conversation: conversation_id, unread_ids: unread });
                    let mut stmt = tx.prepare("SELECT id FROM messages WHERE conversation_id = ?1 AND starred = 0")?;
                    starred.extend(
                        stmt.query_map([conversation_id], |r| r.get::<_, i64>(0))?
                            .collect::<std::result::Result<Vec<_>, _>>()?,
                    );
                    let mut stmt = tx.prepare("SELECT id FROM messages WHERE conversation_id = ?1")?;
                    tagged.extend(
                        stmt.query_map([conversation_id], |r| r.get::<_, i64>(0))?
                            .collect::<std::result::Result<Vec<_>, _>>()?,
                    );
                }
                if let Some(person) = sender
                    && seen_person.insert(person)
                {
                    undo.push(UndoOp::Person { person, state: person_state(tx, person)? });
                }
                for action in &rule.actions {
                    apply_action(tx, action, rule.id, message_id, conversation_id, sender, true, now, &mut sink)?;
                }
                conversations.insert(conversation_id);
                if let Some(person) = sender {
                    people.insert(person);
                }
            }
            if rule.actions.contains(&RuleAction::Star) {
                undo.push(UndoOp::Star { message_ids: starred, starred: false });
            }
            for action in &rule.actions {
                if let RuleAction::Label { name } = action {
                    undo.push(UndoOp::Untag { tag: tag_for(name), message_ids: tagged.clone() });
                }
            }
            tx.execute(
                "UPDATE rules SET hit_count = hit_count + ?2, last_hit_at = ?3 WHERE id = ?1",
                params![rule_id, i64::try_from(conversations.len()).unwrap_or(0), now],
            )?;
            let count = u32::try_from(conversations.len()).unwrap_or(u32::MAX);
            let conversations: Vec<i64> = conversations.into_iter().collect();
            let people: Vec<i64> = people.into_iter().collect();
            let touched = finish(tx, &conversations, &people)?;
            Ok((touched, UndoOp::Many(undo), count))
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use jiff::tz::TimeZone;

    use super::*;
    use crate::demo::{DemoOptions, seed_demo};
    use crate::dto::PeopleFilter;
    use crate::store::ingest::{Address, IncomingMessage, IngestContext};
    use crate::store::query::Scope;

    pub(crate) const NOW: i64 = 1_795_082_400_000;

    pub(crate) fn demo() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        seed_demo(&mut store, &DemoOptions { now_ms: NOW, stress_people: 0, tz: TimeZone::UTC }).unwrap();
        store
    }

    pub(crate) fn incoming(id: &str, from: &str, subject: &str, text: &str) -> IncomingMessage {
        IncomingMessage {
            provider_id: id.into(),
            thread_id: format!("thread-{id}"),
            rfc_message_id: None,
            in_reply_to: None,
            from: Address::new(from, None),
            to: vec![Address::new("you@zsortmail.demo", None)],
            cc: vec![],
            subject: subject.into(),
            sent_at: NOW,
            snippet: None,
            text: Some(text.into()),
            html: None,
            attachments: vec![],
            labels: vec!["INBOX".into(), "UNREAD".into()],
            unread: true,
            starred: false,
            headers: Default::default(),
            size_bytes: None,
        }
    }

    pub(crate) fn arrive(
        store: &mut Store,
        msg: &IncomingMessage,
        screener: bool,
    ) -> crate::store::ingest::IngestOutcome {
        let ctx = IngestContext {
            account_id: 1,
            self_addresses: vec!["you@zsortmail.demo".into()],
            screener,
            automation: true,
        };
        store.ingest(&ctx, msg, NOW).unwrap()
    }

    fn rule(query: &str, actions: Vec<RuleAction>) -> RuleInput {
        RuleInput { id: None, name: String::new(), enabled: true, query: query.into(), actions, stop: false }
    }

    #[test]
    fn a_rule_needs_something_to_look_for_and_something_to_do() {
        let mut store = demo();
        assert!(
            store.save_rule(&rule("", vec![RuleAction::Archive]), NOW).is_err(),
            "an empty query would match everything"
        );
        assert!(store.save_rule(&rule("   ", vec![RuleAction::Archive]), NOW).is_err());
        assert!(store.save_rule(&rule("from:shop", vec![]), NOW).is_err());
        assert!(store.save_rule(&rule("from:shop", vec![RuleAction::Label { name: "  ".into() }]), NOW).is_err());
        let id = store.save_rule(&rule("from:shop.example", vec![RuleAction::Archive]), NOW).unwrap();
        let saved = store.list_rules().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "from:shop.example", "an unnamed rule is named by its query");
        assert!(store.delete_rule(id).is_ok() && store.delete_rule(id).is_err());
    }

    #[test]
    fn the_dry_run_counts_and_shows_what_a_rule_would_catch() {
        let store = demo();
        let preview = store.preview_query("category:newsletter", NOW).unwrap();
        assert!(preview.count >= 1);
        assert!(preview.samples.len() <= 5 && !preview.samples.is_empty());
        assert!(preview.samples.iter().all(|s| s.subject.contains("Issue")));
        assert_eq!(store.preview_query("", NOW).unwrap().count, 0, "nothing typed, nothing matched");
        assert_eq!(store.preview_query("zzzznothing", NOW).unwrap().samples.len(), 0);
    }

    #[test]
    fn rules_sort_new_mail_as_it_arrives_and_count_their_hits() {
        let mut store = demo();
        store
            .save_rule(
                &RuleInput {
                    name: "Receipts".into(),
                    stop: false,
                    ..rule(
                        "from:shop.example",
                        vec![RuleAction::Archive, RuleAction::MarkRead, RuleAction::Label { name: "Receipts".into() }],
                    )
                },
                NOW,
            )
            .unwrap();
        let out =
            arrive(&mut store, &incoming("r1", "orders@shop.example", "Your order 1234", "thanks for shopping"), false);
        assert!(out.automation.rules.len() == 1);
        let (in_inbox, unread): (bool, i64) = store
            .conn()
            .query_row(
                "SELECT c.in_inbox, c.unread_count FROM conversations c WHERE c.id = ?1",
                [out.conversation_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(!in_inbox, "archived on arrival");
        assert_eq!(unread, 0, "and marked read");
        assert_eq!(store.list_rules().unwrap()[0].hit_count, 1);
        assert_eq!(store.count_matching(&crate::query::parse("label:receipts"), NOW, Scope::Visible).unwrap(), 1);
        let other = arrive(&mut store, &incoming("r2", "friend@home.example", "Hello", "hi"), false);
        assert!(other.automation.rules.is_empty());
    }

    #[test]
    fn hide_stop_notify_mute_and_the_whitelist_work_as_described() {
        let mut store = demo();
        let first = store.save_rule(&rule("subject:promo", vec![RuleAction::Hide, RuleAction::Mute]), NOW).unwrap();
        store.save_rule(&RuleInput { stop: true, ..rule("subject:promo", vec![RuleAction::Notify]) }, NOW).unwrap();
        store.save_rule(&rule("subject:promo", vec![RuleAction::Star]), NOW).unwrap();
        let out = arrive(&mut store, &incoming("p1", "deals@store.example", "Big promo today", "sale"), false);
        assert!(out.automation.hidden && out.automation.muted && out.automation.notify);
        assert_eq!(out.automation.rules.len(), 2, "the stop rule ended it, so the third never ran");
        let hidden: Option<i64> = store
            .conn()
            .query_row("SELECT hidden_at FROM conversations WHERE id = ?1", [out.conversation_id], |r| r.get(0))
            .unwrap();
        assert!(hidden.is_some());
        let muted: bool = store
            .conn()
            .query_row("SELECT muted FROM people WHERE id = ?1", [out.sender_person_id.unwrap()], |r| r.get(0))
            .unwrap();
        assert!(muted);
        let starred: bool = store
            .conn()
            .query_row("SELECT starred FROM messages WHERE id = ?1", [out.message_id], |r| r.get(0))
            .unwrap();
        assert!(!starred);

        let maya = store.list_people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        store.conn().execute("UPDATE people SET whitelisted = 1 WHERE id = ?1", [maya]).unwrap();
        let mut promo = incoming("p2", "maya.okafor@northwind.example", "Promo from Maya", "hello");
        promo.from = Address::new("maya.okafor@northwind.example", Some("Maya Okafor"));
        let out = arrive(&mut store, &promo, false);
        assert!(out.automation.rules.is_empty(), "whitelisted: rules do not touch Maya's mail");

        store
            .save_rule(
                &RuleInput {
                    id: Some(first),
                    enabled: false,
                    ..rule("subject:promo", vec![RuleAction::Hide, RuleAction::Mute])
                },
                NOW,
            )
            .unwrap();
        let out = arrive(&mut store, &incoming("p3", "deals@store.example", "Another promo", "sale"), false);
        assert!(!out.automation.hidden);
    }

    #[test]
    fn a_rule_can_let_in_or_block_the_senders_it_catches() {
        let mut store = demo();
        store.save_rule(&rule("from:friendly.example", vec![RuleAction::Allow]), NOW).unwrap();
        store.save_rule(&rule("from:scam.example", vec![RuleAction::Block]), NOW).unwrap();
        arrive(&mut store, &incoming("a1", "pal@friendly.example", "Hi", "hey"), true);
        arrive(&mut store, &incoming("a2", "boss@scam.example", "Wire money", "now"), true);
        let status = |email: &str| -> String {
            store
                .conn()
                .query_row(
                    "SELECT p.status FROM people p JOIN person_addresses a ON a.person_id = p.id WHERE a.address = ?1",
                    [email],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(status("pal@friendly.example"), "known");
        assert_eq!(status("boss@scam.example"), "blocked");
    }

    #[test]
    fn applying_a_rule_to_existing_mail_is_one_step_and_one_undo() {
        let mut store = demo();
        let id = store
            .save_rule(
                &rule("category:newsletter", vec![RuleAction::Archive, RuleAction::MarkRead, RuleAction::Star]),
                NOW,
            )
            .unwrap();
        let dispatch = store.list_people(PeopleFilter::All, "Dispatch").unwrap()[0].id;
        let before = store.person_summary(dispatch).unwrap().unwrap();
        assert!(before.unread > 0);

        let (touched, undo, count) = store.apply_rule_to_existing(id, NOW).unwrap();
        assert!(count >= 1 && touched.people.contains(&dispatch));
        let after = store.person_summary(dispatch).unwrap().unwrap();
        assert_eq!(after.unread, 0);
        let convo = store.list_conversations(dispatch).unwrap().remove(0);
        let in_inbox: bool = store
            .conn()
            .query_row("SELECT in_inbox FROM conversations WHERE id = ?1", [convo.id], |r| r.get(0))
            .unwrap();
        assert!(!in_inbox);
        let detail = store.get_conversation(convo.id, Some(dispatch)).unwrap().unwrap();
        assert!(detail.messages.iter().all(|m| m.starred));
        assert_eq!(store.list_rules().unwrap()[0].hit_count, count);

        store.apply_undo(&undo, NOW).unwrap();
        assert_eq!(
            store.person_summary(dispatch).unwrap().unwrap().unread,
            before.unread,
            "unread is exactly as it was"
        );
        let in_inbox: bool = store
            .conn()
            .query_row("SELECT in_inbox FROM conversations WHERE id = ?1", [convo.id], |r| r.get(0))
            .unwrap();
        assert!(in_inbox);
        let detail = store.get_conversation(convo.id, Some(dispatch)).unwrap().unwrap();
        assert!(detail.messages.iter().all(|m| !m.starred));
    }

    #[test]
    fn rules_run_in_the_order_they_are_given() {
        let mut store = demo();
        let a = store.save_rule(&rule("x", vec![RuleAction::Star]), NOW).unwrap();
        let b = store.save_rule(&rule("y", vec![RuleAction::Star]), NOW).unwrap();
        let c = store.save_rule(&rule("z", vec![RuleAction::Star]), NOW).unwrap();
        assert_eq!(store.list_rules().unwrap().iter().map(|r| r.id).collect::<Vec<_>>(), [a, b, c]);
        store.order_rules(&[c, a, b]).unwrap();
        assert_eq!(store.list_rules().unwrap().iter().map(|r| r.id).collect::<Vec<_>>(), [c, a, b]);
    }
}

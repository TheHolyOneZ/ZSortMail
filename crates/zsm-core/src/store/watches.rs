use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::query::Scope;
use crate::error::{Error, Result};
use crate::extract::{Extracted, extract_details};
use crate::query::{MessageFacts, Query, parse};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum WatchMode {
    Once,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum WatchPriority {
    Normal,
    Urgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    Waiting,
    Arrived,
    Expired,
    Paused,
}

fn mode_str(m: WatchMode) -> &'static str {
    match m {
        WatchMode::Once => "once",
        WatchMode::Keep => "keep",
    }
}

fn priority_str(p: WatchPriority) -> &'static str {
    match p {
        WatchPriority::Normal => "normal",
        WatchPriority::Urgent => "urgent",
    }
}

fn parse_mode(s: &str) -> WatchMode {
    if s == "keep" { WatchMode::Keep } else { WatchMode::Once }
}

fn parse_priority(s: &str) -> WatchPriority {
    if s == "urgent" { WatchPriority::Urgent } else { WatchPriority::Normal }
}

fn parse_status(s: &str) -> WatchStatus {
    match s {
        "arrived" => WatchStatus::Arrived,
        "expired" => WatchStatus::Expired,
        "paused" => WatchStatus::Paused,
        _ => WatchStatus::Waiting,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct WatchInput {
    pub id: Option<i64>,
    pub name: String,
    pub query: String,
    pub mode: WatchMode,
    pub priority: WatchPriority,
    pub expires_at: Option<i64>,
    pub include_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct WatchHitView {
    pub id: i64,
    pub message_id: i64,
    pub conversation_id: i64,
    pub person_id: Option<i64>,
    pub person_name: String,
    pub subject: String,
    pub snippet: String,
    pub matched_at: i64,
    pub extracted: Vec<Extracted>,
    pub seen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct WatchView {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub mode: WatchMode,
    pub priority: WatchPriority,
    pub status: WatchStatus,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub arrived_at: Option<i64>,
    pub hit_count: u32,
    pub last_hit_at: Option<i64>,
    pub hits: Vec<WatchHitView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchHit {
    pub watch_id: i64,
    pub watch_name: String,
    pub priority: WatchPriority,
    pub message_id: i64,
    pub conversation_id: i64,
    pub extracted: Vec<Extracted>,
}

struct Row {
    id: i64,
    name: String,
    query: String,
    mode: String,
    priority: String,
    status: String,
    created_at: i64,
    expires_at: Option<i64>,
    arrived_at: Option<i64>,
    hit_count: i64,
    last_hit_at: Option<i64>,
}

fn watch_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        name: r.get(1)?,
        query: r.get(2)?,
        mode: r.get(3)?,
        priority: r.get(4)?,
        status: r.get(5)?,
        created_at: r.get(6)?,
        expires_at: r.get(7)?,
        arrived_at: r.get(8)?,
        hit_count: r.get(9)?,
        last_hit_at: r.get(10)?,
    })
}

const WATCH_COLUMNS: &str =
    "id, name, query, mode, priority, status, created_at, expires_at, arrived_at, hit_count, last_hit_at";

impl Store {
    fn hit_views(&self, watch_id: i64, limit: usize) -> Result<Vec<WatchHitView>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT h.id, h.message_id, h.conversation_id, m.from_person_id, COALESCE(p.display_name, m.from_address),
                    m.subject, m.snippet, h.matched_at, h.extracted, h.seen_at IS NOT NULL
             FROM watch_hits h JOIN messages m ON m.id = h.message_id LEFT JOIN people p ON p.id = m.from_person_id
             WHERE h.watch_id = ?1 ORDER BY h.matched_at DESC, h.id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![watch_id, i64::try_from(limit).unwrap_or(3)], |r| {
            let extracted: String = r.get(8)?;
            Ok(WatchHitView {
                id: r.get(0)?,
                message_id: r.get(1)?,
                conversation_id: r.get(2)?,
                person_id: r.get(3)?,
                person_name: r.get(4)?,
                subject: r.get(5)?,
                snippet: r.get(6)?,
                matched_at: r.get(7)?,
                extracted: serde_json::from_str(&extracted).unwrap_or_default(),
                seen: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn view_of(&self, row: Row) -> Result<WatchView> {
        Ok(WatchView {
            hits: self.hit_views(row.id, 3)?,
            id: row.id,
            name: row.name,
            query: row.query,
            mode: parse_mode(&row.mode),
            priority: parse_priority(&row.priority),
            status: parse_status(&row.status),
            created_at: row.created_at,
            expires_at: row.expires_at,
            arrived_at: row.arrived_at,
            hit_count: u32::try_from(row.hit_count).unwrap_or(0),
            last_hit_at: row.last_hit_at,
        })
    }

    pub fn list_watches(&self) -> Result<Vec<WatchView>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {WATCH_COLUMNS} FROM watches ORDER BY
               CASE status WHEN 'waiting' THEN 0 WHEN 'arrived' THEN 1 WHEN 'paused' THEN 2 ELSE 3 END,
               CASE WHEN status = 'waiting' AND priority = 'urgent' THEN 0 ELSE 1 END,
               COALESCE(last_hit_at, created_at) DESC, id DESC"
        ))?;
        let rows: Vec<Row> = stmt.query_map([], watch_row)?.collect::<std::result::Result<_, _>>()?;
        rows.into_iter().map(|r| self.view_of(r)).collect()
    }

    pub fn get_watch(&self, id: i64) -> Result<Option<WatchView>> {
        let row = self
            .conn()
            .query_row(&format!("SELECT {WATCH_COLUMNS} FROM watches WHERE id = ?1"), [id], watch_row)
            .optional()?;
        row.map(|r| self.view_of(r)).transpose()
    }

    pub fn save_watch(&mut self, input: &WatchInput, now: i64) -> Result<(i64, u32)> {
        let query = parse(&input.query);
        if query.is_empty() {
            return Err(Error::Invalid("Say what mail you are waiting for, like xbox 394".into()));
        }
        if input.expires_at.is_some_and(|t| t <= now) {
            return Err(Error::Invalid("The end date has already passed".into()));
        }
        let name =
            if input.name.trim().is_empty() { input.query.trim().to_string() } else { input.name.trim().to_string() };
        let id = match input.id {
            Some(id) => {
                let changed = self.conn().execute(
                    "UPDATE watches SET name = ?2, query = ?3, mode = ?4, priority = ?5, expires_at = ?6,
                       status = CASE WHEN status IN ('expired', 'arrived') THEN 'waiting' ELSE status END
                     WHERE id = ?1",
                    params![
                        id,
                        name,
                        input.query.trim(),
                        mode_str(input.mode),
                        priority_str(input.priority),
                        input.expires_at
                    ],
                )?;
                if changed == 0 {
                    return Err(Error::NotFound("watch"));
                }
                id
            }
            None => {
                self.conn().execute(
                    "INSERT INTO watches (name, query, mode, priority, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![name, input.query.trim(), mode_str(input.mode), priority_str(input.priority), now, input.expires_at],
                )?;
                self.conn().last_insert_rowid()
            }
        };

        let mut counted = 0u32;
        if input.include_existing {
            let ids = self.matching_message_ids(&query, now, Scope::Visible, 500)?;
            for message_id in ids {
                let facts = self.message_facts(message_id)?;
                if facts.is_from_me && !query.mentions_sent() {
                    continue;
                }
                let conversation_id: i64 =
                    self.conn()
                        .query_row("SELECT conversation_id FROM messages WHERE id = ?1", [message_id], |r| r.get(0))?;
                let extracted = serde_json::to_string(&extract_details(&facts.subject, &facts.body))
                    .unwrap_or_else(|_| "[]".into());
                let added = self.conn().execute(
                    "INSERT OR IGNORE INTO watch_hits (watch_id, message_id, conversation_id, matched_at, extracted, seen_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?4)",
                    params![id, message_id, conversation_id, facts.sent_at, extracted],
                )?;
                counted += u32::try_from(added).unwrap_or(0);
            }
            if counted > 0 {
                let (newest, total): (i64, i64) = self.conn().query_row(
                    "SELECT MAX(matched_at), COUNT(*) FROM watch_hits WHERE watch_id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?;
                self.conn().execute(
                    "UPDATE watches SET hit_count = ?2, last_hit_at = ?3,
                       status = CASE WHEN mode = 'once' THEN 'arrived' ELSE status END,
                       arrived_at = CASE WHEN mode = 'once' THEN ?3 ELSE arrived_at END
                     WHERE id = ?1",
                    params![id, total, newest],
                )?;
            }
        }
        Ok((id, counted))
    }

    pub fn delete_watch(&mut self, id: i64) -> Result<()> {
        if self.conn().execute("DELETE FROM watches WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("watch"));
        }
        Ok(())
    }

    pub fn set_watch_active(&mut self, id: i64, active: bool) -> Result<()> {
        let status = if active { "waiting" } else { "paused" };
        if self.conn().execute("UPDATE watches SET status = ?2 WHERE id = ?1", params![id, status])? == 0 {
            return Err(Error::NotFound("watch"));
        }
        Ok(())
    }

    pub fn mark_hits_seen(&mut self, conversation_id: i64, now: i64) -> Result<u32> {
        let n = self.conn().execute(
            "UPDATE watch_hits SET seen_at = ?2 WHERE conversation_id = ?1 AND seen_at IS NULL",
            params![conversation_id, now],
        )?;
        Ok(u32::try_from(n).unwrap_or(0))
    }

    pub fn unseen_hits(&self) -> Result<u32> {
        let n: i64 =
            self.conn().query_row("SELECT COUNT(*) FROM watch_hits WHERE seen_at IS NULL", [], |r| r.get(0))?;
        Ok(u32::try_from(n).unwrap_or(0))
    }

    pub fn expire_watches(&mut self, now: i64) -> Result<Vec<i64>> {
        let ids: Vec<i64> = {
            let mut stmt = self.conn().prepare(
                "SELECT id FROM watches WHERE status = 'waiting' AND expires_at IS NOT NULL AND expires_at <= ?1",
            )?;
            stmt.query_map([now], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
        };
        for id in &ids {
            self.conn().execute("UPDATE watches SET status = 'expired' WHERE id = ?1", [id])?;
        }
        Ok(ids)
    }

    pub(crate) fn evaluate_watches_in(
        tx: &Transaction<'_>,
        message_id: i64,
        facts: &MessageFacts,
        now: i64,
    ) -> Result<Vec<WatchHit>> {
        let conversation_id: i64 =
            tx.query_row("SELECT conversation_id FROM messages WHERE id = ?1", [message_id], |r| r.get(0))?;
        let watches: Vec<Row> = {
            let mut stmt = tx.prepare_cached(&format!(
                "SELECT {WATCH_COLUMNS} FROM watches WHERE status = 'waiting' AND (expires_at IS NULL OR expires_at > ?1)"
            ))?;
            stmt.query_map([now], watch_row)?.collect::<std::result::Result<_, _>>()?
        };
        let mut hits = Vec::new();
        for watch in watches {
            let query: Query = parse(&watch.query);
            if query.is_empty() || (facts.is_from_me && !query.mentions_sent()) || !query.matches(facts, now) {
                continue;
            }
            let extracted = extract_details(&facts.subject, &facts.body);
            let added = tx.execute(
                "INSERT OR IGNORE INTO watch_hits (watch_id, message_id, conversation_id, matched_at, extracted)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    watch.id,
                    message_id,
                    conversation_id,
                    now,
                    serde_json::to_string(&extracted).unwrap_or_else(|_| "[]".into())
                ],
            )?;
            if added == 0 {
                continue;
            }
            tx.execute(
                "UPDATE watches SET hit_count = hit_count + 1, last_hit_at = ?2,
                   status = CASE WHEN mode = 'once' THEN 'arrived' ELSE status END,
                   arrived_at = CASE WHEN mode = 'once' THEN ?2 ELSE arrived_at END
                 WHERE id = ?1",
                params![watch.id, now],
            )?;
            hits.push(WatchHit {
                watch_id: watch.id,
                watch_name: watch.name.clone(),
                priority: parse_priority(&watch.priority),
                message_id,
                conversation_id,
                extracted,
            });
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::rules::tests::{NOW, arrive, demo, incoming};

    const DAY: i64 = 86_400_000;

    fn watch(query: &str, mode: WatchMode, priority: WatchPriority) -> WatchInput {
        WatchInput {
            id: None,
            name: String::new(),
            query: query.into(),
            mode,
            priority,
            expires_at: None,
            include_existing: false,
        }
    }

    const PARCEL: &str =
        "Your order 394-2211 has been handed to DHL.\nXbox Series X (1 item)\nTracking number: JD014600003394512";

    #[test]
    fn a_watch_waits_for_mail_that_has_not_arrived_and_pulls_out_the_numbers() {
        let mut store = demo();
        let (id, counted) = store.save_watch(&watch("xbox 394", WatchMode::Once, WatchPriority::Normal), NOW).unwrap();
        assert_eq!(counted, 0, "by default only mail that arrives from now on counts");
        let view = store.get_watch(id).unwrap().unwrap();
        assert_eq!((view.status, view.hit_count, view.name.as_str()), (WatchStatus::Waiting, 0, "xbox 394"));

        let miss = arrive(&mut store, &incoming("w0", "friend@home.example", "Hello", "xbox night?"), false);
        assert!(miss.automation.watch_hits.is_empty(), "needs both words: xbox and 394");

        let out = arrive(&mut store, &incoming("w1", "noreply@parcel.dhl.example", "Out for delivery", PARCEL), false);
        assert_eq!(out.automation.watch_hits.len(), 1);
        let hit = &out.automation.watch_hits[0];
        assert_eq!((hit.watch_id, hit.priority), (id, WatchPriority::Normal));
        let values: Vec<(&str, &str)> = hit.extracted.iter().map(|e| (e.kind.as_str(), e.value.as_str())).collect();
        assert_eq!(values, [("tracking", "JD014600003394512"), ("order", "394-2211")]);

        let view = store.get_watch(id).unwrap().unwrap();
        assert_eq!((view.status, view.hit_count, view.arrived_at), (WatchStatus::Arrived, 1, Some(NOW)));
        assert_eq!(view.hits[0].subject, "Out for delivery");
        assert_eq!(view.hits[0].extracted.len(), 2);
        assert!(!view.hits[0].seen);
        assert_eq!(store.unseen_hits().unwrap(), 1);

        let again =
            arrive(&mut store, &incoming("w2", "noreply@parcel.dhl.example", "Delivered", "xbox 394 delivered"), false);
        assert!(again.automation.watch_hits.is_empty());

        assert_eq!(store.mark_hits_seen(out.conversation_id, NOW).unwrap(), 1);
        assert_eq!(store.unseen_hits().unwrap(), 0);
    }

    #[test]
    fn a_keep_watch_keeps_watching_and_counts() {
        let mut store = demo();
        let (id, _) =
            store.save_watch(&watch("from:parcel.dhl.example", WatchMode::Keep, WatchPriority::Urgent), NOW).unwrap();
        arrive(&mut store, &incoming("k1", "noreply@parcel.dhl.example", "Label created", "x"), false);
        arrive(&mut store, &incoming("k2", "noreply@parcel.dhl.example", "Out for delivery", "y"), false);
        let view = store.get_watch(id).unwrap().unwrap();
        assert_eq!((view.status, view.hit_count, view.priority), (WatchStatus::Waiting, 2, WatchPriority::Urgent));
        assert_eq!(view.hits.len(), 2);
    }

    #[test]
    fn paused_expired_and_outgoing_mail_do_not_fire_a_watch() {
        let mut store = demo();
        let (paused, _) = store.save_watch(&watch("alpha", WatchMode::Keep, WatchPriority::Normal), NOW).unwrap();
        store.set_watch_active(paused, false).unwrap();
        let (short, _) = store
            .save_watch(
                &WatchInput { expires_at: Some(NOW + DAY), ..watch("beta", WatchMode::Keep, WatchPriority::Normal) },
                NOW,
            )
            .unwrap();
        let (any, _) = store.save_watch(&watch("gamma", WatchMode::Keep, WatchPriority::Normal), NOW).unwrap();

        assert!(
            arrive(&mut store, &incoming("n1", "a@b.example", "alpha", "alpha"), false)
                .automation
                .watch_hits
                .is_empty()
        );
        assert_eq!(
            arrive(&mut store, &incoming("n2", "a@b.example", "beta", "beta"), false).automation.watch_hits.len(),
            1,
            "not yet expired"
        );
        assert_eq!(store.expire_watches(NOW + 2 * DAY).unwrap(), vec![short]);
        assert!(
            arrive(&mut store, &incoming("n3", "a@b.example", "beta again", "beta"), false)
                .automation
                .watch_hits
                .is_empty()
        );

        let mut mine = incoming("n4", "you@zsortmail.demo", "gamma", "gamma");
        mine.labels = vec!["SENT".into()];
        mine.unread = false;
        let out = arrive(&mut store, &mine, false);
        assert!(out.automation.watch_hits.is_empty());
        assert_eq!(store.get_watch(any).unwrap().unwrap().hit_count, 0);

        store.set_watch_active(paused, true).unwrap();
        assert_eq!(
            arrive(&mut store, &incoming("n5", "a@b.example", "alpha", "alpha"), false).automation.watch_hits.len(),
            1
        );
    }

    #[test]
    fn an_explicit_watch_beats_hidden_blocked_and_muted() {
        let mut store = demo();
        store.save_watch(&watch("visa appointment", WatchMode::Once, WatchPriority::Urgent), NOW).unwrap();
        let first = arrive(&mut store, &incoming("b0", "consulate@visa.example", "Welcome", "hello"), true);
        let person = first.sender_person_id.unwrap();
        store.decide_sender(person, crate::dto::SenderDecision::Block, NOW).unwrap();
        let out = arrive(
            &mut store,
            &incoming("b1", "consulate@visa.example", "Your visa appointment", "Monday 14:00"),
            true,
        );
        assert_eq!(out.automation.watch_hits.len(), 1, "the watch is not stopped by the block");
    }

    #[test]
    fn existing_mail_can_be_counted_when_the_watch_is_made() {
        let mut store = demo();
        let (id, counted) = store
            .save_watch(
                &WatchInput { include_existing: true, ..watch("xbox 394", WatchMode::Once, WatchPriority::Normal) },
                NOW,
            )
            .unwrap();
        assert!(counted >= 1, "the demo parcel email is already here");
        let view = store.get_watch(id).unwrap().unwrap();
        assert_eq!(view.status, WatchStatus::Arrived);
        assert!(view.hits[0].seen, "mail that was already here is not news");
        assert_eq!(store.unseen_hits().unwrap(), 0);
        assert!(view.hits[0].extracted.iter().any(|e| e.value == "394-2211"));
    }

    #[test]
    fn watches_are_validated_and_listed_with_the_waiting_ones_first() {
        let mut store = demo();
        assert!(store.save_watch(&watch("  ", WatchMode::Once, WatchPriority::Normal), NOW).is_err());
        assert!(
            store
                .save_watch(
                    &WatchInput { expires_at: Some(NOW - 1), ..watch("x", WatchMode::Once, WatchPriority::Normal) },
                    NOW
                )
                .is_err()
        );
        let (done, _) = store
            .save_watch(
                &WatchInput { include_existing: true, ..watch("xbox 394", WatchMode::Once, WatchPriority::Normal) },
                NOW,
            )
            .unwrap();
        let (calm, _) = store.save_watch(&watch("passport", WatchMode::Once, WatchPriority::Normal), NOW).unwrap();
        let (urgent, _) = store.save_watch(&watch("visa", WatchMode::Once, WatchPriority::Urgent), NOW).unwrap();
        let order: Vec<i64> = store.list_watches().unwrap().iter().map(|w| w.id).collect();
        assert_eq!(order, [urgent, calm, done], "urgent waiting, other waiting, then arrived");
        store.delete_watch(calm).unwrap();
        assert!(store.delete_watch(calm).is_err() && store.set_watch_active(calm, true).is_err());
        store
            .save_watch(
                &WatchInput { id: Some(done), ..watch("xbox 394", WatchMode::Keep, WatchPriority::Normal) },
                NOW,
            )
            .unwrap();
        assert_eq!(store.get_watch(done).unwrap().unwrap().status, WatchStatus::Waiting);
    }
}

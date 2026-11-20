use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::actions::Touched;
use super::sorting::{UndoOp, finish};
use crate::dto::{Category, ConversationSummary};
use crate::error::{Error, Result};

pub const WAITING_AFTER_MS: i64 = 2 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ConversationRow {
    pub conversation: ConversationSummary,
    pub person_id: Option<i64>,
    pub person_name: String,
    pub person_ink: u8,
    pub person_kind: crate::dto::PersonKind,
    pub person_vip: bool,
    pub snoozed_until: Option<i64>,
}

impl Store {
    pub(crate) fn rows_for(&self, ids: Vec<i64>) -> Result<Vec<ConversationRow>> {
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let person_id = self.primary_person_of(id)?;
            let Some(conversation) = self.conversation_summary(id, person_id)? else { continue };
            let (name, ink, kind, vip) = match person_id {
                Some(p) => self
                    .person_summary(p)?
                    .map(|p| (p.name, p.ink, p.kind, p.vip))
                    .unwrap_or_else(|| (String::new(), 0, crate::dto::PersonKind::Person, false)),
                None => (String::new(), 0, crate::dto::PersonKind::Person, false),
            };
            let snoozed_until: Option<i64> =
                self.conn().query_row("SELECT snoozed_until FROM conversations WHERE id = ?1", [id], |r| r.get(0))?;
            out.push(ConversationRow {
                conversation,
                person_id,
                person_name: name,
                person_ink: ink,
                person_kind: kind,
                person_vip: vip,
                snoozed_until,
            });
        }
        Ok(out)
    }

    pub fn needs_reply(&self, now: i64, limit: usize) -> Result<Vec<ConversationRow>> {
        let ids = {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT c.id FROM conversations c
                 WHERE c.category = 'person' AND c.last_from_me = 0 AND c.in_inbox = 1 AND c.trashed = 0{}
                   AND c.hidden_at IS NULL AND (c.snoozed_until IS NULL OR c.snoozed_until <= ?1)
                   AND EXISTS (SELECT 1 FROM conversation_people cp JOIN people p ON p.id = cp.person_id
                               WHERE cp.conversation_id = c.id AND p.status = 'known' AND p.hidden_at IS NULL)
                 ORDER BY c.last_message_at DESC LIMIT ?2",
                self.in_account("c")
            ))?;
            stmt.query_map(params![now, i64::try_from(limit).unwrap_or(12)], |r| r.get::<_, i64>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        self.rows_for(ids)
    }

    pub fn waiting_on_them(&self, now: i64, limit: usize) -> Result<Vec<ConversationRow>> {
        let ids = {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT c.id FROM conversations c
                 WHERE c.category = 'person' AND c.last_from_me = 1 AND c.trashed = 0 AND c.hidden_at IS NULL{}
                   AND c.last_message_at <= ?1
                   AND EXISTS (SELECT 1 FROM conversation_people cp JOIN people p ON p.id = cp.person_id
                               WHERE cp.conversation_id = c.id AND p.status = 'known' AND p.hidden_at IS NULL)
                 ORDER BY c.last_message_at ASC LIMIT ?2",
                self.in_account("c")
            ))?;
            stmt.query_map(params![now - WAITING_AFTER_MS, i64::try_from(limit).unwrap_or(8)], |r| r.get::<_, i64>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        self.rows_for(ids)
    }

    pub fn inbox(
        &self,
        category: Option<Category>,
        now: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ConversationRow>> {
        let cat = category.map(|c| match c {
            Category::Person => "person",
            Category::Newsletter => "newsletter",
            Category::Receipt => "receipt",
            Category::Automated => "automated",
        });
        let ids = {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT c.id FROM conversations c
                 WHERE c.in_inbox = 1 AND c.trashed = 0 AND c.hidden_at IS NULL{}
                   AND (c.snoozed_until IS NULL OR c.snoozed_until <= ?1)
                   AND (?2 IS NULL OR c.category = ?2)
                   AND NOT EXISTS (SELECT 1 FROM conversation_people cp JOIN people p ON p.id = cp.person_id
                                   WHERE cp.conversation_id = c.id AND (p.status <> 'known' OR p.hidden_at IS NOT NULL))
                 ORDER BY c.last_message_at DESC LIMIT ?3 OFFSET ?4",
                self.in_account("c")
            ))?;
            stmt.query_map(
                params![now, cat, i64::try_from(limit).unwrap_or(50), i64::try_from(offset).unwrap_or(0)],
                |r| r.get::<_, i64>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        self.rows_for(ids)
    }

    pub fn inbox_counts(&self, now: i64) -> Result<[u32; 4]> {
        let mut out = [0u32; 4];
        for (i, cat) in ["person", "newsletter", "receipt", "automated"].iter().enumerate() {
            let n: i64 = self.conn().query_row(
                &format!(
                    "SELECT COUNT(*) FROM conversations c WHERE c.in_inbox = 1 AND c.trashed = 0 AND c.hidden_at IS NULL{}
                   AND (c.snoozed_until IS NULL OR c.snoozed_until <= ?1) AND c.category = ?2
                   AND NOT EXISTS (SELECT 1 FROM conversation_people cp JOIN people p ON p.id = cp.person_id
                                   WHERE cp.conversation_id = c.id AND (p.status <> 'known' OR p.hidden_at IS NOT NULL))",
                    self.in_account("c")
                ),
                params![now, cat],
                |r| r.get(0),
            )?;
            out[i] = u32::try_from(n).unwrap_or(0);
        }
        Ok(out)
    }

    pub fn snoozed(&self, now: i64) -> Result<Vec<ConversationRow>> {
        let ids = {
            let mut stmt = self.conn().prepare(&format!(
                "SELECT c.id FROM conversations c WHERE c.snoozed_until IS NOT NULL AND c.snoozed_until > ?1
                   AND c.trashed = 0 AND c.hidden_at IS NULL{} ORDER BY c.snoozed_until",
                self.in_account("c")
            ))?;
            stmt.query_map([now], |r| r.get::<_, i64>(0))?.collect::<std::result::Result<Vec<_>, _>>()?
        };
        self.rows_for(ids)
    }

    pub fn set_snoozed(&mut self, conversation_id: i64, until: Option<i64>) -> Result<(Touched, UndoOp)> {
        self.in_transaction(|tx| {
            let before: Option<Option<i64>> = tx
                .query_row("SELECT snoozed_until FROM conversations WHERE id = ?1", [conversation_id], |r| r.get(0))
                .optional()?;
            let before = before.ok_or(Error::NotFound("conversation"))?;
            tx.execute("UPDATE conversations SET snoozed_until = ?2 WHERE id = ?1", params![conversation_id, until])?;
            let touched = finish(tx, &[conversation_id], &[])?;
            Ok((touched, UndoOp::Snooze { conversation: conversation_id, until: before }))
        })
    }

    pub fn wake_snoozed(&mut self, now: i64) -> Result<Vec<i64>> {
        self.in_transaction(|tx| {
            let ids: Vec<i64> = {
                let mut stmt = tx.prepare("SELECT id FROM conversations WHERE snoozed_until IS NOT NULL AND snoozed_until <= ?1")?;
                stmt.query_map([now], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?
            };
            for id in &ids {
                tx.execute("UPDATE conversations SET snoozed_until = NULL WHERE id = ?1", [id])?;
                tx.execute(
                    "UPDATE messages SET unread = 1 WHERE id = (SELECT id FROM messages WHERE conversation_id = ?1 AND is_from_me = 0
                                                              ORDER BY sent_at DESC, id DESC LIMIT 1)",
                    [id],
                )?;
            }
            finish(tx, &ids, &[])?;
            Ok(ids)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::rules::tests::{NOW, demo};

    const DAY: i64 = 86_400_000;

    #[test]
    fn today_lists_replies_owed_and_conversations_gone_quiet() {
        let store = demo();
        let owed = store.needs_reply(NOW, 20).unwrap();
        assert!(!owed.is_empty());
        assert!(owed.iter().all(|r| r.conversation.status == crate::dto::ConversationStatus::NeedsReply));
        assert!(owed.windows(2).all(|w| w[0].conversation.last_at >= w[1].conversation.last_at), "newest first");
        assert!(owed.iter().any(|r| r.person_name == "Maya Okafor"));
        assert!(
            owed.iter().all(|r| r.conversation.category == Category::Person),
            "newsletters and parcels never need a reply"
        );

        let waiting = store.waiting_on_them(NOW, 20).unwrap();
        assert!(waiting.iter().all(|r| r.conversation.status == crate::dto::ConversationStatus::WaitingOnThem));
        assert!(waiting.iter().all(|r| r.conversation.last_at <= NOW - WAITING_AFTER_MS));
        assert!(
            waiting.windows(2).all(|w| w[0].conversation.last_at <= w[1].conversation.last_at),
            "longest quiet first"
        );
        assert!(waiting.iter().all(|r| r.conversation.subject != "Coffee Thursday?"));
    }

    #[test]
    fn the_inbox_is_flat_filterable_and_leaves_out_what_is_away() {
        let mut store = demo();
        let all = store.inbox(None, NOW, 200, 0).unwrap();
        assert!(all.windows(2).all(|w| w[0].conversation.last_at >= w[1].conversation.last_at));
        let news = store.inbox(Some(Category::Newsletter), NOW, 50, 0).unwrap();
        assert!(!news.is_empty() && news.iter().all(|r| r.conversation.category == Category::Newsletter));
        let counts = store.inbox_counts(NOW).unwrap();
        assert_eq!(counts.iter().sum::<u32>() as usize, store.inbox(None, NOW, 1000, 0).unwrap().len());
        assert_eq!(store.inbox(None, NOW, 3, 0).unwrap().len(), 3);
        assert_eq!(store.inbox(None, NOW, 3, 3).unwrap()[0].conversation.id, all[3].conversation.id, "paging");

        assert!(all.iter().all(|r| r.person_name != "Priya Raman"));
        let first = all[0].conversation.id;
        store.set_snoozed(first, Some(NOW + DAY)).unwrap();
        assert!(
            store.inbox(None, NOW, 200, 0).unwrap().iter().all(|r| r.conversation.id != first),
            "snoozed mail is away"
        );
    }

    #[test]
    fn snoozing_hides_until_the_time_comes_then_returns_it_unread_and_undo_restores_it() {
        let mut store = demo();
        let convo = store.needs_reply(NOW, 1).unwrap().remove(0).conversation.id;
        let (_, undo) = store.set_snoozed(convo, Some(NOW + DAY)).unwrap();
        assert!(store.needs_reply(NOW, 50).unwrap().iter().all(|r| r.conversation.id != convo));
        assert_eq!(store.snoozed(NOW).unwrap().len(), 1);
        assert!(store.wake_snoozed(NOW).unwrap().is_empty(), "not yet");

        store.apply_undo(&undo, NOW).unwrap();
        assert!(store.snoozed(NOW).unwrap().is_empty());
        assert!(store.needs_reply(NOW, 50).unwrap().iter().any(|r| r.conversation.id == convo));

        store.set_snoozed(convo, Some(NOW + DAY)).unwrap();
        store.conn().execute("UPDATE messages SET unread = 0 WHERE conversation_id = ?1", [convo]).unwrap();
        assert_eq!(store.wake_snoozed(NOW + DAY + 1).unwrap(), vec![convo]);
        let back = store.conversation_summary(convo, None).unwrap().unwrap();
        assert!(back.unread > 0);
        assert!(store.snoozed(NOW + DAY + 1).unwrap().is_empty());
        assert!(store.set_snoozed(99_999, Some(NOW)).is_err());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SavedView {
    pub id: i64,
    pub name: String,
    pub query: String,
}

impl Store {
    pub fn view_rows(&self, text: &str, now: i64, limit: usize) -> Result<Vec<ConversationRow>> {
        let query = crate::query::parse(text);
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let mut ids = self.matching_conversation_ids(&query, now, super::query::Scope::Visible, limit)?;
        self.keep_account(&mut ids)?;
        self.rows_for(ids)
    }

    pub(crate) fn keep_account(&self, ids: &mut Vec<i64>) -> Result<()> {
        let Some(account) = self.account_scope() else { return Ok(()) };
        let mut stmt = self.conn().prepare_cached("SELECT account_id FROM conversations WHERE id = ?1")?;
        ids.retain(|id| stmt.query_row([id], |r| r.get::<_, i64>(0)).map(|a| a == account).unwrap_or(false));
        Ok(())
    }

    pub fn list_saved_views(&self) -> Result<Vec<SavedView>> {
        let mut stmt = self.conn().prepare("SELECT id, name, query FROM saved_views ORDER BY position, id")?;
        Ok(stmt
            .query_map([], |r| Ok(SavedView { id: r.get(0)?, name: r.get(1)?, query: r.get(2)? }))?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn save_view(&self, name: &str, query: &str, now: i64) -> Result<i64> {
        let (name, query) = (name.trim(), query.trim());
        if query.is_empty() {
            return Err(crate::error::Error::Invalid("Type a search to save".into()));
        }
        let name = if name.is_empty() { query } else { name };
        let position: i64 =
            self.conn().query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM saved_views", [], |r| r.get(0))?;
        self.conn().execute(
            "INSERT INTO saved_views (name, query, position, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![name, query, position, now],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn delete_saved_view(&self, id: i64) -> Result<()> {
        if self.conn().execute("DELETE FROM saved_views WHERE id = ?1", [id])? == 0 {
            return Err(crate::error::Error::NotFound("view"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod view_tests {
    use crate::store::rules::tests::{NOW, demo};

    #[test]
    fn a_search_gives_conversations_and_can_be_kept_as_a_view() {
        let store = demo();
        let rows = store.view_rows("category:newsletter", NOW, 50).unwrap();
        assert!(!rows.is_empty() && rows.iter().all(|r| r.conversation.subject.contains("Issue")));
        assert!(store.view_rows("", NOW, 50).unwrap().is_empty());
        assert!(store.view_rows("zzzznothing", NOW, 50).unwrap().is_empty());

        assert!(store.save_view("x", "  ", NOW).is_err());
        let id = store.save_view("", "from:dhl is:unread", NOW).unwrap();
        let named = store.save_view("Newsletters", "category:newsletter", NOW).unwrap();
        let views = store.list_saved_views().unwrap();
        assert_eq!(views.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["from:dhl is:unread", "Newsletters"]);
        store.delete_saved_view(id).unwrap();
        assert_eq!(store.list_saved_views().unwrap().len(), 1);
        assert!(store.delete_saved_view(id).is_err());
        let _ = named;
    }
}

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use zsm_core::dto::Category;
use zsm_core::store::notifications::NotificationKind;
use zsm_core::store::rules::{Rule, RuleInput, RulePreview};
use zsm_core::store::today::ConversationRow;
use zsm_core::store::watches::{WatchInput, WatchView};

use crate::core::Core;
use crate::error::Result;
use crate::notify::Reminder;
use crate::sorting::Done;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct TodayView {
    pub needs_reply: Vec<ConversationRow>,
    pub waiting_on: Vec<ConversationRow>,
    pub snoozed: Vec<ConversationRow>,
    pub watches: Vec<WatchView>,
    pub unseen_hits: u32,
    pub new_senders: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SavedWatch {
    pub id: i64,
    pub counted: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct InboxPage {
    pub rows: Vec<ConversationRow>,
    pub counts: Vec<u32>,
}

impl Core {
    pub fn rules(&self) -> Result<Vec<Rule>> {
        self.with_store(|s| Ok(s.list_rules()?))
    }

    pub fn save_rule(&self, input: &RuleInput) -> Result<i64> {
        let now = self.now_ms();
        let id = self.with_store(|s| Ok(s.save_rule(input, now)?))?;
        self.changed("rules", vec![id]);
        Ok(id)
    }

    pub fn delete_rule(&self, id: i64) -> Result<()> {
        self.with_store(|s| Ok(s.delete_rule(id)?))?;
        self.changed("rules", vec![id]);
        Ok(())
    }

    pub fn order_rules(&self, ids: &[i64]) -> Result<()> {
        self.with_store(|s| Ok(s.order_rules(ids)?))?;
        self.changed("rules", Vec::new());
        Ok(())
    }

    pub fn preview_query(&self, text: &str) -> Result<RulePreview> {
        let now = self.now_ms();
        self.with_store(|s| Ok(s.preview_query(text, now)?))
    }

    pub fn view_rows(&self, text: &str) -> Result<Vec<ConversationRow>> {
        let now = self.now_ms();
        self.with_store(|s| Ok(s.view_rows(text, now, 200)?))
    }

    pub fn saved_views(&self) -> Result<Vec<zsm_core::store::today::SavedView>> {
        self.with_store(|s| Ok(s.list_saved_views()?))
    }

    pub fn save_view(&self, name: &str, query: &str) -> Result<i64> {
        let now = self.now_ms();
        let id = self.with_store(|s| Ok(s.save_view(name, query, now)?))?;
        self.changed("views", vec![id]);
        Ok(id)
    }

    pub fn delete_view(&self, id: i64) -> Result<()> {
        self.with_store(|s| Ok(s.delete_saved_view(id)?))?;
        self.changed("views", vec![id]);
        Ok(())
    }

    pub fn apply_rule(&self, id: i64) -> Result<Done> {
        let now = self.now_ms();
        let (touched, undo, count) = self.with_store(|s| Ok(s.apply_rule_to_existing(id, now)?))?;
        self.touched(&touched);
        self.changed("rules", vec![id]);
        let label = if count == 1 {
            "Rule applied to 1 conversation".to_string()
        } else {
            format!("Rule applied to {count} conversations")
        };
        Ok(self.remember(label, undo))
    }

    pub fn watches(&self) -> Result<Vec<WatchView>> {
        self.with_store(|s| Ok(s.list_watches()?))
    }

    pub fn save_watch(&self, input: &WatchInput) -> Result<SavedWatch> {
        let now = self.now_ms();
        let (id, counted) = self.with_store(|s| Ok(s.save_watch(input, now)?))?;
        self.changed("watches", vec![id]);
        Ok(SavedWatch { id, counted })
    }

    pub fn delete_watch(&self, id: i64) -> Result<()> {
        self.with_store(|s| Ok(s.delete_watch(id)?))?;
        self.changed("watches", vec![id]);
        Ok(())
    }

    pub fn set_watch_active(&self, id: i64, active: bool) -> Result<()> {
        self.with_store(|s| Ok(s.set_watch_active(id, active)?))?;
        self.changed("watches", vec![id]);
        Ok(())
    }

    pub fn mark_conversation_seen(&self, conversation_id: i64) -> Result<()> {
        let now = self.now_ms();
        let n = self.with_store(|s| Ok(s.mark_hits_seen(conversation_id, now)?))?;
        if n > 0 {
            self.changed("watches", Vec::new());
        }
        Ok(())
    }

    pub fn today(&self) -> Result<TodayView> {
        let now = self.now_ms();
        self.with_store(|s| {
            let (new_senders, _) = s.counts()?;
            Ok(TodayView {
                needs_reply: s.needs_reply(now, 12)?,
                waiting_on: s.waiting_on_them(now, 8)?,
                snoozed: s.snoozed(now)?,
                watches: s
                    .list_watches()?
                    .into_iter()
                    .filter(|w| w.status != zsm_core::store::watches::WatchStatus::Expired)
                    .collect(),
                unseen_hits: s.unseen_hits()?,
                new_senders,
            })
        })
    }

    pub fn inbox(&self, category: Option<Category>, limit: u32, offset: u32) -> Result<InboxPage> {
        let now = self.now_ms();
        self.with_store(|s| {
            Ok(InboxPage {
                rows: s.inbox(category, now, limit as usize, offset as usize)?,
                counts: s.inbox_counts(now)?.to_vec(),
            })
        })
    }

    pub fn snooze_conversation(&self, id: i64, until: Option<i64>) -> Result<Done> {
        let (touched, undo) = self.with_store(|s| Ok(s.set_snoozed(id, until)?))?;
        self.touched(&touched);
        self.changed("sorting", Vec::new());
        let label = match until {
            Some(ms) => {
                let when = jiff::Timestamp::from_millisecond(ms)
                    .map(|t| t.to_zoned_datetime(self.timezone.clone()))
                    .map(|z| format!("{} {:02}:{:02}", z.strftime("%a %-d %b"), z.hour(), z.minute()))
                    .unwrap_or_default();
                format!("Snoozed until {when}")
            }
            None => "Brought back".to_string(),
        };
        Ok(self.remember(label, undo))
    }

    pub fn tick(&self) -> Result<()> {
        let now = self.now_ms();
        let expired = self.with_store(|s| Ok(s.expire_watches(now)?))?;
        if !expired.is_empty() {
            self.changed("watches", expired);
        }
        self.lift_expired_hides()?;

        let woken = self.with_store(|s| Ok(s.wake_snoozed(now)?))?;
        for id in &woken {
            let info = self.with_store(|s| {
                Ok(s.conversation_summary(*id, None)?.map(|c| (c.subject, s.primary_person_of(*id).ok().flatten())))
            })?;
            if let Some((subject, person_id)) = info {
                self.announce(Reminder {
                    kind: NotificationKind::System,
                    title: "Back from snooze".into(),
                    body: subject,
                    reason: "You snoozed it until now".into(),
                    person_id,
                    conversation_id: Some(*id),
                    draft_id: None,
                })?;
            }
        }
        if !woken.is_empty() {
            self.changed("conversations", woken);
            self.changed("people", Vec::new());
        }
        self.run_timed_writing(now)?;
        let _ = self.with_store(|s| Ok(s.prune_notifications(now - 90 * 24 * 60 * 60 * 1000)?));
        Ok(())
    }

    pub fn start_scheduler(self: &Arc<Self>) {
        let Some(runtime) = self.runtime.clone() else { return };
        let weak = self.weak.clone();
        let every = self.tick_interval;
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(every).await;
                let Some(core) = weak.upgrade() else { return };
                if let Err(error) = tokio::task::spawn_blocking(move || core.tick()).await.unwrap_or(Ok(())) {
                    tracing::warn!(%error, "the scheduler could not finish a pass");
                }
            }
        });
    }
}

pub(crate) const DEFAULT_TICK: Duration = Duration::from_secs(30);

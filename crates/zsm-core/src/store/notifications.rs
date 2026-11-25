use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Store;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    Watch,
    Favourite,
    Important,
    Rule,
    Routine,
    Mail,
    System,
}

impl NotificationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::Favourite => "favourite",
            Self::Important => "important",
            Self::Rule => "rule",
            Self::Routine => "routine",
            Self::Mail => "mail",
            Self::System => "system",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "watch" => Self::Watch,
            "favourite" => Self::Favourite,
            "important" => Self::Important,
            "rule" => Self::Rule,
            "routine" => Self::Routine,
            "mail" => Self::Mail,
            _ => Self::System,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNotification {
    pub at: i64,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    pub reason: String,
    pub person_id: Option<i64>,
    pub conversation_id: Option<i64>,
    pub watch_id: Option<i64>,
    pub draft_id: Option<i64>,
    pub urgent: bool,
    pub shown: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NotificationRecord {
    pub id: i64,
    pub at: i64,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    pub reason: String,
    pub person_id: Option<i64>,
    pub conversation_id: Option<i64>,
    pub watch_id: Option<i64>,
    pub draft_id: Option<i64>,
    pub urgent: bool,
    pub shown: bool,
    pub opened: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalInfo {
    pub subject: String,
    pub snippet: String,
    pub conversation_id: i64,
    pub person_id: Option<i64>,
    pub from_name: String,
    pub vip: bool,
    pub muted: bool,
    pub whitelisted: bool,
    pub hidden: bool,
    pub important: bool,
    pub screened: bool,
}

impl Store {
    pub fn arrival_info(&self, message_id: i64) -> Result<Option<ArrivalInfo>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT m.subject, m.snippet, m.conversation_id, m.from_person_id,
                        COALESCE(p.display_name, m.from_address), COALESCE(p.vip, 0), COALESCE(p.muted, 0),
                        COALESCE(p.whitelisted, 0),
                        (c.hidden_at IS NOT NULL OR p.hidden_at IS NOT NULL OR COALESCE(p.status, '') = 'blocked'),
                        EXISTS (SELECT 1 FROM message_labels ml JOIN labels l ON l.id = ml.label_id
                                WHERE ml.message_id = m.id AND l.provider_id = 'IMPORTANT'),
                        COALESCE(p.status, '') = 'new_sender'
                 FROM messages m JOIN conversations c ON c.id = m.conversation_id
                 LEFT JOIN people p ON p.id = m.from_person_id WHERE m.id = ?1",
                [message_id],
                |r| {
                    Ok(ArrivalInfo {
                        subject: r.get(0)?,
                        snippet: r.get(1)?,
                        conversation_id: r.get(2)?,
                        person_id: r.get(3)?,
                        from_name: r.get(4)?,
                        vip: r.get(5)?,
                        muted: r.get(6)?,
                        whitelisted: r.get(7)?,
                        hidden: r.get(8)?,
                        important: r.get(9)?,
                        screened: r.get(10)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn log_notification(&self, n: &NewNotification) -> Result<i64> {
        self.conn().execute(
            "INSERT INTO notifications (at, kind, title, body, reason, person_id, conversation_id, watch_id, draft_id, urgent, shown)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                n.at,
                n.kind.as_str(),
                n.title,
                n.body,
                n.reason,
                n.person_id,
                n.conversation_id,
                n.watch_id,
                n.draft_id,
                n.urgent,
                n.shown
            ],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn list_notifications(&self, limit: usize) -> Result<Vec<NotificationRecord>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, at, kind, title, body, reason, person_id, conversation_id, watch_id, urgent, shown, opened_at IS NOT NULL,
                    draft_id FROM notifications ORDER BY at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([i64::try_from(limit).unwrap_or(100)], |r| {
            Ok(NotificationRecord {
                id: r.get(0)?,
                at: r.get(1)?,
                kind: NotificationKind::parse(&r.get::<_, String>(2)?),
                title: r.get(3)?,
                body: r.get(4)?,
                reason: r.get(5)?,
                person_id: r.get(6)?,
                conversation_id: r.get(7)?,
                watch_id: r.get(8)?,
                urgent: r.get(9)?,
                shown: r.get(10)?,
                opened: r.get(11)?,
                draft_id: r.get(12)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_notification(&self, id: i64) -> Result<Option<NotificationRecord>> {
        Ok(self.list_notifications(500)?.into_iter().find(|n| n.id == id))
    }

    pub fn mark_notification_opened(&self, id: i64, now: i64) -> Result<Option<i64>> {
        self.conn()
            .execute("UPDATE notifications SET opened_at = COALESCE(opened_at, ?2) WHERE id = ?1", params![id, now])?;
        Ok(self
            .conn()
            .query_row("SELECT conversation_id FROM notifications WHERE id = ?1", [id], |r| r.get::<_, Option<i64>>(0))
            .optional()?
            .flatten())
    }

    pub fn prune_notifications(&self, older_than: i64) -> Result<usize> {
        Ok(self.conn().execute("DELETE FROM notifications WHERE at < ?1", [older_than])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_keeps_shown_and_held_back_notifications_newest_first() {
        let store = Store::open_in_memory().unwrap();
        let base = NewNotification {
            at: 1000,
            kind: NotificationKind::Watch,
            title: "xbox 394 arrived".into(),
            body: "DHL: Out for delivery".into(),
            reason: "Watching for xbox 394".into(),
            person_id: Some(3),
            conversation_id: Some(9),
            watch_id: Some(1),
            draft_id: None,
            urgent: false,
            shown: true,
        };
        let a = store.log_notification(&base).unwrap();
        store
            .log_notification(&NewNotification {
                at: 2000,
                kind: NotificationKind::Favourite,
                shown: false,
                reason: "Quiet hours".into(),
                ..base.clone()
            })
            .unwrap();
        let log = store.list_notifications(10).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!((log[0].at, log[0].shown, log[0].reason.as_str()), (2000, false, "Quiet hours"));
        assert_eq!(log[1].kind, NotificationKind::Watch);
        assert!(!log[1].opened);
        assert_eq!(store.mark_notification_opened(a, 3000).unwrap(), Some(9));
        assert!(store.get_notification(a).unwrap().unwrap().opened);
        assert_eq!(store.prune_notifications(1500).unwrap(), 1);
        assert_eq!(store.list_notifications(10).unwrap().len(), 1);
    }
}

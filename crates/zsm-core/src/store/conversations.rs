use rusqlite::{Connection, OptionalExtension};

use super::Store;
use super::messages::message_views;
use super::people::parse_kind;
use crate::dto::{Category, ConversationDetail, ConversationStatus, ConversationSummary, ParticipantRef, PersonKind};
use crate::error::Result;

pub(crate) fn me() -> ParticipantRef {
    ParticipantRef { person_id: None, name: "You".into(), ink: 0, kind: PersonKind::Myself, is_me: true }
}

pub(crate) fn parse_category(s: &str) -> Category {
    match s {
        "newsletter" => Category::Newsletter,
        "receipt" => Category::Receipt,
        "automated" => Category::Automated,
        _ => Category::Person,
    }
}

pub(crate) fn recompute_conversation(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE conversations SET
           first_message_at = COALESCE((SELECT MIN(sent_at) FROM messages WHERE conversation_id = ?1), first_message_at),
           last_message_at = COALESCE((SELECT MAX(sent_at) FROM messages WHERE conversation_id = ?1), last_message_at),
           message_count = (SELECT COUNT(*) FROM messages WHERE conversation_id = ?1),
           unread_count = COALESCE((SELECT SUM(unread) FROM messages WHERE conversation_id = ?1), 0),
           has_attachments = EXISTS (SELECT 1 FROM attachments a JOIN messages m ON m.id = a.message_id
                                     WHERE m.conversation_id = ?1 AND a.inline = 0),
           last_from_me = COALESCE((SELECT is_from_me FROM messages WHERE conversation_id = ?1
                                    ORDER BY sent_at DESC, id DESC LIMIT 1), 0),
           snippet = COALESCE((SELECT snippet FROM messages WHERE conversation_id = ?1
                               ORDER BY sent_at DESC, id DESC LIMIT 1), ''),
           category = COALESCE((SELECT category FROM messages WHERE conversation_id = ?1
                                ORDER BY (category = 'person') DESC, sent_at DESC LIMIT 1), category),
           starred = EXISTS (SELECT 1 FROM messages WHERE conversation_id = ?1 AND starred = 1),
           in_inbox = EXISTS (SELECT 1 FROM messages m
                              JOIN message_labels ml ON ml.message_id = m.id
                              JOIN labels l ON l.id = ml.label_id
                              WHERE m.conversation_id = ?1 AND l.provider_id = 'INBOX'),
           trashed = (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = ?1) > 0
                     AND NOT EXISTS (SELECT 1 FROM messages m WHERE m.conversation_id = ?1
                                     AND NOT EXISTS (SELECT 1 FROM message_labels ml
                                                     JOIN labels l ON l.id = ml.label_id
                                                     WHERE ml.message_id = m.id AND l.provider_id = 'TRASH'))
         WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

const SUMMARY_COLUMNS: &str = "c.id, c.subject, c.snippet, c.first_message_at, c.last_message_at, c.message_count,
    c.unread_count, c.has_attachments, c.last_from_me, c.category";

fn status_of(last_from_me: bool, category: Category) -> ConversationStatus {
    if last_from_me {
        ConversationStatus::WaitingOnThem
    } else if category == Category::Person {
        ConversationStatus::NeedsReply
    } else {
        ConversationStatus::None
    }
}

impl Store {
    fn participants_of(&self, conversation_id: i64, viewer: Option<i64>) -> Result<Vec<ParticipantRef>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT p.id, p.display_name, p.ink, p.kind FROM conversation_people cp
             JOIN people p ON p.id = cp.person_id
             WHERE cp.conversation_id = ?1 AND p.id IS NOT ?2 ORDER BY p.id",
        )?;
        let rows = stmt.query_map(rusqlite::params![conversation_id, viewer], |r| {
            Ok(ParticipantRef {
                person_id: Some(r.get(0)?),
                name: r.get(1)?,
                ink: r.get::<_, i64>(2)? as u8,
                kind: parse_kind(&r.get::<_, String>(3)?),
                is_me: false,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn summary_from_conversation_row(&self, r: &rusqlite::Row<'_>, viewer: Option<i64>) -> Result<ConversationSummary> {
        let id: i64 = r.get(0)?;
        let last_from_me: bool = r.get(8)?;
        let category = parse_category(&r.get::<_, String>(9)?);
        Ok(ConversationSummary {
            id,
            subject: r.get(1)?,
            snippet: r.get(2)?,
            first_at: r.get(3)?,
            last_at: r.get(4)?,
            message_count: u32::try_from(r.get::<_, i64>(5)?).unwrap_or(0),
            unread: u32::try_from(r.get::<_, i64>(6)?).unwrap_or(0),
            has_files: r.get(7)?,
            status: status_of(last_from_me, category),
            category,
            others: self.participants_of(id, viewer)?,
        })
    }

    pub fn list_conversations(&self, person_id: i64) -> Result<Vec<ConversationSummary>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM conversation_people cp JOIN conversations c ON c.id = cp.conversation_id
             WHERE cp.person_id = ?1 AND c.hidden_at IS NULL AND c.trashed = 0
             ORDER BY c.last_message_at DESC, c.id DESC"
        ))?;
        let mut rows = stmt.query([person_id])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(self.summary_from_conversation_row(row, Some(person_id))?);
        }
        Ok(out)
    }

    pub fn conversation_summary(&self, id: i64, viewer: Option<i64>) -> Result<Option<ConversationSummary>> {
        let mut stmt =
            self.conn.prepare_cached(&format!("SELECT {SUMMARY_COLUMNS} FROM conversations c WHERE c.id = ?1"))?;
        let mut rows = stmt.query([id])?;
        match rows.next()? {
            Some(row) => Ok(Some(self.summary_from_conversation_row(row, viewer)?)),
            None => Ok(None),
        }
    }

    pub fn get_conversation(&self, id: i64, viewer: Option<i64>) -> Result<Option<ConversationDetail>> {
        let Some(summary) = self.conversation_summary(id, viewer)? else {
            return Ok(None);
        };
        let participants = self.participants_of(id, None)?;
        let messages = message_views(self, id)?;
        Ok(Some(ConversationDetail { summary, participants, messages }))
    }

    pub fn primary_person_of(&self, conversation_id: i64) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT person_id FROM conversation_people WHERE conversation_id = ?1 ORDER BY person_id LIMIT 1",
                [conversation_id],
                |r| r.get(0),
            )
            .optional()?)
    }
}

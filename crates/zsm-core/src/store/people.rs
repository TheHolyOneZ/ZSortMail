use rusqlite::{Connection, OptionalExtension, params};

use super::Store;
use crate::dto::{PeopleFilter, PersonDetail, PersonKind, PersonStatus, PersonSummary};
use crate::error::Result;
use crate::text::fts_prefix_query;

pub(crate) fn parse_kind(s: &str) -> PersonKind {
    match s {
        "service" => PersonKind::Service,
        "list" => PersonKind::List,
        "self" => PersonKind::Myself,
        _ => PersonKind::Person,
    }
}

pub(crate) fn parse_status(s: &str) -> PersonStatus {
    match s {
        "new_sender" => PersonStatus::NewSender,
        "blocked" => PersonStatus::Blocked,
        _ => PersonStatus::Known,
    }
}

pub(crate) const SUMMARY_COLUMNS: &str = "p.id, p.display_name, p.kind, p.status, p.ink, p.title, p.last_subject,
    COALESCE(p.last_message_at, 0), p.unread_count, p.pinned, p.needs_reply, p.waiting_on_them, p.has_files, p.photo_path, p.vip";

pub(crate) fn summary_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PersonSummary> {
    Ok(PersonSummary {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: parse_kind(&r.get::<_, String>(2)?),
        status: parse_status(&r.get::<_, String>(3)?),
        ink: r.get::<_, i64>(4)? as u8,
        title: r.get(5)?,
        last_subject: r.get(6)?,
        last_at: r.get(7)?,
        unread: u32::try_from(r.get::<_, i64>(8)?).unwrap_or(0),
        pinned: r.get(9)?,
        needs_reply: r.get(10)?,
        waiting_on_them: r.get(11)?,
        has_files: r.get(12)?,
        photo_url: r.get(13)?,
        vip: r.get(14)?,
    })
}

pub(crate) fn refresh_person(conn: &Connection, person_id: i64) -> Result<()> {
    const LIVE: &str = "FROM conversation_people cp JOIN conversations c ON c.id = cp.conversation_id
                        WHERE cp.person_id = ?1 AND c.hidden_at IS NULL AND c.trashed = 0";
    conn.execute(
        &format!(
            "UPDATE people SET
               first_message_at = (SELECT MIN(c.first_message_at) {LIVE}),
               last_message_at = (SELECT MAX(c.last_message_at) {LIVE}),
               last_subject = COALESCE((SELECT c.subject {LIVE} ORDER BY c.last_message_at DESC LIMIT 1), ''),
               unread_count = COALESCE((SELECT SUM(c.unread_count) {LIVE}), 0),
               needs_reply = EXISTS (SELECT 1 {LIVE} AND c.category = 'person' AND c.last_from_me = 0),
               waiting_on_them = EXISTS (SELECT 1 {LIVE} AND c.last_from_me = 1),
               has_files = EXISTS (SELECT 1 {LIVE} AND c.has_attachments = 1),
               conversation_count = (SELECT COUNT(*) {LIVE}),
               file_count = COALESCE((
                 SELECT COUNT(*) FROM conversation_people cp
                 JOIN conversations c ON c.id = cp.conversation_id AND c.hidden_at IS NULL AND c.trashed = 0
                 JOIN messages m ON m.conversation_id = cp.conversation_id
                 JOIN attachments a ON a.message_id = m.id
                 WHERE cp.person_id = ?1 AND a.inline = 0), 0)
             WHERE id = ?1"
        ),
        [person_id],
    )?;
    conn.execute("DELETE FROM people_fts WHERE rowid = ?1", [person_id])?;
    conn.execute(
        "INSERT INTO people_fts (rowid, name, addresses, organization, title, subject)
         SELECT p.id, p.display_name,
                COALESCE((SELECT group_concat(address, ' ') FROM person_addresses WHERE person_id = p.id), ''),
                COALESCE(p.organization, ''), COALESCE(p.title, ''), p.last_subject
         FROM people p WHERE p.id = ?1",
        [person_id],
    )?;
    Ok(())
}

impl Store {
    pub fn list_people(&self, filter: PeopleFilter, query: &str) -> Result<Vec<PersonSummary>> {
        let mut sql = format!(
            "SELECT {SUMMARY_COLUMNS} FROM people p
             WHERE p.status = 'known' AND p.kind <> 'self' AND p.hidden_at IS NULL AND p.last_message_at IS NOT NULL"
        );
        sql.push_str(&self.in_account("p"));
        sql.push_str(match filter {
            PeopleFilter::All => "",
            PeopleFilter::NeedsReply => " AND p.needs_reply = 1",
            PeopleFilter::WaitingOnThem => " AND p.waiting_on_them = 1",
            PeopleFilter::HasFiles => " AND p.has_files = 1",
            PeopleFilter::Pinned => " AND p.pinned = 1",
        });
        let fts = fts_prefix_query(query);
        if fts.is_some() {
            sql.push_str(" AND p.id IN (SELECT rowid FROM people_fts WHERE people_fts MATCH ?1)");
        }
        sql.push_str(" ORDER BY p.pinned DESC, p.last_message_at DESC, p.id");

        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = match &fts {
            Some(q) => stmt.query_map([q], summary_from_row)?.collect::<std::result::Result<Vec<_>, _>>()?,
            None => stmt.query_map([], summary_from_row)?.collect::<std::result::Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    pub fn person_summary(&self, id: i64) -> Result<Option<PersonSummary>> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {SUMMARY_COLUMNS} FROM people p WHERE p.id = ?1"), [id], summary_from_row)
            .optional()?)
    }

    pub fn get_person(&self, id: i64) -> Result<Option<PersonDetail>> {
        let Some(summary) = self.person_summary(id)? else {
            return Ok(None);
        };
        let (description, organization, notes, whitelisted, conversation_count, file_count, first) =
            self.conn.query_row(
                "SELECT description, organization, notes, whitelisted, conversation_count, file_count,
                    COALESCE(first_message_at, 0) FROM people WHERE id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, bool>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                    ))
                },
            )?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT address FROM person_addresses WHERE person_id = ?1 ORDER BY is_primary DESC, address",
        )?;
        let addresses =
            stmt.query_map(params![id], |r| r.get::<_, String>(0))?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Some(PersonDetail {
            summary,
            description,
            organization,
            notes,
            whitelisted,
            addresses,
            conversation_count: u32::try_from(conversation_count).unwrap_or(0),
            file_count: u32::try_from(file_count).unwrap_or(0),
            first_contact_at: first,
        }))
    }
}

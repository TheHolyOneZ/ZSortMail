use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Store;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TimelineEntry {
    pub message_id: i64,
    pub conversation_id: i64,
    pub subject: String,
    pub sent_at: i64,
    pub from_me: bool,
    pub from_name: String,
    pub snippet: String,
    pub unread: bool,
    pub files: u32,
}

impl Store {
    pub fn person_timeline(&self, person_id: i64, before: Option<i64>, limit: usize) -> Result<Vec<TimelineEntry>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT m.id, m.conversation_id, c.subject, m.sent_at, m.is_from_me,
                    COALESCE(p.display_name, m.from_address), m.snippet, m.unread,
                    (SELECT COUNT(*) FROM attachments a WHERE a.message_id = m.id AND a.inline = 0)
             FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             JOIN conversation_people cp ON cp.conversation_id = c.id AND cp.person_id = ?1
             LEFT JOIN people p ON p.id = m.from_person_id
             WHERE c.hidden_at IS NULL AND c.trashed = 0 AND m.hidden_at IS NULL
               AND (?2 IS NULL OR m.sent_at < ?2)
             ORDER BY m.sent_at DESC, m.id DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![person_id, before, i64::try_from(limit).unwrap_or(100)], |r| {
            Ok(TimelineEntry {
                message_id: r.get(0)?,
                conversation_id: r.get(1)?,
                subject: r.get(2)?,
                sent_at: r.get(3)?,
                from_me: r.get(4)?,
                from_name: r.get(5)?,
                snippet: r.get(6)?,
                unread: r.get(7)?,
                files: u32::try_from(r.get::<_, i64>(8)?).unwrap_or(0),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::dto::PeopleFilter;
    use crate::store::rules::tests::demo;

    #[test]
    fn the_timeline_interleaves_every_conversation_with_a_person_newest_first_and_pages() {
        let store = demo();
        let maya = store.list_people(PeopleFilter::All, "Okafor").unwrap()[0].id;
        let all = store.person_timeline(maya, None, 500).unwrap();
        let conversations: std::collections::BTreeSet<i64> = all.iter().map(|e| e.conversation_id).collect();
        assert!(conversations.len() >= 3, "several conversations are interleaved: {}", conversations.len());
        assert!(all.windows(2).all(|w| w[0].sent_at >= w[1].sent_at), "newest first");
        assert!(all.iter().any(|e| e.from_me) && all.iter().any(|e| !e.from_me));
        assert!(all.iter().all(|e| !e.subject.is_empty()));

        let first = store.person_timeline(maya, None, 5).unwrap();
        assert_eq!(first.len(), 5);
        let next = store.person_timeline(maya, Some(first[4].sent_at), 5).unwrap();
        assert!(next.iter().all(|e| e.sent_at < first[4].sent_at));
        assert!(store.person_timeline(999_999, None, 5).unwrap().is_empty());
    }
}

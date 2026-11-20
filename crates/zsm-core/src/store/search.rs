use super::Store;
use super::conversations::me;
use super::people::parse_kind;
use super::query::Scope;
use crate::dto::{ParticipantRef, PeopleFilter, PersonKind, SearchHit, SearchResults};
use crate::error::Result;
use crate::ink::ink_for;
use crate::query::parse;
use crate::text::fts_prefix_query;

impl Store {
    pub fn search(&self, text: &str, now_ms: i64, limit: usize) -> Result<SearchResults> {
        let query = parse(text);
        if query.is_empty() {
            return Ok(SearchResults { people: Vec::new(), messages: Vec::new() });
        }

        let ids: Vec<i64> = if query.words_only() {
            let fts = fts_prefix_query(text).unwrap_or_default();
            let mut stmt = self.conn().prepare(&format!(
                "SELECT m.id FROM messages_fts f JOIN messages m ON m.id = f.rowid
                 JOIN conversations c ON c.id = m.conversation_id
                 LEFT JOIN people p ON p.id = m.from_person_id
                 WHERE messages_fts MATCH ?1 AND m.hidden_at IS NULL AND c.hidden_at IS NULL AND c.trashed = 0
                   AND p.hidden_at IS NULL{}
                 ORDER BY rank, m.sent_at DESC LIMIT ?2",
                self.in_account("m")
            ))?;
            if text.contains('"') {
                self.matching_message_ids(&query, now_ms, Scope::Visible, limit)?
            } else {
                stmt.query_map(rusqlite::params![fts, i64::try_from(limit).unwrap_or(30)], |r| r.get(0))?
                    .collect::<std::result::Result<_, _>>()?
            }
        } else {
            self.matching_message_ids(&query, now_ms, Scope::Visible, limit)?
        };

        let ids = match self.account_scope() {
            Some(account) => {
                let mut stmt = self.conn().prepare_cached("SELECT account_id FROM messages WHERE id = ?1")?;
                ids.into_iter()
                    .filter(|id| stmt.query_row([id], |r| r.get::<_, i64>(0)).map(|a| a == account).unwrap_or(false))
                    .collect()
            }
            None => ids,
        };
        let mut messages = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(hit) = self.search_hit(id)? {
                messages.push(hit);
            }
        }

        let people = if query.words_only() {
            let mut people = self.list_people(PeopleFilter::All, text)?;
            people.truncate(8);
            people
        } else {
            Vec::new()
        };
        Ok(SearchResults { people, messages })
    }

    pub(crate) fn search_hit(&self, id: i64) -> Result<Option<SearchHit>> {
        use rusqlite::OptionalExtension;
        let row = self
            .conn()
            .query_row(
                "SELECT m.id, m.conversation_id, m.subject, m.snippet, m.sent_at, m.is_from_me, m.from_person_id,
                        p.display_name, p.ink, p.kind, m.from_address
                 FROM messages m LEFT JOIN people p ON p.id = m.from_person_id WHERE m.id = ?1",
                [id],
                |r| {
                    let is_from_me: bool = r.get(5)?;
                    let from_address: String = r.get(10)?;
                    let from = if is_from_me {
                        me()
                    } else {
                        ParticipantRef {
                            person_id: r.get(6)?,
                            name: r
                                .get::<_, Option<String>>(7)?
                                .unwrap_or_else(|| from_address.split('@').next().unwrap_or(&from_address).to_string()),
                            ink: r.get::<_, Option<i64>>(8)?.map_or_else(|| ink_for(&from_address), |n| n as u8),
                            kind: r.get::<_, Option<String>>(9)?.map_or(PersonKind::Person, |k| parse_kind(&k)),
                            is_me: false,
                        }
                    };
                    Ok((
                        SearchHit {
                            message_id: r.get(0)?,
                            conversation_id: r.get(1)?,
                            person_id: None,
                            subject: r.get(2)?,
                            snippet: r.get(3)?,
                            from,
                            sent_at: r.get(4)?,
                        },
                        r.get::<_, i64>(1)?,
                    ))
                },
            )
            .optional()?;
        let Some((mut hit, conversation_id)) = row else { return Ok(None) };
        hit.person_id = hit.from.person_id.or(self.primary_person_of(conversation_id)?);
        Ok(Some(hit))
    }
}

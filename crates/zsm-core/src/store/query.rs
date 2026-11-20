use rusqlite::params_from_iter;
use rusqlite::types::Value;

use super::Store;
use super::ingest::Address;
use super::messages::decompress;
use crate::error::{Error, Result};
use crate::query::{AddressFact, FROM_CLAUSE, MessageFacts, Query};

pub const INDEXED_CHARS: usize = 20_000;

fn address_facts(json: &str) -> Vec<AddressFact> {
    serde_json::from_str::<Vec<Address>>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|a| AddressFact { email: a.email, name: a.name.unwrap_or_default() })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Visible,
    Everything,
}

pub(crate) fn load_facts(conn: &rusqlite::Connection, message_id: i64) -> Result<MessageFacts> {
    let facts = conn
        .query_row(
            &format!(
                "SELECT m.from_address, COALESCE(p.display_name, ''), m.to_addresses, m.cc_addresses, m.subject,
                        m.is_from_me, m.unread, m.starred, m.category, COALESCE(m.list_id, ''),
                        m.unsubscribe IS NOT NULL, COALESCE(m.size_bytes, 0), m.sent_at, c.in_inbox, c.trashed,
                        (c.hidden_at IS NOT NULL OR p.hidden_at IS NOT NULL),
                        (c.last_from_me = 0 AND c.category = 'person'), m.body_text, m.participants, p.id
                 FROM {FROM_CLAUSE} WHERE m.id = ?1"
            ),
            [message_id],
            |r| {
                let body: Option<Vec<u8>> = r.get(17)?;
                let person: Option<i64> = r.get(19)?;
                Ok((
                    MessageFacts {
                        from_address: r.get(0)?,
                        from_name: r.get(1)?,
                        to: address_facts(&r.get::<_, String>(2)?),
                        cc: address_facts(&r.get::<_, String>(3)?),
                        subject: r.get(4)?,
                        is_from_me: r.get(5)?,
                        unread: r.get(6)?,
                        starred: r.get(7)?,
                        category: r.get(8)?,
                        list_id: r.get(9)?,
                        has_unsubscribe: r.get(10)?,
                        size: r.get(11)?,
                        sent_at: r.get(12)?,
                        in_inbox: r.get(13)?,
                        trashed: r.get(14)?,
                        hidden: r.get(15)?,
                        conversation_needs_reply: r.get(16)?,
                        body: body.map(|b| decompress(&b)).unwrap_or_default().chars().take(INDEXED_CHARS).collect(),
                        participants: r.get(18)?,
                        ..MessageFacts::default()
                    },
                    person,
                ))
            },
        )
        .map_err(|_| Error::NotFound("message"))?;
    let person = facts.1;
    let mut facts = facts.0;
    if let Some(person) = person {
        let mut stmt = conn.prepare_cached("SELECT address FROM person_addresses WHERE person_id = ?1")?;
        facts.person_addresses = stmt.query_map([person], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        let mut stmt = conn.prepare_cached(
            "SELECT g.name FROM person_groups pg JOIN groups g ON g.id = pg.group_id WHERE pg.person_id = ?1",
        )?;
        facts.groups = stmt.query_map([person], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    }
    let mut stmt = conn.prepare_cached("SELECT filename FROM attachments WHERE message_id = ?1 AND inline = 0")?;
    facts.filenames = stmt.query_map([message_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
    let mut stmt = conn.prepare_cached(
        "SELECT l.provider_id, l.name FROM message_labels ml JOIN labels l ON l.id = ml.label_id WHERE ml.message_id = ?1",
    )?;
    for row in stmt.query_map([message_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (provider_id, name) = row?;
        facts.labels.push(provider_id);
        facts.label_names.push(name);
    }
    Ok(facts)
}

impl Store {
    pub fn message_facts(&self, message_id: i64) -> Result<MessageFacts> {
        load_facts(self.conn(), message_id)
    }

    fn scope_sql(query: &Query, scope: Scope) -> &'static str {
        if scope == Scope::Everything || query.wants_hidden() {
            ""
        } else {
            " AND c.hidden_at IS NULL AND c.trashed = 0 AND p.hidden_at IS NULL"
        }
    }

    pub fn matching_message_ids(&self, query: &Query, now_ms: i64, scope: Scope, limit: usize) -> Result<Vec<i64>> {
        let compiled = query.to_sql(now_ms);
        let sql = format!(
            "SELECT m.id FROM {FROM_CLAUSE} WHERE {}{} ORDER BY m.sent_at DESC, m.id DESC LIMIT {}",
            compiled.where_sql,
            Self::scope_sql(query, scope),
            limit.max(1)
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(compiled.params.iter()), |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn count_matching(&self, query: &Query, now_ms: i64, scope: Scope) -> Result<u32> {
        let compiled = query.to_sql(now_ms);
        let sql =
            format!("SELECT COUNT(*) FROM {FROM_CLAUSE} WHERE {}{}", compiled.where_sql, Self::scope_sql(query, scope));
        let n: i64 = self.conn().query_row(&sql, params_from_iter(compiled.params.iter()), |r| r.get(0))?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    pub fn matching_conversation_ids(
        &self,
        query: &Query,
        now_ms: i64,
        scope: Scope,
        limit: usize,
    ) -> Result<Vec<i64>> {
        let compiled = query.to_sql(now_ms);
        let sql = format!(
            "SELECT c.id FROM {FROM_CLAUSE} WHERE {}{} GROUP BY c.id ORDER BY MAX(m.sent_at) DESC LIMIT {}",
            compiled.where_sql,
            Self::scope_sql(query, scope),
            limit.max(1)
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(compiled.params.iter()), |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn text_value(text: &str) -> Value {
        Value::Text(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;

    use super::*;
    use crate::demo::{DemoOptions, seed_demo};
    use crate::query::parse;

    const NOW: i64 = 1_795_082_400_000;

    fn demo() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        seed_demo(&mut store, &DemoOptions { now_ms: NOW, stress_people: 0, tz: TimeZone::UTC }).unwrap();
        store
    }

    fn all_ids(store: &Store) -> Vec<i64> {
        let mut stmt = store.conn().prepare("SELECT id FROM messages ORDER BY id").unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().collect::<std::result::Result<_, _>>().unwrap()
    }

    const QUERIES: &[&str] = &[
        "xbox 394",
        "xbox",
        "394",
        "checklist",
        "\"launch checklist\"",
        "\"checklist launch\"",
        "xbo",
        "AMELIE",
        "réservation",
        "reservation samedi",
        "northwind",
        "maya",
        "okafor dan",
        "corkage",
        "from:maya",
        "from:MAYA.okafor",
        "from:\"Maya Okafor\"",
        "from:dhl",
        "from:amelie",
        "from:田中",
        "from:@northwind.example",
        "to:dan",
        "to:you@zsortmail.demo",
        "to:maya",
        "cc:dan",
        "subject:invoice",
        "subject:\"matter 2291\"",
        "subject:CAFÉ",
        "filename:pdf",
        "filename:invoice",
        "filename:zip",
        "label:inbox",
        "label:SENT",
        "label:nonexistent",
        "list:dispatch",
        "person:maya",
        "person:northwind",
        "person:\"Dad\"",
        "group:work",
        "group:WORK -group:close",
        "group:friends",
        "-group:work is:unread",
        "has:attachment",
        "has:unsubscribe",
        "-has:attachment",
        "is:unread",
        "is:read",
        "is:starred",
        "is:inbox",
        "is:archived",
        "is:trashed",
        "is:hidden",
        "is:sent",
        "is:needsreply",
        "category:person",
        "category:newsletter",
        "category:receipt",
        "category:automated",
        "in:inbox",
        "in:sent",
        "in:trash",
        "in:archive",
        "in:anywhere",
        "newer_than:1d",
        "newer_than:7d",
        "newer_than:30d",
        "older_than:30d",
        "older_than:1y",
        "after:2026-11-10",
        "before:2026-11-01",
        "after:2026-10-01 before:2026-11-01",
        "larger:1k",
        "smaller:2000",
        "from:maya has:attachment",
        "from:maya OR from:dan",
        "from:maya OR from:dan has:attachment",
        "(from:maya OR from:dan) has:attachment",
        "-(from:maya OR from:dan)",
        "-from:maya -from:dan -from:dad",
        "is:unread category:person -from:dad",
        "\"launch checklist\" -subject:invoice newer_than:30d is:unread",
        "is:unread OR is:starred OR has:attachment",
        "xbox OR corkage",
        "-is:read -is:sent",
        "from:maya subject:tooltip",
        "subject:re",
        "to:me OR to:you",
        "",
    ];

    #[test]
    fn sql_and_memory_agree_on_every_message_for_every_query() {
        let mut store = demo();
        let work = store.create_group("Work", 3, NOW).unwrap();
        let close = store.create_group("Close friends", 5, NOW).unwrap();
        for (name, groups) in
            [("Maya", vec![work]), ("Dan", vec![work]), ("Dad", vec![close]), ("Noa", vec![close, work])]
        {
            let id: i64 = store
                .conn()
                .query_row("SELECT id FROM people WHERE display_name LIKE ?1", [format!("{name}%")], |r| r.get(0))
                .unwrap();
            store.set_person_groups(id, &groups).unwrap();
        }
        let ids = all_ids(&store);
        assert!(ids.len() > 150, "the demo corpus is big enough to mean something: {}", ids.len());
        let facts: Vec<(i64, MessageFacts)> = ids.iter().map(|id| (*id, store.message_facts(*id).unwrap())).collect();
        let mut total_hits = 0;
        let mut queries_with_hits = 0;
        for text in QUERIES {
            let query = parse(text);
            let mut sql_ids = store.matching_message_ids(&query, NOW, Scope::Everything, 100_000).unwrap();
            sql_ids.sort_unstable();
            let mut memory_ids: Vec<i64> =
                facts.iter().filter(|(_, f)| query.matches(f, NOW)).map(|(id, _)| *id).collect();
            memory_ids.sort_unstable();
            assert_eq!(sql_ids, memory_ids, "SQL and memory disagree for {text:?} ({query})");
            if !memory_ids.is_empty() {
                queries_with_hits += 1;
            }
            total_hits += memory_ids.len();
        }
        assert!(queries_with_hits >= 60, "only {queries_with_hits} of {} queries matched anything", QUERIES.len());
        assert!(total_hits > 1000);
    }

    #[test]
    fn the_queries_find_what_a_person_would_expect() {
        let store = demo();
        let count = |q: &str| store.count_matching(&parse(q), NOW, Scope::Everything).unwrap();
        assert_eq!(count("corkage"), 1, "one message wrote the word, replies that only quote it do not count");
        assert!(count("xbox 394") >= 1, "the parcel message");
        assert!(count("amelie") >= 3, "accent-insensitive");
        assert_eq!(count("from:田中"), 1);
        assert_eq!(count("filename:zip"), 1);
        assert!(count("has:attachment") >= 8);
        assert_eq!(count(""), all_ids(&store).len() as u32);
        assert_eq!(count("is:read") + count("is:unread"), count(""));
        assert_eq!(count("is:sent") + count("-is:sent"), count(""));
        assert_eq!(count("is:inbox"), count(""));
    }

    #[test]
    fn lists_leave_out_hidden_and_trashed_mail_unless_asked() {
        let mut store = demo();
        let person = store.list_people(crate::dto::PeopleFilter::All, "Dad").unwrap()[0].id;
        let hidden = store
            .conn()
            .execute("UPDATE people SET hidden_at = ?1 WHERE id = ?2", rusqlite::params![NOW, person])
            .unwrap();
        assert_eq!(hidden, 1);
        let q = parse("from:dad");
        assert_eq!(store.count_matching(&q, NOW, Scope::Visible).unwrap(), 0, "hidden people vanish from results");
        assert!(store.count_matching(&q, NOW, Scope::Everything).unwrap() > 0);
        assert!(
            store.count_matching(&parse("from:dad is:hidden"), NOW, Scope::Visible).unwrap() > 0,
            "asking for them gets them"
        );
        let _ = &mut store;
    }

    #[test]
    fn conversations_and_limits() {
        let store = demo();
        let convos = store.matching_conversation_ids(&parse("from:maya"), NOW, Scope::Visible, 50).unwrap();
        assert!(convos.len() >= 3);
        let mut dedup = convos.clone();
        dedup.dedup();
        assert_eq!(convos, dedup, "each conversation once");
        assert_eq!(store.matching_message_ids(&parse("from:maya"), NOW, Scope::Visible, 2).unwrap().len(), 2);
    }
}

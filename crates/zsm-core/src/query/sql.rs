use rusqlite::types::Value;

use super::{Folder, HasKind, IsKind, Query, Term};
use crate::text::fold;

pub const FROM_CLAUSE: &str = "messages m JOIN conversations c ON c.id = m.conversation_id \
LEFT JOIN people p ON p.id = m.from_person_id";

#[derive(Debug, Clone)]
pub struct Compiled {
    pub where_sql: String,
    pub params: Vec<Value>,
}

impl Query {
    pub fn to_sql(&self, now_ms: i64) -> Compiled {
        let mut params = Vec::new();
        let where_sql = compile(self, now_ms, &mut params);
        Compiled { where_sql, params }
    }
}

fn text(params: &mut Vec<Value>, value: &str) {
    params.push(Value::Text(fold(value)));
}

fn compile(query: &Query, now: i64, params: &mut Vec<Value>) -> String {
    match query {
        Query::And(items) if items.is_empty() => "1".into(),
        Query::And(items) => {
            format!("({})", items.iter().map(|q| compile(q, now, params)).collect::<Vec<_>>().join(" AND "))
        }
        Query::Or(items) if items.is_empty() => "0".into(),
        Query::Or(items) => {
            format!("({})", items.iter().map(|q| compile(q, now, params)).collect::<Vec<_>>().join(" OR "))
        }
        Query::Not(inner) => format!("(NOT {})", compile(inner, now, params)),
        Query::Term(term) => compile_term(term, now, params),
    }
}

fn compile_term(term: &Term, now: i64, params: &mut Vec<Value>) -> String {
    match term {
        Term::Words(w) => {
            if w.is_empty() {
                return "1".into();
            }
            params.push(Value::Text(w.iter().map(|t| format!("\"{t}\"*")).collect::<Vec<_>>().join(" ")));
            "m.id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?)".into()
        }
        Term::Phrase(w) => {
            if w.is_empty() {
                return "1".into();
            }
            params.push(Value::Text(format!("\"{}\"", w.join(" "))));
            "m.id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?)".into()
        }
        Term::From(v) => {
            text(params, v);
            "instr(zfold(m.from_address || ' ' || COALESCE(p.display_name, '')), ?) > 0".into()
        }
        Term::To(v) => address_list("m.to_addresses", v, params),
        Term::Cc(v) => address_list("m.cc_addresses", v, params),
        Term::Subject(v) => {
            text(params, v);
            "instr(zfold(m.subject), ?) > 0".into()
        }
        Term::Filename(v) => {
            text(params, v);
            "EXISTS (SELECT 1 FROM attachments a WHERE a.message_id = m.id AND a.inline = 0 AND instr(zfold(a.filename), ?) > 0)".into()
        }
        Term::Label(v) => {
            text(params, v);
            text(params, v);
            "EXISTS (SELECT 1 FROM message_labels ml JOIN labels l ON l.id = ml.label_id \
             WHERE ml.message_id = m.id AND (zfold(l.provider_id) = ? OR zfold(l.name) = ?))"
                .into()
        }
        Term::ListId(v) => {
            text(params, v);
            "instr(zfold(COALESCE(m.list_id, '')), ?) > 0".into()
        }
        Term::Person(v) => {
            text(params, v);
            text(params, v);
            "(instr(zfold(COALESCE(p.display_name, '')), ?) > 0 OR EXISTS (SELECT 1 FROM person_addresses pa \
             WHERE pa.person_id = p.id AND instr(zfold(pa.address), ?) > 0))"
                .into()
        }
        Term::Group(v) => {
            text(params, v);
            "EXISTS (SELECT 1 FROM person_groups pg JOIN groups g ON g.id = pg.group_id \
             WHERE pg.person_id = p.id AND instr(zfold(g.name), ?) > 0)"
                .into()
        }
        Term::Has(HasKind::Attachment) => {
            "EXISTS (SELECT 1 FROM attachments a WHERE a.message_id = m.id AND a.inline = 0)".into()
        }
        Term::Has(HasKind::Unsubscribe) => "m.unsubscribe IS NOT NULL".into(),
        Term::Is(kind) => match kind {
            IsKind::Unread => "m.unread = 1",
            IsKind::Read => "m.unread = 0",
            IsKind::Starred => "m.starred = 1",
            IsKind::Inbox => "c.in_inbox = 1",
            IsKind::Archived => "(c.in_inbox = 0 AND c.trashed = 0)",
            IsKind::Trashed => "c.trashed = 1",
            IsKind::Hidden => "(c.hidden_at IS NOT NULL OR p.hidden_at IS NOT NULL)",
            IsKind::Sent => "m.is_from_me = 1",
            IsKind::NeedsReply => "(c.last_from_me = 0 AND c.category = 'person')",
        }
        .into(),
        Term::Category(c) => {
            params.push(Value::Text(c.clone()));
            "m.category = ?".into()
        }
        Term::In(folder) => match folder {
            Folder::Inbox => "c.in_inbox = 1",
            Folder::Sent => "m.is_from_me = 1",
            Folder::Trash => "c.trashed = 1",
            Folder::Archive => "(c.in_inbox = 0 AND c.trashed = 0)",
            Folder::Anywhere => "1",
        }
        .into(),
        Term::NewerThan(ms) => {
            params.push(Value::Integer(now - ms));
            "m.sent_at >= ?".into()
        }
        Term::OlderThan(ms) => {
            params.push(Value::Integer(now - ms));
            "m.sent_at < ?".into()
        }
        Term::After(t) => {
            params.push(Value::Integer(*t));
            "m.sent_at >= ?".into()
        }
        Term::Before(t) => {
            params.push(Value::Integer(*t));
            "m.sent_at < ?".into()
        }
        Term::Larger(n) => {
            params.push(Value::Integer(*n));
            "COALESCE(m.size_bytes, 0) > ?".into()
        }
        Term::Smaller(n) => {
            params.push(Value::Integer(*n));
            "COALESCE(m.size_bytes, 0) < ?".into()
        }
    }
}

fn address_list(column: &str, needle: &str, params: &mut Vec<Value>) -> String {
    text(params, needle);
    format!(
        "EXISTS (SELECT 1 FROM json_each({column}) j WHERE instr(zfold(COALESCE(json_extract(j.value, '$.email'), '') \
         || ' ' || COALESCE(json_extract(j.value, '$.name'), '')), ?) > 0)"
    )
}

pub(super) fn format_span(ms: i64) -> String {
    const MINUTE: i64 = 60_000;
    for (unit, per) in [
        ("y", 365 * 24 * 60 * MINUTE),
        ("m", 30 * 24 * 60 * MINUTE),
        ("w", 7 * 24 * 60 * MINUTE),
        ("d", 24 * 60 * MINUTE),
        ("h", 60 * MINUTE),
    ] {
        if ms % per == 0 {
            return format!("{}{unit}", ms / per);
        }
    }
    format!("{}h", (ms / (60 * MINUTE)).max(1))
}

pub(super) fn format_date(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms)
        .map(|t| t.to_zoned_datetime(jiff::tz::TimeZone::UTC).date().to_string())
        .unwrap_or_default()
}

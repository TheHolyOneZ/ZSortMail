use super::{Folder, HasKind, IsKind, Query, Term};
use crate::text::{fold, words};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressFact {
    pub email: String,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct MessageFacts {
    pub from_address: String,
    pub from_name: String,
    pub person_addresses: Vec<String>,
    pub groups: Vec<String>,
    pub to: Vec<AddressFact>,
    pub cc: Vec<AddressFact>,
    pub subject: String,
    pub body: String,
    pub participants: String,
    pub is_from_me: bool,
    pub unread: bool,
    pub starred: bool,
    pub filenames: Vec<String>,
    pub labels: Vec<String>,
    pub label_names: Vec<String>,
    pub category: String,
    pub list_id: String,
    pub has_unsubscribe: bool,
    pub size: i64,
    pub sent_at: i64,
    pub in_inbox: bool,
    pub trashed: bool,
    pub hidden: bool,
    pub conversation_needs_reply: bool,
}

fn contains(haystack: &str, needle: &str) -> bool {
    fold(haystack).contains(&fold(needle))
}

fn address_hit(list: &[AddressFact], needle: &str) -> bool {
    list.iter().any(|a| contains(&format!("{} {}", a.email, a.name), needle))
}

fn words_hit(columns: &[&str], query_words: &[String]) -> bool {
    let tokens: Vec<String> = columns.iter().flat_map(|c| words(c)).collect();
    query_words.iter().all(|w| tokens.iter().any(|t| t.starts_with(w.as_str())))
}

fn phrase_hit(columns: &[&str], phrase: &[String]) -> bool {
    if phrase.is_empty() {
        return true;
    }
    columns.iter().any(|column| {
        let tokens = words(column);
        tokens.windows(phrase.len()).any(|w| w == phrase)
    })
}

impl Query {
    pub fn matches(&self, f: &MessageFacts, now_ms: i64) -> bool {
        match self {
            Query::And(items) => items.iter().all(|q| q.matches(f, now_ms)),
            Query::Or(items) => items.iter().any(|q| q.matches(f, now_ms)),
            Query::Not(inner) => !inner.matches(f, now_ms),
            Query::Term(term) => term_matches(term, f, now_ms),
        }
    }
}

fn term_matches(term: &Term, f: &MessageFacts, now_ms: i64) -> bool {
    match term {
        Term::Words(w) => words_hit(&[&f.subject, &f.body, &f.participants], w),
        Term::Phrase(w) => phrase_hit(&[&f.subject, &f.body, &f.participants], w),
        Term::From(v) => contains(&format!("{} {}", f.from_address, f.from_name), v),
        Term::To(v) => address_hit(&f.to, v),
        Term::Cc(v) => address_hit(&f.cc, v),
        Term::Subject(v) => contains(&f.subject, v),
        Term::Filename(v) => f.filenames.iter().any(|n| contains(n, v)),
        Term::Label(v) => {
            let needle = fold(v);
            f.labels.iter().chain(f.label_names.iter()).any(|l| fold(l) == needle)
        }
        Term::ListId(v) => contains(&f.list_id, v),
        Term::Person(v) => contains(&f.from_name, v) || f.person_addresses.iter().any(|a| contains(a, v)),
        Term::Group(v) => f.groups.iter().any(|g| contains(g, v)),
        Term::Has(HasKind::Attachment) => !f.filenames.is_empty(),
        Term::Has(HasKind::Unsubscribe) => f.has_unsubscribe,
        Term::Is(kind) => match kind {
            IsKind::Unread => f.unread,
            IsKind::Read => !f.unread,
            IsKind::Starred => f.starred,
            IsKind::Inbox => f.in_inbox,
            IsKind::Archived => !f.in_inbox && !f.trashed,
            IsKind::Trashed => f.trashed,
            IsKind::Hidden => f.hidden,
            IsKind::Sent => f.is_from_me,
            IsKind::NeedsReply => f.conversation_needs_reply,
        },
        Term::Category(c) => f.category == *c,
        Term::In(folder) => match folder {
            Folder::Inbox => f.in_inbox,
            Folder::Sent => f.is_from_me,
            Folder::Trash => f.trashed,
            Folder::Archive => !f.in_inbox && !f.trashed,
            Folder::Anywhere => true,
        },
        Term::NewerThan(ms) => f.sent_at >= now_ms - ms,
        Term::OlderThan(ms) => f.sent_at < now_ms - ms,
        Term::After(t) => f.sent_at >= *t,
        Term::Before(t) => f.sent_at < *t,
        Term::Larger(n) => f.size > *n,
        Term::Smaller(n) => f.size < *n,
    }
}

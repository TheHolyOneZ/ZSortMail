mod eval;
mod parse;
mod sql;

use serde::{Deserialize, Serialize};

pub use eval::{AddressFact, MessageFacts};
pub use parse::parse;
pub use sql::{Compiled, FROM_CLAUSE};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Query {
    And(Vec<Query>),
    Or(Vec<Query>),
    Not(Box<Query>),
    Term(Term),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HasKind {
    Attachment,
    Unsubscribe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsKind {
    Unread,
    Read,
    Starred,
    Inbox,
    Archived,
    Trashed,
    Hidden,
    Sent,
    NeedsReply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Folder {
    Inbox,
    Sent,
    Trash,
    Archive,
    Anywhere,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Term {
    Words(Vec<String>),
    Phrase(Vec<String>),
    From(String),
    To(String),
    Cc(String),
    Subject(String),
    Filename(String),
    Label(String),
    ListId(String),
    Person(String),
    Group(String),
    Has(HasKind),
    Is(IsKind),
    Category(String),
    In(Folder),
    NewerThan(i64),
    OlderThan(i64),
    After(i64),
    Before(i64),
    Larger(i64),
    Smaller(i64),
}

impl Query {
    pub fn all() -> Query {
        Query::And(Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Query::And(items) if items.is_empty())
    }

    pub fn wants_hidden(&self) -> bool {
        match self {
            Query::And(items) | Query::Or(items) => items.iter().any(Query::wants_hidden),
            Query::Not(inner) => inner.wants_hidden(),
            Query::Term(Term::Is(IsKind::Hidden | IsKind::Trashed))
            | Query::Term(Term::In(Folder::Trash | Folder::Anywhere)) => true,
            Query::Term(_) => false,
        }
    }

    pub fn mentions_sent(&self) -> bool {
        match self {
            Query::And(items) | Query::Or(items) => items.iter().any(Query::mentions_sent),
            Query::Not(inner) => inner.mentions_sent(),
            Query::Term(Term::Is(IsKind::Sent) | Term::In(Folder::Sent)) => true,
            Query::Term(_) => false,
        }
    }

    pub fn words_only(&self) -> bool {
        match self {
            Query::And(items) => items.iter().all(|q| matches!(q, Query::Term(Term::Words(_) | Term::Phrase(_)))),
            Query::Term(Term::Words(_) | Term::Phrase(_)) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn quote(value: &str) -> String {
            if value.is_empty() || value.contains(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == '"') {
                format!("\"{}\"", value.replace('"', ""))
            } else {
                value.to_string()
            }
        }
        match self {
            Query::And(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|q| match q {
                        Query::Or(_) => format!("({q})"),
                        other => other.to_string(),
                    })
                    .collect();
                write!(f, "{}", parts.join(" "))
            }
            Query::Or(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|q| match q {
                        Query::And(inner) if inner.len() > 1 => format!("({q})"),
                        other => other.to_string(),
                    })
                    .collect();
                write!(f, "{}", parts.join(" OR "))
            }
            Query::Not(inner) => match **inner {
                Query::And(ref items) if items.len() > 1 => write!(f, "-({inner})"),
                Query::Or(_) => write!(f, "-({inner})"),
                _ => write!(f, "-{inner}"),
            },
            Query::Term(term) => match term {
                Term::Words(words) => write!(f, "{}", words.join(" ")),
                Term::Phrase(words) => write!(f, "\"{}\"", words.join(" ")),
                Term::From(v) => write!(f, "from:{}", quote(v)),
                Term::To(v) => write!(f, "to:{}", quote(v)),
                Term::Cc(v) => write!(f, "cc:{}", quote(v)),
                Term::Subject(v) => write!(f, "subject:{}", quote(v)),
                Term::Filename(v) => write!(f, "filename:{}", quote(v)),
                Term::Label(v) => write!(f, "label:{}", quote(v)),
                Term::ListId(v) => write!(f, "list:{}", quote(v)),
                Term::Person(v) => write!(f, "person:{}", quote(v)),
                Term::Group(v) => write!(f, "group:{}", quote(v)),
                Term::Has(HasKind::Attachment) => write!(f, "has:attachment"),
                Term::Has(HasKind::Unsubscribe) => write!(f, "has:unsubscribe"),
                Term::Is(kind) => write!(
                    f,
                    "is:{}",
                    match kind {
                        IsKind::Unread => "unread",
                        IsKind::Read => "read",
                        IsKind::Starred => "starred",
                        IsKind::Inbox => "inbox",
                        IsKind::Archived => "archived",
                        IsKind::Trashed => "trashed",
                        IsKind::Hidden => "hidden",
                        IsKind::Sent => "sent",
                        IsKind::NeedsReply => "needsreply",
                    }
                ),
                Term::Category(v) => write!(f, "category:{v}"),
                Term::In(folder) => write!(
                    f,
                    "in:{}",
                    match folder {
                        Folder::Inbox => "inbox",
                        Folder::Sent => "sent",
                        Folder::Trash => "trash",
                        Folder::Archive => "archive",
                        Folder::Anywhere => "anywhere",
                    }
                ),
                Term::NewerThan(ms) => write!(f, "newer_than:{}", sql::format_span(*ms)),
                Term::OlderThan(ms) => write!(f, "older_than:{}", sql::format_span(*ms)),
                Term::After(ms) => write!(f, "after:{}", sql::format_date(*ms)),
                Term::Before(ms) => write!(f, "before:{}", sql::format_date(*ms)),
                Term::Larger(n) => write!(f, "larger:{n}"),
                Term::Smaller(n) => write!(f, "smaller:{n}"),
            },
        }
    }
}

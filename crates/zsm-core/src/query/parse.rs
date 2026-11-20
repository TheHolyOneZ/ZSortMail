use jiff::civil::Date;
use jiff::tz::TimeZone;

use super::{Folder, HasKind, IsKind, Query, Term};
use crate::text::words;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Or,
    Not,
    Word(String, bool),
    Quoted(String),
}

fn tokenize(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                out.push(Token::Open);
                i += 1;
            }
            ')' => {
                out.push(Token::Close);
                i += 1;
            }
            '"' => {
                let (text, next) = read_quoted(&chars, i);
                out.push(Token::Quoted(text));
                i = next;
            }
            '-' if chars.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != '-') => {
                out.push(Token::Not);
                i += 1;
            }
            _ => {
                let start = i;
                let mut quoted = false;
                let mut value = String::new();
                while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '(' && chars[i] != ')' {
                    if chars[i] == '"' && chars[start..i].contains(&':') {
                        let (text, next) = read_quoted(&chars, i);
                        value.push_str(&text);
                        quoted = true;
                        i = next;
                    } else {
                        value.push(chars[i]);
                        i += 1;
                    }
                }
                if value == "OR" || value == "|" {
                    out.push(Token::Or);
                } else {
                    out.push(Token::Word(value, quoted));
                }
            }
        }
    }
    out
}

fn read_quoted(chars: &[char], open: usize) -> (String, usize) {
    let mut i = open + 1;
    let mut text = String::new();
    while i < chars.len() && chars[i] != '"' {
        text.push(chars[i]);
        i += 1;
    }
    (text, (i + 1).min(chars.len()))
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn or_expr(&mut self) -> Query {
        let mut branches = vec![self.and_expr()];
        while self.peek() == Some(&Token::Or) {
            self.at += 1;
            branches.push(self.and_expr());
        }
        branches.retain(|b| !b.is_empty());
        match branches.len() {
            0 => Query::all(),
            1 => branches.remove(0),
            _ => Query::Or(branches),
        }
    }

    fn and_expr(&mut self) -> Query {
        let mut items = Vec::new();
        while let Some(token) = self.peek() {
            if matches!(token, Token::Close | Token::Or) {
                break;
            }
            if let Some(item) = self.unary() {
                items.push(item);
            }
        }
        coalesce(items)
    }

    fn unary(&mut self) -> Option<Query> {
        let token = self.tokens.get(self.at)?.clone();
        self.at += 1;
        match token {
            Token::Not => self.unary().map(|inner| Query::Not(Box::new(inner))),
            Token::Open => {
                let inner = self.or_expr();
                if self.peek() == Some(&Token::Close) {
                    self.at += 1;
                }
                (!inner.is_empty()).then_some(inner)
            }
            Token::Close | Token::Or => None,
            Token::Quoted(text) => {
                let w = words(&text);
                match w.len() {
                    0 => None,
                    1 => Some(Query::Term(Term::Words(w))),
                    _ => Some(Query::Term(Term::Phrase(w))),
                }
            }
            Token::Word(text, quoted) => atom(&text, quoted),
        }
    }
}

fn coalesce(items: Vec<Query>) -> Query {
    let mut out: Vec<Query> = Vec::new();
    for item in items {
        match (out.last_mut(), item) {
            (Some(Query::Term(Term::Words(existing))), Query::Term(Term::Words(more))) => existing.extend(more),
            (_, item) => out.push(item),
        }
    }
    if out.len() == 1 { out.remove(0) } else { Query::And(out) }
}

fn atom(text: &str, quoted: bool) -> Option<Query> {
    if let Some((field, value)) = text.split_once(':')
        && !field.is_empty()
        && !value.is_empty()
        && field.chars().all(|c| c.is_ascii_alphabetic() || c == '_')
        && let Some(term) = field_term(&field.to_ascii_lowercase(), value, quoted)
    {
        return Some(Query::Term(term));
    }
    let w = words(text);
    (!w.is_empty()).then_some(Query::Term(Term::Words(w)))
}

fn field_term(field: &str, value: &str, _quoted: bool) -> Option<Term> {
    let value = value.trim();
    Some(match field {
        "from" => Term::From(value.into()),
        "to" => Term::To(value.into()),
        "cc" => Term::Cc(value.into()),
        "subject" => Term::Subject(value.into()),
        "filename" => Term::Filename(value.into()),
        "label" => Term::Label(value.into()),
        "list" => Term::ListId(value.into()),
        "person" | "name" => Term::Person(value.into()),
        "group" => Term::Group(value.into()),
        "has" => match value.to_ascii_lowercase().as_str() {
            "attachment" | "attachments" | "file" | "files" => Term::Has(HasKind::Attachment),
            "unsubscribe" => Term::Has(HasKind::Unsubscribe),
            _ => return None,
        },
        "is" => Term::Is(match value.to_ascii_lowercase().as_str() {
            "unread" => IsKind::Unread,
            "read" => IsKind::Read,
            "starred" | "star" => IsKind::Starred,
            "inbox" => IsKind::Inbox,
            "archived" | "archive" => IsKind::Archived,
            "trashed" | "trash" => IsKind::Trashed,
            "hidden" => IsKind::Hidden,
            "sent" => IsKind::Sent,
            "needsreply" | "reply" => IsKind::NeedsReply,
            _ => return None,
        }),
        "in" => Term::In(match value.to_ascii_lowercase().as_str() {
            "inbox" => Folder::Inbox,
            "sent" => Folder::Sent,
            "trash" => Folder::Trash,
            "archive" | "archived" => Folder::Archive,
            "anywhere" | "all" => Folder::Anywhere,
            _ => return None,
        }),
        "category" => match value.to_ascii_lowercase().trim_end_matches('s') {
            "person" | "people" => Term::Category("person".into()),
            "newsletter" => Term::Category("newsletter".into()),
            "receipt" => Term::Category("receipt".into()),
            "automated" => Term::Category("automated".into()),
            _ => return None,
        },
        "newer_than" => Term::NewerThan(parse_span(value)?),
        "older_than" => Term::OlderThan(parse_span(value)?),
        "after" => Term::After(parse_date(value)?),
        "before" => Term::Before(parse_date(value)?),
        "larger" => Term::Larger(parse_size(value)?),
        "smaller" => Term::Smaller(parse_size(value)?),
        _ => return None,
    })
}

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

fn parse_span(value: &str) -> Option<i64> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = digits.parse().ok()?;
    let unit = &value[digits.len()..];
    let per = match unit.to_ascii_lowercase().as_str() {
        "" | "d" => DAY,
        "h" => HOUR,
        "w" => 7 * DAY,
        "m" => 30 * DAY,
        "y" => 365 * DAY,
        _ => return None,
    };
    n.checked_mul(per).filter(|ms| *ms > 0)
}

fn parse_date(value: &str) -> Option<i64> {
    let parts: Vec<&str> = value.split(['-', '/']).collect();
    let [y, m, d] = parts[..] else { return None };
    let date = Date::new(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?).ok()?;
    date.to_zoned_datetime(TimeZone::UTC).ok().map(|z| z.timestamp().as_millisecond())
}

fn parse_size(value: &str) -> Option<i64> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    let n: i64 = digits.parse().ok()?;
    let per: i64 = match value[digits.len()..].to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" => 1_000,
        "m" | "mb" => 1_000_000,
        "g" | "gb" => 1_000_000_000,
        _ => return None,
    };
    n.checked_mul(per)
}

pub fn parse(input: &str) -> Query {
    let mut parser = Parser { tokens: tokenize(input), at: 0 };
    let mut items = Vec::new();
    while parser.at < parser.tokens.len() {
        items.push(parser.or_expr());
        if parser.peek() == Some(&Token::Close) {
            parser.at += 1;
        }
    }
    let items: Vec<Query> = items.into_iter().filter(|q| !q.is_empty()).collect();
    match items.len() {
        0 => Query::all(),
        1 => items.into_iter().next().unwrap_or_else(Query::all),
        _ => coalesce(items),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::Term::*;

    fn t(term: Term) -> Query {
        Query::Term(term)
    }

    #[test]
    fn plain_words_become_one_words_term_and_fold() {
        assert_eq!(parse("xbox 394"), t(Words(vec!["xbox".into(), "394".into()])));
        assert_eq!(parse("  Amélie  "), t(Words(vec!["amelie".into()])));
        assert_eq!(parse(""), Query::all());
        assert_eq!(parse("   "), Query::all());
        assert_eq!(parse("order #394-2211"), t(Words(vec!["order".into(), "394".into(), "2211".into()])));
    }

    #[test]
    fn fields_flags_and_phrases() {
        let q = parse("from:maya has:attachment \"launch checklist\" -subject:invoice newer_than:30d is:unread");
        assert_eq!(
            q,
            Query::And(vec![
                t(From("maya".into())),
                t(Has(HasKind::Attachment)),
                t(Phrase(vec!["launch".into(), "checklist".into()])),
                Query::Not(Box::new(t(Subject("invoice".into())))),
                t(NewerThan(30 * DAY)),
                t(Is(IsKind::Unread)),
            ])
        );
        assert_eq!(parse("from:\"Maya Okafor\""), t(From("Maya Okafor".into())));
        assert_eq!(parse("category:newsletters"), t(Category("newsletter".into())));
        assert_eq!(parse("in:archive"), t(In(Folder::Archive)));
    }

    #[test]
    fn or_and_parentheses_group_as_expected() {
        let q = parse("from:maya OR from:dan has:attachment");
        assert_eq!(
            q,
            Query::Or(vec![
                t(From("maya".into())),
                Query::And(vec![t(From("dan".into())), t(Has(HasKind::Attachment))])
            ])
        );
        let q = parse("(from:maya OR from:dan) has:attachment");
        assert_eq!(
            q,
            Query::And(vec![
                Query::Or(vec![t(From("maya".into())), t(From("dan".into()))]),
                t(Has(HasKind::Attachment))
            ])
        );
        assert_eq!(
            parse("-(from:maya OR from:dan)"),
            Query::Not(Box::new(Query::Or(vec![t(From("maya".into())), t(From("dan".into()))])))
        );
    }

    #[test]
    fn spans_dates_and_sizes() {
        assert_eq!(parse("newer_than:2h"), t(NewerThan(2 * HOUR)));
        assert_eq!(parse("older_than:3m"), t(OlderThan(90 * DAY)));
        assert_eq!(parse("newer_than:1y"), t(NewerThan(365 * DAY)));
        assert_eq!(parse("after:2026-11-01"), t(After(1_793_491_200_000)));
        assert_eq!(parse("before:2026/12/31"), t(Before(1_798_675_200_000)));
        assert_eq!(parse("larger:5M"), t(Larger(5_000_000)));
        assert_eq!(parse("smaller:200k"), t(Smaller(200_000)));
    }

    #[test]
    fn nonsense_degrades_to_words_instead_of_failing() {
        assert_eq!(parse("from:"), t(Words(vec!["from".into()])));
        assert_eq!(parse("is:banana"), t(Words(vec!["is".into(), "banana".into()])));
        assert_eq!(parse("newer_than:soon"), t(Words(vec!["newer".into(), "than".into(), "soon".into()])));
        assert_eq!(parse("after:2026-13-45"), t(Words(vec!["after".into(), "2026".into(), "13".into(), "45".into()])));
        assert_eq!(parse("((("), Query::all());
        assert_eq!(parse("a ) b"), t(Words(vec!["a".into(), "b".into()])));
        assert_eq!(parse("OR"), Query::all());
        assert_eq!(parse("- - -"), Query::all());
    }

    #[test]
    fn a_hyphen_inside_a_word_is_not_a_negation() {
        assert_eq!(parse("e-mail"), t(Words(vec!["e".into(), "mail".into()])));
        assert_eq!(parse("-spam"), Query::Not(Box::new(t(Words(vec!["spam".into()])))));
    }

    #[test]
    fn canonical_text_parses_back_to_the_same_query() {
        for text in [
            "from:maya has:attachment \"launch checklist\" -subject:invoice newer_than:30d is:unread",
            "(from:maya OR from:dan) -category:newsletter",
            "xbox 394",
            "subject:\"order shipped\" in:anywhere",
            "after:2026-11-01 before:2026-12-01 larger:5000000",
            "person:\"Maya Okafor\" label:Clients -(is:starred OR is:hidden)",
            "group:Work -group:\"Close friends\"",
        ] {
            let q = parse(text);
            let printed = q.to_string();
            assert_eq!(parse(&printed), q, "{text} printed as {printed}");
        }
    }

    #[test]
    fn it_reports_what_a_query_asks_about() {
        assert!(parse("is:hidden").wants_hidden());
        assert!(parse("from:a OR in:trash").wants_hidden());
        assert!(!parse("from:a is:unread").wants_hidden());
        assert!(parse("xbox \"launch checklist\"").words_only());
        assert!(!parse("xbox from:maya").words_only());
    }
}

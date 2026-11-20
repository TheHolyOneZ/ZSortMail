use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    pub text: String,
    pub quoted: Option<String>,
}

static ATTRIBUTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?is)^\s*(on|le|am|el|il giorno|em|op|den|på|dne|w dniu|в|am)\b.{4,300}?\b(wrote|a écrit|écrit|schrieb|geschreven|escribió|ha scritto|escreveu|schreef|skrev|napisał|написал[аи]?)\b[^\n]{0,80}:\s*$",
    )
    .expect("valid regex")
});

static ORIGINAL_MESSAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*-{2,}\s*(original message|ursprüngliche nachricht|message d'origine|mensaje original|messaggio originale|mensagem original)\s*-{2,}\s*$")
        .expect("valid regex")
});

static OUTLOOK_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*_{20,}\s*$").expect("valid regex"));

static HEADER_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(from|von|de|da|van|sent|gesendet|envoyé|enviado|date|datum|to|an|à|para|a|subject|betreff|objet|asunto)\s*:").expect("valid regex")
});

fn is_quote_line(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

fn rest_is_quoted(lines: &[&str], from: usize) -> bool {
    let mut saw_quote = false;
    for line in &lines[from.min(lines.len())..] {
        if line.trim().is_empty() {
            continue;
        }
        if !is_quote_line(line) {
            return false;
        }
        saw_quote = true;
    }
    saw_quote
}

fn attribution_len(lines: &[&str], i: usize) -> Option<usize> {
    for span in 1..=3usize {
        if i + span > lines.len() {
            break;
        }
        let joined = lines[i..i + span].join("\n");
        if ATTRIBUTION.is_match(&joined) {
            return Some(span);
        }
    }
    None
}

fn header_block(lines: &[&str], i: usize) -> bool {
    let window: Vec<&str> = lines[i..].iter().take(6).copied().collect();
    let heads = window.iter().take_while(|l| HEADER_LINE.is_match(l)).count();
    heads >= 3
        && HEADER_LINE.is_match(lines[i])
        && lines[i].trim_start().to_ascii_lowercase().starts_with(['f', 'v', 'd'])
}

pub fn strip_quotes(input: &str) -> Split {
    let normalised = input.replace("\r\n", "\n");
    let lines: Vec<&str> = normalised.lines().collect();

    let mut cut: Option<usize> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(span) = attribution_len(&lines, i) {
            if rest_is_quoted(&lines, i + span) {
                cut = Some(i);
                break;
            }
            i += span;
            continue;
        }
        if ORIGINAL_MESSAGE.is_match(line)
            || (OUTLOOK_RULE.is_match(line) && lines.get(i + 1).is_some_and(|l| HEADER_LINE.is_match(l)))
            || header_block(&lines, i)
        {
            cut = Some(i);
            break;
        }
        if is_quote_line(line) && rest_is_quoted(&lines, i) {
            cut = Some(i);
            break;
        }
        i += 1;
    }

    let Some(cut) = cut else {
        return Split { text: input.trim().to_string(), quoted: None };
    };
    let text = lines[..cut].join("\n").trim_end().to_string();
    let quoted = lines[cut..].join("\n").trim().to_string();
    if text.trim().is_empty() {
        return Split { text: input.trim().to_string(), quoted: None };
    }
    Split { text, quoted: Some(quoted) }
}

static HTML_QUOTE_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<div[^>]*class="[^"]*(?:gmail_quote|yahoo_quoted|moz-cite-prefix|protonmail_quote)[^"]*"|<div[^>]*id="(?:appendonsend|divRplyFwdMsg)"|<blockquote[^>]*type="cite"|<div[^>]*class="[^"]*gmail_attr"#,
    )
    .expect("valid regex")
});

pub fn split_quoted_html(html: &str) -> (String, Option<String>) {
    let Some(found) = HTML_QUOTE_START.find(html) else {
        return (html.to_string(), None);
    };
    let (before, after) = html.split_at(found.start());
    if crate::text::html_to_text(before).trim().is_empty() {
        return (html.to_string(), None);
    }
    (before.to_string(), Some(after.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_an_english_reply() {
        let s = strip_quotes(
            "Sounds good, Thursday works.\n\nOn Tue, 9 Sep 2026 at 09:12, Maya Okafor <maya@northwind.example> wrote:\n> Can we meet?\n> Thursday?\n",
        );
        assert_eq!(s.text, "Sounds good, Thursday works.");
        assert!(s.quoted.as_deref().unwrap().contains("Can we meet?"));
    }

    #[test]
    fn handles_an_attribution_wrapped_over_two_lines() {
        let s = strip_quotes(
            "Yes.\n\nOn Tuesday, September 9, 2026 at 9:12:44 AM GMT+2, Maya Okafor <maya@northwind.example>\nwrote:\n> hi",
        );
        assert_eq!(s.text, "Yes.");
        assert!(s.quoted.is_some());
    }

    #[test]
    fn handles_other_languages() {
        let fr = strip_quotes(
            "Parfait.\n\nLe mar. 9 sept. 2026 à 09:12, Amélie <a@lepetitzinc.example> a écrit :\n> Bonjour",
        );
        assert_eq!(fr.text, "Parfait.");
        let de = strip_quotes("Danke!\n\nAm 09.09.2026 um 09:12 schrieb Hanna Berg <h@bergtandvard.example>:\n> Hallo");
        assert_eq!(de.text, "Danke!");
        let es = strip_quotes("Gracias.\n\nEl mar, 9 sep 2026 a las 9:12, Ana <a@x.example> escribió:\n> Hola");
        assert_eq!(es.text, "Gracias.");
    }

    #[test]
    fn handles_outlook_separators() {
        let original = strip_quotes(
            "Will do.\n\n-----Original Message-----\nFrom: Rasmus\nSent: Monday\nTo: me\nSubject: Invoice\n\nPlease send.",
        );
        assert_eq!(original.text, "Will do.");
        let rule = strip_quotes(
            "Done.\n\n________________________________\nFrom: Rasmus Lindqvist <r@x.example>\nSent: Monday, 9 September 2026 09:12\nTo: me\nSubject: Invoice\n\nBody",
        );
        assert_eq!(rule.text, "Done.");
        let block = strip_quotes("OK.\n\nFrom: Rasmus <r@x.example>\nSent: Monday\nTo: me\nSubject: Invoice\n\nBody");
        assert_eq!(block.text, "OK.");
    }

    #[test]
    fn splits_a_bare_quote_block_at_the_end() {
        let s = strip_quotes("Agreed.\n\n> earlier text\n> more");
        assert_eq!(s.text, "Agreed.");
        assert_eq!(s.quoted.as_deref(), Some("> earlier text\n> more"));
    }

    #[test]
    fn keeps_a_bottom_posted_reply_whole() {
        let body =
            "On Tue, 9 Sep 2026, Maya wrote:\n> Can we meet Thursday?\nYes, Thursday.\n> And the budget?\nApproved.";
        let s = strip_quotes(body);
        assert_eq!(s.quoted, None);
        assert!(s.text.contains("Yes, Thursday.") && s.text.contains("Approved."));
    }

    #[test]
    fn a_message_that_is_all_quote_is_shown_whole() {
        let s = strip_quotes("> only\n> quoted");
        assert_eq!(s.quoted, None);
        assert_eq!(s.text, "> only\n> quoted");
    }

    #[test]
    fn leaves_plain_messages_and_signatures_alone() {
        let s = strip_quotes("Hello,\n\nSee you Friday.\n\n-- \nMaya Okafor\nDesign lead");
        assert_eq!(s.quoted, None);
        assert!(s.text.contains("Design lead"));
    }

    #[test]
    fn crlf_is_handled() {
        let s = strip_quotes("Ok.\r\n\r\nOn Mon, 1 Jan 2026, X wrote:\r\n> hi\r\n");
        assert_eq!(s.text, "Ok.");
    }

    #[test]
    fn splits_gmail_html_quotes() {
        let (new, quoted) = split_quoted_html(
            r#"<div dir="ltr">Thanks!</div><br><div class="gmail_quote"><div class="gmail_attr">On Tue, X wrote:<br></div><blockquote class="gmail_quote">old</blockquote></div>"#,
        );
        assert!(new.contains("Thanks!") && !new.contains("old"));
        assert!(quoted.unwrap().contains("old"));
    }

    #[test]
    fn splits_outlook_and_apple_quotes() {
        let (new, quoted) = split_quoted_html(
            r#"<p>Yes</p><div id="appendonsend"></div><hr><div id="divRplyFwdMsg"><b>From:</b> A</div><p>history</p>"#,
        );
        assert!(new.contains("Yes") && !new.contains("history"));
        assert!(quoted.is_some());
        let (new, quoted) = split_quoted_html(r#"<div>Hi</div><blockquote type="cite">old</blockquote>"#);
        assert!(!new.contains("old"));
        assert!(quoted.is_some());
    }

    #[test]
    fn html_without_a_quote_is_untouched() {
        let (new, quoted) = split_quoted_html("<p>Just a message</p>");
        assert_eq!(new, "<p>Just a message</p>");
        assert_eq!(quoted, None);
    }

    #[test]
    fn a_forward_that_starts_with_the_quote_is_kept_whole() {
        let html = r#"<div class="gmail_quote">forwarded</div>"#;
        assert_eq!(split_quoted_html(html), (html.to_string(), None));
    }
}

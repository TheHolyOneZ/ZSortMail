use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use mail_parser::MessageParser;
use zsm_core::store::ingest::{Address, HeaderFacts, IncomingAttachment, IncomingMessage};

use crate::error::{Error, Result};
use crate::model::{GmailMessage, Part};

pub const METADATA_HEADERS: &[&str] = &[
    "From",
    "To",
    "Cc",
    "Subject",
    "Date",
    "Message-ID",
    "In-Reply-To",
    "References",
    "List-Id",
    "List-Unsubscribe",
    "List-Unsubscribe-Post",
    "Precedence",
    "Auto-Submitted",
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParsedHeaders {
    pub from: Vec<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub subject: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
}

pub fn parse_headers(headers: &[(&str, &str)]) -> ParsedHeaders {
    let mut block = String::new();
    for (name, value) in headers {
        let value: String = value.chars().map(|c| if c == '\r' || c == '\n' { ' ' } else { c }).collect();
        block.push_str(name);
        block.push_str(": ");
        block.push_str(&value);
        block.push_str("\r\n");
    }
    block.push_str("\r\n");
    let Some(message) = MessageParser::new().parse_headers(&block) else {
        return ParsedHeaders::default();
    };
    let list = |list: Option<mail_parser::AddressList<'_>>| -> Vec<Address> {
        list.map(|l| {
            l.mailboxes()
                .filter_map(|m| m.address().map(|email| Address::new(email, m.name())))
                .filter(|a| a.email.contains('@'))
                .collect()
        })
        .unwrap_or_default()
    };
    ParsedHeaders {
        from: list(message.from()),
        to: list(message.to()),
        cc: list(message.cc()),
        subject: message.subject().map(str::to_string),
        message_id: message.message_id().map(|id| format!("<{id}>")),
        in_reply_to: message.in_reply_to().and_then(|l| l.first()).map(|id| format!("<{id}>")),
    }
}

fn decode_base64url(data: &str) -> Vec<u8> {
    URL_SAFE_NO_PAD.decode(data.trim_end_matches('=')).unwrap_or_default()
}

fn decode_charset(bytes: &[u8], charset: Option<&str>) -> String {
    let encoding = charset
        .and_then(|c| encoding_rs::Encoding::for_label(c.trim().trim_matches('"').as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    encoding.decode(bytes).0.into_owned()
}

fn charset_of(content_type: Option<&str>) -> Option<String> {
    let value = content_type?;
    let lower = value.to_ascii_lowercase();
    let at = lower.find("charset=")?;
    let rest = &value[at + "charset=".len()..];
    let end = rest.find(';').unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('"').to_string())
}

#[derive(Default)]
struct Walked {
    text: Vec<String>,
    html: Vec<String>,
    attachments: Vec<IncomingAttachment>,
}

fn is_attachment(part: &Part) -> bool {
    if !part.filename.is_empty() {
        return true;
    }
    part.header("Content-Disposition").is_some_and(|d| d.trim_start().to_ascii_lowercase().starts_with("attachment"))
}

fn walk(part: &Part, out: &mut Walked) {
    if !part.parts.is_empty() {
        for child in &part.parts {
            walk(child, out);
        }
        return;
    }
    let mime = part.mime_type.to_ascii_lowercase();
    if is_attachment(part) || part.body.attachment_id.is_some() && !mime.starts_with("text/") {
        let content_id = part
            .header("Content-ID")
            .map(|c| c.trim().trim_start_matches('<').trim_end_matches('>').to_string())
            .filter(|c| !c.is_empty());
        let inline = content_id.is_some()
            && part
                .header("Content-Disposition")
                .is_none_or(|d| !d.trim_start().to_ascii_lowercase().starts_with("attachment"))
            && mime.starts_with("image/");
        out.attachments.push(IncomingAttachment {
            provider_id: part.body.attachment_id.clone(),
            filename: if part.filename.is_empty() { "attachment".into() } else { part.filename.clone() },
            mime: if mime.is_empty() { "application/octet-stream".into() } else { mime },
            size: i64::try_from(part.body.size).unwrap_or(i64::MAX),
            content_id,
            inline,
        });
        return;
    }
    let Some(data) = &part.body.data else { return };
    let bytes = decode_base64url(data);
    let charset = charset_of(part.header("Content-Type"));
    match mime.as_str() {
        "text/plain" => out.text.push(decode_charset(&bytes, charset.as_deref())),
        "text/html" => out.html.push(decode_charset(&bytes, charset.as_deref())),
        _ => {}
    }
}

pub fn to_incoming(msg: &GmailMessage) -> Result<IncomingMessage> {
    let payload =
        msg.payload.as_ref().ok_or_else(|| Error::Unexpected(format!("message {} has no payload", msg.id)))?;
    let pairs: Vec<(&str, &str)> = payload.headers.iter().map(|h| (h.name.as_str(), h.value.as_str())).collect();
    let parsed = parse_headers(&pairs);
    let header = |name: &str| payload.header(name).map(str::to_string);

    let from = parsed.from.into_iter().next().unwrap_or_else(|| Address::new("unknown@invalid", None));

    let mut walked = Walked::default();
    walk(payload, &mut walked);

    let sent_at = msg
        .internal_date
        .as_deref()
        .and_then(|d| d.parse::<i64>().ok())
        .ok_or_else(|| Error::Unexpected(format!("message {} has no date", msg.id)))?;
    let has_body = !walked.text.is_empty() || !walked.html.is_empty();
    let join = |parts: Vec<String>| (!parts.is_empty()).then(|| parts.join("\n\n"));

    Ok(IncomingMessage {
        provider_id: msg.id.clone(),
        thread_id: if msg.thread_id.is_empty() { msg.id.clone() } else { msg.thread_id.clone() },
        rfc_message_id: parsed.message_id,
        in_reply_to: parsed.in_reply_to,
        from,
        to: parsed.to,
        cc: parsed.cc,
        subject: parsed.subject.unwrap_or_default(),
        sent_at,
        snippet: (!msg.snippet.is_empty()).then(|| html_unescape(&msg.snippet)),
        text: join(walked.text),
        html: join(walked.html),
        attachments: walked.attachments,
        unread: msg.label_ids.iter().any(|l| l == "UNREAD"),
        starred: msg.label_ids.iter().any(|l| l == "STARRED"),
        labels: msg.label_ids.clone(),
        headers: HeaderFacts {
            list_id: header("List-Id"),
            list_unsubscribe: header("List-Unsubscribe"),
            list_unsubscribe_post: header("List-Unsubscribe-Post"),
            precedence: header("Precedence"),
            auto_submitted: header("Auto-Submitted"),
        },
        size_bytes: (has_body || msg.size_estimate > 0).then(|| i64::try_from(msg.size_estimate).unwrap_or(i64::MAX)),
    })
}

fn html_unescape(snippet: &str) -> String {
    snippet
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Body, Header};

    fn b64(text: &str) -> String {
        URL_SAFE_NO_PAD.encode(text.as_bytes())
    }

    fn header(name: &str, value: &str) -> Header {
        Header { name: name.into(), value: value.into() }
    }

    fn text_part(mime: &str, body: &str) -> Part {
        Part {
            mime_type: mime.into(),
            headers: vec![header("Content-Type", &format!("{mime}; charset=\"UTF-8\""))],
            body: Body { data: Some(b64(body)), size: body.len() as u64, attachment_id: None },
            ..Default::default()
        }
    }

    fn message(headers: Vec<Header>, parts: Vec<Part>, labels: &[&str]) -> GmailMessage {
        GmailMessage {
            id: "18f".into(),
            thread_id: "t9".into(),
            label_ids: labels.iter().map(|s| (*s).to_string()).collect(),
            snippet: "It&#39;s a snippet &amp; more".into(),
            history_id: Some("100".into()),
            internal_date: Some("1795082400000".into()),
            size_estimate: 2048,
            payload: Some(Part { mime_type: "multipart/alternative".into(), headers, parts, ..Default::default() }),
        }
    }

    #[test]
    fn addresses_and_encoded_words_are_decoded() {
        let parsed = parse_headers(&[
            ("From", "=?UTF-8?B?QW3DqWxpZSBEdWJvaXM=?= <amelie@lepetitzinc.example>"),
            ("To", "\"Okafor, Maya\" <maya@northwind.example>, dan@northwind.example, undisclosed-recipients:;"),
            ("Cc", "Team <team@northwind.example>"),
            ("Subject", "=?UTF-8?Q?R=C3=A9servation_pour_samedi?="),
            ("Message-ID", "<abc.123@mail.example>"),
            ("In-Reply-To", "<prev.1@mail.example>"),
        ]);
        assert_eq!(parsed.from, [Address::new("amelie@lepetitzinc.example", Some("Amélie Dubois"))]);
        assert_eq!(parsed.to.len(), 2);
        assert_eq!(parsed.to[0], Address::new("maya@northwind.example", Some("Okafor, Maya")));
        assert_eq!(parsed.to[1].email, "dan@northwind.example");
        assert_eq!(parsed.cc[0].email, "team@northwind.example");
        assert_eq!(parsed.subject.as_deref(), Some("Réservation pour samedi"));
        assert_eq!(parsed.message_id.as_deref(), Some("<abc.123@mail.example>"));
        assert_eq!(parsed.in_reply_to.as_deref(), Some("<prev.1@mail.example>"));
    }

    #[test]
    fn a_header_value_cannot_smuggle_in_another_header() {
        let parsed = parse_headers(&[("Subject", "Hello\r\nBcc: attacker@evil.example"), ("From", "a@b.example")]);
        let subject = parsed.subject.unwrap_or_default();
        assert_eq!(
            subject.split_whitespace().collect::<Vec<_>>(),
            ["Hello", "Bcc:", "attacker@evil.example"],
            "it stays text"
        );
        assert!(parsed.cc.is_empty(), "and never becomes a header");
    }

    #[test]
    fn a_multipart_message_becomes_text_html_and_attachments() {
        let mut pdf = Part {
            mime_type: "application/pdf".into(),
            filename: "Invoice 2291.pdf".into(),
            headers: vec![header("Content-Disposition", "attachment; filename=\"Invoice 2291.pdf\"")],
            body: Body { attachment_id: Some("ANGjdJ".into()), size: 96_000, data: None },
            ..Default::default()
        };
        pdf.part_id = "2".into();
        let logo = Part {
            mime_type: "image/png".into(),
            headers: vec![header("Content-ID", "<logo@mail>"), header("Content-Disposition", "inline")],
            body: Body { attachment_id: Some("ANGlogo".into()), size: 900, data: None },
            ..Default::default()
        };
        let msg = message(
            vec![
                header("From", "Maya Okafor <maya@northwind.example>"),
                header("To", "me@example.com"),
                header("Subject", "Invoice"),
                header("List-Id", "<news.example>"),
                header("List-Unsubscribe", "<https://x.example/u>"),
                header("Auto-Submitted", "auto-generated"),
            ],
            vec![text_part("text/plain", "Plain words"), text_part("text/html", "<p>Rich words</p>"), pdf, logo],
            &["INBOX", "UNREAD", "STARRED", "CATEGORY_UPDATES"],
        );
        let incoming = to_incoming(&msg).unwrap();
        assert_eq!(incoming.provider_id, "18f");
        assert_eq!(incoming.thread_id, "t9");
        assert_eq!(incoming.from.email, "maya@northwind.example");
        assert_eq!(incoming.text.as_deref(), Some("Plain words"));
        assert_eq!(incoming.html.as_deref(), Some("<p>Rich words</p>"));
        assert_eq!(incoming.sent_at, 1_795_082_400_000);
        assert!(incoming.unread && incoming.starred);
        assert_eq!(incoming.snippet.as_deref(), Some("It's a snippet & more"));
        assert_eq!(incoming.headers.list_id.as_deref(), Some("<news.example>"));
        assert_eq!(incoming.headers.auto_submitted.as_deref(), Some("auto-generated"));
        assert_eq!(incoming.attachments.len(), 2);
        assert_eq!(incoming.attachments[0].filename, "Invoice 2291.pdf");
        assert_eq!(incoming.attachments[0].provider_id.as_deref(), Some("ANGjdJ"));
        assert!(!incoming.attachments[0].inline);
        assert!(incoming.attachments[1].inline, "an image with a Content-ID is part of the body");
        assert_eq!(incoming.attachments[1].content_id.as_deref(), Some("logo@mail"));
    }

    #[test]
    fn text_in_other_character_sets_is_decoded() {
        let latin1 = Part {
            mime_type: "text/plain".into(),
            headers: vec![header("Content-Type", "text/plain; charset=ISO-8859-1")],
            body: Body { data: Some(URL_SAFE_NO_PAD.encode([0x43u8, 0x61, 0x66, 0xE9])), size: 4, attachment_id: None },
            ..Default::default()
        };
        let shift_jis = Part {
            mime_type: "text/plain".into(),
            headers: vec![header("Content-Type", "text/plain; charset=shift_jis")],
            body: Body { data: Some(URL_SAFE_NO_PAD.encode([0x93u8, 0xfa, 0x96, 0x7b])), size: 4, attachment_id: None },
            ..Default::default()
        };
        let a = to_incoming(&message(vec![header("From", "a@b.example")], vec![latin1], &[])).unwrap();
        let b = to_incoming(&message(vec![header("From", "a@b.example")], vec![shift_jis], &[])).unwrap();
        assert_eq!(a.text.as_deref(), Some("Café"));
        assert_eq!(b.text.as_deref(), Some("日本"));
        assert_eq!(decode_charset(b"abc", Some("no-such-charset")), "abc");
        assert_eq!(decode_charset(&[0xff, 0x61], None), "\u{fffd}a", "bad bytes are replaced, never a failure");
    }

    #[test]
    fn a_single_part_message_and_padded_base64_work() {
        let payload = Part {
            mime_type: "text/plain".into(),
            headers: vec![header("From", "Dad <pete@mail.example>"), header("Subject", "dinner sat?")],
            body: Body { data: Some(format!("{}==", b64("Are you coming?"))), size: 15, attachment_id: None },
            ..Default::default()
        };
        let msg = GmailMessage {
            id: "1".into(),
            thread_id: String::new(),
            label_ids: vec!["INBOX".into()],
            snippet: String::new(),
            history_id: None,
            internal_date: Some("1000".into()),
            size_estimate: 0,
            payload: Some(payload),
        };
        let incoming = to_incoming(&msg).unwrap();
        assert_eq!(incoming.text.as_deref(), Some("Are you coming?"));
        assert_eq!(incoming.thread_id, "1", "a message without a thread id is its own thread");
        assert!(!incoming.unread);
    }

    #[test]
    fn a_message_without_a_date_or_payload_is_reported_not_guessed() {
        let mut msg = message(vec![header("From", "a@b.example")], vec![], &[]);
        msg.internal_date = None;
        assert!(to_incoming(&msg).is_err());
        msg.payload = None;
        assert!(to_incoming(&msg).is_err());
    }

    #[test]
    fn charset_is_read_from_the_content_type() {
        assert_eq!(
            charset_of(Some("text/html; charset=\"windows-1252\"; format=flowed")).as_deref(),
            Some("windows-1252")
        );
        assert_eq!(charset_of(Some("text/plain")), None);
    }
}

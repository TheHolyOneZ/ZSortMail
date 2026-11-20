use mail_builder::MessageBuilder;
use mail_builder::headers::address::Address as MbAddress;
use zsm_core::store::ingest::Address;
use zsm_core::store::outgoing::OutgoingMessage;

use crate::error::{Error, Result};

fn addresses(list: &[Address]) -> MbAddress<'_> {
    MbAddress::new_list(
        list.iter()
            .map(|a| MbAddress::new_address(a.name.as_deref().filter(|n| !n.is_empty()), a.email.as_str()))
            .collect(),
    )
}

fn bare(id: &str) -> &str {
    id.trim().trim_start_matches('<').trim_end_matches('>')
}

pub fn build_raw(message: &OutgoingMessage) -> Result<Vec<u8>> {
    let mut builder = MessageBuilder::new()
        .from(MbAddress::new_address(
            message.from.name.as_deref().filter(|n| !n.is_empty()),
            message.from.email.as_str(),
        ))
        .subject(message.subject.as_str())
        .text_body(message.text.as_str());
    if !message.to.is_empty() {
        builder = builder.to(addresses(&message.to));
    }
    if !message.cc.is_empty() {
        builder = builder.cc(addresses(&message.cc));
    }
    if !message.bcc.is_empty() {
        builder = builder.bcc(addresses(&message.bcc));
    }
    if let Some(html) = message.html.as_deref() {
        builder = builder.html_body(html);
    }
    if let Some(id) = &message.rfc_message_id {
        builder = builder.message_id(bare(id));
    }
    if let Some(id) = &message.in_reply_to {
        builder = builder.in_reply_to(bare(id));
    }
    if !message.references.is_empty() {
        let refs: Vec<&str> = message.references.iter().map(|r| bare(r)).collect();
        builder = builder.references(refs);
    }
    for file in &message.files {
        let data = std::fs::read(&file.path)
            .map_err(|e| Error::Unexpected(format!("The file “{}” could not be read: {e}", file.filename)))?;
        builder = builder.attachment(file.mime.as_str(), file.filename.as_str(), data);
    }
    Ok(builder.write_to_vec()?)
}

#[cfg(test)]
mod tests {
    use mail_parser::MessageParser;
    use zsm_core::store::outgoing::OutgoingFile;

    use super::*;

    fn sample() -> OutgoingMessage {
        OutgoingMessage {
            message_id: 7,
            account_id: 1,
            from: Address::new("me@example.com", Some("Zo Ryx")),
            to: vec![
                Address::new("maya@northwind.example", Some("Maya Okafor")),
                Address::new("dan@northwind.example", None),
            ],
            cc: vec![Address::new("team@northwind.example", None)],
            bcc: vec![],
            subject: "Re: Réservation ☕".into(),
            text: "See you at 9:30.\nBring the good bread.".into(),
            html: None,
            files: vec![],
            rfc_message_id: Some("<abc123@zsortmail.local>".into()),
            in_reply_to: Some("<prev1@mail.example>".into()),
            references: vec!["<first@mail.example>".into(), "<prev1@mail.example>".into()],
            thread_id: "t1".into(),
        }
    }

    #[test]
    fn the_built_email_reads_back_the_way_it_was_written() {
        let raw = build_raw(&sample()).unwrap();
        let parsed = MessageParser::new().parse(&raw).expect("valid message");
        assert_eq!(parsed.subject(), Some("Re: Réservation ☕"));
        let from = parsed.from().unwrap().first().unwrap();
        assert_eq!((from.name(), from.address()), (Some("Zo Ryx"), Some("me@example.com")));
        let to: Vec<_> = parsed.to().unwrap().mailboxes().map(|m| m.address().unwrap().to_string()).collect();
        assert_eq!(to, ["maya@northwind.example", "dan@northwind.example"]);
        assert_eq!(parsed.cc().unwrap().first().unwrap().address(), Some("team@northwind.example"));
        assert_eq!(parsed.message_id(), Some("abc123@zsortmail.local"));
        assert_eq!(parsed.in_reply_to().unwrap().first(), Some("prev1@mail.example"));
        assert_eq!(parsed.references().unwrap().len(), 2);
        let body = parsed.body_text(0).unwrap().replace("\r\n", "\n");
        assert_eq!(body.trim_end(), "See you at 9:30.\nBring the good bread.");
    }

    #[test]
    fn a_formatted_message_with_a_file_and_a_hidden_recipient_is_built_whole() {
        let dir = std::env::temp_dir().join(format!("zsm-outgoing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plan.bin");
        std::fs::write(&path, [0u8, 1, 2, 255]).unwrap();
        let message = OutgoingMessage {
            bcc: vec![Address::new("boss@example.com", None)],
            html: Some("<p>See the <b>plan</b>.</p>".into()),
            files: vec![OutgoingFile {
                filename: "plan.bin".into(),
                mime: "application/octet-stream".into(),
                path: path.to_string_lossy().into_owned(),
            }],
            ..sample()
        };
        let raw = build_raw(&message).unwrap();
        let parsed = MessageParser::new().parse(&raw).expect("valid message");
        assert!(parsed.body_html(0).unwrap().contains("<b>plan</b>"));
        assert!(parsed.body_text(0).unwrap().contains("See you at 9:30."), "the plain-text twin is there");
        assert_eq!(parsed.attachments().count(), 1);
        let file = parsed.attachments().next().unwrap();
        assert_eq!(file.attachment_name(), Some("plan.bin"));
        assert!(
            String::from_utf8_lossy(file.raw_body()).contains("AAEC/w=="),
            "the file's bytes travel (base64 of 00 01 02 ff)"
        );
        assert_eq!(parsed.bcc().unwrap().first().unwrap().address(), Some("boss@example.com"));

        let gone = OutgoingMessage {
            files: vec![OutgoingFile {
                filename: "lost.pdf".into(),
                mime: "application/pdf".into(),
                path: dir.join("lost.pdf").to_string_lossy().into_owned(),
            }],
            ..sample()
        };
        assert!(build_raw(&gone).unwrap_err().to_string().contains("lost.pdf"));

        let sneaky = OutgoingMessage {
            files: vec![OutgoingFile {
                filename: "a.txt\r\nBcc: attacker@evil.example".into(),
                mime: "text/plain".into(),
                path: path.to_string_lossy().into_owned(),
            }],
            ..sample()
        };
        let raw = String::from_utf8_lossy(&build_raw(&sneaky).unwrap()).into_owned();
        assert!(!raw.lines().any(|l| l.to_ascii_lowercase().starts_with("bcc:")), "{raw}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_subject_cannot_inject_headers() {
        let mut message = sample();
        message.subject = "Hi\r\nBcc: attacker@evil.example".into();
        let raw = String::from_utf8(build_raw(&message).unwrap()).unwrap();
        let headers = raw.split("\r\n\r\n").next().unwrap();
        assert!(!headers.lines().any(|l| l.to_ascii_lowercase().starts_with("bcc:")), "{headers}");
    }

    #[test]
    fn a_message_with_no_recipients_or_threading_is_still_valid() {
        let message = OutgoingMessage {
            to: vec![],
            cc: vec![],
            rfc_message_id: None,
            in_reply_to: None,
            references: vec![],
            ..sample()
        };
        let raw = build_raw(&message).unwrap();
        assert!(MessageParser::new().parse(&raw).is_some());
    }
}

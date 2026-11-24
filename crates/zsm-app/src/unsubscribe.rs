use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use zsm_core::store::ingest::Address;

use crate::core::Core;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Unsubscribed {
    Done { sender: String, person_id: Option<i64> },
    Open { url: String, sender: String },
    Draft { draft_id: i64, sender: String },
}

fn invalid(text: &str) -> Error {
    Error::Store(zsm_core::Error::Invalid(text.into()))
}

pub(crate) fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || o[0] == 0
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            !(v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local() || v6.is_unicast_link_local())
        }
    }
}

pub(crate) fn check_url(raw: &str, allow_private: bool) -> Result<url::Url> {
    let url = url::Url::parse(raw.trim()).map_err(|_| invalid("The sender's unsubscribe address is not valid"))?;
    let scheme_ok = url.scheme() == "https" || (allow_private && url.scheme() == "http");
    if !scheme_ok {
        return Err(invalid("The sender's unsubscribe address is not a secure web address"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("The sender's unsubscribe address carries a password, so it was not used"));
    }
    let Some(host) = url.host_str() else {
        return Err(invalid("The sender's unsubscribe address has no website in it"));
    };
    let lower = host.to_ascii_lowercase();
    if !allow_private
        && (lower == "localhost"
            || lower.ends_with(".localhost")
            || lower.ends_with(".local")
            || lower.ends_with(".internal"))
    {
        return Err(invalid("The sender's unsubscribe address points at this computer, so it was not used"));
    }
    Ok(url)
}

pub(crate) async fn post_one_click(raw: &str, allow_private: bool) -> Result<()> {
    let url = check_url(raw, allow_private)?;
    let host = url.host_str().unwrap_or_default().to_string();
    let port = url.port_or_known_default().unwrap_or(443);

    let mut answers: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| invalid(&format!("{host} could not be found")))?
        .collect();
    if !allow_private && answers.iter().any(|a| !is_public(a.ip())) {
        return Err(invalid("The sender's unsubscribe address points at a private network, so it was not used"));
    }
    answers.truncate(4);
    if answers.is_empty() {
        return Err(invalid(&format!("{host} could not be found")));
    }

    let client = reqwest::Client::builder()
        .user_agent(concat!("ZSortMail/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(&host, &answers)
        .build()
        .map_err(|_| invalid("Unsubscribing is not possible on this computer"))?;
    let response = client
        .post(url)
        .form(&[("List-Unsubscribe", "One-Click")])
        .send()
        .await
        .map_err(|_| invalid(&format!("{host} did not answer. Nothing was changed. You can try again later")))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(invalid(&format!("{host} answered {}. Nothing was changed", response.status().as_u16())))
    }
}

pub(crate) fn parse_mailto(raw: &str) -> Option<(Address, String, String)> {
    let url = url::Url::parse(raw.trim()).ok()?;
    if url.scheme() != "mailto" {
        return None;
    }
    let to = url.path().split(',').next()?.trim().to_string();
    if !to.contains('@') || to.chars().any(char::is_whitespace) {
        return None;
    }
    let mut subject = String::new();
    let mut body = String::new();
    for (key, value) in url.query_pairs() {
        match key.to_ascii_lowercase().as_str() {
            "subject" => subject = value.replace(['\r', '\n'], " "),
            "body" => body = value.into_owned(),
            _ => {}
        }
    }
    if subject.trim().is_empty() {
        subject = "Unsubscribe".into();
    }
    if body.trim().is_empty() {
        body = "Please unsubscribe me from this list.".into();
    }
    Some((Address::new(&to, None), subject, body))
}

impl Core {
    pub async fn unsubscribe(&self, message_id: i64) -> Result<Unsubscribed> {
        let target = self
            .with_store(|s| Ok(s.unsubscribe_target(message_id)?))?
            .ok_or_else(|| invalid("This message has no way to unsubscribe"))?;
        let sender = target.from_name.clone();
        let info = &target.info;

        if let (true, Some(url)) = (info.one_click, info.https.as_deref()) {
            if target.provider != "demo" {
                post_one_click(url, self.unsubscribe_allow_private).await?;
            }
            let now = self.now_ms();
            self.with_store(|s| Ok(s.mark_unsubscribed(&target.from_address, now)?))?;
            self.changed("conversations", Vec::new());
            return Ok(Unsubscribed::Done { sender, person_id: target.person_id });
        }
        if let Some(url) = info.https.as_deref() {
            check_url(url, self.unsubscribe_allow_private)?;
            return Ok(Unsubscribed::Open { url: url.to_string(), sender });
        }
        if let Some((to, subject, body)) = info.mailto.as_deref().and_then(parse_mailto) {
            let draft = self.new_draft(vec![to])?;
            let html = format!("<p>{}</p>", html_escape::encode_text(&body).replace('\n', "<br>"));
            self.save_draft(&zsm_core::store::drafts::DraftInput {
                id: draft.id,
                to: draft.to.clone(),
                cc: Vec::new(),
                bcc: Vec::new(),
                subject,
                body_html: html,
                nudge_days: None,
            })?;
            return Ok(Unsubscribed::Draft { draft_id: draft.id, sender });
        }
        Err(invalid("This message has no way to unsubscribe"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_open_internet_counts_as_public() {
        for ok in ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(is_public(ok.parse().unwrap()), "{ok}");
        }
        for bad in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "255.255.255.255",
            "::1",
            "::",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
        ] {
            assert!(!is_public(bad.parse().unwrap()), "{bad}");
        }
    }

    #[test]
    fn only_secure_web_addresses_that_are_not_ours_are_used() {
        assert!(check_url("https://list.example/u?id=1", false).is_ok());
        for bad in [
            "http://list.example/u",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ftp://list.example",
            "https://user:pass@list.example/u",
            "https://localhost/u",
            "https://printer.local/u",
            "https://db.internal/u",
            "https://app.localhost/u",
            "not a url",
            "",
        ] {
            assert!(check_url(bad, false).is_err(), "{bad}");
        }
        assert!(check_url("http://127.0.0.1:9/u", true).is_ok(), "tests may talk to this computer");
    }

    #[test]
    fn a_mailto_address_becomes_a_recipient_a_subject_and_some_text() {
        let (to, subject, body) =
            parse_mailto("mailto:unsubscribe@list.example?subject=Remove%20me&body=Hi%0Athere").unwrap();
        assert_eq!(
            (to.email.as_str(), subject.as_str(), body.as_str()),
            ("unsubscribe@list.example", "Remove me", "Hi\nthere")
        );
        let (_, subject, body) = parse_mailto("mailto:u@list.example").unwrap();
        assert_eq!((subject.as_str(), body.as_str()), ("Unsubscribe", "Please unsubscribe me from this list."));
        let (to, subject, _) =
            parse_mailto("mailto:a@x.example,b@x.example?subject=Hi%0D%0ABcc:evil@x.example").unwrap();
        assert_eq!(to.email, "a@x.example", "the first recipient only");
        assert!(!subject.contains('\n') && !subject.contains('\r'), "a subject cannot carry a header: {subject:?}");
        for bad in ["https://x.example", "mailto:", "mailto:not an address", ""] {
            assert!(parse_mailto(bad).is_none(), "{bad}");
        }
    }

    async fn one_shot_server(status: &'static str) -> (u16, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut seen = Vec::new();
            let mut chunk = [0u8; 2048];
            loop {
                let n = socket.read(&mut chunk).await.unwrap();
                seen.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&seen).into_owned();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let wanted = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if body.len() >= wanted {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            socket
                .write_all(format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").as_bytes())
                .await
                .unwrap();
            String::from_utf8_lossy(&seen).into_owned()
        });
        (port, task)
    }

    #[tokio::test]
    async fn one_click_sends_exactly_the_post_the_standard_asks_for() {
        let (port, server) = one_shot_server("200 OK").await;
        post_one_click(&format!("http://127.0.0.1:{port}/u?id=77&t=abc"), true).await.unwrap();
        let request = server.await.unwrap();
        assert!(request.starts_with("POST /u?id=77&t=abc HTTP/1.1"), "{request}");
        assert!(request.to_ascii_lowercase().contains("content-type: application/x-www-form-urlencoded"), "{request}");
        assert!(request.ends_with("List-Unsubscribe=One-Click"), "{request}");
        assert!(
            !request.to_ascii_lowercase().contains("cookie:")
                && !request.to_ascii_lowercase().contains("authorization:"),
            "{request}"
        );
    }

    #[tokio::test]
    async fn a_refusal_a_redirect_and_a_private_address_are_all_reported_and_change_nothing() {
        let (port, server) = one_shot_server("500 Internal Server Error").await;
        let error = post_one_click(&format!("http://127.0.0.1:{port}/u"), true).await.unwrap_err().to_string();
        assert!(error.contains("answered 500") && error.contains("Nothing was changed"), "{error}");
        server.await.unwrap();

        let (port, server) = one_shot_server("302 Found").await;
        let error = post_one_click(&format!("http://127.0.0.1:{port}/u"), true).await.unwrap_err().to_string();
        assert!(error.contains("answered 302"), "{error}");
        server.await.unwrap();

        let error = post_one_click("https://127.0.0.1/u", false).await.unwrap_err().to_string();
        assert!(error.contains("private network") || error.contains("this computer"), "{error}");
        let error = post_one_click("https://localhost/u", false).await.unwrap_err().to_string();
        assert!(error.contains("this computer"), "{error}");
    }

    fn core_with(list_unsubscribe: &str, post: Option<&str>) -> (std::sync::Arc<Core>, i64) {
        use zsm_core::store::ingest::{HeaderFacts, IncomingMessage, IngestContext};
        let core =
            Core::new(crate::CoreConfig { unsubscribe_allow_private: true, ..crate::CoreConfig::in_memory() }).unwrap();
        core.start_demo(0).unwrap();
        let message = IncomingMessage {
            provider_id: "list-1".into(),
            thread_id: "thread-list-1".into(),
            rfc_message_id: None,
            in_reply_to: None,
            from: Address::new("news@list.example", Some("The List")),
            to: vec![Address::new("you@zsortmail.demo", None)],
            cc: vec![],
            bcc: Vec::new(),
            subject: "Issue 1".into(),
            sent_at: core.now_ms(),
            snippet: None,
            text: Some("Hello".into()),
            html: None,
            attachments: vec![],
            labels: vec!["INBOX".into()],
            unread: true,
            starred: false,
            headers: HeaderFacts {
                list_id: Some("<list.example>".into()),
                list_unsubscribe: Some(list_unsubscribe.into()),
                list_unsubscribe_post: post.map(str::to_string),
                ..HeaderFacts::default()
            },
            size_bytes: None,
        };
        let id = core
            .with_store(|s| {
                let ctx = IngestContext {
                    account_id: 1,
                    self_addresses: vec!["you@zsortmail.demo".into()],
                    screener: false,
                    automation: false,
                };
                Ok(s.ingest(&ctx, &message, core.now_ms()).map_err(Error::from)?.message_id)
            })
            .unwrap();
        (core, id)
    }

    #[tokio::test]
    async fn the_demo_mailbox_agrees_at_once_and_remembers_it() {
        let (core, message) = core_with("<https://list.example/u?id=1>", Some("List-Unsubscribe=One-Click"));
        let before = core.with_store(|s| Ok(s.unsubscribe_target(message)?)).unwrap().unwrap();
        assert!(before.info.one_click && before.info.done_at.is_none());
        let outcome = core.unsubscribe(message).await.unwrap();
        assert!(matches!(outcome, Unsubscribed::Done { ref sender, .. } if sender == "The List"), "{outcome:?}");
        let after = core.with_store(|s| Ok(s.unsubscribe_target(message)?)).unwrap().unwrap();
        assert_eq!(after.info.done_at, Some(core.now_ms()));
    }

    #[tokio::test]
    async fn a_page_is_opened_when_one_click_is_not_offered_and_an_email_is_drafted_when_only_mail_is() {
        let (core, message) = core_with("<https://list.example/leave?id=1>", None);
        let outcome = core.unsubscribe(message).await.unwrap();
        assert_eq!(
            outcome,
            Unsubscribed::Open { url: "https://list.example/leave?id=1".into(), sender: "The List".into() }
        );

        let (core, message) = core_with("<mailto:leave@list.example?subject=Remove%20me>", None);
        let Unsubscribed::Draft { draft_id, .. } = core.unsubscribe(message).await.unwrap() else { panic!("a draft") };
        let draft = core.draft(draft_id).unwrap().unwrap();
        assert_eq!((draft.to[0].email.as_str(), draft.subject.as_str()), ("leave@list.example", "Remove me"));
        assert!(draft.body_html.contains("Please unsubscribe me from this list."));

        let (core, message) = core_with("<http://list.example/leave>", None);
        assert_eq!(core.unsubscribe(message).await.unwrap_err().to_string(), "This message has no way to unsubscribe");
        let strict = Core::new(crate::CoreConfig::in_memory()).unwrap();
        strict.start_demo(0).unwrap();
        let nothing =
            strict.with_store(|s| Ok(s.list_people(zsm_core::dto::PeopleFilter::All, "Maya")?[0].id)).unwrap();
        let convo = strict.conversations(nothing).unwrap().remove(0);
        let first = strict.conversation(convo.id, None).unwrap().unwrap().messages[0].id;
        let error = strict.unsubscribe(first).await.unwrap_err().to_string();
        assert_eq!(error, "This message has no way to unsubscribe");
    }
}

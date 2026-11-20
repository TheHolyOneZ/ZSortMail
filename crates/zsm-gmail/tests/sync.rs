use std::sync::{Arc, Mutex};
use std::time::Duration;

use zsm_core::clock::{Clock, FixedClock};
use zsm_core::dto::{PeopleFilter, PersonStatus};
use zsm_core::store::ingest::BatchOutcome;
use zsm_core::store::{SharedStore, Store};
use zsm_gmail::client::{Credentials, GmailClient, RetryPolicy, build_http};
use zsm_gmail::sync::{Level, Phase, SyncEngine, SyncEvents, SyncOptions};
use zsm_testkit::{AttachmentSpec, Failure, FakeGoogle, MessageSpec};

const NOW: i64 = 1_795_082_400_000;
const DAY: i64 = 86_400_000;

#[derive(Default)]
struct Recorder {
    phases: Mutex<Vec<(Phase, u32, u32)>>,
    notices: Mutex<Vec<(Level, String)>>,
    arrived: Mutex<Vec<usize>>,
    changed: Mutex<Vec<String>>,
}

impl SyncEvents for Recorder {
    fn progress(&self, phase: Phase, done: u32, total: u32) {
        self.phases.lock().unwrap().push((phase, done, total));
    }
    fn changed(&self, entity: &str, _ids: Vec<i64>) {
        self.changed.lock().unwrap().push(entity.to_string());
    }
    fn arrived(&self, _account_id: i64, batch: &BatchOutcome) {
        self.arrived.lock().unwrap().push(batch.messages.iter().filter(|m| m.inserted).count());
    }
    fn notice(&self, level: Level, text: &str) {
        self.notices.lock().unwrap().push((level, text.to_string()));
    }
}

struct Rig {
    fake: FakeGoogle,
    store: SharedStore,
    account: i64,
    engine: SyncEngine,
    events: Arc<Recorder>,
    clock: Arc<FixedClock>,
}

fn spec(from: &str, subject: &str, text: &str, days_ago: i64) -> MessageSpec {
    MessageSpec {
        from: from.into(),
        to: vec!["me@example.com".into()],
        subject: subject.into(),
        text: Some(text.into()),
        date_ms: NOW - days_ago * DAY,
        ..Default::default()
    }
}

async fn rig_with(options: SyncOptions) -> Rig {
    let fake = FakeGoogle::start("me@example.com").await;
    fake.set_now(NOW);
    let mut store = Store::open_in_memory().unwrap();
    let account = store.upsert_account("gmail", "me@example.com", Some("Me"), NOW).unwrap();
    let _ = &mut store;
    let store = SharedStore::new(store);
    let client = GmailClient::new(
        build_http(),
        fake.gmail_base(),
        Credentials::Fixed(fake.access_token()),
        RetryPolicy { max_attempts: 4, base: Duration::from_millis(2), max: Duration::from_millis(20) },
    );
    let events = Arc::new(Recorder::default());
    let clock = Arc::new(FixedClock::new(NOW));
    let engine = SyncEngine {
        store: store.clone(),
        client,
        account_id: account,
        events: events.clone(),
        clock: clock.clone(),
        options,
    };
    Rig { fake, store, account, engine, events, clock }
}

async fn rig() -> Rig {
    rig_with(SyncOptions::default()).await
}

fn people(rig: &Rig, query: &str) -> Vec<zsm_core::dto::PersonSummary> {
    rig.store.with(|s| s.list_people(PeopleFilter::All, query)).unwrap()
}

#[tokio::test]
async fn the_first_read_builds_people_and_conversations_and_remembers_the_position() {
    let rig = rig().await;
    rig.fake.add_message(spec(
        "Maya Okafor <maya@northwind.example>",
        "Q3 launch checklist",
        "Hi, attaching the checklist.",
        3,
    ));
    let thread =
        rig.fake.add_message(spec("Dan Whitaker <dan@northwind.example>", "Screenshots", "They are in the folder.", 2));
    rig.fake.add_message(MessageSpec {
        thread: Some("t1".into()),
        ..spec("Maya Okafor <maya@northwind.example>", "Q3 launch checklist", "Second message", 1)
    });
    let _ = thread;

    let report = rig.engine.run().await.unwrap();
    assert_eq!((report.fetched, report.ingested), (3, 3));

    let maya = people(&rig, "Okafor");
    assert_eq!(maya.len(), 1);
    assert_eq!(maya[0].unread, 2, "both messages arrived unread");
    let convos = rig.store.with(|s| s.list_conversations(maya[0].id)).unwrap();
    assert_eq!(convos.len(), 1, "two messages in one thread are one conversation");
    assert_eq!(convos[0].message_count, 2);

    let state = rig.store.with(|s| s.sync_state(rig.account)).unwrap();
    assert_eq!(state.history_id.as_deref(), Some(rig.fake.history_id().as_str()));
    assert!(state.backfill_done && state.last_sync_at == Some(NOW));

    let phases = rig.events.phases.lock().unwrap().clone();
    assert_eq!(phases.first().map(|p| p.0), Some(Phase::Reading));
    assert!(
        phases.iter().any(|p| p.0 == Phase::Reading && p.1 == 3 && p.2 == 3),
        "progress reached 3 of 3: {phases:?}"
    );
    assert_eq!(phases.last().map(|p| p.0), Some(Phase::Idle));
    assert!(rig.events.arrived.lock().unwrap().is_empty(), "the first read is not new mail, so nothing is announced");
}

#[tokio::test]
async fn bodies_attachments_and_other_character_sets_survive_the_trip() {
    let rig = rig().await;
    rig.fake.add_message(MessageSpec {
        from: "=?UTF-8?B?QW3DqWxpZSBEdWJvaXM=?= <amelie@lepetitzinc.example>".into(),
        subject: "=?UTF-8?Q?R=C3=A9servation?=".into(),
        html: Some("<p>Bonjour <b>Zo</b></p>".into()),
        text: Some("Bonjour Zo".into()),
        attachments: vec![
            AttachmentSpec {
                filename: "menu.pdf".into(),
                mime: "application/pdf".into(),
                bytes: b"%PDF-1.4 fake".to_vec(),
                content_id: None,
            },
            AttachmentSpec {
                filename: "logo.png".into(),
                mime: "image/png".into(),
                bytes: vec![1, 2, 3],
                content_id: Some("logo@zinc".into()),
            },
        ],
        date_ms: NOW - DAY,
        to: vec!["me@example.com".into()],
        ..Default::default()
    });
    rig.fake.add_message(MessageSpec {
        text_bytes: Some(("iso-8859-1".into(), vec![0x43, 0x61, 0x66, 0xE9])),
        text: None,
        ..spec("Jean <jean@mail.example>", "Café", "", 2)
    });
    rig.engine.run().await.unwrap();

    let amelie = people(&rig, "Amélie");
    assert_eq!(amelie.len(), 1, "the encoded display name was decoded");
    let convo = rig.store.with(|s| s.list_conversations(amelie[0].id)).unwrap().remove(0);
    assert_eq!(convo.subject, "Réservation");
    let detail = rig.store.with(|s| s.get_conversation(convo.id, Some(amelie[0].id))).unwrap().unwrap();
    let message = &detail.messages[0];
    assert_eq!(message.text, "Bonjour Zo");
    assert!(message.html_url.is_some());
    assert_eq!(message.attachments.len(), 2);
    assert_eq!(message.attachments.iter().filter(|a| a.inline).count(), 1, "the logo is part of the body");
    let files = rig.store.with(|s| s.list_files(amelie[0].id)).unwrap();
    assert_eq!(files.len(), 1, "Files lists only the real attachment");

    let jean = people(&rig, "Jean");
    let convo = rig.store.with(|s| s.list_conversations(jean[0].id)).unwrap().remove(0);
    let detail = rig.store.with(|s| s.get_conversation(convo.id, Some(jean[0].id))).unwrap().unwrap();
    assert_eq!(detail.messages[0].text, "Café", "ISO-8859-1 was decoded");

    let attachment = rig.store.with(|s| s.attachment(files[0].attachment.id)).unwrap().unwrap();
    let bytes = rig.engine.client.attachment("18c00001", attachment.provider_id.as_deref().unwrap()).await.unwrap();
    assert_eq!(bytes, b"%PDF-1.4 fake");
}

#[tokio::test]
async fn catching_up_applies_new_mail_read_state_stars_trash_and_deletions() {
    let rig = rig().await;
    let a = rig.fake.add_message(spec("Maya <maya@northwind.example>", "One", "first", 5));
    let b = rig.fake.add_message(spec("Dan <dan@northwind.example>", "Two", "second", 4));
    let c = rig.fake.add_message(spec("Rasmus <rasmus@lindqvist.example>", "Three", "third", 3));
    rig.engine.run().await.unwrap();
    assert_eq!(people(&rig, "").len(), 3);

    let fresh = rig.fake.add_message(spec("Maya <maya@northwind.example>", "Four", "fourth", 0));
    rig.fake.change_labels(&a, &[], &["UNREAD"]);
    rig.fake.change_labels(&b, &["STARRED"], &[]);
    rig.fake.change_labels(&c, &["TRASH"], &["INBOX"]);
    let doomed = rig.fake.add_message(spec("Gone <gone@mail.example>", "Soon deleted", "bye", 0));
    rig.fake.delete_message(&doomed);

    let report = rig.engine.run().await.unwrap();
    assert_eq!((report.ingested, report.removed), (1, 1));
    assert!(report.label_changes >= 3);

    let maya = &people(&rig, "Maya")[0];
    let convos = rig.store.with(|s| s.list_conversations(maya.id)).unwrap();
    assert_eq!(convos.len(), 2, "the new message started a conversation");
    assert_eq!(
        convos.iter().map(|c| c.unread).sum::<u32>(),
        1,
        "the old one was read elsewhere, the new one is unread"
    );
    let dan = &people(&rig, "Dan")[0];
    let convo = rig.store.with(|s| s.list_conversations(dan.id)).unwrap().remove(0);
    let detail = rig.store.with(|s| s.get_conversation(convo.id, Some(dan.id))).unwrap().unwrap();
    assert!(detail.messages[0].starred);
    assert!(people(&rig, "Rasmus").is_empty(), "a trashed conversation leaves the people list");
    assert!(people(&rig, "Gone").is_empty());
    assert!(rig.store.with(|s| s.known_provider_ids(rig.account, std::slice::from_ref(&doomed))).unwrap().is_empty());
    assert!(!rig.store.with(|s| s.known_provider_ids(rig.account, std::slice::from_ref(&fresh))).unwrap().is_empty());

    assert_eq!(rig.events.arrived.lock().unwrap().iter().sum::<usize>(), 1);

    let before = rig.fake.count_requests("messages/");
    let again = rig.engine.run().await.unwrap();
    assert_eq!((again.fetched, again.ingested, again.removed, again.label_changes), (0, 0, 0, 0));
    assert_eq!(rig.fake.count_requests("messages/"), before);
}

#[tokio::test]
async fn unknown_senders_wait_in_new_senders_but_people_already_in_the_history_do_not() {
    let rig = rig().await;
    rig.fake.add_message(spec("Maya <maya@northwind.example>", "Old thread", "hello", 30));
    rig.engine.run().await.unwrap();
    rig.fake.add_message(spec("Maya <maya@northwind.example>", "Again", "me again", 0));
    rig.fake.add_message(spec("Stranger <stranger@elsewhere.example>", "Hello there", "we have not met", 0));
    rig.engine.run().await.unwrap();

    let maya = people(&rig, "Maya");
    assert_eq!(maya[0].status, PersonStatus::Known);
    assert!(people(&rig, "Stranger").is_empty(), "not in People yet");
    let waiting: i64 = rig
        .store
        .with(|s| {
            Ok(s.conn().query_row("SELECT COUNT(*) FROM people WHERE status = 'new_sender'", [], |r| r.get(0))?)
        })
        .unwrap();
    assert_eq!(waiting, 1);
}

#[tokio::test]
async fn gmail_forgetting_our_place_triggers_a_clean_reread_without_duplicates() {
    let rig = rig().await;
    rig.fake.add_message(spec("Maya <maya@northwind.example>", "One", "first", 3));
    rig.engine.run().await.unwrap();
    rig.fake.add_message(spec("Maya <maya@northwind.example>", "Two", "second", 1));
    rig.fake.expire_history();

    let report = rig.engine.run().await.unwrap();
    assert_eq!(report.ingested, 1, "only the message we did not have is new");
    let maya = &people(&rig, "Maya")[0];
    let total: u32 = rig.store.with(|s| s.list_conversations(maya.id)).unwrap().iter().map(|c| c.message_count).sum();
    assert_eq!(total, 2);
    assert!(rig.events.notices.lock().unwrap().iter().any(|(_, t)| t.contains("reading your mail again")));
    let state = rig.store.with(|s| s.sync_state(rig.account)).unwrap();
    assert_eq!(state.history_id.as_deref(), Some(rig.fake.history_id().as_str()));
}

#[tokio::test]
async fn rate_limits_and_server_errors_are_retried_with_backoff() {
    let rig = rig().await;
    rig.fake.add_message(spec("Maya <maya@northwind.example>", "One", "first", 3));
    rig.fake.fail_next(
        2,
        Failure {
            status: 429,
            reason: "rateLimitExceeded".into(),
            path_contains: Some("messages".into()),
            retry_after: Some(0),
        },
    );
    rig.fake.fail_next(
        1,
        Failure {
            status: 503,
            reason: "backendError".into(),
            path_contains: Some("profile".into()),
            retry_after: None,
        },
    );
    let report = rig.engine.run().await.unwrap();
    assert_eq!(report.ingested, 1, "it got there in the end");
    assert!(rig.fake.count_requests("messages") >= 3, "and asked more than once: {:?}", rig.fake.requests());
}

#[tokio::test]
async fn a_failure_that_keeps_happening_is_reported_and_leaves_the_store_consistent() {
    let rig = rig().await;
    rig.fake.add_message(spec("Maya <maya@northwind.example>", "One", "first", 3));
    rig.fake
        .fail_next(100, Failure { status: 503, reason: "backendError".into(), path_contains: None, retry_after: None });
    let error = rig.engine.run().await.unwrap_err();
    assert!(error.is_transient(), "{error}");
    assert!(people(&rig, "").is_empty(), "nothing half-written");
    assert!(
        rig.store.with(|s| s.sync_state(rig.account)).unwrap().history_id.is_none(),
        "so the next run starts from the top"
    );

    rig.fake.clear_failures();
    assert_eq!(rig.engine.run().await.unwrap().ingested, 1);
}

#[tokio::test]
async fn older_mail_is_filled_in_page_by_page_after_the_newest_is_ready() {
    let rig = rig_with(SyncOptions { first_read: 5, backfill_page: 4, ..SyncOptions::default() }).await;
    for i in 0..14 {
        rig.fake.add_message(spec(&format!("P{i} <p{i}@mail.example>"), &format!("Subject {i}"), "body", 20 - i));
    }
    let report = rig.engine.run().await.unwrap();
    assert_eq!(report.ingested, 5, "only the newest five at first");
    let newest: Vec<String> = people(&rig, "").iter().map(|p| p.name.clone()).collect();
    assert_eq!(newest.len(), 5);
    assert!(newest.contains(&"P13".to_string()), "the newest mail came first: {newest:?}");
    let state = rig.store.with(|s| s.sync_state(rig.account)).unwrap();
    assert!(!state.backfill_done && state.backfill_cursor.is_some());

    let mut steps = 0;
    while rig.engine.backfill_step().await.unwrap() > 0
        || !rig.store.with(|s| s.sync_state(rig.account)).unwrap().backfill_done
    {
        steps += 1;
        assert!(steps < 10, "backfill must end");
    }
    assert_eq!(people(&rig, "").len(), 14);
    assert!(rig.store.with(|s| s.sync_state(rig.account)).unwrap().backfill_done);
    assert_eq!(rig.engine.backfill_step().await.unwrap(), 0);
}

#[tokio::test]
async fn the_first_read_can_be_limited_to_a_window() {
    let rig = rig_with(SyncOptions { window: Some("newer_than:30d".into()), ..SyncOptions::default() }).await;
    rig.fake.add_message(spec("New <new@mail.example>", "Recent", "x", 3));
    rig.fake.add_message(spec("Old <old@mail.example>", "Ancient", "x", 400));
    rig.engine.run().await.unwrap();
    assert_eq!(people(&rig, "").len(), 1);
}

#[tokio::test]
async fn changes_made_here_reach_gmail_through_the_outbox_in_order() {
    let rig = rig().await;
    let id = rig.fake.add_message(spec("Maya <maya@northwind.example>", "One", "first", 3));
    rig.engine.run().await.unwrap();
    let maya = people(&rig, "Maya")[0].id;
    let convo = rig.store.with(|s| s.list_conversations(maya)).unwrap().remove(0);
    assert!(rig.fake.labels_of(&id).contains(&"UNREAD".to_string()));

    rig.store.with(|s| s.set_conversation_read(convo.id, true, NOW)).unwrap();
    assert_eq!(rig.store.with(|s| s.outbox_len()).unwrap(), 1);
    let report = rig.engine.run().await.unwrap();
    assert_eq!(report.sent, 1);
    assert!(!rig.fake.labels_of(&id).contains(&"UNREAD".to_string()), "Gmail now has it read");
    assert_eq!(rig.store.with(|s| s.outbox_len()).unwrap(), 0);
    assert_eq!(rig.store.with(|s| s.person_summary(maya)).unwrap().unwrap().unread, 0);
}

#[tokio::test]
async fn a_reply_is_sent_threaded_and_not_stored_twice_when_sync_sees_it_come_back() {
    let rig = rig().await;
    rig.fake.add_message(MessageSpec {
        headers: vec![("Date".into(), "ignored".into())],
        ..spec("Maya Okafor <maya@northwind.example>", "Coffee Thursday?", "Coffee Thursday?", 1)
    });
    rig.engine.run().await.unwrap();
    let maya = people(&rig, "Maya")[0].id;
    let convo = rig.store.with(|s| s.list_conversations(maya)).unwrap().remove(0);

    rig.clock.set(NOW + 1000);
    let outcome = rig.store.with(|s| s.store_reply(convo.id, "Thursday works. 9:30?", NOW + 1000)).unwrap();
    assert_eq!(rig.store.with(|s| s.outbox_len()).unwrap(), 1);

    let report = rig.engine.run().await.unwrap();
    assert_eq!(report.sent, 1);
    let sent = rig.fake.sent();
    assert_eq!(sent.len(), 1);
    let raw = String::from_utf8_lossy(&sent[0]).to_string();
    assert!(raw.contains("Subject: Re: Coffee Thursday?"), "{raw}");
    assert!(raw.contains("To: \"Maya Okafor\" <maya@northwind.example>"), "{raw}");
    assert!(raw.contains("In-Reply-To: <18c00001@fake.example>"), "it answers the message it replies to: {raw}");
    assert!(raw.contains("Thursday works. 9:30?"));

    let total: u32 = rig.store.with(|s| s.list_conversations(maya)).unwrap().iter().map(|c| c.message_count).sum();
    assert_eq!(total, 2, "the question and the reply, not the reply twice");
    let provider: String = rig
        .store
        .with(|s| {
            Ok(s.conn()
                .query_row("SELECT provider_id FROM messages WHERE id = ?1", [outcome.message_id], |r| r.get(0))?)
        })
        .unwrap();
    assert!(!provider.starts_with("local:"), "the message now carries Gmail's id: {provider}");
    assert_eq!(rig.store.with(|s| s.outbox_len()).unwrap(), 0);

    let again = rig.engine.run().await.unwrap();
    assert_eq!((again.ingested, again.sent), (0, 0));
}

#[tokio::test]
async fn a_change_that_cannot_be_sent_yet_waits_and_one_gmail_refuses_is_dropped_with_a_notice() {
    let rig = rig().await;
    let id = rig.fake.add_message(spec("Maya <maya@northwind.example>", "One", "first", 3));
    rig.engine.run().await.unwrap();
    let maya = people(&rig, "Maya")[0].id;
    let convo = rig.store.with(|s| s.list_conversations(maya)).unwrap().remove(0);

    rig.store.with(|s| s.set_conversation_read(convo.id, true, NOW)).unwrap();
    rig.fake.fail_next(
        10,
        Failure { status: 503, reason: "backendError".into(), path_contains: Some("modify".into()), retry_after: None },
    );
    assert!(rig.engine.run().await.is_err());
    assert_eq!(rig.store.with(|s| s.outbox_len()).unwrap(), 1);
    assert!(rig.fake.labels_of(&id).contains(&"UNREAD".to_string()), "Gmail has not seen it");
    rig.fake.clear_failures();
    rig.clock.advance(60 * 60_000);
    assert_eq!(rig.engine.run().await.unwrap().sent, 1);
    assert!(!rig.fake.labels_of(&id).contains(&"UNREAD".to_string()));

    rig.store
        .with(|s| {
            s.conn().execute(
                "INSERT INTO outbox (account_id, op, payload, next_attempt_at, created_at) VALUES (?1, 'thread_modify', '{\"thread_id\":\"ghost\",\"add\":[\"STARRED\"],\"remove\":[]}', ?2, ?2)",
                rusqlite_params(rig.account, rig.clock.now_ms()),
            )?;
            Ok(())
        })
        .unwrap();
    rig.engine.run().await.unwrap();
    assert_eq!(rig.store.with(|s| s.outbox_len()).unwrap(), 0, "given up on");
    assert!(
        rig.events
            .notices
            .lock()
            .unwrap()
            .iter()
            .any(|(level, text)| *level == Level::Warning && text.contains("did not accept"))
    );
}

fn rusqlite_params(account: i64, now: i64) -> [i64; 2] {
    [account, now]
}

#[tokio::test]
async fn a_big_first_read_stays_fast_and_ordered() {
    let rig = rig_with(SyncOptions { first_read: 600, ..SyncOptions::default() }).await;
    for i in 0..600i64 {
        rig.fake.add_message(spec(
            &format!("Sender {} <s{}@mail.example>", i % 60, i % 60),
            &format!("Subject {i}"),
            "some body text for the message",
            i % 90,
        ));
    }
    let started = std::time::Instant::now();
    let report = rig.engine.run().await.unwrap();
    let took = started.elapsed();
    assert_eq!(report.ingested, 600);
    assert_eq!(people(&rig, "").len(), 60);
    assert!(took < Duration::from_secs(20), "600 messages took {took:?}");
    println!("first read of 600 messages: {took:?}");
}

#[tokio::test]
async fn a_large_untidy_mailbox_is_read_completely_and_only_once() {
    let rig = rig_with(SyncOptions { first_read: 600, backfill_page: 250, ..SyncOptions::default() }).await;
    let total = 2400usize;
    let senders = 180usize;
    for i in 0..total {
        let sender = i % senders;
        let thread = format!("thread-{}", i / 6);
        let mut labels = vec!["INBOX".to_string()];
        labels.push(format!("Label_{}", i % 37));
        if i % 4 == 0 {
            labels.push("UNREAD".into());
        }
        rig.fake.add_message(MessageSpec {
            from: format!("Sender {sender} <sender{sender}@example{}.test>", sender % 20),
            to: vec!["me@example.com".into()],
            cc: if i % 9 == 0 { vec![format!("cc{}@example.test", i % 5)] } else { vec![] },
            subject: match i % 7 {
                0 => format!("=?UTF-8?B?SGVsbG8g4pyTIHRocmVhZCB7aX0=?= {i}"),
                1 => format!("RE: Re: re: Quarterly numbers {}", i / 6),
                2 => format!("Fwd: {}", "very long subject ".repeat(12)),
                3 => String::new(),
                _ => format!("Topic {}", i / 6),
            },
            text: Some(format!("Body {i}\n\n> quoted line\n-- \nSignature {sender}")),
            date_ms: NOW - (i as i64) * 60_000,
            labels,
            thread: Some(thread),
            ..Default::default()
        });
    }

    let started = std::time::Instant::now();
    rig.engine.run().await.unwrap();
    let after_first = rig
        .store
        .with(|s| {
            s.conn()
                .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .map_err(zsm_core::Error::from)
        })
        .unwrap();
    assert_eq!(after_first, 600, "the newest are ready first");

    let mut steps = 0;
    while !rig.store.with(|s| s.sync_state(rig.account)).unwrap().backfill_done {
        rig.engine.backfill_step().await.unwrap();
        steps += 1;
        assert!(steps < 40, "backfill must end");
    }
    let elapsed = started.elapsed();

    let count = |sql: &str| -> i64 {
        rig.store.with(|s| s.conn().query_row(sql, [], |r| r.get::<_, i64>(0)).map_err(zsm_core::Error::from)).unwrap()
    };
    assert_eq!(count("SELECT COUNT(*) FROM messages"), total as i64, "every message is here");
    assert_eq!(count("SELECT COUNT(DISTINCT provider_id) FROM messages"), total as i64, "none is stored twice");
    assert_eq!(count("SELECT COUNT(*) FROM conversations"), (total / 6) as i64, "threads group six messages each");
    let known = count("SELECT COUNT(*) FROM people WHERE kind <> 'self'");
    assert!(known >= senders as i64 && known <= (senders + 5) as i64, "people: {known}");
    assert_eq!(
        count("SELECT COUNT(*) - COUNT(DISTINCT address) FROM person_addresses"),
        0,
        "an address belongs to one person"
    );
    assert_eq!(count("SELECT COUNT(*) FROM labels WHERE kind = 'user'"), 37, "labels are not duplicated");
    assert!(count("SELECT COUNT(*) FROM messages_fts") >= total as i64 - 10, "everything is searchable");
    rig.engine.run().await.unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM messages"), total as i64);
    println!("{total} messages read and filed in {elapsed:?}");
    assert!(elapsed < Duration::from_secs(60), "too slow: {elapsed:?}");
}

#[tokio::test]
async fn awkward_real_world_messages_are_taken_in_and_the_sync_goes_on() {
    let rig = rig().await;
    rig.fake.add_message(MessageSpec {
        from: "\"\" <odd@example.test>".into(),
        to: vec!["undisclosed-recipients:;".into()],
        subject: String::new(),
        text: Some("No subject, no names.".into()),
        date_ms: NOW - DAY,
        ..Default::default()
    });
    let big = format!(
        "<html><body>{}</body></html>",
        "<p>Lots of text, <a href=\"https://x.example/?utm_source=a\">link</a>.</p>".repeat(110_000)
    );
    rig.fake.add_message(MessageSpec {
        from: "Big Sender <big@example.test>".into(),
        subject: "The very large one".into(),
        html: Some(big),
        date_ms: NOW - 2 * DAY,
        headers: vec![("List-Unsubscribe".into(), "<https://x.example/u>".into())],
        ..spec("x", "x", "x", 2)
    });
    let many: Vec<String> = (0..1000).map(|i| format!("person{i}@bulk.test")).collect();
    rig.fake.add_message(MessageSpec {
        to: many,
        ..spec("Organiser <org@example.test>", "Everyone is invited", "Please come.", 3)
    });
    rig.fake.add_message(spec("After Them <after@example.test>", "Still works afterwards", "Fine.", 0));

    let started = std::time::Instant::now();
    let report = rig.engine.run().await.unwrap();
    assert_eq!(report.fetched, 4, "all four were fetched");
    assert_eq!(report.ingested, 4, "all four were stored");
    assert!(started.elapsed() < Duration::from_secs(30), "took {:?}", started.elapsed());

    assert_eq!(people(&rig, "After Them").len(), 1, "the ones after the awkward ones arrived");
    let big = people(&rig, "Big Sender");
    assert_eq!(big.len(), 1);
    let convo = rig.store.with(|s| s.list_conversations(big[0].id)).unwrap().remove(0);
    let detail = rig.store.with(|s| s.get_conversation(convo.id, Some(big[0].id))).unwrap().unwrap();
    assert!(detail.messages[0].html_url.is_some() && detail.messages[0].unsubscribe.is_some());
    let count: i64 = rig
        .store
        .with(|s| s.conn().query_row("SELECT COUNT(*) FROM people", [], |r| r.get(0)).map_err(zsm_core::Error::from))
        .unwrap();
    assert!(count < 60, "{count} people from four messages");
    let odd = people(&rig, "odd");
    assert_eq!(odd.len(), 1);
}

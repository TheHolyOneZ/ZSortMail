use std::sync::{Arc, Mutex};
use std::time::Duration;

use zsm_app::{
    Core, CoreConfig, CoreEvent, EventSink, GoogleEndpoints, LinkPhase, Notice, NotificationSettings, Platform,
    QuietHours, SyncPhase,
};
use zsm_core::clock::FixedClock;
use zsm_core::dto::PeopleFilter;
use zsm_core::store::rules::{RuleAction, RuleInput};
use zsm_core::store::watches::{WatchInput, WatchMode, WatchPriority, WatchStatus};
use zsm_gmail::RetryPolicy;
use zsm_gmail::credentials::{MemorySecrets, Secrets};
use zsm_testkit::{AttachmentSpec, Failure, FakeGoogle, MessageSpec};

const NOW: i64 = 1_795_082_400_000;
const DAY: i64 = 86_400_000;

#[derive(Default)]
struct Recorder(Mutex<Vec<CoreEvent>>);

impl EventSink for Recorder {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[derive(Default)]
struct Desktop(Mutex<Vec<Notice>>);

impl Platform for Desktop {
    fn notify(&self, notice: &Notice) {
        self.0.lock().unwrap().push(notice.clone());
    }
}

struct Rig {
    desktop: Arc<Desktop>,
    fake: FakeGoogle,
    core: Arc<Core>,
    events: Arc<Recorder>,
    secrets: Arc<MemorySecrets>,
    data_dir: std::path::PathBuf,
}

fn unique() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new().build_hasher().finish()
}

async fn rig() -> Rig {
    rig_with(None).await
}

async fn rig_with(write_allowlist: Option<Vec<String>>) -> Rig {
    let fake = FakeGoogle::start("me@example.com").await;
    fake.set_now(NOW);
    let events = Arc::new(Recorder::default());
    let desktop = Arc::new(Desktop::default());
    let secrets = Arc::new(MemorySecrets::default());
    let data_dir = std::env::temp_dir().join(format!("zsm-app-test-{:x}", unique()));
    let core = Core::new(CoreConfig {
        write_allowlist,
        clock: Arc::new(FixedClock::new(NOW)),
        sink: events.clone(),
        runtime: Some(tokio::runtime::Handle::current()),
        secrets: secrets.clone(),
        google: GoogleEndpoints {
            auth_url: fake.auth_url(),
            token_url: fake.token_url(),
            gmail_base: fake.gmail_base(),
            people_base: fake.people_base(),
        },
        retry: RetryPolicy { max_attempts: 3, base: Duration::from_millis(2), max: Duration::from_millis(10) },
        poll_interval: Duration::from_millis(100),
        data_dir: Some(data_dir.clone()),
        platform: desktop.clone(),
        timezone: jiff::tz::TimeZone::UTC,
        ..CoreConfig::in_memory()
    })
    .unwrap();
    Rig { desktop, fake, core, events, secrets, data_dir }
}

fn mail(from: &str, subject: &str, text: &str, days_ago: i64) -> MessageSpec {
    MessageSpec {
        from: from.into(),
        to: vec!["me@example.com".into()],
        subject: subject.into(),
        text: Some(text.into()),
        date_ms: NOW - days_ago * DAY,
        ..Default::default()
    }
}

async fn eventually<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(std::time::Instant::now() < deadline, "timed out waiting for: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn approve(url: &str) {
    let response = zsm_gmail::client::build_http().get(url).send().await.expect("the browser reaches the app");
    assert!(response.status().is_success());
}

async fn link(rig: &Rig) {
    rig.core.save_google_client("test-client.apps.example", "test-secret").unwrap();
    let url = rig.core.begin_gmail_link().await.unwrap();
    approve(&url).await;
    eventually("the account to be linked", || rig.core.app_state().unwrap().accounts.first().cloned()).await;
}

fn people(core: &Core) -> Vec<zsm_core::dto::PersonSummary> {
    core.people(PeopleFilter::All, "").unwrap()
}

#[tokio::test]
async fn linking_through_the_browser_starts_a_sync_that_fills_in_people() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya Okafor <maya@northwind.example>", "Q3 launch", "Hello", 2));
    rig.fake.add_message(mail("Dan Whitaker <dan@northwind.example>", "Screenshots", "Attached", 1));

    link(&rig).await;
    let state = rig.core.app_state().unwrap();
    assert_eq!(state.accounts[0].provider, "gmail");
    assert_eq!(state.accounts[0].email, "me@example.com");
    assert_eq!(rig.secrets.get("me@example.com").unwrap().as_deref(), Some(rig.fake.refresh_token().as_str()));

    eventually("people to appear", || (people(&rig.core).len() == 2).then_some(())).await;
    let names: Vec<String> = people(&rig.core).into_iter().map(|p| p.name).collect();
    assert!(names.contains(&"Maya Okafor".to_string()) && names.contains(&"Dan Whitaker".to_string()), "{names:?}");

    let status = eventually("sync to settle", || {
        rig.core.sync_status().into_iter().find(|s| s.phase == SyncPhase::Idle && s.last_sync_at.is_some())
    })
    .await;
    assert_eq!(status.queued, 0);

    let events = rig.events.0.lock().unwrap().clone();
    assert!(events.iter().any(|e| matches!(e, CoreEvent::LinkState { phase: LinkPhase::Waiting, .. })));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CoreEvent::LinkState { phase: LinkPhase::Linked, message } if message.contains("me@example.com")))
    );
    assert!(events.iter().any(|e| matches!(e, CoreEvent::SyncProgress { phase: SyncPhase::Reading, .. })));
}

#[tokio::test]
async fn new_mail_shows_up_by_itself_without_anyone_asking() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "First", "one", 2));
    link(&rig).await;
    eventually("first mail", || (people(&rig.core).len() == 1).then_some(())).await;

    rig.fake.add_message(mail("Maya <maya@northwind.example>", "Second", "two", 0));
    eventually("the new message", || {
        let maya = people(&rig.core).into_iter().next()?;
        let convos = rig.core.conversations(maya.id).ok()?;
        (convos.len() == 2).then_some(())
    })
    .await;
}

#[tokio::test]
async fn a_change_made_here_reaches_gmail_promptly() {
    let rig = rig().await;
    let id = rig.fake.add_message(mail("Maya <maya@northwind.example>", "One", "first", 2));
    link(&rig).await;
    let maya = eventually("maya", || people(&rig.core).into_iter().next()).await;
    let convo = rig.core.conversations(maya.id).unwrap().remove(0);

    rig.core.set_conversation_read(convo.id, true).unwrap();
    eventually("Gmail to learn the message was read", || {
        (!rig.fake.labels_of(&id).contains(&"UNREAD".to_string())).then_some(())
    })
    .await;

    rig.core.reply(convo.id, "Thanks, got it.").unwrap();
    let sent = eventually("the reply to be delivered", || (!rig.fake.sent().is_empty()).then(|| rig.fake.sent())).await;
    assert!(String::from_utf8_lossy(&sent[0]).contains("Thanks, got it."));
}

#[tokio::test]
async fn files_are_downloaded_when_first_wanted_and_then_served_from_disk() {
    let rig = rig().await;
    rig.fake.add_message(MessageSpec {
        attachments: vec![AttachmentSpec {
            filename: "Invoice.pdf".into(),
            mime: "application/pdf".into(),
            bytes: b"%PDF-1.4 real bytes".to_vec(),
            content_id: None,
        }],
        ..mail("Maya <maya@northwind.example>", "Invoice", "see attached", 1)
    });
    link(&rig).await;
    let maya = eventually("maya", || people(&rig.core).into_iter().next()).await;
    let file = eventually("the file", || rig.core.files(maya.id).ok()?.into_iter().next()).await;

    let path = format!("att/{}", file.attachment.id);
    assert_eq!(rig.core.serve(&path, Some("inline=1")).status, 404);
    rig.core.prepare_attachment(file.attachment.id).await.unwrap();
    let served = rig.core.serve(&path, Some("inline=1"));
    assert_eq!(served.status, 200);
    assert_eq!(served.content_type, "application/pdf");
    assert_eq!(served.body, b"%PDF-1.4 real bytes");
    let requests = rig.fake.count_requests("attachments");
    rig.core.prepare_attachment(file.attachment.id).await.unwrap();
    assert_eq!(rig.fake.count_requests("attachments"), requests);
    assert!(rig.data_dir.join("attachments").exists());
}

#[tokio::test]
async fn a_revoked_sign_in_is_reported_and_unlinking_removes_everything_local() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "One", "first", 2));
    link(&rig).await;
    eventually("first mail", || (people(&rig.core).len() == 1).then_some(())).await;
    let account = rig.core.app_state().unwrap().accounts[0].id;

    rig.fake.revoke_refresh_token();
    rig.fake.expire_access_token();
    rig.fake.add_message(mail("Dan <dan@northwind.example>", "Two", "second", 0));
    let status = eventually("the app to notice", || {
        rig.core.sync_status().into_iter().find(|s| s.phase == SyncPhase::NeedsLink)
    })
    .await;
    assert!(status.message.as_deref().is_some_and(|m| m.contains("Link the account again")), "{status:?}");

    rig.core.unlink_account(account).unwrap();
    assert!(rig.core.app_state().unwrap().accounts.is_empty());
    assert!(people(&rig.core).is_empty());
    assert_eq!(rig.secrets.get("me@example.com").unwrap(), None);
    assert!(rig.core.search("first").unwrap().messages.is_empty(), "the search index is cleared too");
    assert!(rig.core.unlink_account(account).is_err());
}

#[tokio::test]
async fn linking_explains_what_is_missing_and_can_be_cancelled() {
    let rig = rig().await;
    let error = rig.core.begin_gmail_link().await.unwrap_err();
    assert!(error.to_string().contains("needs a Google client"), "{error}");
    assert!(!rig.core.google_setup().unwrap().configured);

    assert!(rig.core.save_google_client("", "x").is_err());
    assert!(rig.core.save_google_client("has space", "x").is_err());
    rig.core.save_google_client("  id.apps.example ", " secret ").unwrap();
    let setup = rig.core.google_setup().unwrap();
    assert!(setup.configured && !setup.built_in);
    assert_eq!(setup.client_id.as_deref(), Some("id.apps.example"));

    let url = rig.core.begin_gmail_link().await.unwrap();
    assert!(url.starts_with(&rig.fake.auth_url()));
    rig.core.cancel_gmail_link();
    let events = rig.events.0.lock().unwrap().clone();
    assert!(events.iter().any(|e| matches!(e, CoreEvent::LinkState { phase: LinkPhase::Cancelled, .. })));
    assert!(rig.core.app_state().unwrap().accounts.is_empty());
}

fn parcel(from: &str) -> MessageSpec {
    mail(
        from,
        "Out for delivery",
        "Your order 394-2211 has been handed to DHL.\nXbox Series X (1 item)\nTracking number: JD014600003394512",
        0,
    )
}

fn watch(query: &str, priority: WatchPriority) -> WatchInput {
    WatchInput {
        id: None,
        name: String::new(),
        query: query.into(),
        mode: WatchMode::Once,
        priority,
        expires_at: None,
        include_existing: false,
    }
}

#[tokio::test]
async fn a_parcel_you_are_waiting_for_notifies_with_the_numbers_pulled_out_of_it() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "First", "one", 2));
    link(&rig).await;
    eventually("first sync", || (people(&rig.core).len() == 1).then_some(())).await;

    let saved = rig.core.save_watch(&watch("xbox 394", WatchPriority::Normal)).unwrap();
    assert_eq!(saved.counted, 0);
    rig.fake.add_message(MessageSpec {
        headers: vec![("Auto-Submitted".into(), "auto-generated".into())],
        ..parcel("DHL Parcel <noreply@parcel.dhl.example>")
    });

    let notice = eventually("the notification", || rig.desktop.0.lock().unwrap().first().cloned()).await;
    assert_eq!(notice.title, "“xbox 394” has arrived");
    assert!(notice.body.contains("DHL Parcel: Out for delivery"), "{}", notice.body);
    assert!(
        notice.body.contains("Tracking JD014600003394512") && notice.body.contains("Order 394-2211"),
        "{}",
        notice.body
    );
    assert!(notice.conversation_id.is_some() && !notice.urgent);

    let events = rig.events.0.lock().unwrap().clone();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CoreEvent::Notification { reason, .. } if reason == "You are waiting for “xbox 394”"))
    );
    let log = rig.core.notification_log().unwrap();
    assert_eq!((log.len(), log[0].shown), (1, true));
    let view = &rig.core.watches().unwrap()[0];
    assert_eq!(view.status, WatchStatus::Arrived);
    assert_eq!(view.hits[0].extracted.len(), 2);
    assert!(rig.core.today().unwrap().unseen_hits >= 1);

    assert_eq!(rig.core.open_notification(notice.id).unwrap(), notice.conversation_id);
    assert_eq!(rig.core.today().unwrap().unseen_hits, 0);
}

#[tokio::test]
async fn quiet_hours_hold_back_a_normal_watch_but_not_an_urgent_one_and_both_are_logged() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "First", "one", 2));
    link(&rig).await;
    eventually("first sync", || (people(&rig.core).len() == 1).then_some(())).await;
    rig.core
        .save_notification_settings(&NotificationSettings {
            enabled: true,
            quiet: Some(QuietHours { from: 9 * 60, to: 11 * 60 }),
            focus: false,
            important: false,
            everyone: false,
        })
        .unwrap();
    rig.core.save_watch(&watch("alpha", WatchPriority::Normal)).unwrap();
    rig.core.save_watch(&watch("beta", WatchPriority::Urgent)).unwrap();

    rig.fake.add_message(mail("Dan <dan@northwind.example>", "alpha news", "alpha", 0));
    eventually("the held-back notification to be logged", || {
        (!rig.core.notification_log().unwrap().is_empty()).then_some(())
    })
    .await;
    rig.fake.add_message(mail("Dan <dan@northwind.example>", "beta news", "beta", 0));
    eventually("the urgent one", || (!rig.desktop.0.lock().unwrap().is_empty()).then_some(())).await;

    let shown = rig.desktop.0.lock().unwrap().clone();
    assert_eq!(shown.len(), 1, "only the urgent watch interrupted");
    assert!(shown[0].urgent && shown[0].title.contains("beta"));
    let log = rig.core.notification_log().unwrap();
    assert_eq!(log.len(), 2);
    let held = log.iter().find(|n| !n.shown).unwrap();
    assert!(held.reason.starts_with("Quiet hours, 09:00 to 11:00"), "{}", held.reason);
    assert!(log.iter().find(|n| n.shown).unwrap().reason.contains("(urgent)"));
}

#[tokio::test]
async fn rules_sort_mail_as_it_arrives_from_gmail() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "First", "one", 2));
    link(&rig).await;
    eventually("first sync", || (people(&rig.core).len() == 1).then_some(())).await;
    rig.core
        .save_rule(&RuleInput {
            id: None,
            name: "Receipts".into(),
            enabled: true,
            query: "from:shop.example subject:receipt".into(),
            actions: vec![RuleAction::Archive, RuleAction::MarkRead],
            stop: false,
        })
        .unwrap();
    rig.core
        .save_rule(&RuleInput {
            id: None,
            name: String::new(),
            enabled: true,
            query: "from:shop.example".into(),
            actions: vec![RuleAction::Allow],
            stop: false,
        })
        .unwrap();

    rig.fake.add_message(mail("Shop <orders@shop.example>", "Your receipt", "thanks for your order", 0));
    let shop =
        eventually("the shop to appear", || rig.core.people(PeopleFilter::All, "Shop").unwrap().into_iter().next())
            .await;
    let convo = rig.core.conversations(shop.id).unwrap().remove(0);
    assert_eq!(convo.unread, 0, "marked read by the rule");
    let page = rig.core.inbox(None, 50, 0).unwrap();
    assert!(page.rows.iter().all(|r| r.conversation.id != convo.id), "archived out of the inbox");
    assert_eq!(rig.core.rules().unwrap()[0].hit_count, 1);
    eventually("Gmail to learn it was archived", || (rig.fake.count_requests("modify") > 0).then_some(())).await;
}

#[tokio::test]
async fn a_written_email_with_a_file_and_a_hidden_recipient_reaches_gmail_once_the_undo_window_has_passed() {
    use zsm_core::store::drafts::{ComposeSettings, DraftInput};
    use zsm_core::store::ingest::Address;

    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "Hello", "hi", 1));
    link(&rig).await;
    eventually("first mail", || (people(&rig.core).len() == 1).then_some(())).await;
    rig.core.save_compose_settings(&ComposeSettings { signature: String::new(), undo_seconds: 0 }).unwrap();

    let draft = rig.core.new_draft(vec![Address::new("stranger@example.com", Some("Sam Stranger"))]).unwrap();
    rig.core
        .save_draft(&DraftInput {
            id: draft.id,
            to: draft.to.clone(),
            cc: vec![],
            bcc: vec![Address::new("boss@example.com", None)],
            subject: "Introduction".into(),
            body_html: "<p>Nice to <b>meet</b> you.</p>".into(),
            nudge_days: None,
        })
        .unwrap();
    rig.core.add_draft_file(draft.id, "plan.bin", "application/octet-stream", &[0, 1, 2, 255]).unwrap();
    let ticket = rig.core.send_draft(draft.id).unwrap();

    let sent = eventually("the email to be delivered", || (!rig.fake.sent().is_empty()).then(|| rig.fake.sent())).await;
    let raw = String::from_utf8_lossy(&sent[0]).into_owned();
    assert!(raw.contains("Subject: Introduction"), "{raw}");
    assert!(raw.contains("stranger@example.com"), "to");
    assert!(raw.contains("boss@example.com"), "the hidden recipient is in the envelope");
    assert!(raw.contains("text/html") && raw.contains("<b>meet</b>"), "the formatted version");
    assert!(raw.contains("plan.bin") && raw.contains("AAEC/w=="), "the file travels, base64 encoded");

    eventually("the message to take the server's id", || {
        let provider: String = rig
            .core
            .shared_store()
            .lock()
            .conn()
            .query_row("SELECT provider_id FROM messages WHERE id = ?1", [ticket.message_id], |r| r.get(0))
            .ok()?;
        (!provider.starts_with("local:")).then_some(())
    })
    .await;
}

#[tokio::test]
async fn a_message_taken_back_inside_the_undo_window_never_reaches_gmail() {
    use zsm_core::store::drafts::DraftInput;
    use zsm_core::store::ingest::Address;

    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "Hello", "hi", 1));
    link(&rig).await;
    eventually("first mail", || (people(&rig.core).len() == 1).then_some(())).await;

    let draft = rig.core.new_draft(vec![Address::new("stranger@example.com", None)]).unwrap();
    rig.core
        .save_draft(&DraftInput {
            id: draft.id,
            to: draft.to.clone(),
            cc: vec![],
            bcc: vec![],
            subject: "Oops".into(),
            body_html: "<p>Wrong person.</p>".into(),
            nudge_days: None,
        })
        .unwrap();
    let ticket = rig.core.send_draft(draft.id).unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(rig.fake.sent().is_empty(), "the message waits out the window");

    let back = rig.core.unsend(ticket.message_id).unwrap();
    assert_eq!(back.subject, "Oops");
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(rig.fake.sent().is_empty(), "and what was taken back stays here");
    assert_eq!(rig.core.shared_store().lock().outbox_len().unwrap(), 0);
}

#[tokio::test]
async fn google_contacts_are_read_once_across_pages_and_only_fill_in_people_already_there() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya Okafor <maya@northwind.example>", "Q3 launch", "Hello", 2));
    link(&rig).await;
    eventually("people to appear", || (people(&rig.core).len() == 1).then_some(())).await;

    rig.fake.set_contacts(vec![
        serde_json::json!({"names": [{"displayName": "Maya Okafor"}], "emailAddresses": [{"value": "maya@northwind.example"}],
            "organizations": [{"name": "Northwind", "title": "Design lead"}]}),
        serde_json::json!({"names": [{"displayName": "No Mail"}]}),
        serde_json::json!({"names": [{"displayName": "Stranger"}], "emailAddresses": [{"value": "stranger@elsewhere.example"}]}),
    ]);
    let before = rig.fake.count_requests("people");
    let url = rig.core.begin_contacts_import().await.unwrap();
    assert!(url.contains("contacts.readonly") && !url.contains("gmail.modify"), "{url}");
    assert!(!url.contains("access_type"), "no lasting access is asked for");
    approve(&url).await;
    eventually("the import to report", || {
        rig.events
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, CoreEvent::Notice { text, .. } if text.contains("from Google Contacts")))
            .then_some(())
    })
    .await;
    assert_eq!(rig.fake.count_requests("people") - before, 2, "two pages");
    let maya = people(&rig.core).into_iter().next().unwrap();
    let detail = rig.core.person(maya.id).unwrap().unwrap();
    assert_eq!(detail.summary.title.as_deref(), Some("Design lead"));
    assert_eq!(detail.organization.as_deref(), Some("Northwind"));
    assert_eq!(people(&rig.core).len(), 1, "it never adds people");
    assert_eq!(rig.secrets.get("me@example.com").unwrap().as_deref(), Some(rig.fake.refresh_token().as_str()));
}

#[tokio::test]
async fn a_tag_becomes_a_gmail_label_only_when_mirroring_is_on_and_never_deletes_anything() {
    let rig = rig().await;
    let id = rig.fake.add_message(mail("Maya <maya@northwind.example>", "One", "first", 2));
    link(&rig).await;
    let maya = eventually("maya", || people(&rig.core).into_iter().next()).await;
    let convo = rig.core.conversations(maya.id).unwrap().remove(0);

    assert!(!rig.core.mirror_tags().unwrap());
    rig.core.tag_conversation(convo.id, "Private").unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !rig.fake.user_label_names().iter().any(|n| n == "Private"),
        "nothing is made in Gmail while mirroring is off"
    );

    rig.core.set_mirror_tags(true).unwrap();
    rig.core.tag_conversation(convo.id, "Taxes").unwrap();
    eventually("the label to exist in Gmail", || {
        rig.fake.user_label_names().iter().any(|n| n == "Taxes").then_some(())
    })
    .await;
    eventually("the label to be on the message", || {
        rig.fake.labels_of(&id).iter().any(|l| l.starts_with("Label_")).then_some(())
    })
    .await;

    let tags = rig.core.conversation_tags(convo.id).unwrap();
    let taxes = tags.iter().find(|t| t.name == "Taxes").expect("one entry for it, mirrored or not");
    assert!(taxes.local, "still removable from here");
    assert_eq!(tags.iter().filter(|t| t.name == "Taxes").count(), 1, "not listed twice");
    rig.core.untag_conversation(convo.id, taxes.id).unwrap();
    eventually("the label to come off the message", || {
        (!rig.fake.labels_of(&id).iter().any(|l| l.starts_with("Label_") && l != "Label_1")).then_some(())
    })
    .await;
    assert!(rig.fake.user_label_names().iter().any(|n| n == "Taxes"));
}

#[tokio::test]
async fn an_unticked_gmail_permission_is_explained_once_and_nothing_is_linked() {
    let rig = rig().await;
    rig.core.save_google_client("test-client.apps.example", "test-secret").unwrap();
    rig.fake.set_granted_scope("openid email");
    let url = rig.core.begin_gmail_link().await.unwrap();
    approve(&url).await;
    eventually("the link to fail", || {
        rig.events
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, CoreEvent::LinkState { phase: LinkPhase::Failed, message } if message.contains("Gmail box")))
            .then_some(())
    })
    .await;
    assert!(rig.core.app_state().unwrap().accounts.is_empty(), "no half-linked mailbox");
    assert!(rig.secrets.get("me@example.com").unwrap().is_none(), "no token kept");
}

#[tokio::test]
async fn a_permission_error_from_gmail_stops_the_sync_and_does_not_keep_asking() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "One", "first", 2));
    link(&rig).await;
    eventually("the first sync", || people(&rig.core).first().cloned()).await;

    rig.fake.fail_next(
        1000,
        Failure {
            status: 403,
            reason: "insufficientPermissions".into(),
            path_contains: Some("history".into()),
            retry_after: None,
        },
    );
    rig.fake.add_message(mail("Dan <dan@northwind.example>", "Two", "second", 1));
    rig.core.wake_sync();
    let id = rig.core.app_state().unwrap().accounts[0].id;
    eventually("the account to ask for a new sign-in", || {
        rig.core
            .sync_status()
            .into_iter()
            .find(|s| s.account_id == id && s.phase == zsm_app::SyncPhase::NeedsLink)
            .filter(|s| s.message.as_deref().is_some_and(|m| m.contains("keep the Gmail box ticked")))
            .map(|_| ())
    })
    .await;
    let asked = rig.fake.count_requests("history");
    rig.core.wake_sync();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(rig.fake.count_requests("history"), asked, "it stopped asking");
}

#[tokio::test]
async fn with_an_allow_list_nothing_reaches_gmail_for_mail_that_involves_anyone_else() {
    let rig = rig_with(Some(vec!["me@example.com".into(), "maya@northwind.example".into()])).await;
    let mine = rig.fake.add_message(mail("Maya <maya@northwind.example>", "Between us", "hello", 3));
    let other = rig.fake.add_message(mail("Newsletter <news@shop.example>", "Big sale", "buy", 2));
    link(&rig).await;
    eventually("both people", || (people(&rig.core).len() == 2).then_some(())).await;
    let find = |name: &str| people(&rig.core).into_iter().find(|p| p.name.contains(name)).unwrap();
    let maya = find("Maya");
    let shop = find("Newsletter");
    let maya_convo = rig.core.conversations(maya.id).unwrap().remove(0);
    let shop_convo = rig.core.conversations(shop.id).unwrap().remove(0);

    rig.core.set_conversation_read(maya_convo.id, true).unwrap();
    eventually("the listed thread to change in Gmail", || {
        (!rig.fake.labels_of(&mine).contains(&"UNREAD".to_string())).then_some(())
    })
    .await;

    rig.core.set_conversation_read(shop_convo.id, true).unwrap();
    eventually("a warning that it was held back", || {
        rig.events
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, CoreEvent::Notice { text, .. } if text.contains("allow-list")))
            .then_some(())
    })
    .await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(rig.fake.labels_of(&other).contains(&"UNREAD".to_string()), "the other mail was not touched in Gmail");
}

#[tokio::test]
async fn gmail_labels_show_the_names_you_gave_them_not_their_ids() {
    let rig = rig().await;
    let id = rig.fake.add_message(mail("Maya <maya@northwind.example>", "One", "first", 2));
    rig.fake.change_labels(&id, &["Label_1"], &[]);
    link(&rig).await;
    let maya = eventually("maya", || people(&rig.core).into_iter().next()).await;
    let convo = rig.core.conversations(maya.id).unwrap().remove(0);
    eventually("the label to be named", || {
        rig.core.conversation_tags(convo.id).unwrap().iter().any(|t| t.name == "Clients").then_some(())
    })
    .await;
    assert!(!rig.core.conversation_tags(convo.id).unwrap().iter().any(|t| t.name == "Label_1"), "no bare ids");
}

#[tokio::test]
async fn ordinary_mail_is_logged_as_not_notified_until_everyone_is_switched_on() {
    let rig = rig().await;
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "First", "one", 2));
    link(&rig).await;
    eventually("first sync", || (people(&rig.core).len() == 1).then_some(())).await;

    rig.fake.add_message(mail("Maya <maya@northwind.example>", "Lunch?", "are you free", 0));
    let quiet = eventually("the quiet arrival to be logged", || {
        rig.core.notification_log().unwrap().into_iter().find(|n| n.body == "Lunch?")
    })
    .await;
    assert!(!quiet.shown);
    assert!(quiet.reason.contains("not a favourite") && quiet.reason.contains("off in Settings"), "{}", quiet.reason);
    assert!(rig.desktop.0.lock().unwrap().is_empty(), "nothing interrupted");

    rig.fake.add_message(mail("Dan <dan@northwind.example>", "Hello", "we have not met", 0));
    let stranger = eventually("the stranger to be logged", || {
        rig.core.notification_log().unwrap().into_iter().find(|n| n.body == "Hello")
    })
    .await;
    assert!(!stranger.shown && stranger.reason.contains("New senders"), "{}", stranger.reason);

    rig.core
        .save_notification_settings(&NotificationSettings {
            enabled: true,
            quiet: None,
            focus: false,
            important: false,
            everyone: true,
        })
        .unwrap();
    rig.fake.add_message(mail("Maya <maya@northwind.example>", "Plans", "saturday", 0));
    let notice = eventually("the notification", || rig.desktop.0.lock().unwrap().first().cloned()).await;
    assert_eq!(notice.title, "Maya");
    assert!(notice.body.starts_with("Plans"), "{}", notice.body);
    let shown = rig.core.notification_log().unwrap().into_iter().find(|n| n.body.starts_with("Plans")).unwrap();
    assert!(shown.shown && shown.reason == "New mail from Maya", "{}", shown.reason);
}

#[tokio::test]
async fn a_test_notification_reaches_the_desktop_and_is_logged() {
    let rig = rig().await;
    rig.core.send_test_notification().unwrap();
    let sent = rig.desktop.0.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].urgent && sent[0].body.contains("notifications reach your desktop"));
    let log = rig.core.notification_log().unwrap();
    assert_eq!((log.len(), log[0].shown, log[0].reason.as_str()), (1, true, "You asked for a test"));
}

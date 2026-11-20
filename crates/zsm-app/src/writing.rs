use std::path::{Path, PathBuf};
use std::time::Duration;

use zsm_core::store::drafts::{
    ComposeSettings, Draft, DraftAttachment, DraftInput, DraftMode, MAX_EMAIL_BYTES, Nudge, RecipientSuggestion,
    SendTicket,
};
use zsm_core::store::ingest::Address;
use zsm_core::store::notifications::NotificationKind;
use zsm_core::store::routines::{Routine, RoutineInput, RoutineRun, RunOutcome, RunReason, RunResult};

use crate::core::Core;
use crate::error::{Error, Result};
use crate::notify::Reminder;

fn invalid(text: &str) -> Error {
    Error::Store(zsm_core::Error::Invalid(text.into()))
}

pub(crate) fn safe_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        return "file".into();
    }
    if cleaned.chars().count() <= 120 {
        return cleaned;
    }
    match cleaned.rsplit_once('.') {
        Some((stem, ext)) if ext.chars().count() <= 12 => {
            format!("{}.{ext}", stem.chars().take(120 - ext.chars().count() - 1).collect::<String>())
        }
        _ => cleaned.chars().take(120).collect(),
    }
}

fn clean_mime(mime: &str) -> String {
    let ok = mime.len() <= 100
        && mime.matches('/').count() == 1
        && mime.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '+' | '-' | '_'));
    if ok { mime.to_ascii_lowercase() } else { "application/octet-stream".into() }
}

fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    (2..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap_or_else(|| dir.join(name))
}

impl Core {
    pub fn compose_settings(&self) -> Result<ComposeSettings> {
        self.with_store(|s| Ok(s.compose_settings()?))
    }

    pub fn save_compose_settings(&self, settings: &ComposeSettings) -> Result<()> {
        self.with_store(|s| Ok(s.save_compose_settings(settings)?))
    }

    pub fn new_draft(&self, to: Vec<Address>) -> Result<Draft> {
        let now = self.now_ms();
        let draft = self.with_store(|s| {
            let account = s.default_account()?;
            let id = s.new_draft(account, &to, now)?;
            s.get_draft(id)?.ok_or(Error::Store(zsm_core::Error::NotFound("draft")))
        })?;
        self.changed("drafts", vec![draft.id]);
        Ok(draft)
    }

    pub fn reply_draft(&self, conversation_id: i64, mode: DraftMode) -> Result<Draft> {
        let now = self.now_ms();
        let draft = self.with_store(|s| {
            let id = s.prefill_draft(conversation_id, mode, &self.timezone, now)?;
            s.get_draft(id)?.ok_or(Error::Store(zsm_core::Error::NotFound("draft")))
        })?;
        self.changed("drafts", vec![draft.id]);
        Ok(draft)
    }

    pub fn save_draft(&self, input: &DraftInput) -> Result<i64> {
        let now = self.now_ms();
        self.with_store(|s| Ok(s.save_draft(input, now)?))?;
        self.changed("drafts", vec![input.id]);
        Ok(now)
    }

    pub fn draft(&self, id: i64) -> Result<Option<Draft>> {
        self.with_store(|s| Ok(s.get_draft(id)?))
    }

    pub fn drafts(&self) -> Result<Vec<Draft>> {
        self.with_store(|s| Ok(s.list_drafts()?))
    }

    pub fn delete_draft(&self, id: i64) -> Result<()> {
        let paths = self.with_store(|s| Ok(s.delete_draft(id)?))?;
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
        if let Some(dir) = self.draft_dir(id) {
            let _ = std::fs::remove_dir(dir);
        }
        self.changed("drafts", vec![id]);
        Ok(())
    }

    fn draft_dir(&self, draft_id: i64) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|d| d.join("drafts").join(draft_id.to_string()))
    }

    pub fn add_draft_file(&self, draft_id: i64, filename: &str, mime: &str, data: &[u8]) -> Result<DraftAttachment> {
        let dir = self.draft_dir(draft_id).ok_or_else(|| invalid("There is nowhere to keep files for a draft"))?;
        if i64::try_from(data.len()).unwrap_or(i64::MAX) > MAX_EMAIL_BYTES {
            return Err(invalid("Gmail accepts up to 25 MB of files in one email"));
        }
        std::fs::create_dir_all(&dir)?;
        let name = safe_filename(filename);
        let path = free_name(&dir, &name);
        let shown = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(name);
        std::fs::write(&path, data)?;
        let added = self.with_store(|s| {
            Ok(s.add_draft_attachment(
                draft_id,
                &shown,
                &clean_mime(mime),
                i64::try_from(data.len()).unwrap_or(i64::MAX),
                &path.to_string_lossy(),
            )?)
        });
        if added.is_err() {
            let _ = std::fs::remove_file(&path);
        }
        added
    }

    pub fn add_draft_file_base64(
        &self,
        draft_id: i64,
        filename: &str,
        mime: &str,
        data: &str,
    ) -> Result<DraftAttachment> {
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|_| invalid("That file could not be read"))?;
        self.add_draft_file(draft_id, filename, mime, &bytes)
    }

    pub fn remove_draft_file(&self, id: i64) -> Result<()> {
        let path = self.with_store(|s| Ok(s.remove_draft_attachment(id)?))?;
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    pub fn suggest_recipients(&self, query: &str) -> Result<Vec<RecipientSuggestion>> {
        self.with_store(|s| Ok(s.suggest_recipients(query, 8)?))
    }

    pub fn send_draft(&self, id: i64) -> Result<SendTicket> {
        let undo = self.with_store(|s| Ok(i64::from(s.compose_settings()?.undo_seconds) * 1000))?;
        self.send_draft_after(id, undo)
    }

    fn send_draft_after(&self, id: i64, undo_ms: i64) -> Result<SendTicket> {
        let now = self.now_ms();
        let ticket = self.with_store(|s| Ok(s.send_draft(id, now, undo_ms)?))?;
        self.changed("drafts", vec![id]);
        self.changed("conversations", vec![ticket.conversation_id]);
        self.changed("people", Vec::new());
        self.changed("messages", vec![ticket.message_id]);
        self.changed("nudges", Vec::new());
        self.wake_sync_in(undo_ms);
        Ok(ticket)
    }

    fn wake_sync_in(&self, ms: i64) {
        if ms <= 0 {
            self.wake_sync();
            return;
        }
        let (Some(runtime), weak) = (self.runtime.clone(), self.weak.clone()) else { return };
        runtime.spawn(async move {
            tokio::time::sleep(Duration::from_millis(u64::try_from(ms).unwrap_or(0) + 250)).await;
            if let Some(core) = weak.upgrade() {
                core.wake_sync();
            }
        });
    }

    pub fn unsend(&self, message_id: i64) -> Result<Draft> {
        let now = self.now_ms();
        let draft = self.with_store(|s| {
            let id = s.unsend(message_id, now)?;
            s.get_draft(id)?.ok_or(Error::Store(zsm_core::Error::NotFound("draft")))
        })?;
        self.changed("drafts", vec![draft.id]);
        self.changed("conversations", Vec::new());
        self.changed("people", Vec::new());
        self.changed("messages", Vec::new());
        self.changed("nudges", Vec::new());
        Ok(draft)
    }

    pub fn schedule_draft(&self, id: i64, send_at: Option<i64>) -> Result<()> {
        let now = self.now_ms();
        self.with_store(|s| Ok(s.schedule_draft(id, send_at, now)?))?;
        self.changed("drafts", vec![id]);
        Ok(())
    }

    pub fn nudges(&self) -> Result<Vec<Nudge>> {
        self.with_store(|s| Ok(s.waiting_nudges()?))
    }

    pub fn cancel_nudge(&self, id: i64) -> Result<()> {
        self.with_store(|s| Ok(s.cancel_nudge(id)?))?;
        self.changed("nudges", vec![id]);
        Ok(())
    }

    pub fn preview_routine_text(&self, text: &str) -> String {
        zsm_core::store::routines::expand(text, self.now_ms(), None, &self.timezone)
    }

    pub fn routines(&self) -> Result<Vec<Routine>> {
        self.with_store(|s| Ok(s.list_routines()?))
    }

    pub fn save_routine(&self, input: &RoutineInput) -> Result<i64> {
        let now = self.now_ms();
        let id = self.with_store(|s| Ok(s.save_routine(input, &self.timezone, now)?))?;
        self.changed("routines", vec![id]);
        Ok(id)
    }

    pub fn delete_routine(&self, id: i64) -> Result<()> {
        self.with_store(|s| Ok(s.delete_routine(id)?))?;
        self.changed("routines", vec![id]);
        self.changed("drafts", Vec::new());
        Ok(())
    }

    pub fn skip_routine(&self, id: i64, skip: bool) -> Result<()> {
        self.with_store(|s| Ok(s.set_routine_skip(id, skip)?))?;
        self.changed("routines", vec![id]);
        Ok(())
    }

    pub fn routine_runs(&self, id: i64) -> Result<Vec<RoutineRun>> {
        self.with_store(|s| Ok(s.routine_runs(id, 20)?))
    }

    pub fn run_routine(&self, id: i64) -> Result<RunOutcome> {
        let now = self.now_ms();
        let outcome = self.with_store(|s| Ok(s.run_routine(id, RunReason::Manual, &self.timezone, now)?))?;
        self.changed("routines", vec![id]);
        self.changed("drafts", Vec::new());
        Ok(outcome)
    }

    pub(crate) fn run_timed_writing(&self, now: i64) -> Result<()> {
        for id in self.with_store(|s| Ok(s.due_drafts(now)?))? {
            match self.send_draft_after(id, 0) {
                Ok(_) => {}
                Err(error) => {
                    let _ = self.with_store(|s| s.schedule_draft(id, None, now).map_err(Error::from));
                    self.changed("drafts", vec![id]);
                    let subject = self.draft(id).ok().flatten().map(|d| d.subject).unwrap_or_default();
                    self.announce(Reminder {
                        kind: NotificationKind::System,
                        title: "A scheduled email was not sent".into(),
                        body: format!(
                            "{} {}",
                            error.for_person(),
                            if subject.is_empty() { String::new() } else { format!("({subject})") }
                        )
                        .trim()
                        .to_string(),
                        reason: "You scheduled it, and it could not be sent".into(),
                        person_id: None,
                        conversation_id: None,
                        draft_id: Some(id),
                    })?;
                }
            }
        }

        for id in self.with_store(|s| Ok(s.due_routines(now)?))? {
            let outcome = match self.with_store(|s| Ok(s.run_routine(id, RunReason::Due, &self.timezone, now)?)) {
                Ok(outcome) => outcome,
                Err(error) => {
                    tracing::warn!(%error, routine = id, "a routine could not run");
                    continue;
                }
            };
            self.changed("routines", vec![id]);
            self.changed("drafts", Vec::new());
            let late = if outcome.late { " It ran late: the computer was off or asleep at its time." } else { "" };
            match outcome.result {
                RunResult::Skipped => {}
                RunResult::Sent => {
                    if let Some(ticket) = &outcome.ticket {
                        self.changed("conversations", vec![ticket.conversation_id]);
                        self.changed("people", Vec::new());
                        self.wake_sync();
                        self.announce(Reminder {
                            kind: NotificationKind::Routine,
                            title: format!("{} was sent", outcome.name),
                            body: "It went out by itself".into(),
                            reason: format!("Your routine “{}” sends by itself", outcome.name),
                            person_id: ticket.person_id,
                            conversation_id: Some(ticket.conversation_id),
                            draft_id: None,
                        })?;
                    }
                }
                RunResult::Prepared => {
                    let (title, body) = match &outcome.problem {
                        Some(problem) => {
                            (format!("{} is ready, but was not sent", outcome.name), format!("{problem}.{late}"))
                        }
                        None => (format!("{} is ready", outcome.name), format!("Look it over and send it.{late}")),
                    };
                    self.announce(Reminder {
                        kind: NotificationKind::Routine,
                        title,
                        body,
                        reason: format!("Your routine “{}” came due", outcome.name),
                        person_id: None,
                        conversation_id: None,
                        draft_id: outcome.draft_id,
                    })?;
                }
            }
        }

        let stale = self.with_store(|s| Ok(s.purge_untouched_drafts(now - 86_400_000)?))?;
        for path in stale {
            let _ = std::fs::remove_file(path);
        }

        let fired = self.with_store(|s| Ok(s.resolve_nudges(now)?))?;
        if !fired.is_empty() {
            self.changed("nudges", Vec::new());
        }
        for nudge in fired {
            let days = ((nudge.due_at - nudge.sent_at) / 86_400_000).max(1);
            self.announce(Reminder {
                kind: NotificationKind::Watch,
                title: format!("No reply from {} yet", nudge.who),
                body: nudge.subject,
                reason: format!(
                    "You asked to be reminded if nobody replied in {days} {}",
                    if days == 1 { "day" } else { "days" }
                ),
                person_id: nudge.person_id,
                conversation_id: Some(nudge.conversation_id),
                draft_id: None,
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use jiff::tz::TimeZone;
    use zsm_core::clock::FixedClock;
    use zsm_core::dto::PeopleFilter;
    use zsm_core::store::routines::{Recurrence, RecurrenceKind, RoutineMode};

    use super::*;
    use crate::events::{CoreEvent, EventSink};
    use crate::{Core, CoreConfig};

    const NOW: i64 = 1_795_082_400_000;
    const HOUR: i64 = 3_600_000;
    const DAY: i64 = 24 * HOUR;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<CoreEvent>>);

    impl EventSink for Recorder {
        fn emit(&self, event: CoreEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    impl Recorder {
        fn notices(&self) -> Vec<crate::notify::Notice> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .filter_map(|e| match e {
                    CoreEvent::Notification { notice, .. } => Some(notice.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    fn core() -> (Arc<Core>, Arc<Recorder>, Arc<FixedClock>, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("zsm-writing-{}-{}", std::process::id(), NOW.wrapping_add(rand_id())));
        let recorder = Arc::new(Recorder::default());
        let clock = Arc::new(FixedClock::new(NOW));
        let core = Core::new(CoreConfig {
            clock: clock.clone(),
            sink: recorder.clone(),
            timezone: TimeZone::UTC,
            data_dir: Some(dir.clone()),
            ..CoreConfig::in_memory()
        })
        .unwrap();
        core.start_demo(0).unwrap();
        recorder.0.lock().unwrap().clear();
        (core, recorder, clock, dir)
    }

    fn rand_id() -> i64 {
        i64::from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos())
    }

    fn write(core: &Core, to: &str, subject: &str, body: &str) -> Draft {
        let draft = core.new_draft(vec![Address::new(to, None)]).unwrap();
        core.save_draft(&DraftInput {
            id: draft.id,
            to: draft.to.clone(),
            cc: vec![],
            bcc: vec![],
            subject: subject.into(),
            body_html: body.into(),
            nudge_days: None,
        })
        .unwrap();
        core.draft(draft.id).unwrap().unwrap()
    }

    #[test]
    fn many_conversations_change_together_and_come_back_with_one_undo() {
        use crate::BulkAction;
        let (core, _, _, _) = core();
        let inbox = core.inbox(None, 100, 0).unwrap().rows;
        assert!(inbox.len() >= 4);
        let ids: Vec<i64> = inbox.iter().take(3).map(|r| r.conversation.id).collect();

        let done = core.bulk(&ids, BulkAction::Archive).unwrap();
        assert_eq!(done.label, "Archived 3 conversations");
        let after = core.inbox(None, 100, 0).unwrap().rows;
        assert!(ids.iter().all(|id| after.iter().all(|r| r.conversation.id != *id)));

        core.undo().unwrap();
        let back = core.inbox(None, 100, 0).unwrap().rows;
        assert!(ids.iter().all(|id| back.iter().any(|r| r.conversation.id == *id)), "one Undo restores all three");

        let read = core.bulk(&ids, BulkAction::Read).unwrap();
        assert!(read.label.starts_with("Marked"));
        assert_eq!(core.bulk(&ids, BulkAction::Read).unwrap().label, "Nothing to change", "already read");
        assert_eq!(core.bulk(&[], BulkAction::Hide).unwrap().label, "Nothing to change");
    }

    #[test]
    fn a_message_is_written_sent_and_taken_back() {
        let (core, events, _, _) = core();
        let draft = write(&core, "new.friend@elsewhere.example", "Hello", "<p>Nice to meet you.</p>");
        let ticket = core.send_draft(draft.id).unwrap();
        assert_eq!(ticket.undo_until, NOW + 10_000, "ten seconds to change your mind");
        assert!(core.draft(draft.id).unwrap().is_none());
        let friend = core.people(PeopleFilter::All, "new.friend").unwrap();
        assert_eq!(friend.len(), 1);
        assert_eq!(core.conversations(friend[0].id).unwrap()[0].subject, "Hello");
        let seen = events.0.lock().unwrap().clone();
        assert!(
            seen.contains(&CoreEvent::StoreChanged {
                entity: "conversations".into(),
                ids: vec![ticket.conversation_id]
            })
        );

        let back = core.unsend(ticket.message_id).unwrap();
        assert_eq!((back.subject.as_str(), back.to[0].email.as_str()), ("Hello", "new.friend@elsewhere.example"));
        assert!(core.people(PeopleFilter::All, "new.friend").unwrap().is_empty(), "the new person went with it");
        assert_eq!(core.drafts().unwrap().len(), 1);
    }

    #[test]
    fn the_undo_window_is_a_setting() {
        let (core, _, _, _) = core();
        core.save_compose_settings(&ComposeSettings { signature: String::new(), undo_seconds: 0 }).unwrap();
        let draft = write(&core, "a@elsewhere.example", "Now", "<p>Now.</p>");
        assert_eq!(core.send_draft(draft.id).unwrap().undo_until, NOW, "no window at all");
        assert!(core.save_compose_settings(&ComposeSettings { signature: String::new(), undo_seconds: 31 }).is_err());
    }

    #[test]
    fn files_are_kept_next_to_the_draft_and_cleaned_up_with_it() {
        let (core, _, _, dir) = core();
        let draft = write(&core, "files@elsewhere.example", "Files", "<p>Attached.</p>");
        let first = core.add_draft_file(draft.id, "../plan.pdf", "application/pdf", b"%PDF-1").unwrap();
        let second = core.add_draft_file(draft.id, "plan.pdf", "application/pdf", b"%PDF-2").unwrap();
        assert_eq!((first.filename.as_str(), second.filename.as_str()), ("plan.pdf", "plan (2).pdf"));
        let folder = dir.join("drafts").join(draft.id.to_string());
        assert_eq!(std::fs::read(folder.join("plan.pdf")).unwrap(), b"%PDF-1");
        assert_eq!(core.draft(draft.id).unwrap().unwrap().attachments.len(), 2);

        core.remove_draft_file(second.id).unwrap();
        assert!(!folder.join("plan (2).pdf").exists());
        let too_big = vec![0u8; usize::try_from(MAX_EMAIL_BYTES).unwrap() + 1];
        assert!(core.add_draft_file(draft.id, "big.bin", "", &too_big).unwrap_err().to_string().contains("25 MB"));

        let other = write(&core, "files@elsewhere.example", "Other", "<p>x</p>");
        core.add_draft_file(other.id, "x.txt", "text/plain", b"x").unwrap();
        let other_folder = dir.join("drafts").join(other.id.to_string());
        core.delete_draft(other.id).unwrap();
        assert!(!other_folder.exists());
        core.send_draft(draft.id).unwrap();
        assert!(folder.join("plan.pdf").exists(), "the sent message still needs its file until Gmail has it");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_scheduled_email_goes_out_when_its_time_comes_and_not_before() {
        let (core, events, clock, _) = core();
        let draft = write(&core, "later@elsewhere.example", "Later", "<p>See you then.</p>");
        core.schedule_draft(draft.id, Some(NOW + 3 * HOUR)).unwrap();
        clock.set(NOW + 2 * HOUR);
        core.tick().unwrap();
        assert!(core.draft(draft.id).unwrap().is_some(), "not yet");
        clock.set(NOW + 3 * HOUR);
        core.tick().unwrap();
        assert!(core.draft(draft.id).unwrap().is_none(), "sent");
        assert!(!core.people(PeopleFilter::All, "later").unwrap().is_empty(), "the recipient became a person");
        assert!(events.notices().is_empty(), "sending what you scheduled is not news");

        let broken = write(&core, "later@elsewhere.example", "Broken", "<p>Oops.</p>");
        core.schedule_draft(broken.id, Some(NOW + 4 * HOUR)).unwrap();
        core.save_draft(&DraftInput {
            id: broken.id,
            to: vec![Address::new("not an address", None)],
            cc: vec![],
            bcc: vec![],
            subject: "Broken".into(),
            body_html: "<p>Oops.</p>".into(),
            nudge_days: None,
        })
        .unwrap();
        clock.set(NOW + 4 * HOUR);
        core.tick().unwrap();
        let after = core.draft(broken.id).unwrap().unwrap();
        assert_eq!(after.send_at, None);
        let notices = events.notices();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].title, "A scheduled email was not sent");
        assert!(notices[0].body.contains("is not an email address"), "{}", notices[0].body);
        assert_eq!(notices[0].draft_id, Some(broken.id));
        clock.set(NOW + 5 * HOUR);
        core.tick().unwrap();
        assert_eq!(events.notices().len(), 1, "it does not try again every half minute");
    }

    #[test]
    fn a_routine_prepares_its_draft_on_time_and_says_so() {
        let (core, events, clock, _) = core();
        let id = core
            .save_routine(&RoutineInput {
                id: None,
                name: "Monday report".into(),
                enabled: true,
                to: vec![Address::new("boss@elsewhere.example", Some("Boss"))],
                cc: vec![],
                subject: "Report {{date}}".into(),
                body: "Numbers for {{weekday}}.".into(),
                recurrence: Recurrence { kind: RecurrenceKind::Daily, minutes: 9 * 60, days: vec![], day_of_month: 1 },
                mode: RoutineMode::Draft,
            })
            .unwrap();
        let due = core.routines().unwrap()[0].next_run_at.unwrap();
        clock.set(due - 1);
        core.tick().unwrap();
        assert!(events.notices().is_empty());
        clock.set(due + 60_000);
        core.tick().unwrap();
        let notices = events.notices();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].title, "Monday report is ready");
        let draft = core.draft(notices[0].draft_id.unwrap()).unwrap().unwrap();
        assert_eq!(draft.routine_id, Some(id));
        assert!(draft.subject.starts_with("Report "), "{}", draft.subject);
        let log = core.notification_log().unwrap();
        assert_eq!(
            (log[0].kind, log[0].shown, log[0].draft_id),
            (NotificationKind::Routine, true, notices[0].draft_id)
        );
        assert_eq!(log[0].reason, "Your routine “Monday report” came due");

        clock.set(due + 4 * DAY);
        core.tick().unwrap();
        let notices = events.notices();
        assert_eq!(notices.len(), 2, "once, not four times");
        assert!(notices[1].body.contains("ran late"), "{}", notices[1].body);
        assert_eq!(core.routines().unwrap()[0].streak, 0);
        assert_eq!(core.run_routine(id).unwrap().result, RunResult::Prepared, "and it can be run on demand");
        core.delete_routine(id).unwrap();
        assert!(core.drafts().unwrap().iter().all(|d| d.routine_id.is_none()), "drafts it made are yours now");
    }

    #[test]
    fn a_reminder_fires_when_nobody_answers_and_not_when_somebody_does() {
        let (core, events, clock, _) = core();
        let ask = |to: &str| {
            let draft = write(&core, to, "Quick question", "<p>Can you check?</p>");
            core.save_draft(&DraftInput {
                id: draft.id,
                to: draft.to.clone(),
                cc: vec![],
                bcc: vec![],
                subject: draft.subject.clone(),
                body_html: draft.body_html.clone(),
                nudge_days: Some(2),
            })
            .unwrap();
            core.send_draft(draft.id).unwrap()
        };
        let quiet = ask("quiet.one@elsewhere.example");
        assert_eq!(core.nudges().unwrap().len(), 1);
        clock.set(NOW + 2 * DAY + HOUR);
        core.tick().unwrap();
        let notices = events.notices();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].title, "No reply from Quiet One yet");
        assert_eq!(
            (notices[0].conversation_id, notices[0].body.as_str()),
            (Some(quiet.conversation_id), "Quick question")
        );
        assert!(core.nudges().unwrap().is_empty());
        let log = core.notification_log().unwrap();
        assert_eq!(log[0].reason, "You asked to be reminded if nobody replied in 2 days");
        core.tick().unwrap();
        assert_eq!(events.notices().len(), 1, "once");
    }

    #[test]
    fn file_names_cannot_carry_paths_or_odd_characters() {
        assert_eq!(safe_filename("plan.pdf"), "plan.pdf");
        assert_eq!(safe_filename("../../etc/passwd"), "passwd");
        assert_eq!(safe_filename("C:\\Users\\me\\report final.docx"), "report final.docx");
        assert_eq!(safe_filename("a<b>:c|d?.txt"), "a_b__c_d_.txt");
        assert_eq!(safe_filename("line\nbreak\r.txt"), "line_break_.txt");
        assert_eq!(safe_filename("...hidden"), "hidden");
        assert_eq!(safe_filename("   "), "file");
        assert_eq!(safe_filename(""), "file");
        assert_eq!(safe_filename("日本語.txt"), "日本語.txt");
        let long = format!("{}.pdf", "x".repeat(300));
        let short = safe_filename(&long);
        assert!(short.chars().count() <= 120 && short.ends_with(".pdf"), "{short}");
    }

    #[test]
    fn media_types_are_checked_and_names_are_kept_apart() {
        assert_eq!(clean_mime("Application/PDF"), "application/pdf");
        assert_eq!(clean_mime("image/svg+xml"), "image/svg+xml");
        assert_eq!(clean_mime(""), "application/octet-stream");
        assert_eq!(clean_mime("text/plain\r\nBcc: x@y.z"), "application/octet-stream");
        assert_eq!(clean_mime("a/b/c"), "application/octet-stream");
        let dir = std::env::temp_dir().join(format!("zsm-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plan.pdf"), b"1").unwrap();
        std::fs::write(dir.join("plan (2).pdf"), b"2").unwrap();
        assert_eq!(free_name(&dir, "plan.pdf"), dir.join("plan (3).pdf"));
        assert_eq!(free_name(&dir, "other.pdf"), dir.join("other.pdf"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

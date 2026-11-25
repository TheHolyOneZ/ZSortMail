use serde::{Deserialize, Serialize};
use zsm_core::dto::Category;
use zsm_core::extract::Extracted;
use zsm_core::store::ingest::BatchOutcome;
use zsm_core::store::notifications::{ArrivalInfo, NewNotification, NotificationKind, NotificationRecord};
use zsm_core::store::watches::{WatchHit, WatchPriority};

use crate::core::Core;
use crate::error::{Error, Result};
use crate::events::CoreEvent;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Notice {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub conversation_id: Option<i64>,
    pub person_id: Option<i64>,
    pub draft_id: Option<i64>,
    pub urgent: bool,
}

pub(crate) struct Reminder {
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    pub reason: String,
    pub person_id: Option<i64>,
    pub conversation_id: Option<i64>,
    pub draft_id: Option<i64>,
}

pub trait Platform: Send + Sync + 'static {
    fn notify(&self, notice: &Notice);

    fn test(&self, notice: &Notice) -> std::result::Result<(), String> {
        self.notify(notice);
        Ok(())
    }
}

pub struct NullPlatform;

impl Platform for NullPlatform {
    fn notify(&self, _notice: &Notice) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct QuietHours {
    pub from: u16,
    pub to: u16,
}

impl QuietHours {
    pub fn contains(&self, minute: u16) -> bool {
        match self.from.cmp(&self.to) {
            std::cmp::Ordering::Less => minute >= self.from && minute < self.to,
            std::cmp::Ordering::Greater => minute >= self.from || minute < self.to,
            std::cmp::Ordering::Equal => false,
        }
    }

    fn label(&self) -> String {
        format!("{:02}:{:02} to {:02}:{:02}", self.from / 60, self.from % 60, self.to / 60, self.to % 60)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub quiet: Option<QuietHours>,
    pub focus: bool,
    pub important: bool,
    pub everyone: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self { enabled: true, quiet: None, focus: false, important: false, everyone: false }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Trigger {
    Watch { name: String, once: bool, urgent: bool, extracted: Vec<Extracted> },
    Rule,
    Favourite,
    Important,
    Everyone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    Show(String),
    Hold(String),
    Ignore,
}

pub(crate) fn decide(settings: &NotificationSettings, minute: u16, who: &ArrivalInfo, trigger: &Trigger) -> Verdict {
    let quiet = settings.quiet.filter(|q| q.contains(minute));
    if !settings.enabled {
        return if matches!(trigger, Trigger::Important) && !settings.important {
            Verdict::Ignore
        } else {
            Verdict::Hold("Notifications are turned off".into())
        };
    }
    match trigger {
        Trigger::Watch { name, urgent, .. } => {
            if *urgent {
                return Verdict::Show(format!("You are waiting for “{name}” (urgent)"));
            }
            match quiet {
                Some(q) => Verdict::Hold(format!("Quiet hours, {}", q.label())),
                None => Verdict::Show(format!("You are waiting for “{name}”")),
            }
        }
        Trigger::Rule | Trigger::Favourite | Trigger::Important | Trigger::Everyone => {
            if matches!(trigger, Trigger::Important) && !settings.important {
                return Verdict::Ignore;
            }
            if who.hidden {
                return Verdict::Hold("It is hidden".into());
            }
            if who.muted {
                return Verdict::Hold(format!("{} is muted", who.from_name));
            }
            if settings.focus && !(who.vip || who.whitelisted) {
                return Verdict::Hold("Focus mode lets only favourites and always-allowed people through".into());
            }
            if let Some(q) = quiet {
                return Verdict::Hold(format!("Quiet hours, {}", q.label()));
            }
            Verdict::Show(match trigger {
                Trigger::Rule => "A rule asked to be told".into(),
                Trigger::Favourite => format!("{} is a favourite", who.from_name),
                Trigger::Everyone => format!("New mail from {}", who.from_name),
                _ => "Gmail marked it important".into(),
            })
        }
    }
}

fn kind_of(t: &Trigger) -> NotificationKind {
    match t {
        Trigger::Watch { .. } => NotificationKind::Watch,
        Trigger::Rule => NotificationKind::Rule,
        Trigger::Favourite => NotificationKind::Favourite,
        Trigger::Important => NotificationKind::Important,
        Trigger::Everyone => NotificationKind::Mail,
    }
}

fn why_quiet(settings: &NotificationSettings, who: &ArrivalInfo, category: Option<Category>) -> String {
    if !settings.enabled {
        return "Notifications are turned off".into();
    }
    if who.hidden {
        return "It is hidden".into();
    }
    if who.screened {
        return "A new sender, waiting for your decision in New senders".into();
    }
    if who.muted {
        return format!("{} is muted", who.from_name);
    }
    if who.important && !settings.important {
        return "Gmail marked it important, but that setting is off".into();
    }
    let only =
        if settings.everyone { "Only mail from people notifies" } else { "Only favourites, watches and rules notify" };
    match category {
        Some(Category::Newsletter) => format!("A newsletter. {only}"),
        Some(Category::Receipt) => format!("A receipt. {only}"),
        Some(Category::Automated) => format!("Automated mail. {only}"),
        _ => format!(
            "{} is not a favourite and no watch or rule matched. Every new message from people is off in Settings",
            who.from_name
        ),
    }
}

fn describe(trigger: &Trigger, who: &ArrivalInfo) -> (String, String) {
    match trigger {
        Trigger::Watch { name, once, extracted, .. } => {
            let title = if *once { format!("“{name}” has arrived") } else { format!("New mail for “{name}”") };
            let mut body = format!("{}: {}", who.from_name, who.subject);
            for e in extracted.iter().take(2) {
                let label = match e.kind.as_str() {
                    "tracking" => "Tracking",
                    "order" => "Order",
                    _ => "Reference",
                };
                body.push_str(&format!("\n{label} {}", e.value));
            }
            (title, body)
        }
        _ => (
            who.from_name.clone(),
            if who.snippet.is_empty() { who.subject.clone() } else { format!("{}\n{}", who.subject, who.snippet) },
        ),
    }
}

impl Core {
    pub fn notification_settings(&self) -> Result<NotificationSettings> {
        self.with_store(|s| {
            let quiet = s.setting("notify.quiet")?.and_then(|text| {
                let (from, to) = text.split_once('-')?;
                let minutes = |t: &str| -> Option<u16> {
                    let (h, m) = t.split_once(':')?;
                    Some(h.parse::<u16>().ok()? * 60 + m.parse::<u16>().ok()?)
                };
                Some(QuietHours { from: minutes(from)?, to: minutes(to)? })
            });
            Ok(NotificationSettings {
                enabled: s.setting("notify.enabled")?.as_deref() != Some("0"),
                quiet,
                focus: s.setting("notify.focus")?.as_deref() == Some("1"),
                important: s.setting("notify.important")?.as_deref() == Some("1"),
                everyone: s.setting("notify.everyone")?.as_deref() == Some("1"),
            })
        })
    }

    pub fn save_notification_settings(&self, settings: &NotificationSettings) -> Result<()> {
        self.with_store(|s| {
            s.set_setting("notify.enabled", if settings.enabled { "1" } else { "0" })?;
            s.set_setting("notify.focus", if settings.focus { "1" } else { "0" })?;
            s.set_setting("notify.important", if settings.important { "1" } else { "0" })?;
            s.set_setting("notify.everyone", if settings.everyone { "1" } else { "0" })?;
            match settings.quiet {
                Some(q) if q.from != q.to => s.set_setting(
                    "notify.quiet",
                    &format!("{:02}:{:02}-{:02}:{:02}", q.from / 60, q.from % 60, q.to / 60, q.to % 60),
                )?,
                _ => s.delete_setting("notify.quiet")?,
            }
            Ok(())
        })?;
        self.changed("notifications", Vec::new());
        Ok(())
    }

    pub fn notification_log(&self) -> Result<Vec<NotificationRecord>> {
        self.with_store(|s| Ok(s.list_notifications(500)?))
    }

    pub(crate) fn minute_of_day(&self) -> u16 {
        jiff::Timestamp::from_millisecond(self.now_ms())
            .map(|t| {
                let z = t.to_zoned_datetime(self.timezone.clone());
                u16::try_from(i32::from(z.hour()) * 60 + i32::from(z.minute())).unwrap_or(0)
            })
            .unwrap_or(0)
    }

    pub(crate) fn on_arrival(&self, account_id: i64, batch: &BatchOutcome) {
        let settings = self.notification_settings().unwrap_or_default();
        let minute = self.minute_of_day();
        let now = self.now_ms();
        let mut touched = false;
        for out in batch.messages.iter().filter(|m| m.inserted && m.sender_person_id.is_some()) {
            let Ok(Some(who)) = self.with_store(|s| Ok(s.arrival_info(out.message_id)?)) else { continue };
            let mut triggers: Vec<Trigger> = out
                .automation
                .watch_hits
                .iter()
                .map(|h: &WatchHit| {
                    let once = self
                        .with_store(|s| {
                            Ok(s.get_watch(h.watch_id)?.map(|w| w.mode == zsm_core::store::watches::WatchMode::Once))
                        })
                        .ok()
                        .flatten()
                        .unwrap_or(true);
                    Trigger::Watch {
                        name: h.watch_name.clone(),
                        once,
                        urgent: h.priority == WatchPriority::Urgent,
                        extracted: h.extracted.clone(),
                    }
                })
                .collect();
            triggers.sort_by_key(|t| !matches!(t, Trigger::Watch { urgent: true, .. }));
            if out.automation.notify {
                triggers.push(Trigger::Rule);
            }
            if who.vip {
                triggers.push(Trigger::Favourite);
            }
            if who.important {
                triggers.push(Trigger::Important);
            }
            if settings.everyone && out.category == Some(Category::Person) && !who.screened {
                triggers.push(Trigger::Everyone);
            }

            let mut held: Option<(&Trigger, String)> = None;
            let mut chosen: Option<(&Trigger, String)> = None;
            for t in &triggers {
                match decide(&settings, minute, &who, t) {
                    Verdict::Show(reason) => {
                        chosen = Some((t, reason));
                        break;
                    }
                    Verdict::Hold(reason) => {
                        held.get_or_insert((t, reason));
                    }
                    Verdict::Ignore => {}
                }
            }
            let (trigger, reason, shown) = match (chosen, held) {
                (Some((t, r)), _) => (t, r, true),
                (None, Some((t, r))) => (t, r, false),
                (None, None) => {
                    let reason = why_quiet(&settings, &who, out.category);
                    let logged = self.with_store(|s| {
                        Ok(s.log_notification(&NewNotification {
                            at: now,
                            kind: NotificationKind::Mail,
                            title: who.from_name.clone(),
                            body: who.subject.clone(),
                            reason,
                            person_id: who.person_id,
                            conversation_id: Some(who.conversation_id),
                            watch_id: None,
                            draft_id: None,
                            urgent: false,
                            shown: false,
                        })?)
                    });
                    touched |= logged.is_ok();
                    continue;
                }
            };
            let (title, body) = describe(trigger, &who);
            let urgent = matches!(trigger, Trigger::Watch { urgent: true, .. });
            let watch_id = match trigger {
                Trigger::Watch { .. } => out.automation.watch_hits.first().map(|h| h.watch_id),
                _ => None,
            };
            let logged = self.with_store(|s| {
                Ok(s.log_notification(&NewNotification {
                    at: now,
                    kind: kind_of(trigger),
                    title: title.clone(),
                    body: body.clone(),
                    reason: reason.clone(),
                    person_id: who.person_id,
                    conversation_id: Some(who.conversation_id),
                    watch_id,
                    draft_id: None,
                    urgent,
                    shown,
                })?)
            });
            let Ok(id) = logged else { continue };
            touched = true;
            if shown {
                let notice = Notice {
                    id,
                    title,
                    body,
                    conversation_id: Some(who.conversation_id),
                    person_id: who.person_id,
                    draft_id: None,
                    urgent,
                };
                self.emit(CoreEvent::Notification { notice: notice.clone(), reason });
                self.platform.notify(&notice);
            }
        }
        let _ = account_id;
        if touched {
            self.changed("notifications", Vec::new());
        }
        if batch.messages.iter().any(|m| !m.automation.watch_hits.is_empty()) {
            self.changed("watches", Vec::new());
        }
    }

    pub(crate) fn announce(&self, r: Reminder) -> Result<()> {
        let settings = self.notification_settings().unwrap_or_default();
        let (shown, reason) = if !settings.enabled {
            (false, "Notifications are turned off".to_string())
        } else if let Some(q) = settings.quiet.filter(|q| q.contains(self.minute_of_day())) {
            (false, format!("Quiet hours, {}", q.label()))
        } else {
            (true, r.reason.clone())
        };
        let id = self.with_store(|s| {
            Ok(s.log_notification(&NewNotification {
                at: self.now_ms(),
                kind: r.kind,
                title: r.title.clone(),
                body: r.body.clone(),
                reason: reason.clone(),
                person_id: r.person_id,
                conversation_id: r.conversation_id,
                watch_id: None,
                draft_id: r.draft_id,
                urgent: false,
                shown,
            })?)
        })?;
        if shown {
            let notice = Notice {
                id,
                title: r.title,
                body: r.body,
                conversation_id: r.conversation_id,
                person_id: r.person_id,
                draft_id: r.draft_id,
                urgent: false,
            };
            self.emit(CoreEvent::Notification { notice: notice.clone(), reason });
            self.platform.notify(&notice);
        }
        self.changed("notifications", Vec::new());
        Ok(())
    }

    pub fn send_test_notification(&self) -> Result<()> {
        let notice = Notice {
            id: 0,
            title: "ZSortMail".into(),
            body: "If you can read this, notifications reach your desktop.".into(),
            conversation_id: None,
            person_id: None,
            draft_id: None,
            urgent: true,
        };
        let sent = self.platform.test(&notice);
        let now = self.now_ms();
        self.with_store(|s| {
            Ok(s.log_notification(&NewNotification {
                at: now,
                kind: NotificationKind::System,
                title: notice.title.clone(),
                body: notice.body.clone(),
                reason: match &sent {
                    Ok(()) => "You asked for a test".into(),
                    Err(err) => format!("You asked for a test, and the system would not show it: {err}"),
                },
                person_id: None,
                conversation_id: None,
                watch_id: None,
                draft_id: None,
                urgent: false,
                shown: sent.is_ok(),
            })?)
        })?;
        self.changed("notifications", Vec::new());
        sent.map_err(|err| Error::Io(std::io::Error::other(format!("The system would not show a notification: {err}"))))
    }

    pub fn open_notification(&self, id: i64) -> Result<Option<i64>> {
        let now = self.now_ms();
        let conversation = self.with_store(|s| Ok(s.mark_notification_opened(id, now)?))?;
        if let Some(c) = conversation {
            self.with_store(|s| Ok(s.mark_hits_seen(c, now)?))?;
        }
        self.changed("notifications", vec![id]);
        self.changed("watches", Vec::new());
        Ok(conversation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who() -> ArrivalInfo {
        ArrivalInfo {
            subject: "Out for delivery".into(),
            snippet: "Your driver is 6 stops away".into(),
            conversation_id: 7,
            person_id: Some(3),
            from_name: "DHL Parcel".into(),
            vip: false,
            muted: false,
            whitelisted: false,
            hidden: false,
            important: false,
            screened: false,
        }
    }

    fn watch(urgent: bool) -> Trigger {
        Trigger::Watch { name: "xbox 394".into(), once: true, urgent, extracted: vec![] }
    }

    const NIGHT: u16 = 23 * 60;
    const NOON: u16 = 12 * 60;

    fn quiet() -> NotificationSettings {
        NotificationSettings { quiet: Some(QuietHours { from: 22 * 60, to: 7 * 60 }), ..Default::default() }
    }

    #[test]
    fn quiet_hours_wrap_midnight_and_an_empty_range_is_never() {
        let q = QuietHours { from: 22 * 60, to: 7 * 60 };
        assert!(q.contains(23 * 60) && q.contains(0) && q.contains(6 * 60 + 59));
        assert!(!q.contains(7 * 60) && !q.contains(12 * 60) && !q.contains(21 * 60 + 59));
        let day = QuietHours { from: 9 * 60, to: 17 * 60 };
        assert!(day.contains(12 * 60) && !day.contains(18 * 60));
        assert!(!QuietHours { from: 60, to: 60 }.contains(60));
    }

    #[test]
    fn mail_from_people_notifies_only_when_asked_and_respects_muting_and_quiet_hours() {
        let everyone = NotificationSettings { everyone: true, ..Default::default() };
        assert!(
            matches!(decide(&everyone, NOON, &who(), &Trigger::Everyone), Verdict::Show(r) if r == "New mail from DHL Parcel")
        );
        let muted = ArrivalInfo { muted: true, ..who() };
        assert!(matches!(decide(&everyone, NOON, &muted, &Trigger::Everyone), Verdict::Hold(r) if r.contains("muted")));
        let q = NotificationSettings { everyone: true, ..quiet() };
        assert!(matches!(decide(&q, NIGHT, &who(), &Trigger::Everyone), Verdict::Hold(r) if r.contains("Quiet hours")));
    }

    #[test]
    fn a_message_that_did_not_notify_says_why() {
        let off = NotificationSettings::default();
        let on = NotificationSettings { everyone: true, ..Default::default() };
        let plain = why_quiet(&off, &who(), Some(Category::Person));
        assert!(plain.contains("not a favourite") && plain.contains("off in Settings"), "{plain}");
        assert_eq!(
            why_quiet(&off, &who(), Some(Category::Newsletter)),
            "A newsletter. Only favourites, watches and rules notify"
        );
        assert_eq!(why_quiet(&on, &who(), Some(Category::Receipt)), "A receipt. Only mail from people notifies");
        let new_sender = ArrivalInfo { screened: true, ..who() };
        assert!(why_quiet(&on, &new_sender, Some(Category::Person)).contains("New senders"));
        let hidden = ArrivalInfo { hidden: true, ..who() };
        assert_eq!(why_quiet(&on, &hidden, Some(Category::Person)), "It is hidden");
        let important = ArrivalInfo { important: true, ..who() };
        assert!(why_quiet(&off, &important, Some(Category::Newsletter)).contains("that setting is off"));
        let disabled = NotificationSettings { enabled: false, ..Default::default() };
        assert_eq!(why_quiet(&disabled, &who(), Some(Category::Person)), "Notifications are turned off");
    }

    #[test]
    fn a_watch_notifies_and_only_an_urgent_one_ignores_quiet_hours() {
        let s = NotificationSettings::default();
        assert_eq!(decide(&s, NOON, &who(), &watch(false)), Verdict::Show("You are waiting for “xbox 394”".into()));
        assert!(
            matches!(decide(&quiet(), NIGHT, &who(), &watch(false)), Verdict::Hold(r) if r.contains("Quiet hours, 22:00 to 07:00"))
        );
        assert!(matches!(decide(&quiet(), NIGHT, &who(), &watch(true)), Verdict::Show(r) if r.contains("urgent")));
        assert!(matches!(decide(&quiet(), NOON, &who(), &watch(false)), Verdict::Show(_)));
    }

    #[test]
    fn a_watch_beats_hidden_muted_and_focus_because_you_asked_for_it() {
        let hidden_muted = ArrivalInfo { hidden: true, muted: true, ..who() };
        let focus = NotificationSettings { focus: true, ..Default::default() };
        assert!(matches!(decide(&focus, NOON, &hidden_muted, &watch(false)), Verdict::Show(_)));
    }

    #[test]
    fn favourites_and_rules_notify_unless_muted_hidden_quiet_or_out_of_focus() {
        let s = NotificationSettings::default();
        let fav = ArrivalInfo { vip: true, ..who() };
        assert_eq!(decide(&s, NOON, &fav, &Trigger::Favourite), Verdict::Show("DHL Parcel is a favourite".into()));
        assert!(
            matches!(decide(&s, NOON, &ArrivalInfo { muted: true, ..fav.clone() }, &Trigger::Favourite), Verdict::Hold(r) if r == "DHL Parcel is muted")
        );
        assert!(
            matches!(decide(&s, NOON, &ArrivalInfo { hidden: true, ..fav.clone() }, &Trigger::Favourite), Verdict::Hold(r) if r == "It is hidden")
        );
        assert!(matches!(decide(&quiet(), NIGHT, &fav, &Trigger::Favourite), Verdict::Hold(_)));
        assert!(matches!(decide(&s, NOON, &who(), &Trigger::Rule), Verdict::Show(r) if r.contains("rule")));

        let focus = NotificationSettings { focus: true, ..Default::default() };
        assert!(
            matches!(decide(&focus, NOON, &who(), &Trigger::Rule), Verdict::Hold(r) if r.starts_with("Focus mode"))
        );
        assert!(
            matches!(decide(&focus, NOON, &fav, &Trigger::Favourite), Verdict::Show(_)),
            "a favourite gets through Focus"
        );
        assert!(matches!(
            decide(&focus, NOON, &ArrivalInfo { whitelisted: true, ..who() }, &Trigger::Rule),
            Verdict::Show(_)
        ));
    }

    #[test]
    fn gmails_important_marker_is_ignored_unless_turned_on() {
        let on = NotificationSettings { important: true, ..Default::default() };
        assert_eq!(decide(&NotificationSettings::default(), NOON, &who(), &Trigger::Important), Verdict::Ignore);
        assert_eq!(decide(&on, NOON, &who(), &Trigger::Important), Verdict::Show("Gmail marked it important".into()));
    }

    #[test]
    fn turning_notifications_off_holds_everything_back_with_a_reason() {
        let off = NotificationSettings { enabled: false, ..Default::default() };
        assert!(
            matches!(decide(&off, NOON, &who(), &watch(true)), Verdict::Hold(r) if r == "Notifications are turned off")
        );
        assert_eq!(
            decide(&off, NOON, &who(), &Trigger::Important),
            Verdict::Ignore,
            "never asked for, so never logged"
        );
    }
}

use std::sync::LazyLock;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use regex::Regex;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Store;
use super::drafts::{DraftMode, SendTicket};
use super::ingest::Address;
use crate::error::{Error, Result};

const LATE_AFTER_MS: i64 = 15 * 60 * 1000;
const SEND_UNTIL_LATE_MS: i64 = 2 * 60 * 60 * 1000;

pub const VARIABLES: [(&str, &str); 7] = [
    ("date", "the date it runs, like Mon, Nov 23"),
    ("weekday", "the day of the week, like Monday"),
    ("day", "the day of the month, like 23"),
    ("month", "the month, like November"),
    ("year", "the year, like 2026"),
    ("week", "the week number of the year"),
    ("last_sent", "the date it last ran, or “never”"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceKind {
    Daily,
    Weekdays,
    Weekly,
    Monthly,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Recurrence {
    pub kind: RecurrenceKind,
    pub minutes: u32,
    pub days: Vec<u8>,
    pub day_of_month: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RoutineMode {
    Draft,
    Send,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
    Prepared,
    Sent,
    Skipped,
}

impl RunResult {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Sent => "sent",
            Self::Skipped => "skipped",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "sent" => Self::Sent,
            "skipped" => Self::Skipped,
            _ => Self::Prepared,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Routine {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub subject: String,
    pub body: String,
    pub recurrence: Recurrence,
    pub mode: RoutineMode,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub streak: u32,
    pub skip_next: bool,
    pub last_result: Option<RunResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RoutineInput {
    pub id: Option<i64>,
    pub name: String,
    pub enabled: bool,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub subject: String,
    pub body: String,
    pub recurrence: Recurrence,
    pub mode: RoutineMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RoutineRun {
    pub due_at: i64,
    pub ran_at: i64,
    pub result: RunResult,
    pub late: bool,
    pub draft_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RunOutcome {
    pub routine_id: i64,
    pub name: String,
    pub result: RunResult,
    pub late: bool,
    pub draft_id: Option<i64>,
    pub ticket: Option<SendTicket>,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunReason {
    Due,
    Manual,
}

fn matches(rec: &Recurrence, date: Date) -> bool {
    let weekday = u8::try_from(date.weekday().to_monday_one_offset()).unwrap_or(1);
    match rec.kind {
        RecurrenceKind::Daily => true,
        RecurrenceKind::Weekdays => weekday <= 5,
        RecurrenceKind::Weekly => {
            if rec.days.is_empty() {
                weekday == 1
            } else {
                rec.days.contains(&weekday)
            }
        }
        RecurrenceKind::Monthly => {
            let last = u8::try_from(date.days_in_month()).unwrap_or(28);
            u8::try_from(date.day()).unwrap_or(1) == rec.day_of_month.clamp(1, 31).min(last)
        }
        RecurrenceKind::Manual => false,
    }
}

pub fn next_run(rec: &Recurrence, after: i64, tz: &TimeZone) -> Option<i64> {
    if rec.kind == RecurrenceKind::Manual {
        return None;
    }
    let minutes = rec.minutes.min(1439);
    let (hour, minute) = (i8::try_from(minutes / 60).ok()?, i8::try_from(minutes % 60).ok()?);
    let mut date = jiff::Timestamp::from_millisecond(after).ok()?.to_zoned_datetime(tz.clone()).date();
    for _ in 0..800 {
        if matches(rec, date)
            && let Ok(z) = date.at(hour, minute, 0, 0).to_zoned_datetime(tz.clone())
            && z.timestamp().as_millisecond() > after
        {
            return Some(z.timestamp().as_millisecond());
        }
        date = date.tomorrow().ok()?;
    }
    None
}

static VARIABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{\s*([A-Za-z_]+)\s*\}\}").expect("valid regex"));

pub fn unknown_variables(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for caps in VARIABLE.captures_iter(text) {
        let name = caps[1].to_lowercase();
        if !VARIABLES.iter().any(|(v, _)| *v == name) && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

pub fn expand(text: &str, now: i64, last_run: Option<i64>, tz: &TimeZone) -> String {
    let at = |ms: i64| jiff::Timestamp::from_millisecond(ms).ok().map(|t| t.to_zoned_datetime(tz.clone()));
    let Some(z) = at(now) else { return text.to_string() };
    VARIABLE
        .replace_all(text, |caps: &regex::Captures<'_>| match caps[1].to_lowercase().as_str() {
            "date" => z.strftime("%a, %b %-d").to_string(),
            "weekday" => z.strftime("%A").to_string(),
            "day" => z.strftime("%-d").to_string(),
            "month" => z.strftime("%B").to_string(),
            "year" => z.strftime("%Y").to_string(),
            "week" => z.strftime("%V").to_string().trim_start_matches('0').to_string(),
            "last_sent" => last_run
                .and_then(at)
                .map(|l| l.strftime("%a, %b %-d").to_string())
                .unwrap_or_else(|| "never".to_string()),
            _ => caps[0].to_string(),
        })
        .into_owned()
}

fn valid(rec: &Recurrence) -> Result<()> {
    if rec.minutes >= 1440 {
        return Err(Error::Invalid("Pick a time of day".into()));
    }
    if rec.days.iter().any(|d| !(1..=7).contains(d)) {
        return Err(Error::Invalid("Days run from Monday to Sunday".into()));
    }
    if rec.kind == RecurrenceKind::Weekly && rec.days.is_empty() {
        return Err(Error::Invalid("Pick at least one day of the week".into()));
    }
    if rec.kind == RecurrenceKind::Monthly && !(1..=31).contains(&rec.day_of_month) {
        return Err(Error::Invalid("Pick a day of the month from 1 to 31".into()));
    }
    Ok(())
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Routine> {
    let addrs = |s: String| serde_json::from_str(&s).unwrap_or_default();
    let recurrence: String = r.get(9)?;
    Ok(Routine {
        id: r.get(0)?,
        name: r.get(1)?,
        enabled: r.get(2)?,
        to: addrs(r.get(3)?),
        cc: addrs(r.get(4)?),
        subject: r.get(5)?,
        body: r.get(6)?,
        recurrence: serde_json::from_str(&recurrence).unwrap_or(Recurrence {
            kind: RecurrenceKind::Manual,
            minutes: 540,
            days: Vec::new(),
            day_of_month: 1,
        }),
        mode: if r.get::<_, String>(10)? == "send" { RoutineMode::Send } else { RoutineMode::Draft },
        next_run_at: r.get(11)?,
        last_run_at: r.get(12)?,
        streak: u32::try_from(r.get::<_, i64>(13)?).unwrap_or(0),
        skip_next: r.get(14)?,
        last_result: r.get::<_, Option<String>>(15)?.map(|s| RunResult::parse(&s)),
    })
}

const COLUMNS: &str =
    "r.id, r.name, r.enabled, r.to_addresses, r.cc_addresses, r.subject, r.body, r.created_at, r.account_id,
    r.recurrence, r.mode, r.next_run_at, r.last_run_at, r.streak, r.skip_next,
    (SELECT result FROM routine_runs u WHERE u.routine_id = r.id ORDER BY u.ran_at DESC, u.id DESC LIMIT 1)";

impl Store {
    pub fn list_routines(&self) -> Result<Vec<Routine>> {
        let scope = match self.account_scope() {
            Some(id) => format!(" WHERE (r.account_id IS NULL OR r.account_id = {id})"),
            None => String::new(),
        };
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLUMNS} FROM routines r{scope} ORDER BY r.enabled DESC, r.next_run_at IS NULL, r.next_run_at, r.name"
        ))?;
        let rows = stmt.query_map([], row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn routine(&self, id: i64) -> Result<Option<Routine>> {
        Ok(self.conn().query_row(&format!("SELECT {COLUMNS} FROM routines r WHERE r.id = ?1"), [id], row).optional()?)
    }

    pub fn save_routine(&mut self, input: &RoutineInput, tz: &TimeZone, now: i64) -> Result<i64> {
        valid(&input.recurrence)?;
        let subject = input.subject.trim();
        let body = input.body.trim();
        if subject.is_empty() {
            return Err(Error::Invalid("Give the email a subject".into()));
        }
        if body.is_empty() {
            return Err(Error::Invalid("Write what the email says".into()));
        }
        let unknown: Vec<String> = unknown_variables(subject).into_iter().chain(unknown_variables(body)).collect();
        if let Some(first) = unknown.first() {
            let names: Vec<&str> = VARIABLES.iter().map(|(v, _)| *v).collect();
            return Err(Error::Invalid(format!("{{{{{first}}}}} is not a variable. Use {}", names.join(", "))));
        }
        let recipients: Vec<&Address> = input.to.iter().chain(input.cc.iter()).collect();
        if let Some(bad) = recipients.iter().find(|a| !super::drafts::valid_email(&a.email)) {
            return Err(Error::Invalid(format!("“{}” is not an email address", bad.email)));
        }
        if input.mode == RoutineMode::Send && input.to.is_empty() {
            return Err(Error::Invalid("A routine that sends by itself needs someone to send to".into()));
        }
        let name = if input.name.trim().is_empty() { subject.to_string() } else { input.name.trim().to_string() };
        let recurrence = serde_json::to_string(&input.recurrence).unwrap_or_default();
        let mode = if input.mode == RoutineMode::Send { "send" } else { "draft" };
        let to = serde_json::to_string(&input.to).unwrap_or_else(|_| "[]".into());
        let cc = serde_json::to_string(&input.cc).unwrap_or_else(|_| "[]".into());

        match input.id {
            Some(id) => {
                let before = self.routine(id)?.ok_or(Error::NotFound("routine"))?;
                let reschedule = before.recurrence != input.recurrence || (!before.enabled && input.enabled);
                let next = if !input.enabled || input.recurrence.kind == RecurrenceKind::Manual {
                    None
                } else if reschedule {
                    next_run(&input.recurrence, now, tz)
                } else {
                    before.next_run_at
                };
                self.conn().execute(
                    "UPDATE routines SET name = ?2, enabled = ?3, to_addresses = ?4, cc_addresses = ?5, subject = ?6, body = ?7,
                       recurrence = ?8, mode = ?9, next_run_at = ?10, skip_next = CASE WHEN ?11 THEN 0 ELSE skip_next END
                     WHERE id = ?1",
                    params![id, name, input.enabled, to, cc, subject, body, recurrence, mode, next, reschedule],
                )?;
                Ok(id)
            }
            None => {
                let next = if input.enabled { next_run(&input.recurrence, now, tz) } else { None };
                self.conn().execute(
                    "INSERT INTO routines (name, enabled, to_addresses, cc_addresses, subject, body, recurrence, mode, next_run_at, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![name, input.enabled, to, cc, subject, body, recurrence, mode, next, now],
                )?;
                Ok(self.conn().last_insert_rowid())
            }
        }
    }

    pub fn delete_routine(&mut self, id: i64) -> Result<()> {
        self.conn().execute("UPDATE drafts SET routine_id = NULL WHERE routine_id = ?1", [id])?;
        if self.conn().execute("DELETE FROM routines WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("routine"));
        }
        Ok(())
    }

    pub fn set_routine_skip(&mut self, id: i64, skip: bool) -> Result<()> {
        if self.conn().execute("UPDATE routines SET skip_next = ?2 WHERE id = ?1", params![id, skip])? == 0 {
            return Err(Error::NotFound("routine"));
        }
        Ok(())
    }

    pub fn due_routines(&self, now: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT id FROM routines WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= ?1 ORDER BY next_run_at, id",
        )?;
        Ok(stmt.query_map([now], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?)
    }

    pub fn routine_runs(&self, id: i64, limit: usize) -> Result<Vec<RoutineRun>> {
        let mut stmt = self.conn().prepare_cached(
            "SELECT due_at, ran_at, result, late, draft_id FROM routine_runs WHERE routine_id = ?1 ORDER BY ran_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![id, i64::try_from(limit).unwrap_or(20)], |r| {
            Ok(RoutineRun {
                due_at: r.get(0)?,
                ran_at: r.get(1)?,
                result: RunResult::parse(&r.get::<_, String>(2)?),
                late: r.get(3)?,
                draft_id: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn run_routine(&mut self, id: i64, reason: RunReason, tz: &TimeZone, now: i64) -> Result<RunOutcome> {
        let routine = self.routine(id)?.ok_or(Error::NotFound("routine"))?;
        let scheduled = reason == RunReason::Due;
        let due_at = if scheduled { routine.next_run_at.unwrap_or(now) } else { now };
        let late_by = (now - due_at).max(0);
        let late = scheduled && late_by > LATE_AFTER_MS;
        let after = if scheduled { next_run(&routine.recurrence, now, tz) } else { routine.next_run_at };

        let mut outcome = RunOutcome {
            routine_id: id,
            name: routine.name.clone(),
            result: RunResult::Prepared,
            late,
            draft_id: None,
            ticket: None,
            problem: None,
        };

        if scheduled && routine.skip_next {
            outcome.result = RunResult::Skipped;
            self.record_run(id, due_at, now, &outcome, after, routine.streak, false)?;
            return Ok(outcome);
        }

        let account_id = self.default_account()?;
        let subject = expand(&routine.subject, now, routine.last_run_at, tz);
        let body = expand(&routine.body, now, routine.last_run_at, tz);
        let signature = self.compose_settings()?.signature;
        let mut html = super::drafts::paragraphs(&body);
        if !signature.trim().is_empty() {
            html.push_str(&format!("<p>-- <br>{}</p>", super::drafts::escape(signature.trim()).replace('\n', "<br>")));
        }
        let draft_id = self.insert_routine_draft(account_id, &routine, &subject, &html, now)?;
        outcome.draft_id = Some(draft_id);

        let may_send = scheduled && routine.mode == RoutineMode::Send && late_by <= SEND_UNTIL_LATE_MS;
        if may_send {
            match self.send_draft(draft_id, now, 0) {
                Ok(ticket) => {
                    outcome.result = RunResult::Sent;
                    outcome.draft_id = None;
                    outcome.ticket = Some(ticket);
                }
                Err(error) => outcome.problem = Some(error.to_string()),
            }
        } else if scheduled && routine.mode == RoutineMode::Send {
            outcome.problem = Some("It ran too long after its time to send by itself".into());
        }

        let streak = if !scheduled {
            routine.streak
        } else if late {
            0
        } else {
            routine.streak + 1
        };
        self.record_run(id, due_at, now, &outcome, after, streak, !scheduled)?;
        Ok(outcome)
    }

    fn insert_routine_draft(
        &mut self,
        account_id: i64,
        routine: &Routine,
        subject: &str,
        html: &str,
        now: i64,
    ) -> Result<i64> {
        let id = self.insert_draft(account_id, None, DraftMode::New, &routine.to, &routine.cc, subject, html, now)?;
        self.conn().execute("UPDATE drafts SET routine_id = ?2 WHERE id = ?1", params![id, routine.id])?;
        self.keep_draft(id)?;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    fn record_run(
        &mut self,
        id: i64,
        due_at: i64,
        now: i64,
        outcome: &RunOutcome,
        next: Option<i64>,
        streak: u32,
        manual: bool,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT INTO routine_runs (routine_id, due_at, ran_at, result, late, draft_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, due_at, now, outcome.result.as_str(), outcome.late, outcome.draft_id],
        )?;
        if manual {
            self.conn().execute("UPDATE routines SET last_run_at = ?2 WHERE id = ?1", params![id, now])?;
        } else if outcome.result == RunResult::Skipped {
            self.conn()
                .execute("UPDATE routines SET next_run_at = ?2, skip_next = 0 WHERE id = ?1", params![id, next])?;
        } else {
            self.conn().execute(
                "UPDATE routines SET last_run_at = ?2, next_run_at = ?3, streak = ?4 WHERE id = ?1",
                params![id, now, next, streak],
            )?;
        }
        self.conn().execute(
            "DELETE FROM routine_runs WHERE routine_id = ?1 AND id NOT IN
               (SELECT id FROM routine_runs WHERE routine_id = ?1 ORDER BY ran_at DESC, id DESC LIMIT 60)",
            [id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::rules::tests::demo;

    fn ms(y: i16, m: i8, d: i8, h: i8, min: i8) -> i64 {
        jiff::civil::date(y, m, d)
            .at(h, min, 0, 0)
            .to_zoned_datetime(TimeZone::UTC)
            .unwrap()
            .timestamp()
            .as_millisecond()
    }

    fn rec(kind: RecurrenceKind, minutes: u32, days: &[u8], dom: u8) -> Recurrence {
        Recurrence { kind, minutes, days: days.to_vec(), day_of_month: dom }
    }

    fn input(name: &str, recurrence: Recurrence) -> RoutineInput {
        RoutineInput {
            id: None,
            name: name.into(),
            enabled: true,
            to: vec![Address::new("boss@example.com", Some("Boss"))],
            cc: vec![],
            subject: "Weekly report, {{date}}".into(),
            body: "Hello,\n\nHere is the report for {{weekday}} week {{week}}.\nLast one: {{last_sent}}".into(),
            recurrence,
            mode: RoutineMode::Draft,
        }
    }

    #[test]
    fn schedules_find_the_next_time_after_now() {
        let utc = TimeZone::UTC;
        let now = ms(2026, 11, 18, 10, 0);
        let daily = rec(RecurrenceKind::Daily, 9 * 60, &[], 1);
        assert_eq!(next_run(&daily, now, &utc), Some(ms(2026, 11, 19, 9, 0)), "today's 09:00 has passed");
        assert_eq!(next_run(&rec(RecurrenceKind::Daily, 11 * 60, &[], 1), now, &utc), Some(ms(2026, 11, 18, 11, 0)));
        assert_eq!(next_run(&daily, ms(2026, 11, 19, 9, 0), &utc), Some(ms(2026, 11, 20, 9, 0)), "strictly after");

        let weekdays = rec(RecurrenceKind::Weekdays, 9 * 60, &[], 1);
        assert_eq!(
            next_run(&weekdays, ms(2026, 11, 20, 10, 0), &utc),
            Some(ms(2026, 11, 23, 9, 0)),
            "Friday to Monday"
        );
        let weekly = rec(RecurrenceKind::Weekly, 8 * 60 + 30, &[1, 4], 1);
        assert_eq!(next_run(&weekly, now, &utc), Some(ms(2026, 11, 19, 8, 30)), "Thursday");
        assert_eq!(
            next_run(&rec(RecurrenceKind::Weekly, 540, &[], 1), now, &utc),
            Some(ms(2026, 11, 23, 9, 0)),
            "no day means Monday"
        );

        let monthly = rec(RecurrenceKind::Monthly, 9 * 60, &[], 1);
        assert_eq!(next_run(&monthly, now, &utc), Some(ms(2026, 12, 1, 9, 0)));
        let thirty_first = rec(RecurrenceKind::Monthly, 9 * 60, &[], 31);
        assert_eq!(
            next_run(&thirty_first, ms(2027, 2, 1, 0, 0), &utc),
            Some(ms(2027, 2, 28, 9, 0)),
            "the last day of a short month"
        );
        assert_eq!(
            next_run(&rec(RecurrenceKind::Monthly, 540, &[], 29), ms(2028, 2, 1, 0, 0), &utc),
            Some(ms(2028, 2, 29, 9, 0)),
            "leap day"
        );
        assert_eq!(next_run(&rec(RecurrenceKind::Manual, 540, &[], 1), now, &utc), None);
    }

    #[test]
    fn a_clock_change_does_not_skip_or_repeat_a_run() {
        let Ok(berlin) = TimeZone::get("Europe/Berlin") else { return };
        let daily = rec(RecurrenceKind::Daily, 9 * 60, &[], 1);
        let local = |y, m, d, h| {
            jiff::civil::date(y, m, d)
                .at(h, 0, 0, 0)
                .to_zoned_datetime(berlin.clone())
                .unwrap()
                .timestamp()
                .as_millisecond()
        };
        assert_eq!(next_run(&daily, local(2026, 10, 24, 10), &berlin), Some(local(2026, 10, 25, 9)));
        assert_eq!(next_run(&daily, local(2026, 10, 25, 10), &berlin), Some(local(2026, 10, 26, 9)));
        let night = rec(RecurrenceKind::Daily, 2 * 60 + 30, &[], 1);
        let next = next_run(&night, local(2026, 3, 28, 12), &berlin).expect("a run on 29 March");
        assert!(next > local(2026, 3, 28, 12) && next < local(2026, 3, 29, 12), "the run is not lost: {next}");
    }

    #[test]
    fn variables_are_filled_in_and_unknown_ones_are_caught() {
        let utc = TimeZone::UTC;
        let now = ms(2026, 11, 23, 9, 0);
        let text = "{{weekday}} {{ date }}, day {{day}} of {{month}} {{year}}, week {{week}}. Last: {{last_sent}}";
        assert_eq!(expand(text, now, None, &utc), "Monday Mon, Nov 23, day 23 of November 2026, week 48. Last: never");
        assert_eq!(expand("{{last_sent}}", now, Some(ms(2026, 11, 16, 9, 0)), &utc), "Mon, Nov 16");
        assert_eq!(
            expand("Week {{WEEK}}", ms(2026, 1, 5, 9, 0), None, &utc),
            "Week 2",
            "capitals do not matter and 02 is 2"
        );
        assert_eq!(expand("no variables {here}", now, None, &utc), "no variables {here}");
        assert_eq!(unknown_variables("{{date}} {{nope}} {{Nope}} {{other}}"), ["nope", "other"]);
    }

    #[test]
    fn a_routine_is_checked_before_it_is_saved() {
        let mut store = demo();
        let utc = TimeZone::UTC;
        let now = ms(2026, 11, 18, 10, 0);
        let ok = input("Report", rec(RecurrenceKind::Weekly, 540, &[1], 1));
        let problem = |change: &dyn Fn(&mut RoutineInput)| {
            let mut r = ok.clone();
            change(&mut r);
            Store::open_in_memory().and_then(|mut s| s.save_routine(&r, &utc, now)).unwrap_err().to_string()
        };
        assert_eq!(problem(&|r| r.subject = "  ".into()), "Give the email a subject");
        assert_eq!(problem(&|r| r.body = "".into()), "Write what the email says");
        assert!(problem(&|r| r.body = "Hi {{nope}}".into()).starts_with("{{nope}} is not a variable. Use date,"));
        assert_eq!(problem(&|r| r.recurrence.days = vec![]), "Pick at least one day of the week");
        assert_eq!(problem(&|r| r.recurrence.days = vec![9]), "Days run from Monday to Sunday");
        assert_eq!(problem(&|r| r.recurrence.minutes = 2000), "Pick a time of day");
        assert!(problem(&|r| r.to = vec![Address::new("nope", None)]).contains("is not an email address"));
        assert_eq!(
            problem(&|r| {
                r.mode = RoutineMode::Send;
                r.to.clear();
            }),
            "A routine that sends by itself needs someone to send to"
        );

        let id = store.save_routine(&ok, &utc, now).unwrap();
        let saved = store.routine(id).unwrap().unwrap();
        assert_eq!(
            (saved.name.as_str(), saved.next_run_at, saved.streak, saved.enabled),
            ("Report", Some(ms(2026, 11, 23, 9, 0)), 0, true)
        );
        assert_eq!(store.list_routines().unwrap().len(), 1);
        store.delete_routine(id).unwrap();
        assert!(store.routine(id).unwrap().is_none() && store.delete_routine(id).is_err());
    }

    #[test]
    fn a_due_routine_prepares_a_draft_with_the_variables_filled_in() {
        let mut store = demo();
        let utc = TimeZone::UTC;
        store
            .save_compose_settings(&super::super::drafts::ComposeSettings { signature: "Zo".into(), undo_seconds: 10 })
            .unwrap();
        let id = store
            .save_routine(&input("Report", rec(RecurrenceKind::Weekly, 540, &[1], 1)), &utc, ms(2026, 11, 18, 10, 0))
            .unwrap();
        assert!(store.due_routines(ms(2026, 11, 23, 8, 59)).unwrap().is_empty());
        let due_at = ms(2026, 11, 23, 9, 0);
        assert_eq!(store.due_routines(due_at).unwrap(), vec![id]);

        let out = store.run_routine(id, RunReason::Due, &utc, due_at + 60_000).unwrap();
        assert_eq!((out.result, out.late, out.ticket.is_none(), out.problem), (RunResult::Prepared, false, true, None));
        let draft = store.get_draft(out.draft_id.unwrap()).unwrap().unwrap();
        assert_eq!(draft.subject, "Weekly report, Mon, Nov 23");
        assert!(
            draft.body_html.contains("Monday week 48") && draft.body_html.contains("Last one: never"),
            "{}",
            draft.body_html
        );
        assert!(draft.body_html.contains("-- <br>Zo"), "the signature goes on");
        assert_eq!((draft.routine_id, draft.to[0].email.as_str()), (Some(id), "boss@example.com"));

        let after = store.routine(id).unwrap().unwrap();
        assert_eq!(
            (after.streak, after.next_run_at, after.last_run_at, after.last_result),
            (1, Some(ms(2026, 11, 30, 9, 0)), Some(due_at + 60_000), Some(RunResult::Prepared))
        );
        assert!(store.due_routines(due_at + 120_000).unwrap().is_empty(), "it does not run twice");

        let again = store.run_routine(id, RunReason::Due, &utc, ms(2026, 11, 30, 9, 0)).unwrap();
        let draft = store.get_draft(again.draft_id.unwrap()).unwrap().unwrap();
        assert!(draft.body_html.contains("Last one: Mon, Nov 23"), "{}", draft.body_html);
        assert_eq!(store.routine(id).unwrap().unwrap().streak, 2);
        assert_eq!(store.routine_runs(id, 10).unwrap().len(), 2);
    }

    #[test]
    fn a_routine_that_sends_by_itself_does_so_only_when_it_runs_on_time() {
        let mut store = demo();
        let utc = TimeZone::UTC;
        let mut r = input("Standup note", rec(RecurrenceKind::Weekdays, 9 * 60, &[], 1));
        r.mode = RoutineMode::Send;
        let id = store.save_routine(&r, &utc, ms(2026, 11, 18, 10, 0)).unwrap();
        let due = ms(2026, 11, 19, 9, 0);

        let on_time = store.run_routine(id, RunReason::Due, &utc, due + 5_000).unwrap();
        assert_eq!((on_time.result, on_time.draft_id, on_time.ticket.is_some()), (RunResult::Sent, None, true));
        assert_eq!(store.list_drafts().unwrap().len(), 0, "nothing is left over");

        let woke = ms(2026, 11, 22, 11, 0);
        assert_eq!(store.due_routines(woke).unwrap(), vec![id]);
        let late = store.run_routine(id, RunReason::Due, &utc, woke).unwrap();
        assert_eq!((late.result, late.late, late.ticket.is_none()), (RunResult::Prepared, true, true));
        assert_eq!(late.problem.as_deref(), Some("It ran too long after its time to send by itself"));
        assert!(late.draft_id.is_some());
        let after = store.routine(id).unwrap().unwrap();
        assert_eq!(
            (after.streak, after.next_run_at),
            (0, Some(ms(2026, 11, 23, 9, 0))),
            "one catch-up, then back on schedule"
        );
        assert!(store.due_routines(woke + 1).unwrap().is_empty(), "missed days are not run one by one");
        assert!(store.routine_runs(id, 5).unwrap()[0].late);
    }

    #[test]
    fn skipping_and_running_on_demand_leave_the_schedule_alone() {
        let mut store = demo();
        let utc = TimeZone::UTC;
        let id = store
            .save_routine(&input("Report", rec(RecurrenceKind::Daily, 540, &[], 1)), &utc, ms(2026, 11, 18, 10, 0))
            .unwrap();
        let first_due = ms(2026, 11, 19, 9, 0);

        let manual = store.run_routine(id, RunReason::Manual, &utc, ms(2026, 11, 18, 12, 0)).unwrap();
        assert_eq!(manual.result, RunResult::Prepared);
        let after = store.routine(id).unwrap().unwrap();
        assert_eq!((after.next_run_at, after.streak), (Some(first_due), 0));
        assert!(after.last_run_at.is_some());

        store.set_routine_skip(id, true).unwrap();
        let skipped = store.run_routine(id, RunReason::Due, &utc, first_due).unwrap();
        assert_eq!((skipped.result, skipped.draft_id), (RunResult::Skipped, None));
        let after = store.routine(id).unwrap().unwrap();
        assert_eq!((after.next_run_at, after.skip_next, after.streak), (Some(ms(2026, 11, 20, 9, 0)), false, 0));
        assert_eq!(store.list_drafts().unwrap().len(), 1, "only the one asked for");

        let manual_only = store
            .save_routine(
                &input("Monthly numbers", rec(RecurrenceKind::Manual, 540, &[], 1)),
                &utc,
                ms(2026, 11, 18, 10, 0),
            )
            .unwrap();
        assert_eq!(store.routine(manual_only).unwrap().unwrap().next_run_at, None);
        let mut off = input("Paused", rec(RecurrenceKind::Daily, 540, &[], 1));
        off.enabled = false;
        let paused = store.save_routine(&off, &utc, ms(2026, 11, 18, 10, 0)).unwrap();
        assert!(store.due_routines(ms(2027, 1, 1, 0, 0)).unwrap().iter().all(|d| *d != paused && *d != manual_only));
    }

    #[test]
    fn editing_the_words_keeps_an_overdue_run_and_changing_the_schedule_moves_it() {
        let mut store = demo();
        let utc = TimeZone::UTC;
        let created = ms(2026, 11, 18, 10, 0);
        let id = store.save_routine(&input("Report", rec(RecurrenceKind::Daily, 540, &[], 1)), &utc, created).unwrap();
        let overdue = ms(2026, 11, 25, 10, 0);
        let mut change = input("Report", rec(RecurrenceKind::Daily, 540, &[], 1));
        change.id = Some(id);
        change.subject = "A new subject".into();
        store.save_routine(&change, &utc, overdue).unwrap();
        assert_eq!(store.routine(id).unwrap().unwrap().next_run_at, Some(ms(2026, 11, 19, 9, 0)), "still due");
        change.recurrence = rec(RecurrenceKind::Weekly, 600, &[2], 1);
        store.save_routine(&change, &utc, overdue).unwrap();
        assert_eq!(
            store.routine(id).unwrap().unwrap().next_run_at,
            Some(ms(2026, 12, 1, 10, 0)),
            "the new schedule counts from now"
        );
        change.enabled = false;
        store.save_routine(&change, &utc, overdue).unwrap();
        assert_eq!(store.routine(id).unwrap().unwrap().next_run_at, None);
        change.enabled = true;
        store.save_routine(&change, &utc, overdue).unwrap();
        assert_eq!(
            store.routine(id).unwrap().unwrap().next_run_at,
            Some(ms(2026, 12, 1, 10, 0)),
            "switching on schedules again"
        );
    }
}

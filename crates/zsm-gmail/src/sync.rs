use std::collections::BTreeSet;
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use zsm_core::clock::Clock;
use zsm_core::store::SharedStore;
use zsm_core::store::ingest::{BatchOutcome, IngestContext};
use zsm_core::store::sync::{LabelChange, OutboxItem, SyncState};

use crate::client::{Format, GmailClient};
use crate::convert::to_incoming;
use crate::error::{Error, Result};
use crate::model::GmailMessage;
use crate::outgoing::build_raw;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Reading,
    Backfilling,
    Catching,
    Offline,
    NeedsLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

pub trait SyncEvents: Send + Sync {
    fn progress(&self, phase: Phase, done: u32, total: u32);
    fn changed(&self, entity: &str, ids: Vec<i64>);
    fn arrived(&self, _account_id: i64, _batch: &BatchOutcome) {}
    fn notice(&self, level: Level, text: &str);
}

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub window: Option<String>,
    pub first_read: usize,
    pub concurrency: usize,
    pub chunk: usize,
    pub backfill_page: u32,
    pub screener: bool,
    pub allow_only: Option<Vec<String>>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            window: None,
            first_read: 2000,
            concurrency: 12,
            chunk: 40,
            backfill_page: 100,
            screener: true,
            allow_only: None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub fetched: usize,
    pub ingested: usize,
    pub removed: usize,
    pub label_changes: usize,
    pub sent: usize,
}

pub struct SyncEngine {
    pub store: SharedStore,
    pub client: GmailClient,
    pub account_id: i64,
    pub events: Arc<dyn SyncEvents>,
    pub clock: Arc<dyn Clock>,
    pub options: SyncOptions,
}

impl SyncEngine {
    pub async fn run(&self) -> Result<Report> {
        let mut report = Report { sent: self.drain_outbox().await?, ..Report::default() };
        self.learn_label_names().await;
        let state = self.store.with(|s| s.sync_state(self.account_id))?;
        let mut read = match state.history_id {
            None => self.first_read().await?,
            Some(position) => match self.catch_up(&position).await {
                Err(Error::HistoryGone) => {
                    self.events
                        .notice(Level::Info, "Gmail history was too old, so ZSortMail is reading your mail again.");
                    self.store.with(|s| s.save_sync_state(self.account_id, &SyncState::default()))?;
                    self.first_read().await?
                }
                other => other?,
            },
        };
        read.sent = report.sent;
        report = read;
        self.events.progress(Phase::Idle, 0, 0);
        Ok(report)
    }

    async fn learn_label_names(&self) {
        if let Ok(list) = self.client.labels().await {
            let named: Vec<(String, String)> =
                list.labels.into_iter().filter(|l| l.kind == "user").map(|l| (l.id, l.name)).collect();
            if let Ok(n) = self.store.with(|s| s.name_labels(self.account_id, &named))
                && n > 0
            {
                self.events.changed("tags", Vec::new());
            }
        }
    }

    fn self_context(&self, screener: bool, automation: bool) -> Result<IngestContext> {
        let self_addresses = self.store.with(|s| s.self_addresses(self.account_id))?;
        Ok(IngestContext { account_id: self.account_id, self_addresses, screener, automation })
    }

    async fn fetch_many(&self, ids: &[String], format: Format) -> Result<Vec<GmailMessage>> {
        let permits = Arc::new(Semaphore::new(self.options.concurrency.max(1)));
        let mut set = JoinSet::new();
        for id in ids {
            let client = self.client.clone();
            let permits = Arc::clone(&permits);
            let id = id.clone();
            set.spawn(async move {
                let _permit = permits.acquire_owned().await.ok();
                client.get_message(&id, format).await
            });
        }
        let mut out = Vec::with_capacity(ids.len());
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(message)) => out.push(message),
                Ok(Err(Error::Api { status: 404, .. })) => {}
                Ok(Err(error)) => {
                    set.abort_all();
                    return Err(error);
                }
                Err(join) => return Err(Error::Unexpected(format!("a download stopped unexpectedly: {join}"))),
            }
        }
        Ok(out)
    }

    fn ingest(&self, messages: &[GmailMessage], announce: bool, screener: bool) -> Result<usize> {
        let ctx = self.self_context(screener, announce)?;
        let mut incoming = Vec::with_capacity(messages.len());
        for message in messages {
            match to_incoming(message) {
                Ok(m) => incoming.push(m),
                Err(error) => tracing::warn!(%error, id = %message.id, "skipping a message that could not be read"),
            }
        }
        let now = self.clock.now_ms();
        let mut fresh = 0;
        for chunk in incoming.chunks(self.options.chunk.max(1)) {
            let batch = self.store.with(|s| s.ingest_batch(&ctx, chunk, now))?;
            fresh += batch.messages.iter().filter(|m| m.inserted).count();
            if !batch.touched_people.is_empty() {
                self.events.changed("people", batch.touched_people.clone());
            }
            if !batch.touched_conversations.is_empty() {
                self.events.changed("conversations", batch.touched_conversations.clone());
            }
            if announce && batch.messages.iter().any(|m| m.inserted) {
                self.events.arrived(self.account_id, &batch);
            }
        }
        Ok(fresh)
    }

    async fn first_read(&self) -> Result<Report> {
        self.events.progress(Phase::Reading, 0, 0);
        let profile = self.client.profile().await?;
        let mut report = Report::default();
        let query = self.options.window.as_deref();

        let mut ids: Vec<String> = Vec::new();
        let mut page: Option<String> = None;
        let cursor: Option<String> = loop {
            let want = u32::try_from(self.options.first_read.saturating_sub(ids.len()).clamp(1, 500)).unwrap_or(500);
            let list = self.client.list_messages(query, page.as_deref(), want).await?;
            ids.extend(list.messages.into_iter().map(|m| m.id));
            page = list.next_page_token;
            if page.is_none() || ids.len() >= self.options.first_read {
                break page.clone();
            }
        };
        let known: BTreeSet<String> =
            self.store.with(|s| s.known_provider_ids(self.account_id, &ids))?.into_iter().collect();
        let todo: Vec<String> = ids.iter().filter(|id| !known.contains(*id)).cloned().collect();
        let total = u32::try_from(todo.len()).unwrap_or(u32::MAX);

        let mut done = 0u32;
        for chunk in todo.chunks(self.options.chunk.max(1)) {
            let messages = self.fetch_many(chunk, Format::Full).await?;
            report.fetched += messages.len();
            report.ingested += self.ingest(&messages, false, false)?;
            done += u32::try_from(chunk.len()).unwrap_or(0);
            self.events.progress(Phase::Reading, done.min(total), total);
        }

        let now = self.clock.now_ms();
        self.store.with(|s| {
            s.save_sync_state(
                self.account_id,
                &SyncState {
                    history_id: Some(profile.history_id.clone()),
                    backfill_done: cursor.is_none(),
                    backfill_cursor: cursor.clone(),
                    last_sync_at: Some(now),
                },
            )
        })?;
        self.events.changed("people", Vec::new());
        Ok(report)
    }

    pub async fn backfill_step(&self) -> Result<usize> {
        let state = self.store.with(|s| s.sync_state(self.account_id))?;
        let Some(cursor) = state.backfill_cursor.clone().filter(|_| !state.backfill_done) else {
            return Ok(0);
        };
        let list = self
            .client
            .list_messages(self.options.window.as_deref(), Some(&cursor), self.options.backfill_page)
            .await?;
        let ids: Vec<String> = list.messages.into_iter().map(|m| m.id).collect();
        let known: BTreeSet<String> =
            self.store.with(|s| s.known_provider_ids(self.account_id, &ids))?.into_iter().collect();
        let todo: Vec<String> = ids.into_iter().filter(|id| !known.contains(id)).collect();
        let messages = self.fetch_many(&todo, Format::Full).await?;
        let fresh = self.ingest(&messages, false, false)?;
        let next = list.next_page_token;
        let here = self
            .store
            .with(|s| {
                s.conn()
                    .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                    .map_err(zsm_core::Error::from)
            })
            .unwrap_or(0);
        self.events.progress(Phase::Backfilling, u32::try_from(here).unwrap_or(u32::MAX), 0);
        self.store.with(|s| {
            s.save_sync_state(
                self.account_id,
                &SyncState { backfill_done: next.is_none(), backfill_cursor: next.clone(), ..state.clone() },
            )
        })?;
        Ok(fresh)
    }

    async fn catch_up(&self, position: &str) -> Result<Report> {
        let mut report = Report::default();
        let mut added: Vec<String> = Vec::new();
        let mut deleted: Vec<String> = Vec::new();
        let mut label_changes: Vec<LabelChange> = Vec::new();
        let mut newest = position.to_string();
        let mut page: Option<String> = None;
        loop {
            let history = self.client.history(position, page.as_deref()).await?;
            for record in history.history {
                added.extend(record.messages_added.into_iter().map(|m| m.message.id));
                deleted.extend(record.messages_deleted.into_iter().map(|m| m.message.id));
                label_changes.extend(record.labels_added.into_iter().map(|l| LabelChange {
                    provider_id: l.message.id,
                    add: l.label_ids,
                    remove: vec![],
                }));
                label_changes.extend(record.labels_removed.into_iter().map(|l| LabelChange {
                    provider_id: l.message.id,
                    add: vec![],
                    remove: l.label_ids,
                }));
            }
            if let Some(id) = history.history_id {
                newest = id;
            }
            page = history.next_page_token;
            if page.is_none() {
                break;
            }
        }

        let deleted_set: BTreeSet<&String> = deleted.iter().collect();
        let mut seen = BTreeSet::new();
        added.retain(|id| !deleted_set.contains(id) && seen.insert(id.clone()));
        let known: BTreeSet<String> =
            self.store.with(|s| s.known_provider_ids(self.account_id, &added))?.into_iter().collect();
        let to_fetch: Vec<String> = added.into_iter().filter(|id| !known.contains(id)).collect();

        if !to_fetch.is_empty() {
            self.events.progress(Phase::Catching, 0, u32::try_from(to_fetch.len()).unwrap_or(0));
            let screener = self.options.screener;
            for chunk in to_fetch.chunks(self.options.chunk.max(1)) {
                let messages = self.fetch_many(chunk, Format::Full).await?;
                report.fetched += messages.len();
                report.ingested += self.ingest(&messages, true, screener)?;
            }
        }

        if !label_changes.is_empty() {
            report.label_changes = label_changes.len();
            let touched = self.store.with(|s| s.apply_label_changes(self.account_id, &label_changes))?;
            self.notify(&touched);
        }
        if !deleted.is_empty() {
            report.removed = deleted.len();
            let touched = self.store.with(|s| s.remove_messages(self.account_id, &deleted))?;
            self.notify(&touched);
        }

        let now = self.clock.now_ms();
        self.store.with(|s| {
            let state = s.sync_state(self.account_id)?;
            s.save_sync_state(
                self.account_id,
                &SyncState { history_id: Some(newest.clone()), last_sync_at: Some(now), ..state },
            )
        })?;
        Ok(report)
    }

    fn notify(&self, touched: &zsm_core::store::actions::Touched) {
        if !touched.people.is_empty() {
            self.events.changed("people", touched.people.clone());
        }
        if !touched.conversations.is_empty() {
            self.events.changed("conversations", touched.conversations.clone());
        }
    }

    pub async fn drain_outbox(&self) -> Result<usize> {
        let mut sent = 0;
        loop {
            let now = self.clock.now_ms();
            let due = self.store.with(|s| s.outbox_due(self.account_id, now, 20))?;
            if due.is_empty() {
                return Ok(sent);
            }
            for item in due {
                if let Some(allowed) = &self.options.allow_only {
                    let ok = self.store.with(|s| s.outbox_within(&item, allowed)).unwrap_or(false);
                    if !ok {
                        tracing::warn!(op = %item.op, "blocked by the allow-list: not sent to Gmail");
                        self.store.with(|s| s.outbox_dead(item.id, "blocked by the test allow-list"))?;
                        self.events.notice(
                            Level::Warning,
                            "A change was held back by the test allow-list and not sent to Gmail.",
                        );
                        continue;
                    }
                }
                match self.execute(&item).await {
                    Ok(()) => {
                        self.store.with(|s| s.outbox_done(item.id))?;
                        sent += 1;
                    }
                    Err(error) if error.is_transient() => {
                        let wait = (30_000i64 << item.attempts.min(5)).min(15 * 60_000);
                        let retry_at = self.clock.now_ms() + wait;
                        self.store.with(|s| s.outbox_failed(item.id, &error.to_string(), retry_at))?;
                        return Err(error);
                    }
                    Err(error @ (Error::NeedsRelink(_) | Error::MissingPermission(_))) => return Err(error),
                    Err(error) => {
                        tracing::warn!(%error, op = %item.op, "dropping a change Gmail refused");
                        self.store.with(|s| s.outbox_dead(item.id, &error.to_string()))?;
                        self.events
                            .notice(Level::Warning, &format!("Gmail did not accept a change: {}", error.for_person()));
                    }
                }
            }
        }
    }

    async fn execute(&self, item: &OutboxItem) -> Result<()> {
        if item.account_id != self.account_id {
            return Err(Error::Unexpected(format!(
                "change {} belongs to account {}, not {}",
                item.id, item.account_id, self.account_id
            )));
        }
        let strings = |key: &str| -> Vec<String> {
            item.payload[key]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        match item.op.as_str() {
            "thread_modify" => {
                let thread = item.payload["thread_id"].as_str().unwrap_or_default();
                if thread.starts_with("local:") {
                    return Ok(());
                }
                self.client.modify_thread(thread, &strings("add"), &strings("remove")).await
            }
            "message_modify" => {
                let message = item.payload["message_id"].as_str().unwrap_or_default();
                if message.starts_with("local:") {
                    return Ok(());
                }
                self.client.modify_message(message, &strings("add"), &strings("remove")).await
            }
            "tag_thread" => {
                let thread = item.payload["thread_id"].as_str().unwrap_or_default();
                let name = item.payload["name"].as_str().unwrap_or_default().trim();
                let add = item.payload["add"].as_bool().unwrap_or(true);
                if thread.starts_with("local:") || name.is_empty() {
                    return Ok(());
                }
                let existing = self
                    .client
                    .labels()
                    .await?
                    .labels
                    .into_iter()
                    .find(|l| l.kind == "user" && l.name.eq_ignore_ascii_case(name));
                let id = match (existing, add) {
                    (Some(label), _) => label.id,
                    (None, false) => return Ok(()),
                    (None, true) => self.client.create_label(name).await?.id,
                };
                let (on, off) = if add { (vec![id], vec![]) } else { (vec![], vec![id]) };
                self.client.modify_thread(thread, &on, &off).await
            }
            "thread_trash" => self.client.trash_thread(item.payload["thread_id"].as_str().unwrap_or_default()).await,
            "thread_untrash" => {
                self.client.untrash_thread(item.payload["thread_id"].as_str().unwrap_or_default()).await
            }
            "send_message" => {
                let id = item.payload["message_id"].as_i64().unwrap_or_default();
                let mut outgoing = match self.store.with(|s| s.load_outgoing(id)) {
                    Ok(message) => message,
                    Err(zsm_core::Error::NotFound(_)) => return Ok(()),
                    Err(error) => return Err(error.into()),
                };
                outgoing.bcc =
                    strings("bcc").iter().map(|email| zsm_core::store::ingest::Address::new(email, None)).collect();
                let raw = build_raw(&outgoing)?;
                let thread = (!outgoing.thread_id.starts_with("local:")).then_some(outgoing.thread_id.as_str());
                let sent = self.client.send_raw(&raw, thread).await?;
                self.store.with(|s| s.mark_sent(id, &sent.id, &sent.thread_id))?;
                self.events.changed("conversations", Vec::new());
                Ok(())
            }
            other => Err(Error::Unexpected(format!("unknown queued change {other}"))),
        }
    }
}

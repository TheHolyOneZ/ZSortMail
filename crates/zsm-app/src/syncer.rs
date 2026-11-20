use std::sync::{Arc, Weak};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use zsm_core::dto::AccountInfo;
use zsm_core::store::ingest::BatchOutcome;
use zsm_gmail::client::{Credentials, GmailClient, build_http};
use zsm_gmail::oauth::Session;
use zsm_gmail::sync::{Level, Phase, SyncEngine, SyncEvents, SyncOptions};

use crate::core::Core;
use crate::error::Result;
use crate::events::{CoreEvent, NoticeLevel, SyncPhase};

pub(crate) struct SyncHandle {
    pub wake: Arc<Notify>,
    pub outbox_wake: Arc<Notify>,
    pub task: tokio::task::JoinHandle<()>,
    pub outbox_task: tokio::task::JoinHandle<()>,
    pub client: GmailClient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SyncStatus {
    pub account_id: i64,
    pub phase: SyncPhase,
    pub done: u32,
    pub total: u32,
    pub last_sync_at: Option<i64>,
    pub message: Option<String>,
    pub queued: u32,
}

fn map_phase(phase: Phase) -> SyncPhase {
    match phase {
        Phase::Idle => SyncPhase::Idle,
        Phase::Reading => SyncPhase::Reading,
        Phase::Backfilling => SyncPhase::Backfilling,
        Phase::Catching => SyncPhase::Catching,
        Phase::Offline => SyncPhase::Offline,
        Phase::NeedsLink => SyncPhase::NeedsLink,
    }
}

struct Bridge {
    core: Weak<Core>,
    account_id: i64,
}

impl SyncEvents for Bridge {
    fn progress(&self, phase: Phase, done: u32, total: u32) {
        if let Some(core) = self.core.upgrade() {
            core.set_status(self.account_id, |s| {
                s.phase = map_phase(phase);
                s.done = done;
                s.total = total;
                if phase != Phase::Offline {
                    s.message = None;
                }
            });
        }
    }

    fn changed(&self, entity: &str, ids: Vec<i64>) {
        if let Some(core) = self.core.upgrade() {
            core.changed(entity, ids);
        }
    }

    fn arrived(&self, account_id: i64, batch: &BatchOutcome) {
        if let Some(core) = self.core.upgrade() {
            core.on_arrival(account_id, batch);
        }
    }

    fn notice(&self, level: Level, text: &str) {
        if let Some(core) = self.core.upgrade() {
            let level = match level {
                Level::Info => NoticeLevel::Info,
                Level::Warning => NoticeLevel::Warning,
                Level::Error => NoticeLevel::Error,
            };
            core.emit(CoreEvent::Notice { level, text: text.to_string() });
        }
    }
}

impl Core {
    pub fn start_sync(self: &Arc<Self>) -> Result<()> {
        let accounts = self.with_store(|s| Ok(s.accounts()?))?;
        for account in accounts.iter().filter(|a| a.provider == "gmail") {
            self.spawn_syncer(account);
        }
        Ok(())
    }

    pub(crate) fn set_status(&self, account_id: i64, change: impl FnOnce(&mut SyncStatus)) {
        let (last_sync_at, queued) =
            self.with_store(|s| Ok((s.sync_state(account_id)?.last_sync_at, s.outbox_len()?))).unwrap_or((None, 0));
        let snapshot = {
            let mut all = self.lock(&self.status);
            let status = all.entry(account_id).or_insert_with(|| SyncStatus {
                account_id,
                phase: SyncPhase::Idle,
                done: 0,
                total: 0,
                last_sync_at: None,
                message: None,
                queued: 0,
            });
            change(status);
            status.last_sync_at = last_sync_at;
            status.queued = queued;
            status.clone()
        };
        self.emit(CoreEvent::SyncProgress {
            account_id,
            phase: snapshot.phase,
            done: snapshot.done,
            total: snapshot.total,
        });
    }

    pub fn sync_status(&self) -> Vec<SyncStatus> {
        let mut all: Vec<SyncStatus> = self.lock(&self.status).values().cloned().collect();
        all.sort_by_key(|s| s.account_id);
        all
    }

    pub fn wake_sync(&self) {
        for handle in self.lock(&self.syncers).values() {
            handle.wake.notify_one();
            handle.outbox_wake.notify_one();
        }
    }

    pub(crate) fn stop_syncer(&self, account_id: i64) {
        if let Some(handle) = self.lock(&self.syncers).remove(&account_id) {
            handle.task.abort();
            handle.outbox_task.abort();
        }
    }

    pub(crate) fn client_for(&self, account_id: i64) -> Option<GmailClient> {
        self.lock(&self.syncers).get(&account_id).map(|h| h.client.clone())
    }

    fn sync_options(&self, account_id: i64) -> SyncOptions {
        let (screener, window) = self
            .with_store(|s| {
                Ok((s.setting(&format!("screener.{account_id}"))?, s.setting(&format!("sync.window.{account_id}"))?))
            })
            .unwrap_or((None, None));
        let window = match window.as_deref() {
            Some("all") => None,
            Some(days) if days.ends_with('d') => Some(format!("newer_than:{days}")),
            _ => Some("newer_than:365d".to_string()),
        };
        let allow_only = self.write_allowlist.clone();
        SyncOptions { screener: screener.as_deref() != Some("0"), window, allow_only, ..SyncOptions::default() }
    }

    pub(crate) fn spawn_syncer(self: &Arc<Self>, account: &AccountInfo) {
        let Some(runtime) = self.runtime.clone() else { return };
        self.stop_syncer(account.id);

        let refresh = self.secrets.get(&account.email).ok().flatten();
        let client_credentials = self.oauth_client().ok();
        let (Some(refresh), Some(oauth)) = (refresh, client_credentials) else {
            self.set_status(account.id, |s| {
                s.phase = SyncPhase::NeedsLink;
                s.message = Some("Link this account again to keep syncing.".into());
            });
            return;
        };

        let clock = Arc::clone(&self.clock);
        let secrets = Arc::clone(&self.secrets);
        let email = account.email.clone();
        let session = Session::from_refresh_token(
            oauth,
            build_http(),
            refresh,
            Arc::new(move || clock.now_ms()),
            Some(Arc::new(move |new: &str| {
                let _ = secrets.set(&email, new);
            })),
        );
        let client = GmailClient::new(
            build_http(),
            self.google.gmail_base.clone(),
            Credentials::Session(session),
            self.retry.clone(),
        )
        .with_throttle(Arc::new(zsm_gmail::client::Throttle::per_minute(self.quota_per_minute, 600.0)));
        let wake = Arc::new(Notify::new());
        let task = runtime.spawn(sync_loop(self.weak.clone(), account.clone(), client.clone(), Arc::clone(&wake)));
        let outbox_wake = Arc::new(Notify::new());
        let outbox_task =
            runtime.spawn(outbox_loop(self.weak.clone(), account.clone(), client.clone(), Arc::clone(&outbox_wake)));
        self.lock(&self.syncers).insert(account.id, SyncHandle { wake, outbox_wake, task, outbox_task, client });
    }
}

fn accounts_syncing(weak: &Weak<Core>) -> u32 {
    weak.upgrade().map_or(1, |c| u32::try_from(c.lock(&c.syncers).len()).unwrap_or(1).max(1))
}

fn jitter(base: Duration) -> Duration {
    let factor = 0.8 + 0.4 * rand::random::<f64>();
    base.mul_f64(factor)
}

async fn sync_loop(weak: Weak<Core>, account: AccountInfo, client: GmailClient, wake: Arc<Notify>) {
    let mut failures = 0u32;
    loop {
        let Some(core) = weak.upgrade() else { return };
        let poll = core.poll_interval;
        let engine = SyncEngine {
            store: core.shared_store(),
            client: client.clone(),
            account_id: account.id,
            events: Arc::new(Bridge { core: weak.clone(), account_id: account.id }),
            clock: Arc::clone(&core.clock),
            options: core.sync_options(account.id),
        };
        drop(core);

        let wait = match engine.run().await {
            Ok(_) => {
                failures = 0;
                loop {
                    let pending = engine
                        .store
                        .with(|s| s.sync_state(account.id))
                        .map(|s| !s.backfill_done && s.backfill_cursor.is_some());
                    if !pending.unwrap_or(false) {
                        engine.events.progress(Phase::Idle, 0, 0);
                        break;
                    }
                    match engine.backfill_step().await {
                        Ok(_) => {}
                        Err(_) => {
                            engine.events.progress(Phase::Idle, 0, 0);
                            break;
                        }
                    }
                    tokio::select! {
                        _ = wake.notified() => break,
                        _ = tokio::time::sleep(Duration::from_millis(1500)) => {}
                    }
                }
                jitter(poll * accounts_syncing(&weak))
            }
            Err(zsm_gmail::Error::MissingPermission(reason)) => {
                tracing::warn!(%reason, account = account.id, "Google did not grant the Gmail permission");
                if let Some(core) = weak.upgrade() {
                    core.set_status(account.id, |s| {
                        s.phase = SyncPhase::NeedsLink;
                        s.message = Some(zsm_gmail::MISSING_PERMISSION.into());
                    });
                }
                return;
            }
            Err(zsm_gmail::Error::NeedsRelink(reason)) => {
                tracing::warn!(%reason, account = account.id, "the Google sign-in no longer works");
                if let Some(core) = weak.upgrade() {
                    core.set_status(account.id, |s| {
                        s.phase = SyncPhase::NeedsLink;
                        s.message =
                            Some("Your Google sign-in has expired. Link the account again to keep syncing.".into());
                    });
                }
                return;
            }
            Err(error) => {
                failures += 1;
                tracing::warn!(%error, account = account.id, failures, "sync failed");
                if let Some(core) = weak.upgrade() {
                    let transient = error.is_transient();
                    core.set_status(account.id, |s| {
                        s.phase = if transient { SyncPhase::Offline } else { SyncPhase::Idle };
                        s.message = Some(error.for_person());
                    });
                }
                if matches!(error, zsm_gmail::Error::RateLimited { .. }) {
                    Duration::from_secs((60u64 << failures.min(3)).min(300))
                } else if error.is_transient() {
                    Duration::from_secs((5u64 << failures.min(6)).min(300))
                } else if failures >= 3 {
                    Duration::from_secs(6 * 3600)
                } else {
                    Duration::from_secs(60 << failures)
                }
            }
        };

        tokio::select! {
            _ = wake.notified() => {}
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

async fn outbox_loop(weak: Weak<Core>, account: AccountInfo, client: GmailClient, wake: Arc<Notify>) {
    loop {
        let Some(core) = weak.upgrade() else { return };
        let engine = SyncEngine {
            store: core.shared_store(),
            client: client.clone(),
            account_id: account.id,
            events: Arc::new(Bridge { core: weak.clone(), account_id: account.id }),
            clock: Arc::clone(&core.clock),
            options: core.sync_options(account.id),
        };
        drop(core);
        let wait = match engine.drain_outbox().await {
            Ok(_) => Duration::from_secs(20),
            Err(zsm_gmail::Error::MissingPermission(_) | zsm_gmail::Error::NeedsRelink(_)) => return,
            Err(error) => {
                tracing::warn!(%error, account = account.id, "sending queued changes failed, will retry");
                Duration::from_secs(30)
            }
        };
        tokio::select! {
            _ = wake.notified() => {}
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use zsm_core::clock::{Clock, SystemClock};
use zsm_core::demo::{DemoOptions, seed_demo};
use zsm_core::dto::AccountInfo;
use zsm_core::store::{SharedStore, Store};

use zsm_gmail::RetryPolicy;
use zsm_gmail::credentials::{MemorySecrets, Secrets};

use crate::error::Result;
use crate::events::{Coalescer, CoreEvent, EventSink, NullSink};
use crate::google::{GoogleEndpoints, LinkTask};
use crate::notify::{NullPlatform, Platform};
use crate::sorting::UndoStack;
use crate::syncer::{SyncHandle, SyncStatus};

pub struct CoreConfig {
    pub db_path: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub downloads_dir: Option<PathBuf>,
    pub clock: Arc<dyn Clock>,
    pub sink: Arc<dyn EventSink>,
    pub coalesce: Duration,
    pub timezone: TimeZone,
    pub runtime: Option<tokio::runtime::Handle>,
    pub secrets: Arc<dyn Secrets>,
    pub google: GoogleEndpoints,
    pub retry: RetryPolicy,
    pub poll_interval: Duration,
    pub platform: Arc<dyn Platform>,
    pub tick_interval: Duration,
    pub unsubscribe_allow_private: bool,
    pub write_allowlist: Option<Vec<String>>,
    pub quota_per_minute: f64,
}

impl CoreConfig {
    pub fn in_memory() -> Self {
        Self {
            db_path: None,
            data_dir: None,
            downloads_dir: None,
            clock: Arc::new(SystemClock),
            sink: Arc::new(NullSink),
            coalesce: Duration::ZERO,
            timezone: TimeZone::system(),
            runtime: None,
            secrets: Arc::new(MemorySecrets::default()),
            google: GoogleEndpoints::default(),
            retry: RetryPolicy::default(),
            poll_interval: Duration::from_secs(20),
            platform: Arc::new(NullPlatform),
            tick_interval: crate::automation::DEFAULT_TICK,
            unsubscribe_allow_private: false,
            write_allowlist: None,
            quota_per_minute: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Counts {
    pub new_senders: u32,
    pub unread: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct AppState {
    pub name: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub schema_version: u32,
    pub accounts: Vec<AccountInfo>,
    pub counts: Counts,
}

pub struct Core {
    store: SharedStore,
    pub(crate) clock: Arc<dyn Clock>,
    sink: Arc<dyn EventSink>,
    changes: Coalescer,
    pub(crate) data_dir: Option<PathBuf>,
    pub(crate) downloads_dir: Option<PathBuf>,
    pub(crate) timezone: TimeZone,
    pub(crate) runtime: Option<tokio::runtime::Handle>,
    pub(crate) secrets: Arc<dyn Secrets>,
    pub(crate) google: GoogleEndpoints,
    pub(crate) retry: RetryPolicy,
    pub(crate) poll_interval: Duration,
    pub(crate) platform: Arc<dyn Platform>,
    pub(crate) tick_interval: Duration,
    pub(crate) unsubscribe_allow_private: bool,
    pub(crate) write_allowlist: Option<Vec<String>>,
    pub(crate) quota_per_minute: f64,
    pub(crate) syncers: Mutex<HashMap<i64, SyncHandle>>,
    pub(crate) status: Mutex<HashMap<i64, SyncStatus>>,
    pub(crate) recent_notices: Mutex<HashMap<String, i64>>,
    pub(crate) link: Mutex<Option<LinkTask>>,
    pub(crate) undo: Mutex<UndoStack>,
    pub(crate) weak: Weak<Core>,
}

impl Core {
    pub fn new(config: CoreConfig) -> Result<Arc<Self>> {
        let store = match &config.db_path {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                Store::open(path)?
            }
            None => Store::open_in_memory()?,
        };
        Ok(Arc::new_cyclic(|weak| Self {
            store: SharedStore::new(store),
            clock: config.clock,
            changes: Coalescer::new(Arc::clone(&config.sink), config.coalesce),
            sink: config.sink,
            data_dir: config.data_dir,
            downloads_dir: config.downloads_dir,
            timezone: config.timezone,
            runtime: config.runtime,
            secrets: config.secrets,
            google: config.google,
            retry: config.retry,
            poll_interval: config.poll_interval,
            platform: config.platform,
            tick_interval: config.tick_interval,
            unsubscribe_allow_private: config.unsubscribe_allow_private,
            write_allowlist: config.write_allowlist,
            quota_per_minute: config.quota_per_minute,
            syncers: Mutex::new(HashMap::new()),
            status: Mutex::new(HashMap::new()),
            recent_notices: Mutex::new(HashMap::new()),
            link: Mutex::new(None),
            undo: Mutex::new(UndoStack::new()),
            weak: weak.clone(),
        }))
    }

    pub(crate) fn lock<'a, T>(&self, mutex: &'a Mutex<T>) -> MutexGuard<'a, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        f(&mut self.store.lock())
    }

    pub fn shared_store(&self) -> SharedStore {
        self.store.clone()
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    pub(crate) fn changed(&self, entity: &str, ids: Vec<i64>) {
        self.changes.changed(entity, ids);
    }

    pub(crate) fn touched(&self, touched: &zsm_core::store::actions::Touched) {
        self.wake_sync();
        if !touched.people.is_empty() {
            self.changed("people", touched.people.clone());
        }
        if !touched.conversations.is_empty() {
            self.changed("conversations", touched.conversations.clone());
        }
    }

    pub(crate) fn emit(&self, event: CoreEvent) {
        if let CoreEvent::Notice { text, .. } = &event {
            let now = self.clock.now_ms();
            let mut seen = self.lock(&self.recent_notices);
            seen.retain(|_, at| now - *at < 60_000);
            if seen.contains_key(text) {
                return;
            }
            seen.insert(text.clone(), now);
        }
        self.sink.emit(event);
    }

    pub fn debug_emit(&self, entity: &str) {
        self.changed(entity, Vec::new());
    }

    pub fn app_state(&self) -> Result<AppState> {
        self.with_store(|s| {
            let (new_senders, unread) = s.counts()?;
            Ok(AppState {
                name: "ZSortMail".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                os: std::env::consts::OS.into(),
                arch: std::env::consts::ARCH.into(),
                schema_version: s.schema_version()?,
                accounts: s.accounts()?,
                counts: Counts { new_senders, unread },
            })
        })
    }

    pub fn start_second_demo(&self) -> Result<AccountInfo> {
        let now = self.now_ms();
        let timezone = self.timezone.clone();
        let info = self.with_store(|s| {
            let id = zsm_core::demo::seed_second_demo(s, &DemoOptions { now_ms: now, stress_people: 0, tz: timezone })?;
            Ok(s.accounts()?.into_iter().find(|a| a.id == id).expect("the account just created"))
        })?;
        self.changed("*", Vec::new());
        Ok(info)
    }

    pub fn start_demo(&self, stress_people: usize) -> Result<AccountInfo> {
        let now = self.now_ms();
        let timezone = self.timezone.clone();
        let info = self.with_store(|s| {
            if let Some(existing) = s.accounts()?.into_iter().find(|a| a.provider == "demo") {
                return Ok(existing);
            }
            let id = seed_demo(s, &DemoOptions { now_ms: now, stress_people, tz: timezone })?;
            Ok(s.accounts()?.into_iter().find(|a| a.id == id).expect("the account just created"))
        })?;
        self.changed("*", Vec::new());
        Ok(info)
    }
}

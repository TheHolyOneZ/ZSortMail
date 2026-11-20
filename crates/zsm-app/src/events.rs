use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    StoreChanged { entity: String, ids: Vec<i64> },
    SyncProgress { account_id: i64, phase: SyncPhase, done: u32, total: u32 },
    Notice { level: NoticeLevel, text: String },
    LinkState { phase: LinkPhase, message: String },
    Notification { notice: crate::notify::Notice, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum LinkPhase {
    Waiting,
    Linked,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    Idle,
    Reading,
    Backfilling,
    Catching,
    Offline,
    NeedsLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
}

pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: CoreEvent);
}

pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: CoreEvent) {}
}

pub(crate) struct Coalescer {
    inner: CoalescerKind,
}

enum CoalescerKind {
    Immediate(Arc<dyn EventSink>),
    Windowed(mpsc::Sender<(String, Vec<i64>)>),
}

impl Coalescer {
    pub fn new(sink: Arc<dyn EventSink>, window: Duration) -> Self {
        if window.is_zero() {
            return Self { inner: CoalescerKind::Immediate(sink) };
        }
        let (tx, rx) = mpsc::channel::<(String, Vec<i64>)>();
        std::thread::Builder::new()
            .name("zsm-events".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    let mut pending: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
                    let mut add = |(entity, ids): (String, Vec<i64>)| pending.entry(entity).or_default().extend(ids);
                    add(first);
                    let deadline = Instant::now() + window;
                    while let Ok(next) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                        add(next);
                    }
                    for (entity, ids) in pending {
                        sink.emit(CoreEvent::StoreChanged { entity, ids: ids.into_iter().collect() });
                    }
                }
            })
            .expect("spawn event thread");
        Self { inner: CoalescerKind::Windowed(tx) }
    }

    pub fn changed(&self, entity: &str, ids: Vec<i64>) {
        match &self.inner {
            CoalescerKind::Immediate(sink) => sink.emit(CoreEvent::StoreChanged { entity: entity.to_string(), ids }),
            CoalescerKind::Windowed(tx) => {
                let _ = tx.send((entity.to_string(), ids));
            }
        }
    }
}

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }
}

#[derive(Debug)]
pub struct FixedClock(AtomicI64);

impl FixedClock {
    pub fn new(now_ms: i64) -> Self {
        Self(AtomicI64::new(now_ms))
    }

    pub fn set(&self, now_ms: i64) {
        self.0.store(now_ms, Ordering::SeqCst);
    }

    pub fn advance(&self, by_ms: i64) {
        self.0.fetch_add(by_ms, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

pub const MINUTE: i64 = 60_000;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_only_moves_when_told() {
        let clock = FixedClock::new(1_000);
        assert_eq!(clock.now_ms(), 1_000);
        clock.advance(500);
        assert_eq!(clock.now_ms(), 1_500);
        clock.set(9);
        assert_eq!(clock.now_ms(), 9);
    }

    #[test]
    fn system_clock_is_recent() {
        assert!(SystemClock.now_ms() > 1_700_000_000_000);
    }
}

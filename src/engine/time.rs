//! Swift `Date` values encode as seconds since 2001-01-01T00:00:00Z (JSONEncoder default).
//! Keeping that representation makes backups interchangeable with the iPhone and Android apps.

use serde::{Deserialize, Serialize};

pub const REFERENCE_UNIX_OFFSET: f64 = 978_307_200.0;
pub const DAY: f64 = 86_400.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Date(pub f64);

impl Date {
    pub fn now() -> Self {
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        Date(unix - REFERENCE_UNIX_OFFSET)
    }
    pub const DISTANT_PAST: Date = Date(-63_114_076_800.0);
    pub const DISTANT_FUTURE: Date = Date(63_113_904_000.0);
    pub fn adding(self, seconds: f64) -> Date { Date(self.0 + seconds) }
    pub fn since(self, other: Date) -> f64 { self.0 - other.0 }
    pub fn is_valid(self) -> bool { self.0.is_finite() && self >= Self::DISTANT_PAST && self <= Self::DISTANT_FUTURE }

    fn local(self) -> Option<chrono::DateTime<chrono::Local>> {
        let unix = self.0 + REFERENCE_UNIX_OFFSET;
        let secs = unix.floor();
        let nanos = ((unix - secs) * 1e9) as u32;
        chrono::DateTime::from_timestamp(secs as i64, nanos).map(|d| d.with_timezone(&chrono::Local))
    }
    /// Start of the local calendar day, as a stable key.
    pub fn day_key(self) -> String {
        self.local().map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
    }
    pub fn abbreviated(self) -> String {
        self.local().map(|d| d.format("%-d %b %Y").to_string()).unwrap_or_default()
    }
    pub fn abbreviated_time(self) -> String {
        self.local().map(|d| d.format("%-d %b %Y, %H:%M").to_string()).unwrap_or_default()
    }
}

/// Monotonic seconds for activity timing.
pub fn uptime() -> f64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() + 1_000.0
}

/// Swift's `UUID().uuidString` is uppercase.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

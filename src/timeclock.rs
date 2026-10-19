//! Clock in / clock out, per user. Kept on this PC for now —
//! swap load/save for the database once it's connected.
//! Times are milliseconds since 1970 (same as the web version).

use crate::storage;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Shift {
    #[serde(rename = "in")]
    pub start: i64,
    #[serde(rename = "out")]
    pub end: i64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeSheet {
    #[serde(rename = "activeSince")]
    pub active_since: Option<i64>,
    /// Newest first. Full history — monthly totals need it.
    pub shifts: Vec<Shift>,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl TimeSheet {
    pub fn load(name: &str) -> Self {
        storage::load(&storage::user_file("timeclock", name, "json")).unwrap_or_default()
    }
    fn save(&self, name: &str) {
        storage::save(&storage::user_file("timeclock", name, "json"), self);
    }
    pub fn on_duty(&self) -> bool {
        self.active_since.is_some()
    }

    pub fn clock_in(&mut self, name: &str) -> bool {
        if self.on_duty() {
            return false;
        }
        self.active_since = Some(now_ms());
        self.save(name);
        true
    }

    pub fn clock_out(&mut self, name: &str) -> bool {
        let Some(since) = self.active_since else { return false };
        self.shifts.insert(0, Shift { start: since, end: now_ms() });
        self.active_since = None;
        self.save(name);
        true
    }

    pub fn toggle(&mut self, name: &str) -> bool {
        if self.on_duty() { self.clock_out(name) } else { self.clock_in(name) }
    }

    /// Remove one finished shift (identified by its clock-in time).
    pub fn delete_shift(&mut self, name: &str, start: i64) {
        self.shifts.retain(|s| s.start != start);
        self.save(name);
    }

    /// Change a finished shift's times. Keeps the list newest-first.
    pub fn update_shift(&mut self, name: &str, old_start: i64, shift: Shift) {
        for s in self.shifts.iter_mut() {
            if s.start == old_start {
                *s = shift;
            }
        }
        self.shifts.sort_by(|a, b| b.start.cmp(&a.start));
        self.save(name);
    }

    pub fn clear(&mut self, name: &str) {
        self.shifts.clear();
        self.save(name);
    }
}

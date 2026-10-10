//! Shadowing progress, kept on this computer only.
//!
//! Deliberately separate from the learning archive: that file is the backup the iPhone and Android
//! apps read, and drill attempts are not conversation evidence. Same load-cache-save shape as
//! `provider.rs`.

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::engine::time::{Date, DAY};
use super::store::LearningStore;

/// How a single phrase is going.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhraseProgress {
    pub attempts: u32,
    /// Consecutive attempts that sounded right; a poor one sends it back to the start.
    pub streak: u32,
    pub best: f64,
    pub last: f64,
    pub last_practised: Option<Date>,
    pub due_at: Option<Date>,
}

impl PhraseProgress {
    pub fn is_due(&self, now: Date) -> bool { self.due_at.map(|due| due <= now).unwrap_or(true) }
    /// Three clean repeats on different days is as settled as a drill can show.
    pub fn is_settled(&self) -> bool { self.streak >= 3 }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShadowRecord {
    /// Keyed by phrase id.
    pub phrases: BTreeMap<String, PhraseProgress>,
}

/// The same widening intervals the learning projection uses for words.
fn interval(streak: u32) -> f64 {
    [1.0, 1.0, 4.0, 14.0][(streak as usize).min(3)] * DAY
}

impl ShadowRecord {
    pub fn get(&self, phrase_id: &str) -> PhraseProgress {
        self.phrases.get(phrase_id).cloned().unwrap_or_default()
    }

    /// Records one attempt and schedules the phrase's return.
    pub fn record(&mut self, phrase_id: &str, score: f64, good: bool, now: Date) -> PhraseProgress {
        let entry = self.phrases.entry(phrase_id.to_string()).or_default();
        entry.attempts += 1;
        entry.last = score;
        entry.best = entry.best.max(score);
        entry.streak = if good { entry.streak + 1 } else { 0 };
        entry.last_practised = Some(now);
        entry.due_at = Some(now.adding(interval(entry.streak)));
        entry.clone()
    }

    pub fn practised(&self) -> usize { self.phrases.values().filter(|p| p.attempts > 0).count() }
    pub fn settled(&self) -> usize { self.phrases.values().filter(|p| p.is_settled()).count() }
}

static CURRENT: RwLock<Option<ShadowRecord>> = RwLock::new(None);

fn path() -> std::path::PathBuf { LearningStore::directory().join("shadowing.json") }

pub fn current() -> ShadowRecord {
    if let Some(record) = CURRENT.read().ok().and_then(|r| r.clone()) { return record; }
    let loaded = std::fs::read(path()).ok()
        .and_then(|d| serde_json::from_slice::<ShadowRecord>(&d).ok())
        .unwrap_or_default();
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(loaded.clone()); }
    loaded
}

pub fn update(change: impl FnOnce(&mut ShadowRecord)) -> Result<ShadowRecord, String> {
    let mut record = current();
    change(&mut record);
    let data = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(LearningStore::directory()).map_err(|e| e.to_string())?;
    std::fs::write(path(), data).map_err(|_| "Mural couldn’t save your shadowing progress.".to_string())?;
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(record.clone()); }
    Ok(record)
}

/// Forgets everything, for "delete all learning data".
pub fn remove_all() -> Result<(), String> {
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(ShadowRecord::default()); }
    match std::fs::remove_file(path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Mural couldn’t clear your shadowing progress.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Date = Date(1_000_000.0);

    #[test]
    fn a_new_phrase_is_due_immediately() {
        let record = ShadowRecord::default();
        assert!(record.get("de-0001").is_due(NOW));
        assert_eq!(record.practised(), 0);
    }

    #[test]
    fn a_good_attempt_pushes_the_phrase_further_out_each_time() {
        let mut record = ShadowRecord::default();
        let first = record.record("de-0001", 0.95, true, NOW);
        assert_eq!(first.streak, 1);
        assert_eq!(first.due_at, Some(NOW.adding(DAY)));
        assert!(!first.is_due(NOW));

        let later = NOW.adding(2.0 * DAY);
        record.record("de-0001", 0.95, true, later);
        let third = record.record("de-0001", 0.95, true, later.adding(2.0 * DAY));
        assert_eq!(third.streak, 3);
        assert_eq!(third.due_at, Some(later.adding(2.0 * DAY).adding(14.0 * DAY)));
        assert!(third.is_settled());
    }

    #[test]
    fn a_poor_attempt_brings_it_straight_back() {
        let mut record = ShadowRecord::default();
        record.record("de-0001", 0.95, true, NOW);
        record.record("de-0001", 0.95, true, NOW);
        let slipped = record.record("de-0001", 0.3, false, NOW);
        assert_eq!(slipped.streak, 0);
        assert_eq!(slipped.due_at, Some(NOW.adding(DAY)));
        assert!(!slipped.is_settled());
    }

    #[test]
    fn the_best_attempt_is_remembered_even_after_a_worse_one() {
        let mut record = ShadowRecord::default();
        record.record("de-0001", 0.9, true, NOW);
        let after = record.record("de-0001", 0.4, false, NOW);
        assert_eq!(after.best, 0.9);
        assert_eq!(after.last, 0.4);
        assert_eq!(after.attempts, 2);
        assert_eq!(record.practised(), 1);
    }

    #[test]
    fn progress_survives_a_round_trip() {
        let mut record = ShadowRecord::default();
        record.record("de-0001", 0.88, true, NOW);
        let text = serde_json::to_string(&record).unwrap();
        let back: ShadowRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(back.get("de-0001"), record.get("de-0001"));
        // An unknown phrase in a newer catalogue must not break an older record.
        let partial: ShadowRecord = serde_json::from_str(r#"{"phrases":{"de-0002":{"attempts":1}}}"#).unwrap();
        assert_eq!(partial.get("de-0002").attempts, 1);
        assert_eq!(partial.get("de-0002").streak, 0);
    }
}

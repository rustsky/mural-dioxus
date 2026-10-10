//! Which scenarios have been played, and how far through them the learner got.
//!
//! Kept on this computer only, like shadowing progress: the learning archive is what the phones
//! restore from, and a role play is practice rather than conversation evidence.

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::engine::time::Date;
use super::store::LearningStore;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScenarioProgress {
    pub runs: u32,
    /// The most beats reached in any single run.
    pub best_beats: usize,
    pub completed: bool,
    pub last_played: Option<Date>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DialogRecord {
    pub scenarios: BTreeMap<String, ScenarioProgress>,
}

impl DialogRecord {
    pub fn get(&self, scenario_id: &str) -> ScenarioProgress {
        self.scenarios.get(scenario_id).cloned().unwrap_or_default()
    }

    /// Records a run. `beats` is how many were reached, `total` how many there were.
    pub fn record(&mut self, scenario_id: &str, beats: usize, total: usize, now: Date) -> ScenarioProgress {
        let entry = self.scenarios.entry(scenario_id.to_string()).or_default();
        entry.runs += 1;
        entry.best_beats = entry.best_beats.max(beats);
        entry.completed = entry.completed || (total > 0 && beats >= total);
        entry.last_played = Some(now);
        entry.clone()
    }

    pub fn played(&self) -> usize { self.scenarios.values().filter(|s| s.runs > 0).count() }
    pub fn completed(&self) -> usize { self.scenarios.values().filter(|s| s.completed).count() }
}

static CURRENT: RwLock<Option<DialogRecord>> = RwLock::new(None);

fn path() -> std::path::PathBuf { LearningStore::directory().join("dialogs.json") }

pub fn current() -> DialogRecord {
    if let Some(record) = CURRENT.read().ok().and_then(|r| r.clone()) { return record; }
    let loaded = std::fs::read(path()).ok()
        .and_then(|d| serde_json::from_slice::<DialogRecord>(&d).ok())
        .unwrap_or_default();
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(loaded.clone()); }
    loaded
}

pub fn update(change: impl FnOnce(&mut DialogRecord)) -> Result<DialogRecord, String> {
    let mut record = current();
    change(&mut record);
    let data = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(LearningStore::directory()).map_err(|e| e.to_string())?;
    std::fs::write(path(), data).map_err(|_| "Mural couldn’t save your dialogue progress.".to_string())?;
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(record.clone()); }
    Ok(record)
}

pub fn remove_all() -> Result<(), String> {
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(DialogRecord::default()); }
    match std::fs::remove_file(path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Mural couldn’t clear your dialogue progress.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Date = Date(1_000_000.0);

    #[test]
    fn an_unplayed_scenario_is_blank() {
        let record = DialogRecord::default();
        let p = record.get("sc-0001");
        assert_eq!(p.runs, 0);
        assert!(!p.completed);
        assert_eq!(record.played(), 0);
    }

    #[test]
    fn reaching_every_beat_completes_it() {
        let mut record = DialogRecord::default();
        let partial = record.record("sc-0001", 2, 4, NOW);
        assert_eq!(partial.runs, 1);
        assert_eq!(partial.best_beats, 2);
        assert!(!partial.completed);

        let full = record.record("sc-0001", 4, 4, NOW);
        assert_eq!(full.runs, 2);
        assert!(full.completed);
        assert_eq!(record.completed(), 1);
    }

    #[test]
    fn a_weaker_run_keeps_the_best_and_stays_completed() {
        let mut record = DialogRecord::default();
        record.record("sc-0001", 4, 4, NOW);
        let after = record.record("sc-0001", 1, 4, NOW);
        assert_eq!(after.best_beats, 4, "the best run is remembered");
        assert!(after.completed, "a scenario does not become uncompleted");
        assert_eq!(after.runs, 2);
    }

    #[test]
    fn a_scenario_without_beats_cannot_complete() {
        let mut record = DialogRecord::default();
        let p = record.record("sc-0001", 0, 0, NOW);
        assert!(!p.completed);
    }

    #[test]
    fn progress_survives_a_round_trip() {
        let mut record = DialogRecord::default();
        record.record("sc-0001", 3, 5, NOW);
        let text = serde_json::to_string(&record).unwrap();
        let back: DialogRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(back.get("sc-0001"), record.get("sc-0001"));
        let partial: DialogRecord = serde_json::from_str(r#"{"scenarios":{"sc-0002":{"runs":1}}}"#).unwrap();
        assert_eq!(partial.get("sc-0002").runs, 1);
        assert!(!partial.get("sc-0002").completed);
    }
}

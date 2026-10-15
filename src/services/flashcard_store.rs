//! Flash card schedules, kept on this computer only.
//!
//! Like shadowing progress, deliberately separate from the learning archive: that file is the
//! backup the iPhone and Android apps read, and a card review is not conversation evidence.

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::engine::flashcards::{self, Rating, Schedule};
use crate::engine::time::Date;
use super::store::LearningStore;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CardRecord {
    /// Keyed by word id, which already names the language. A word without an entry is a new card.
    pub cards: BTreeMap<String, Schedule>,
}

impl CardRecord {
    pub fn get(&self, word_id: &str) -> Option<&Schedule> { self.cards.get(word_id) }

    /// Answers a card and schedules its return.
    pub fn review(&mut self, word_id: &str, rating: Rating, now: Date) -> Schedule {
        let next = flashcards::review(self.cards.get(word_id), rating, now);
        self.cards.insert(word_id.to_string(), next.clone());
        next
    }
}

static CURRENT: RwLock<Option<CardRecord>> = RwLock::new(None);

fn path() -> std::path::PathBuf { LearningStore::directory().join("flashcards.json") }

pub fn current() -> CardRecord {
    if let Some(record) = CURRENT.read().ok().and_then(|r| r.clone()) { return record; }
    let loaded = std::fs::read(path()).ok()
        .and_then(|d| serde_json::from_slice::<CardRecord>(&d).ok())
        .unwrap_or_default();
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(loaded.clone()); }
    loaded
}

pub fn update(change: impl FnOnce(&mut CardRecord)) -> Result<CardRecord, String> {
    let mut record = current();
    change(&mut record);
    let data = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(LearningStore::directory()).map_err(|e| e.to_string())?;
    std::fs::write(path(), data).map_err(|_| "Mural couldn’t save your flash cards.".to_string())?;
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(record.clone()); }
    Ok(record)
}

/// Forgets everything, for "delete all learning data".
pub fn remove_all() -> Result<(), String> {
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(CardRecord::default()); }
    match std::fs::remove_file(path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Mural couldn’t clear your flash cards.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_review_is_kept_and_survives_a_round_trip() {
        let mut record = CardRecord::default();
        assert!(record.get("de|haus|house").is_none(), "a word starts as a new card");
        let now = Date(1_000_000.0);
        let card = record.review("de|haus|house", Rating::Good, now);
        assert_eq!(record.get("de|haus|house"), Some(&card));
        let back: CardRecord = serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(back.get("de|haus|house"), Some(&card));
        // An older or partial file must not break loading.
        let empty: CardRecord = serde_json::from_str("{}").unwrap();
        assert!(empty.cards.is_empty());
    }
}

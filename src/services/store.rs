use std::path::PathBuf;

use crate::engine::languages::{self, LanguageModule};
use crate::engine::learning::{LearnerState, LearningEngine};
use crate::engine::models::{Archive, Preferences, SessionRecord, Speaker};
use crate::engine::text::prefix;
use crate::engine::time::Date;

/// The learning record: one JSON archive in Application Support, written atomically.
#[derive(Clone, Debug, PartialEq)]
pub struct LearningStore {
    pub archive: Archive,
    pub error: Option<String>,
    path: Option<PathBuf>,
}

pub const SAVE_ERROR: &str = "Mural couldn’t save your progress. Please export a backup and try again.";

impl LearningStore {
    pub fn directory() -> PathBuf {
        dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Mural")
    }

    pub fn open(in_memory: bool) -> Result<Self, String> {
        if in_memory {
            return Ok(Self { archive: Archive::default(), error: None, path: None });
        }
        let path = Self::directory().join("learning-archive.json");
        let mut store = match std::fs::read(&path) {
            Ok(data) => {
                let archive = Archive::decode(&data).map_err(|e| e.to_string())?;
                let version = serde_json::from_slice::<serde_json::Value>(&data).ok()
                    .and_then(|v| v.get("schemaVersion").and_then(|v| v.as_i64()));
                if version == Some(1) {
                    let backup = Self::directory().join("before-language-modules.json");
                    if !backup.exists() { Self::write_private(&backup, &data).map_err(|e| e.to_string())?; }
                }
                Self { archive, error: None, path: Some(path) }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self { archive: Archive::default(), error: None, path: Some(path) },
            Err(e) => return Err(e.to_string()),
        };
        for s in store.archive.sessions.iter_mut().filter(|s| s.ended_at.is_none()) {
            s.ended_at = Some(Date::now());
            s.end_reason = Some("App closed before finalization".into());
        }
        store.persist();
        if store.error.is_some() { return Err(SAVE_ERROR.into()); }
        Ok(store)
    }

    fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
        let tmp = path.with_extension("tmp");
        {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
            let mut file = options.open(&tmp)?;
            file.write_all(data)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, path)
    }

    fn persist(&mut self) {
        let Some(path) = self.path.clone() else { self.error = None; return };
        let result = self.archive.encoded().map_err(|e| e.to_string())
            .and_then(|data| Self::write_private(&path, &data).map_err(|e| e.to_string()));
        self.error = result.err().map(|_| SAVE_ERROR.to_string());
    }

    pub fn preferences(&self) -> &Preferences { &self.archive.preferences }
    pub fn language(&self) -> &'static LanguageModule {
        languages::module(&self.archive.preferences.learning_language_id).unwrap_or(&languages::all()[0])
    }
    pub fn sessions(&self) -> Vec<SessionRecord> {
        let mut s = self.archive.sessions.clone();
        s.sort_by(|a, b| b.started_at.0.total_cmp(&a.started_at.0));
        s
    }
    pub fn learning_sessions(&self) -> Vec<SessionRecord> {
        let id = self.language().id;
        self.sessions().into_iter().filter(|s| s.language_id == id).collect()
    }
    pub fn session(&self, id: &str) -> Option<&SessionRecord> { self.archive.sessions.iter().find(|s| s.id == id) }
    pub fn learner(&self) -> LearnerState {
        LearningEngine::project(&self.archive.sessions, self.language().id, &self.archive.preferences.hidden_words, Date::now())
    }
    pub fn select_language(&mut self, id: &str) {
        if languages::module(id).is_none() { return; }
        self.archive.preferences.learning_language_id = id.into();
        self.persist();
    }
    pub fn update_preferences(&mut self, change: impl FnOnce(&mut Preferences)) {
        change(&mut self.archive.preferences);
        self.persist();
    }
    pub fn save(&mut self, session: &SessionRecord) {
        match self.archive.sessions.iter().position(|s| s.id == session.id) {
            Some(i) => self.archive.sessions[i] = session.clone(),
            None => self.archive.sessions.push(session.clone()),
        }
        self.persist();
    }
    pub fn delete_session(&mut self, id: &str) {
        self.archive.sessions.retain(|s| s.id != id);
        self.persist();
    }
    pub fn hide_word(&mut self, id: &str) {
        self.archive.preferences.hidden_words.push(id.into());
        self.persist();
    }
    pub fn correct_passage(&mut self, session_id: &str, passage_id: &str, text: &str) -> bool {
        let Some(index) = self.archive.sessions.iter().position(|s| s.id == session_id) else { return false };
        let passages = self.archive.sessions[index].passages();
        let Some(passage) = passages.iter().find(|p| p.id == passage_id && p.speaker == Speaker::User) else { return false };
        for (offset, fragment) in passage.fragments.iter().enumerate() {
            let replacement = if offset == 0 { prefix(text, 10_000) } else { String::new() };
            self.archive.sessions[index].correct_fragment(&fragment.id, &replacement);
        }
        self.persist();
        true
    }
    pub fn delete_all(&mut self) -> Vec<String> {
        let backup = Self::directory().join("before-language-modules.json");
        if self.path.is_some() {
            match std::fs::remove_file(&backup) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    self.error = Some("Mural couldn’t delete the older learning backup. Your conversations are still here. Please try again.".into());
                    return vec![];
                }
            }
        }
        let ids = self.archive.sessions.iter().map(|s| s.id.clone()).collect();
        self.archive.sessions.clear();
        self.archive.preferences.hidden_words.clear();
        self.persist();
        ids
    }
    pub fn export_data(&self) -> Result<Vec<u8>, String> { self.archive.encoded().map_err(|e| e.to_string()) }
    pub fn import_data(&mut self, data: &[u8]) -> Result<(), String> {
        let imported = Archive::decode(data).map_err(|e| e.to_string())?;
        self.archive = self.archive.merging(&imported).map_err(|e| e.to_string())?;
        self.persist();
        Ok(())
    }
}

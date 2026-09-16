//! Which service answers teaching requests. Stored on this Mac only, outside the learning backup.
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use super::store::LearningStore;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Teacher {
    #[default]
    OpenAi,
    OllamaCloud,
    OllamaLocal,
}

impl Teacher {
    pub const ALL: [Teacher; 3] = [Teacher::OpenAi, Teacher::OllamaCloud, Teacher::OllamaLocal];

    pub fn id(self) -> &'static str {
        match self { Teacher::OpenAi => "openAi", Teacher::OllamaCloud => "ollamaCloud", Teacher::OllamaLocal => "ollamaLocal" }
    }
    pub fn from_id(id: &str) -> Option<Self> { Self::ALL.into_iter().find(|t| t.id() == id) }
    pub fn label(self) -> &'static str {
        match self {
            Teacher::OpenAi => "OpenAI · GPT-5.6 Luna",
            Teacher::OllamaCloud => "Ollama Cloud · free account",
            Teacher::OllamaLocal => "Ollama on this Mac",
        }
    }
    pub fn is_ollama(self) -> bool { self != Teacher::OpenAi }
    pub fn default_model(self) -> &'static str {
        match self { Teacher::OpenAi => "", Teacher::OllamaCloud => "gpt-oss:120b", Teacher::OllamaLocal => "llama3.2" }
    }
}

/// How an Ollama conversation listens and speaks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceStyle {
    /// Downloaded models running on this computer.
    #[default]
    Natural,
    /// The operating system's speech recognition and voices.
    System,
}

pub const OLLAMA_CLOUD_HOST: &str = "https://ollama.com";
pub const OLLAMA_LOCAL_HOST: &str = "http://localhost:11434";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderSettings {
    pub teacher: Teacher,
    pub cloud_model: String,
    pub local_model: String,
    pub voice: VoiceStyle,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            teacher: Teacher::OpenAi,
            cloud_model: Teacher::OllamaCloud.default_model().into(),
            local_model: Teacher::OllamaLocal.default_model().into(),
            voice: VoiceStyle::Natural,
        }
    }
}

impl ProviderSettings {
    pub fn model(&self) -> &str {
        match self.teacher {
            Teacher::OpenAi => super::api::TEACHER_MODEL,
            Teacher::OllamaCloud => &self.cloud_model,
            Teacher::OllamaLocal => &self.local_model,
        }
    }
    pub fn host(&self) -> &'static str {
        if self.teacher == Teacher::OllamaLocal { OLLAMA_LOCAL_HOST } else { OLLAMA_CLOUD_HOST }
    }
}

static CURRENT: RwLock<Option<ProviderSettings>> = RwLock::new(None);

fn path() -> std::path::PathBuf { LearningStore::directory().join("provider.json") }

pub fn current() -> ProviderSettings {
    if let Some(settings) = CURRENT.read().ok().and_then(|s| s.clone()) { return settings; }
    let loaded = std::fs::read(path()).ok()
        .and_then(|d| serde_json::from_slice::<ProviderSettings>(&d).ok())
        .unwrap_or_default();
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(loaded.clone()); }
    loaded
}

pub fn update(change: impl FnOnce(&mut ProviderSettings)) -> Result<ProviderSettings, String> {
    let mut settings = current();
    change(&mut settings);
    let data = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(LearningStore::directory()).map_err(|e| e.to_string())?;
    std::fs::write(path(), data).map_err(|_| "Mural couldn’t save the teacher setting.".to_string())?;
    if let Ok(mut slot) = CURRENT.write() { *slot = Some(settings.clone()); }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_with_defaults() {
        let s: ProviderSettings = serde_json::from_str(r#"{"teacher":"ollamaCloud"}"#).unwrap();
        assert_eq!(s.teacher, Teacher::OllamaCloud);
        assert_eq!(s.model(), "gpt-oss:120b");
        assert_eq!(s.host(), OLLAMA_CLOUD_HOST);
        assert_eq!(Teacher::from_id("ollamaLocal"), Some(Teacher::OllamaLocal));
        assert_eq!(ProviderSettings::default().model(), "gpt-5.6-luna");
        assert_eq!(s.voice, VoiceStyle::Natural);
    }
}

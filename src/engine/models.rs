use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::languages::{self, DEFAULT_ID};
use super::learning::LearningEngine;
use super::text::{is_han, is_open_punctuation, is_punctuation};
use super::time::{new_id, Date};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker { User, Assistant }

impl Speaker {
    pub fn raw(self) -> &'static str { match self { Speaker::User => "user", Speaker::Assistant => "assistant" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceKind { Exposure, Understanding, Assisted, Independent, Lapse }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome { Success, Partial, Breakdown, Uncertain }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    pub id: String,
    #[serde(default)]
    pub revision: i64,
    #[serde(default)]
    pub previous_texts: Vec<String>,
    pub speaker: Speaker,
    pub text: String,
    #[serde(rename = "startMS")]
    pub start_ms: i64,
    #[serde(rename = "endMS")]
    pub end_ms: i64,
    pub received_at: Date,
    pub meaning_visible: bool,
    pub typed: bool,
}

impl Fragment {
    pub fn new(speaker: Speaker, text: impl Into<String>, start_ms: i64, end_ms: i64) -> Self {
        Self {
            id: new_id(), revision: 0, previous_texts: vec![], speaker, text: text.into(),
            start_ms, end_ms, received_at: Date::now(), meaning_visible: false, typed: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Passage {
    pub id: String,
    pub speaker: Speaker,
    pub fragments: Vec<Fragment>,
}

impl Passage {
    pub fn text(&self) -> String { Self::join(self.fragments.iter().map(|f| f.text.as_str())) }

    /// Join fragment texts. Insert one space only when both sides lack boundary whitespace
    /// and the next fragment does not start with punctuation (so "Hei" + "!" stays "Hei!").
    pub fn join<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
        let mut result = String::new();
        for part in parts {
            if result.is_empty() { result.push_str(part); continue; }
            let (Some(last), Some(first)) = (result.chars().last(), part.chars().next()) else {
                result.push_str(part);
                continue;
            };
            let needs_space = !last.is_whitespace() && !first.is_whitespace() && !is_punctuation(first)
                && !Self::is_unspaced_boundary(last, first);
            if needs_space { result.push(' '); }
            result.push_str(part);
        }
        result
    }

    fn is_unspaced_boundary(last: char, first: char) -> bool {
        let cjk_punctuation = matches!(last as u32, 0x3000..=0x303F | 0xFF00..=0xFFEF);
        is_open_punctuation(last) || (is_han(first) && (is_han(last) || cjk_punctuation))
    }

    pub fn revision_key(&self) -> String {
        self.fragments.iter().map(|f| format!("{}:{}", f.id, f.revision)).collect::<Vec<_>>().join(",")
    }
    pub fn start_ms(&self) -> i64 { self.fragments.first().map(|f| f.start_ms).unwrap_or(0) }
    pub fn end_ms(&self) -> i64 { self.fragments.iter().map(|f| f.end_ms).max().unwrap_or(0) }
}

/// Presentation grouping only: neither the gap nor the arrival of another speaker proves a completed turn.
pub fn passages(fragments: &[Fragment]) -> Vec<Passage> {
    let mut indexed: Vec<(usize, &Fragment)> = fragments.iter().enumerate().collect();
    indexed.sort_by(|a, b| a.1.start_ms.cmp(&b.1.start_ms).then(a.0.cmp(&b.0)));
    let mut result: Vec<Passage> = Vec::new();
    for (_, fragment) in indexed {
        let target = result.iter().rposition(|p| p.speaker == fragment.speaker);
        if let Some(i) = target {
            let last_typed = result[i].fragments.last().map(|f| f.typed).unwrap_or(false);
            if fragment.start_ms - result[i].end_ms() <= 2200 && !fragment.typed && !last_typed {
                result[i].fragments.push(fragment.clone());
                continue;
            }
        }
        result.push(Passage { id: fragment.id.clone(), speaker: fragment.speaker, fragments: vec![fragment.clone()] });
    }
    // Stable sort, matching Swift's sorted(by:) on already-ordered input.
    result.sort_by_key(|p| p.start_ms());
    result
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordProposal {
    pub lemma: String,
    pub meaning: String,
    pub form: String,
    pub kind: EvidenceKind,
    pub confidence: f64,
    #[serde(rename = "sourceIDs")]
    pub source_ids: Vec<String>,
    pub quote: String,
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String { DEFAULT_ID.into() }

impl WordProposal {
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.language, self.lemma.trim().to_lowercase(), self.meaning.to_lowercase())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    #[serde(rename = "passageID")]
    pub passage_id: String,
    pub revision_key: String,
    pub outcome: Outcome,
    pub suggested_level: i64,
    pub next_goal: String,
    pub capability: String,
    pub words: Vec<WordProposal>,
    pub created_at: Date,
    #[serde(default = "default_context")]
    pub context: String,
}

fn default_context() -> String { "free".into() }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceLink {
    pub title: String,
    pub url: String,
}

impl SourceLink {
    pub fn safe_url(&self) -> Option<String> {
        let u = reqwest::Url::parse(&self.url).ok()?;
        if u.scheme() != "https" || u.host_str().is_none() || !u.username().is_empty() || u.password().is_some() {
            return None;
        }
        Some(self.url.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicBrief {
    #[serde(default = "new_id")]
    pub id: String,
    #[serde(rename = "languageID")]
    pub language_id: String,
    pub query: String,
    pub text: String,
    pub sources: Vec<SourceLink>,
    #[serde(default = "Date::now")]
    pub retrieved_at: Date,
}

impl TopicBrief {
    pub fn new(language_id: &str, query: &str, text: &str, sources: Vec<SourceLink>) -> Self {
        Self { id: new_id(), language_id: language_id.into(), query: query.into(), text: text.into(), sources, retrieved_at: Date::now() }
    }
    pub fn is_fresh(&self) -> bool { Date::now().since(self.retrieved_at) < 6.0 * 3600.0 }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    #[serde(rename = "languageID")]
    pub language_id: String,
    #[serde(rename = "providerID", default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub started_at: Date,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Date>,
    #[serde(rename = "themeID", default, skip_serializing_if = "Option::is_none")]
    pub theme_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub fragments: Vec<Fragment>,
    #[serde(default)]
    pub assessments: Vec<Assessment>,
    #[serde(default)]
    pub translations: BTreeMap<String, String>,
    #[serde(default)]
    pub topics: Vec<TopicBrief>,
    #[serde(default)]
    pub voice_seconds: f64,
    #[serde(default)]
    pub usage_final: bool,
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
    #[serde(default)]
    pub search_calls: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_reason: Option<String>,
}

impl SessionRecord {
    pub fn new(language_id: &str, theme_id: Option<String>, title: Option<String>) -> Self {
        let title = title.unwrap_or_else(|| {
            languages::module(language_id).map(|m| m.default_title()).unwrap_or_else(|| "A conversation".into())
        });
        Self {
            id: new_id(), language_id: language_id.into(), provider_id: None, started_at: Date::now(), ended_at: None,
            theme_id, title, fragments: vec![], assessments: vec![], translations: BTreeMap::new(), topics: vec![],
            voice_seconds: 0.0, usage_final: false, input_tokens: 0, output_tokens: 0, search_calls: 0, end_reason: None,
        }
    }
    pub fn passages(&self) -> Vec<Passage> { passages(&self.fragments) }

    pub fn append(&mut self, fragment: Fragment) {
        if self.fragments.iter().any(|f| f.id == fragment.id) { return; }
        self.fragments.push(fragment);
        self.invalidate_changed_assessments();
    }

    pub fn invalidate_changed_assessments(&mut self) {
        let current: HashMap<String, String> = self.passages().into_iter().map(|p| (p.id.clone(), p.revision_key())).collect();
        self.assessments.retain(|a| current.get(&a.passage_id) == Some(&a.revision_key));
    }

    pub fn correct_fragment(&mut self, id: &str, text: &str) {
        let Some(index) = self.fragments.iter().position(|f| f.id == id) else { return };
        let old = self.fragments[index].text.clone();
        self.fragments[index].previous_texts.push(old);
        self.fragments[index].text = text.into();
        self.fragments[index].revision += 1;
        self.translations.retain(|key, _| !Self::translation_key_includes(key, id));
        self.invalidate_changed_assessments();
    }

    fn translation_key_includes(key: &str, id: &str) -> bool {
        let revision = key.split_once("::").map(|(_, r)| r).unwrap_or(key);
        revision.split(',').any(|part| part.split(':').next() == Some(id))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    #[serde(rename = "learningLanguageID")]
    pub learning_language_id: String,
    pub meaning_visible: bool,
    pub meaning_language: String,
    pub session_minutes: i64,
    #[serde(default)]
    pub hidden_words: Vec<String>,
    #[serde(default)]
    pub interests: String,
    #[serde(default)]
    pub has_onboarded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_consent_version: Option<i64>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            learning_language_id: DEFAULT_ID.into(), meaning_visible: true, meaning_language: "English".into(),
            session_minutes: 15, hidden_words: vec![], interests: String::new(), has_onboarded: false, ai_consent_version: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Archive {
    pub schema_version: i64,
    pub sessions: Vec<SessionRecord>,
    pub preferences: Preferences,
}

impl Default for Archive {
    fn default() -> Self { Self { schema_version: 2, sessions: vec![], preferences: Preferences::default() } }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArchiveError { TooLarge, UnsupportedVersion, UnsupportedLanguage, Invalid }

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ArchiveError::TooLarge => "This backup is too large to import.",
            ArchiveError::UnsupportedVersion => "This backup needs a newer version of Mural.",
            ArchiveError::UnsupportedLanguage => "This backup contains a language module that this version of Mural does not support.",
            ArchiveError::Invalid => "This backup has invalid or duplicate records.",
        })
    }
}
impl std::error::Error for ArchiveError {}

impl Archive {
    pub const MAXIMUM_ENCODED_BYTES: usize = 30_000_000;
    const MAXIMUM_SESSIONS: usize = 10_000;

    pub fn decode(data: &[u8]) -> Result<Archive, ArchiveError> {
        if data.len() > Self::MAXIMUM_ENCODED_BYTES { return Err(ArchiveError::TooLarge); }
        let migrated = Self::migrate(data)?;
        let archive: Archive = serde_json::from_value(migrated).map_err(|_| ArchiveError::Invalid)?;
        archive.validate()?;
        Ok(archive)
    }

    /// Bound the selected file before loading it.
    pub fn read_import_data(path: &std::path::Path) -> Result<Vec<u8>, ArchiveError> {
        use std::io::Read;
        let meta = std::fs::metadata(path).map_err(|_| ArchiveError::Invalid)?;
        if !meta.is_file() { return Err(ArchiveError::Invalid); }
        if meta.len() as usize > Self::MAXIMUM_ENCODED_BYTES { return Err(ArchiveError::TooLarge); }
        let file = std::fs::File::open(path).map_err(|_| ArchiveError::Invalid)?;
        let mut data = Vec::new();
        file.take(Self::MAXIMUM_ENCODED_BYTES as u64 + 1).read_to_end(&mut data).map_err(|_| ArchiveError::Invalid)?;
        if data.len() > Self::MAXIMUM_ENCODED_BYTES { return Err(ArchiveError::TooLarge); }
        Ok(data)
    }

    /// Reject the complete candidate before changing local history, so it remains readable on relaunch.
    pub fn merging(&self, incoming: &Archive) -> Result<Archive, ArchiveError> {
        incoming.validate()?;
        let known: HashSet<&str> = self.sessions.iter().map(|s| s.id.as_str()).collect();
        let additions: Vec<&SessionRecord> = incoming.sessions.iter().filter(|s| !known.contains(s.id.as_str())).collect();
        if additions.len() > Self::MAXIMUM_SESSIONS - self.sessions.len() { return Err(ArchiveError::TooLarge); }
        let mut candidate = self.clone();
        for session in additions {
            let mut session = session.clone();
            session.invalidate_changed_assessments();
            let snapshot = session.clone();
            session.assessments = snapshot.assessments.iter().filter_map(|a| LearningEngine::validate(a, &snapshot)).collect();
            candidate.sessions.push(session);
        }
        candidate.validate()?;
        if candidate.encoded().map_err(|_| ArchiveError::Invalid)?.len() > Self::MAXIMUM_ENCODED_BYTES {
            return Err(ArchiveError::TooLarge);
        }
        Ok(candidate)
    }

    fn validate(&self) -> Result<(), ArchiveError> {
        if languages::module(&self.preferences.learning_language_id).is_none() { return Err(ArchiveError::UnsupportedLanguage); }
        let ids: HashSet<&str> = self.sessions.iter().map(|s| s.id.as_str()).collect();
        if ids.len() != self.sessions.len() || self.sessions.len() > Self::MAXIMUM_SESSIONS
            || !(1..=60).contains(&self.preferences.session_minutes) {
            return Err(ArchiveError::Invalid);
        }
        for s in &self.sessions {
            if languages::module(&s.language_id).is_none() { return Err(ArchiveError::UnsupportedLanguage); }
            let counts_ok = [s.input_tokens, s.output_tokens, s.search_calls].iter().all(|v| (0..=1_000_000_000).contains(v));
            let valid = s.voice_seconds.is_finite() && (0.0..=31_536_000.0).contains(&s.voice_seconds) && counts_ok
                && s.started_at.is_valid() && s.ended_at.map(|d| d.is_valid()).unwrap_or(true)
                && s.assessments.iter().all(|a| a.created_at.is_valid());
            if !valid { return Err(ArchiveError::Invalid); }
            let fragment_ids: HashSet<&str> = s.fragments.iter().map(|f| f.id.as_str()).collect();
            if fragment_ids.len() != s.fragments.len()
                || !s.fragments.iter().all(|f| f.start_ms >= 0 && f.end_ms >= f.start_ms && f.text.chars().count() <= 50_000
                    && (0..=1_000_000).contains(&f.revision) && f.received_at.is_valid())
                || !s.topics.iter().all(|t| t.language_id == s.language_id && t.retrieved_at.is_valid()) {
                return Err(ArchiveError::Invalid);
            }
        }
        Ok(())
    }

    /// Version 1 was Norwegian-only. Migration assigns that provenance once;
    /// version 2 records must explicitly declare their language.
    fn migrate(data: &[u8]) -> Result<Value, ArchiveError> {
        let mut root: Value = serde_json::from_slice(data).map_err(|_| ArchiveError::Invalid)?;
        let version = root.get("schemaVersion").and_then(Value::as_i64).ok_or(ArchiveError::Invalid)?;
        if version != 1 && version != 2 { return Err(ArchiveError::UnsupportedVersion); }
        if version == 2 { return Ok(root); }
        let obj = root.as_object_mut().ok_or(ArchiveError::Invalid)?;
        {
            let prefs = obj.get_mut("preferences").and_then(Value::as_object_mut).ok_or(ArchiveError::Invalid)?;
            prefs.insert("learningLanguageID".into(), DEFAULT_ID.into());
            if let Some(hidden) = prefs.get("hiddenWords").and_then(Value::as_array).cloned() {
                let mapped: Vec<Value> = hidden.iter().filter_map(Value::as_str).map(|h| Value::from(format!("{DEFAULT_ID}|{h}"))).collect();
                prefs.insert("hiddenWords".into(), Value::Array(mapped));
            }
        }
        let sessions = obj.get_mut("sessions").and_then(Value::as_array_mut).ok_or(ArchiveError::Invalid)?;
        for session in sessions.iter_mut() {
            let s = session.as_object_mut().ok_or(ArchiveError::Invalid)?;
            s.insert("languageID".into(), DEFAULT_ID.into());
            if let Some(topics) = s.get_mut("topics").and_then(Value::as_array_mut) {
                for topic in topics.iter_mut() {
                    if let Some(t) = topic.as_object_mut() { t.insert("languageID".into(), DEFAULT_ID.into()); }
                }
            }
        }
        obj.insert("schemaVersion".into(), 2.into());
        Ok(root)
    }

    /// Pretty-printed with sorted keys, like the iPhone export.
    pub fn encoded(&self) -> serde_json::Result<Vec<u8>> {
        let value = sorted(serde_json::to_value(self)?);
        serde_json::to_vec_pretty(&value)
    }
}

fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<String, Value> = map.into_iter().map(|(k, v)| (k, sorted(v))).collect();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

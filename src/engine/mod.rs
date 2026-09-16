pub mod activity;
pub mod captions;
mod content;
pub mod langdetect;
pub mod languages;
pub mod learning;
pub mod models;
pub mod pace;
pub mod pinyin;
pub mod provider;
pub mod teaching;
pub mod text;
pub mod time;

#[cfg(test)]
mod tests;

pub const AI_CONSENT_VERSION: i64 = 1;
pub const AI_CONSENT_SUMMARY: &str = "With your permission, Mural sends audio and selected text to OpenAI to provide conversations and meanings. Provider retention rules apply.";
pub const AI_CONSENT_REQUIRED: &str = "Before using AI features, open Talk and click the microphone to review how OpenAI processes your audio and text.";

pub fn meaning_cache_key(revision_key: &str, language: &str) -> String { format!("{language}::{revision_key}") }

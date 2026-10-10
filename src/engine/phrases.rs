//! The shadowing catalogue: short everyday phrases to hear, repeat and be scored on.
//!
//! Content lives in `assets/phrases/<language>.json` so it reads as data rather than Rust, and a
//! second language is one more file. Only languages with a file appear in the Shadow tab.

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Phrase {
    pub id: String,
    pub text: String,
    /// A short English sense, shown when meaning subtitles are on.
    pub meaning: String,
    pub category: String,
    /// The same internal 0–5 challenge scale the teaching prompts use.
    pub level: i64,
}

#[derive(Debug, Deserialize)]
struct Catalogue {
    #[allow(dead_code)]
    language: String,
    #[allow(dead_code)]
    version: i64,
    phrases: Vec<Phrase>,
}

const GERMAN: &str = include_str!("../../assets/phrases/de.json");

/// Every phrase for a language, in catalogue order, or empty when there is no list yet.
pub fn all(language_id: &str) -> &'static [Phrase] {
    static GERMAN_PHRASES: OnceLock<Vec<Phrase>> = OnceLock::new();
    match language_id {
        "de" => GERMAN_PHRASES.get_or_init(|| {
            serde_json::from_str::<Catalogue>(GERMAN).expect("German phrase catalogue").phrases
        }),
        _ => &[],
    }
}

pub fn available(language_id: &str) -> bool { !all(language_id).is_empty() }

pub fn find<'a>(language_id: &str, id: &str) -> Option<&'a Phrase> {
    all(language_id).iter().find(|p| p.id == id)
}

/// The categories present, in the order they first appear.
pub fn categories(language_id: &str) -> Vec<&'static str> {
    let mut seen: Vec<&str> = vec![];
    for phrase in all(language_id) {
        if !seen.contains(&phrase.category.as_str()) { seen.push(&phrase.category); }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_german_catalogue_is_well_formed() {
        let phrases = all("de");
        assert!(phrases.len() >= 200, "only {} phrases", phrases.len());

        let mut ids: Vec<&str> = phrases.iter().map(|p| p.id.as_str()).collect();
        ids.sort_unstable();
        let unique = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), unique, "duplicate phrase id");

        let mut texts: Vec<&str> = phrases.iter().map(|p| p.text.as_str()).collect();
        texts.sort_unstable();
        let unique = texts.len();
        texts.dedup();
        assert_eq!(texts.len(), unique, "the same phrase appears twice");

        for p in phrases {
            assert!(!p.text.trim().is_empty(), "{} has no text", p.id);
            assert!(!p.meaning.trim().is_empty(), "{} has no meaning", p.id);
            assert!(!p.category.trim().is_empty(), "{} has no category", p.id);
            assert!((0..=5).contains(&p.level), "{} has level {}", p.id, p.level);
            // Shadowing is for phrases a learner can hold in their head and say in one breath.
            assert!(p.text.chars().count() <= 60, "{} is too long to shadow: {}", p.id, p.text);
        }
    }

    #[test]
    fn languages_without_a_list_are_simply_empty() {
        assert!(all("nb").is_empty());
        assert!(!available("nb"));
        assert!(available("de"));
    }

    #[test]
    fn phrases_can_be_found_and_grouped() {
        let first = &all("de")[0];
        assert_eq!(find("de", &first.id), Some(first));
        assert_eq!(find("de", "de-9999"), None);
        assert!(categories("de").len() >= 8);
    }
}

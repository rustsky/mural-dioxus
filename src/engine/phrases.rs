//! The shadowing catalogue: short everyday phrases to hear, repeat and be scored on.
//!
//! Content lives in `assets/phrases/<language>.json` so it reads as data rather than Rust, and a
//! second language is one more file. Only languages with a file appear in the Shadow tab.

use std::sync::OnceLock;

use serde::Deserialize;

/// How demanding a phrase is, in the levels learners already know from a course. It describes the
/// phrase, never the learner: Mural judges ability from conversation and makes no CEFR claim.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
pub enum Band { A1, A2, B1, B2, C1 }

impl Band {
    pub const ALL: [Band; 5] = [Band::A1, Band::A2, Band::B1, Band::B2, Band::C1];

    pub fn label(self) -> &'static str {
        match self { Band::A1 => "A1", Band::A2 => "A2", Band::B1 => "B1", Band::B2 => "B2", Band::C1 => "C1" }
    }
    /// What a learner at this band can do with the phrases in it.
    pub fn gist(self) -> &'static str {
        match self {
            Band::A1 => "First words and set phrases",
            Band::A2 => "Everyday errands and simple exchanges",
            Band::B1 => "Connected talk, plans and reasons",
            Band::B2 => "Nuance, opinions and careful wording",
            Band::C1 => "Idiom, register and lightness of touch",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Phrase {
    pub id: String,
    pub text: String,
    /// A short English sense, shown when meaning subtitles are on.
    pub meaning: String,
    pub category: String,
    /// The same internal 0–5 challenge scale the teaching prompts use.
    pub level: i64,
    pub cefr: Band,
}

#[derive(Debug, Deserialize)]
struct Catalogue {
    #[allow(dead_code)]
    language: String,
    #[allow(dead_code)]
    version: i64,
    phrases: Vec<Phrase>,
}

/// The catalogues that ship with the app, by language id.
const CATALOGUES: &[(&str, &str)] = &[
    ("de", include_str!("../../assets/phrases/de.json")),
    ("es", include_str!("../../assets/phrases/es.json")),
    ("fr", include_str!("../../assets/phrases/fr.json")),
    ("it", include_str!("../../assets/phrases/it.json")),
    ("pt", include_str!("../../assets/phrases/pt.json")),
    ("en", include_str!("../../assets/phrases/en.json")),
    ("nb", include_str!("../../assets/phrases/nb.json")),
];

/// Every phrase for a language, in catalogue order, or empty when there is no list yet.
pub fn all(language_id: &str) -> &'static [Phrase] {
    static LOADED: OnceLock<Vec<(&'static str, Vec<Phrase>)>> = OnceLock::new();
    let loaded = LOADED.get_or_init(|| CATALOGUES.iter().map(|(id, json)| {
        let phrases = serde_json::from_str::<Catalogue>(json).unwrap_or_else(|e| panic!("{id} phrase catalogue: {e}")).phrases;
        (*id, phrases)
    }).collect());
    loaded.iter().find(|(id, _)| *id == language_id).map(|(_, p)| p.as_slice()).unwrap_or(&[])
}

/// The languages that have a phrase list.
pub fn languages() -> impl Iterator<Item = &'static str> { CATALOGUES.iter().map(|(id, _)| *id) }

pub fn available(language_id: &str) -> bool { !all(language_id).is_empty() }

/// The bands that actually have phrases, lowest first.
pub fn bands(language_id: &str) -> Vec<Band> {
    Band::ALL.into_iter().filter(|b| all(language_id).iter().any(|p| p.cefr == *b)).collect()
}

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

/// One of `candidates`, chosen by `seed`. The caller decides what counts as a candidate; this is
/// kept separate so the choice can be tested without a random source.
pub fn pick<'a>(candidates: &[&'a Phrase], seed: u128) -> Option<&'a Phrase> {
    if candidates.is_empty() { return None }
    candidates.get((seed % candidates.len() as u128) as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalogue_is_well_formed() {
        for language in languages() { catalogue_is_well_formed(language) }
    }

    fn catalogue_is_well_formed(language: &str) {
        let phrases = all(language);
        assert_eq!(phrases.len(), 1500, "{language}");

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
            assert!(p.id.starts_with(&format!("{language}-")), "{} is filed under {language}", p.id);
        }
    }

    #[test]
    fn every_band_has_phrases_and_they_rise_in_order() {
        for language in languages() {
            assert_eq!(bands(language), Band::ALL.to_vec(), "{language}");
            for band in Band::ALL {
                let count = all(language).iter().filter(|p| p.cefr == band).count();
                assert_eq!(count, 300, "{language} {} has {count} phrases", band.label());
            }
        }
        assert!(Band::A1 < Band::C1);
        assert!(bands("xx").is_empty());
    }

    #[test]
    fn a_phrases_band_matches_its_teaching_level() {
        // The two scales are written side by side, so they must not drift apart.
        for p in languages().flat_map(all) {
            let expected = match p.level { 0 | 1 => Band::A1, 2 => Band::A2, 3 => Band::B1, 4 => Band::B2, _ => Band::C1 };
            assert_eq!(p.cefr, expected, "{} is level {} but band {}", p.id, p.level, p.cefr.label());
        }
    }

    #[test]
    fn languages_without_a_list_are_simply_empty() {
        assert!(all("xx").is_empty());
        assert!(!available("xx"));
        assert!(available("de"));
    }

    #[test]
    fn picking_handles_none_one_and_many() {
        let all = all("de");
        assert_eq!(pick(&[], 7), None);

        let one: Vec<&Phrase> = all.iter().take(1).collect();
        assert_eq!(pick(&one, 0), Some(one[0]));
        assert_eq!(pick(&one, 12_345), Some(one[0]), "one candidate is always the answer");

        let few: Vec<&Phrase> = all.iter().take(4).collect();
        assert_eq!(pick(&few, 0), Some(few[0]));
        assert_eq!(pick(&few, 6), Some(few[2]));
        // Different seeds reach every candidate, so nothing is unreachable.
        let reached: std::collections::HashSet<&str> =
            (0..40u128).filter_map(|seed| pick(&few, seed)).map(|p| p.id.as_str()).collect();
        assert_eq!(reached.len(), 4);
    }

    #[test]
    fn phrases_can_be_found_and_grouped() {
        let first = &all("de")[0];
        assert_eq!(find("de", &first.id), Some(first));
        assert_eq!(find("de", "de-9999"), None);
        assert!(categories("de").len() >= 8);
    }
}

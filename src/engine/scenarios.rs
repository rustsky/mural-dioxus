//! The Dialog catalogue: everyday situations to talk through from beginning to end.
//!
//! A scenario is a brief, not a script. It says who each of you is, what the learner wants, and
//! the beats the exchange should pass through; the conversation itself is improvised as usual.
//! Content lives in `assets/scenarios/<language>.json`, one file per language.

use std::sync::OnceLock;

use serde::Deserialize;

use super::phrases::Band;

/// One step of the arc. German for the learner, English underneath.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Beat {
    pub de: String,
    pub en: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub id: String,
    pub title: String,
    pub title_en: String,
    pub category: String,
    pub cefr: Band,
    /// How long the exchange should run. Mural is told to start closing near the end.
    pub minutes: i64,
    /// Stage directions for the teacher, never shown as something to say.
    pub setting: String,
    pub partner: String,
    pub goal: String,
    pub goal_en: String,
    pub beats: Vec<Beat>,
}

#[derive(Debug, Deserialize)]
struct Catalogue {
    #[allow(dead_code)]
    language: String,
    #[allow(dead_code)]
    version: i64,
    scenarios: Vec<Scenario>,
}

const GERMAN: &str = include_str!("../../assets/scenarios/de.json");

pub fn all(language_id: &str) -> &'static [Scenario] {
    static GERMAN_SCENARIOS: OnceLock<Vec<Scenario>> = OnceLock::new();
    match language_id {
        "de" => GERMAN_SCENARIOS.get_or_init(|| {
            serde_json::from_str::<Catalogue>(GERMAN).expect("German scenario catalogue").scenarios
        }),
        _ => &[],
    }
}

pub fn available(language_id: &str) -> bool { !all(language_id).is_empty() }

pub fn find(language_id: &str, id: &str) -> Option<&'static Scenario> {
    all(language_id).iter().find(|s| s.id == id)
}

/// The categories present, in the order they first appear.
pub fn categories(language_id: &str) -> Vec<&'static str> {
    let mut seen: Vec<&str> = vec![];
    for scenario in all(language_id) {
        if !seen.contains(&scenario.category.as_str()) { seen.push(&scenario.category); }
    }
    seen
}

/// The bands that have scenarios, lowest first.
pub fn bands(language_id: &str) -> Vec<Band> {
    Band::ALL.into_iter().filter(|b| all(language_id).iter().any(|s| s.cefr == *b)).collect()
}

impl Scenario {
    /// Beats are reported by index; anything else is a model slip and is ignored.
    pub fn valid_beat(&self, index: usize) -> bool { index < self.beats.len() }
}

/// Steps checked after each turn. A scenario has up to 25, so asking about all of them would cost
/// a request each; the conversation is only ever near a handful.
pub const BEAT_WINDOW: usize = 5;
/// Steps a single turn may tick off. Steps are small, so one turn can cover a few.
pub const BEATS_PER_TURN: usize = 3;

/// Reads the checklist answer ("2, 5, 7" or "none"): the steps it names, counted from zero, that
/// exist and are not already done.
pub fn parse_checklist(answer: &str, total: usize, done: &[usize]) -> Vec<usize> {
    let mut found: Vec<usize> = answer.split(|c: char| !c.is_ascii_digit())
        .filter_map(|n| n.parse::<usize>().ok())
        .filter(|&n| n >= 1 && n <= total)
        .map(|n| n - 1)
        .filter(|i| !done.contains(i))
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// The unfinished steps worth asking about: the next few after the furthest one reached. Steps
/// passed over earlier stay open, but no longer hold up the ones ahead.
pub fn beat_window(total: usize, done: &[usize]) -> Vec<usize> {
    let from = done.iter().max().map(|&i| i + 1).unwrap_or(0);
    (from..total).filter(|i| !done.contains(i)).take(BEAT_WINDOW).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_german_catalogue_is_well_formed() {
        let scenarios = all("de");
        assert!(!scenarios.is_empty());

        let mut ids: Vec<&str> = scenarios.iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        let unique = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), unique, "duplicate scenario id");

        let mut titles: Vec<&str> = scenarios.iter().map(|s| s.title.as_str()).collect();
        titles.sort_unstable();
        let unique = titles.len();
        titles.dedup();
        assert_eq!(titles.len(), unique, "the same scenario title appears twice");

        for s in scenarios {
            for (field, value) in [("title", &s.title), ("titleEn", &s.title_en), ("goal", &s.goal),
                                   ("goalEn", &s.goal_en), ("setting", &s.setting), ("partner", &s.partner),
                                   ("category", &s.category)] {
                assert!(!value.trim().is_empty(), "{} has no {field}", s.id);
            }
            // Finer steps the higher the level, and about a minute for every two of them.
            let expected = match s.cefr { Band::A1 => 12, Band::A2 => 15, Band::B1 => 18, Band::B2 => 21, Band::C1 => 25 };
            assert_eq!(s.beats.len(), expected, "{} ({}) has {} beats", s.id, s.cefr.label(), s.beats.len());
            assert_eq!(s.minutes as usize, expected.div_ceil(2), "{} runs {} minutes", s.id, s.minutes);
            for beat in &s.beats {
                assert!(!beat.de.trim().is_empty() && !beat.en.trim().is_empty(), "{} has an empty beat", s.id);
            }
        }
    }

    #[test]
    fn beats_are_addressed_by_index() {
        let s = &all("de")[0];
        assert!(s.valid_beat(0));
        assert!(s.valid_beat(s.beats.len() - 1));
        assert!(!s.valid_beat(s.beats.len()), "an index past the end is not a beat");
        assert!(!s.valid_beat(99));
    }

    #[test]
    fn only_the_next_few_steps_are_checked() {
        assert_eq!(beat_window(25, &[]), vec![0, 1, 2, 3, 4]);
        // A skipped greeting stays open but no longer holds the window back.
        assert_eq!(beat_window(25, &[1, 2]), vec![3, 4, 5, 6, 7]);
        assert_eq!(beat_window(12, &[9, 10]), vec![11]);
        assert!(beat_window(12, &[11]).is_empty());
    }

    #[test]
    fn a_checklist_answer_names_new_steps_only() {
        assert_eq!(parse_checklist("1, 2, 4,9", 12, &[1]), vec![0, 3, 8]);
        assert_eq!(parse_checklist("none", 12, &[]), Vec::<usize>::new());
        // Numbers outside the list are model slips, and repeats count once.
        assert_eq!(parse_checklist("0, 3, 3, 13", 12, &[]), vec![2]);
    }

    #[test]
    fn languages_without_a_list_are_simply_empty() {
        assert!(all("nb").is_empty());
        assert!(!available("nb"));
        assert!(available("de"));
        assert!(bands("nb").is_empty());
    }

    #[test]
    fn scenarios_can_be_found_and_grouped() {
        let first = &all("de")[0];
        assert_eq!(find("de", &first.id), Some(first));
        assert_eq!(find("de", "sc-9999"), None);
        assert!(categories("de").len() >= 5);
        assert_eq!(bands("de"), Band::ALL.to_vec());
    }
}

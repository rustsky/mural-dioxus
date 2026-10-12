//! Comparing what the learner said with the phrase they were shadowing.
//!
//! Recognition is approximate and learners are not typists: case, punctuation and the usual
//! German spelling variants must not count against them. What should count is whether the words
//! came out, in the right order. Chinese is written without spaces, so there each character
//! counts as a word.

/// One word of the phrase, and whether the learner said it.
#[derive(Clone, Debug, PartialEq)]
pub struct WordMatch {
    pub word: String,
    pub matched: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attempt {
    /// 0–1, how much of the phrase came through.
    pub score: f64,
    pub words: Vec<WordMatch>,
}

impl Attempt {
    pub fn verdict(&self) -> Verdict {
        if self.score >= 0.85 { Verdict::Good } else if self.score >= 0.6 { Verdict::Close } else { Verdict::Again }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict { Good, Close, Again }

impl Verdict {
    pub fn label(self) -> &'static str {
        match self { Verdict::Good => "That sounded right", Verdict::Close => "Close", Verdict::Again => "Try that again" }
    }
    pub fn class(self) -> &'static str {
        match self { Verdict::Good => "good", Verdict::Close => "close", Verdict::Again => "again" }
    }
}

/// Spelling variants a learner should never be marked down for: ß written as ss, and umlauts
/// written out, which is both a keyboard habit and a common recogniser output.
fn fold(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for c in word.chars().flat_map(|c| c.to_lowercase()) {
        match c {
            'ß' => out.push_str("ss"),
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            _ => out.push(c),
        }
    }
    out
}

/// Chinese characters, which carry a syllable each and are written without spaces between words.
fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2FA1F}')
}

/// The words as written, stripped of the punctuation that speech has no sound for. Digits stay:
/// "8 Uhr" is said. Each Chinese character stands alone.
fn tokens(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut word = String::new();
    for c in text.chars() {
        if is_han(c) {
            if !word.is_empty() { out.push(std::mem::take(&mut word)); }
            out.push(c.to_string());
        } else if c.is_alphanumeric() || c == '\'' {
            word.push(c);
        } else if !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() { out.push(word); }
    out
}

fn words(text: &str) -> Vec<String> { tokens(text).iter().map(|w| fold(w)).collect() }

/// Which words of the phrase turned up in what was heard, in order.
///
/// This is a longest common subsequence rather than the edit traceback: several edit paths can
/// cost the same, and the cheapest one is free to step over a word the learner clearly said. The
/// subsequence takes the reading most generous to the learner, which is the honest one to show.
fn said(want: &[String], got: &[String]) -> Vec<bool> {
    let (n, m) = (want.len(), got.len());
    let mut common = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            common[i][j] = if want[i] == got[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let mut matched = vec![false; n];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if want[i] == got[j] {
            matched[i] = true;
            i += 1;
            j += 1;
        } else if common[i + 1][j] >= common[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    matched
}

/// Scores `heard` against `expected`, and says which words of the phrase came through.
pub fn score(expected: &str, heard: &str) -> Attempt {
    let want = words(expected);
    let got = words(heard);
    // The display keeps the original spelling; only the comparison is folded.
    let shown = tokens(expected);

    if want.is_empty() {
        return Attempt { score: 0.0, words: vec![] };
    }
    if got.is_empty() {
        return Attempt { score: 0.0, words: shown.into_iter().map(|w| WordMatch { word: w, matched: false }).collect() };
    }

    // Levenshtein over words gives the score; the matched words come from the subsequence above.
    let (n, m) = (want.len(), got.len());
    let mut cost = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in cost.iter_mut().enumerate() { row[0] = i; }
    for (j, slot) in cost[0].iter_mut().enumerate() { *slot = j; }
    for i in 1..=n {
        for j in 1..=m {
            let same = want[i - 1] == got[j - 1];
            cost[i][j] = if same {
                cost[i - 1][j - 1]
            } else {
                1 + cost[i - 1][j - 1].min(cost[i - 1][j]).min(cost[i][j - 1])
            };
        }
    }

    let distance = cost[n][m] as f64;
    let matched = said(&want, &got);
    let score = (1.0 - distance / n.max(m) as f64).clamp(0.0, 1.0);
    let words = shown.into_iter().enumerate()
        .map(|(i, word)| WordMatch { word, matched: matched.get(i).copied().unwrap_or(false) })
        .collect();
    Attempt { score, words }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missed(attempt: &Attempt) -> Vec<&str> {
        attempt.words.iter().filter(|w| !w.matched).map(|w| w.word.as_str()).collect()
    }

    #[test]
    fn an_exact_repeat_scores_full_marks() {
        let a = score("Wie geht es dir?", "Wie geht es dir");
        assert_eq!(a.score, 1.0);
        assert_eq!(a.verdict(), Verdict::Good);
        assert!(missed(&a).is_empty());
    }

    #[test]
    fn case_and_punctuation_are_not_mistakes() {
        assert_eq!(score("Wie geht es dir?", "wie geht es dir!").score, 1.0);
        assert_eq!(score("Ich hätte gern einen Kaffee.", "ich hätte gern einen kaffee").score, 1.0);
    }

    #[test]
    fn german_spelling_variants_are_accepted() {
        // Recognisers and keyboards both produce these; a learner said the word either way.
        assert_eq!(score("Ich hätte gern Straße.", "ich haette gern strasse").score, 1.0);
        assert_eq!(score("Schöne Grüße!", "schoene gruesse").score, 1.0);
    }

    #[test]
    fn a_missing_word_is_named() {
        let a = score("Ich hätte gern einen Kaffee", "Ich hätte einen Kaffee");
        assert_eq!(missed(&a), vec!["gern"]);
        assert!(a.score > 0.6 && a.score < 1.0, "{}", a.score);
        assert_eq!(a.verdict(), Verdict::Close);
    }

    #[test]
    fn a_wrong_word_is_marked_without_losing_the_rest() {
        let a = score("Ich fahre nach Berlin", "Ich fahre nach München");
        assert_eq!(missed(&a), vec!["Berlin"]);
    }

    #[test]
    fn word_order_matters_but_is_not_a_total_loss() {
        let a = score("Kannst du mir helfen", "Du kannst mir helfen");
        assert!(a.score > 0.4 && a.score < 1.0, "{}", a.score);
    }

    #[test]
    fn saying_nothing_scores_nothing_and_marks_every_word() {
        let a = score("Wie geht es dir?", "");
        assert_eq!(a.score, 0.0);
        assert_eq!(a.verdict(), Verdict::Again);
        assert_eq!(missed(&a).len(), 4);
    }

    #[test]
    fn something_else_entirely_scores_nothing() {
        let a = score("Wie geht es dir?", "Das Wetter ist heute schön");
        assert_eq!(a.score, 0.0);
    }

    #[test]
    fn rambling_past_the_phrase_costs_something() {
        let a = score("Guten Morgen", "Guten Morgen und wie geht es Ihnen heute");
        assert!(a.score < 0.6, "{}", a.score);
        // The phrase itself still came through, so nothing is marked missing.
        assert!(missed(&a).is_empty());
    }

    #[test]
    fn a_word_that_was_said_is_never_marked_missing() {
        // The cheapest edit path can step over "wie"; the learner still said it.
        let a = score("Wie geht es Ihnen?", "Hallo wie gehts");
        assert_eq!(missed(&a), vec!["geht", "es", "Ihnen"]);
        assert!(a.words[0].matched, "“Wie” was said");
    }

    #[test]
    fn an_empty_phrase_cannot_be_scored() {
        assert_eq!(score("", "irgendwas"), Attempt { score: 0.0, words: vec![] });
    }

    #[test]
    fn chinese_is_scored_character_by_character() {
        let a = score("我想要一杯咖啡。", "我要一杯咖啡");
        assert_eq!(missed(&a), vec!["想"]);
        assert!(a.score > 0.8 && a.score < 1.0, "{}", a.score);
        assert_eq!(score("你好吗？", "你好吗").score, 1.0);
    }

    #[test]
    fn accents_and_apostrophes_of_other_languages_are_kept() {
        assert_eq!(score("J'ai un rendez-vous à midi.", "j'ai un rendez vous à midi").score, 1.0);
        assert_eq!(missed(&score("¿Dónde está la estación?", "dónde está estación")), vec!["la"]);
    }

    #[test]
    fn numbers_are_kept_because_they_are_spoken() {
        assert_eq!(score("Um 8 Uhr", "um 8 uhr").score, 1.0);
    }
}

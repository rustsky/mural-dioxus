//! Comparing what the learner said with the phrase they were shadowing.
//!
//! Recognition is approximate and learners are not typists: case, punctuation, accents the
//! recogniser chose to write or not, digits for number words and the usual German spelling
//! variants must not count against them. What should count is whether the words
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

/// Spelling variants a learner should never be marked down for: ß written as ss, umlauts
/// written out, and accents, which are the recogniser's spelling rather than the learner's
/// sound ("como estas" for "cómo estás"). ñ and the Nordic å stay, as they are sounds of their own.
fn fold(word: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut out = String::with_capacity(word.len());
    for c in word.chars().flat_map(|c| c.to_lowercase()) {
        match c {
            'ß' => out.push_str("ss"),
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ñ' | 'å' => out.push(c),
            c => out.extend(c.to_string().nfd().filter(|m| !unicode_normalization::char::is_combining_mark(*m))),
        }
    }
    number(&out).map(str::to_string).unwrap_or(out)
}

/// Recognisers write numbers as digits or words as they please; both are the same spoken word.
fn number(word: &str) -> Option<&'static str> {
    const WORDS: &[(&str, &[&str])] = &[
        ("0", &["zero", "cero", "null", "零"]),
        ("1", &["one", "uno", "una", "un", "une", "um", "uma", "eins", "en", "ett", "一"]),
        ("2", &["two", "dos", "deux", "due", "dois", "duas", "zwei", "to", "二", "两"]),
        ("3", &["three", "tres", "trois", "tre", "drei", "三"]),
        ("4", &["four", "cuatro", "quatre", "quattro", "quatro", "vier", "fire", "四"]),
        ("5", &["five", "cinco", "cinq", "cinque", "fuenf", "fem", "五"]),
        ("6", &["six", "seis", "sei", "sechs", "seks", "六"]),
        ("7", &["seven", "siete", "sept", "sette", "sete", "sieben", "sju", "七"]),
        ("8", &["eight", "ocho", "huit", "otto", "oito", "acht", "åtte", "八"]),
        ("9", &["nine", "nueve", "neuf", "nove", "neun", "ni", "九"]),
        ("10", &["ten", "diez", "dix", "dieci", "dez", "zehn", "ti", "十"]),
        ("12", &["twelve", "doce", "douze", "dodici", "doze", "zwoelf", "tolv"]),
        ("15", &["fifteen", "quince", "quinze", "quindici", "fuenfzehn", "femten"]),
        ("20", &["twenty", "veinte", "vingt", "venti", "vinte", "zwanzig", "tjue"]),
        ("30", &["thirty", "treinta", "trente", "trenta", "trinta", "dreissig", "tretti"]),
        ("100", &["hundred", "cien", "cent", "cento", "cem", "hundert", "hundre"]),
    ];
    if word.chars().all(|c| c.is_ascii_digit()) { return None; }
    // Words arrive folded: "fuenf", "tres", "dreissig".
    WORDS.iter().find(|(_, w)| w.contains(&word)).map(|(d, _)| *d)
}

/// Hesitations and markers that recognisers emit for silence; nobody is asked to say them.
fn filler(word: &str) -> bool { matches!(word, "sil" | "嗯" | "呃" | "啊" | "uh" | "uhm" | "hmm" | "mhm") }

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

fn words(text: &str) -> Vec<String> { tokens(text).iter().map(|w| fold(w)).filter(|w| !filler(w)).collect() }

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
    fn accents_are_the_recognisers_spelling_not_the_learners_sound() {
        assert_eq!(score("¿Cómo estás?", "como estas").score, 1.0);
        assert_eq!(score("Comment ça va ?", "comment ca va").score, 1.0);
        assert_eq!(score("Mais cinco minutinhos.", "mais cinco minutinhos").score, 1.0);
        // ñ is a sound of its own.
        assert!(score("Feliz año", "feliz ano").score < 1.0);
    }

    #[test]
    fn digits_and_number_words_are_the_same_word() {
        assert_eq!(score("Cinco minutos más.", "5 minutos más").score, 1.0);
        assert_eq!(score("Encore cinq minutes.", "Encore 5 minutes.").score, 1.0);
        assert_eq!(score("Noch fünf Minuten.", "noch 5 minuten").score, 1.0);
        assert_eq!(score("我要两杯咖啡", "我要2杯咖啡").score, 1.0);
    }

    #[test]
    fn hesitations_and_silence_markers_are_ignored() {
        assert_eq!(score("早上好！", "嗯 早上好").score, 1.0);
        assert_eq!(score("Guten Morgen", "<sil> Guten Morgen").score, 1.0);
    }

    #[test]
    fn numbers_are_kept_because_they_are_spoken() {
        assert_eq!(score("Um 8 Uhr", "um 8 uhr").score, 1.0);
    }
}

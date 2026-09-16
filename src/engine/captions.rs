use super::pinyin;

#[derive(Clone, Debug, PartialEq)]
pub struct CaptionSegment {
    pub text: String,
    pub lookup: Option<String>,
}

/// Keeps every source character, linking Chinese words instead of whole sentences.
pub fn segments(text: &str, language_id: &str) -> Vec<CaptionSegment> {
    if language_id == "zh" {
        return pinyin::tokens(text).into_iter().map(|t| {
            let lookup = t.text.chars().any(char::is_alphabetic).then(|| t.text.clone());
            CaptionSegment { text: t.text, lookup }
        }).collect();
    }
    let mut result = Vec::new();
    let mut run = String::new();
    for c in text.chars() {
        if let Some(last) = run.chars().last() {
            if last.is_whitespace() != c.is_whitespace() {
                result.push(segment(&run));
                run.clear();
            }
        }
        run.push(c);
    }
    if !run.is_empty() { result.push(segment(&run)); }
    result
}

fn segment(text: &str) -> CaptionSegment {
    let word = text.trim_matches(|c: char| c.is_whitespace() || super::text::is_punctuation(c));
    CaptionSegment { text: text.into(), lookup: word.chars().any(char::is_alphabetic).then(|| word.to_string()) }
}

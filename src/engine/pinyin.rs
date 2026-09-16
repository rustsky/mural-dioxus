// Builds on richardguerre's Mandarin contribution in Chuloo/mural#4.
use super::text::contains_han;

#[derive(Clone, Debug, PartialEq)]
pub struct PronunciationToken {
    pub text: String,
    pub pinyin: Option<String>,
}

/// Uses the system's word readings so 银行 and 旅行 keep different readings of 行.
/// Punctuation, spacing and unrecognized characters remain exactly as supplied.
#[cfg(target_os = "macos")]
pub fn tokens(text: &str) -> Vec<PronunciationToken> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{kCFAllocatorDefault, CFRange, CFRelease};
    use core_foundation_sys::locale::CFLocaleCreate;
    use core_foundation_sys::string_tokenizer::*;
    use unicode_normalization::UnicodeNormalization;

    if text.is_empty() { return vec![]; }
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let len = utf16.len() as isize;
    let slice = |start: isize, end: isize| String::from_utf16_lossy(&utf16[start as usize..end as usize]);
    let source = CFString::new(text);
    let locale_id = CFString::new("zh_CN");
    let mut result = Vec::new();
    unsafe {
        let locale = CFLocaleCreate(kCFAllocatorDefault, locale_id.as_concrete_TypeRef());
        let tokenizer = CFStringTokenizerCreate(
            kCFAllocatorDefault, source.as_concrete_TypeRef(), CFRange { location: 0, length: len },
            kCFStringTokenizerUnitWord, locale,
        );
        if tokenizer.is_null() {
            if !locale.is_null() { CFRelease(locale as _); }
            return vec![PronunciationToken { text: text.into(), pinyin: None }];
        }
        let mut cursor: isize = 0;
        while CFStringTokenizerAdvanceToNextToken(tokenizer) != kCFStringTokenizerTokenNone {
            let range = CFStringTokenizerGetCurrentTokenRange(tokenizer);
            if range.location < cursor || range.length <= 0 || range.location + range.length > len { continue; }
            if range.location > cursor {
                result.push(PronunciationToken { text: slice(cursor, range.location), pinyin: None });
            }
            let word = slice(range.location, range.location + range.length);
            let mut reading = None;
            if contains_han(&word) {
                let attr = CFStringTokenizerCopyCurrentTokenAttribute(tokenizer, kCFStringTokenizerAttributeLatinTranscription);
                if !attr.is_null() {
                    let value = CFType::wrap_under_create_rule(attr);
                    if let Some(latin) = value.downcast::<CFString>() {
                        // Apple's dictionary uses v for ü in some readings (e.g. 旅行).
                        let normalized: String = latin.to_string().to_lowercase()
                            .chars().map(|c| if c == 'v' { 'ü' } else { c }).collect::<String>()
                            .nfc().collect::<String>().trim().to_string();
                        if !normalized.is_empty() && !contains_han(&normalized) { reading = Some(normalized); }
                    }
                }
            }
            result.push(PronunciationToken { text: word, pinyin: reading });
            cursor = range.location + range.length;
        }
        if cursor < len { result.push(PronunciationToken { text: slice(cursor, len), pinyin: None }); }
        CFRelease(tokenizer as _);
        if !locale.is_null() { CFRelease(locale as _); }
    }
    result
}

#[cfg(not(target_os = "macos"))]
pub fn tokens(text: &str) -> Vec<PronunciationToken> {
    if text.is_empty() { vec![] } else { vec![PronunciationToken { text: text.into(), pinyin: None }] }
}

/// A separate reading aid; source text and learning evidence are never replaced.
pub fn reading(text: &str) -> Option<String> {
    let parts = tokens(text);
    if !parts.iter().any(|p| p.pinyin.is_some()) { return None; }
    let mut result = String::new();
    for part in parts {
        let value = part.pinyin.unwrap_or(part.text);
        if let (Some(last), Some(first)) = (result.chars().last(), value.chars().next()) {
            if last.is_alphanumeric() && first.is_alphanumeric() { result.push(' '); }
        }
        result.push_str(&value);
    }
    Some(result)
}

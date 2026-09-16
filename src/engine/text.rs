use unicode_general_category::{get_general_category, GeneralCategory as G};

/// Swift `Character.isPunctuation`: Unicode general category P*.
pub fn is_punctuation(c: char) -> bool {
    matches!(
        get_general_category(c),
        G::ConnectorPunctuation | G::DashPunctuation | G::OpenPunctuation | G::ClosePunctuation
            | G::InitialPunctuation | G::FinalPunctuation | G::OtherPunctuation
    )
}

pub fn is_open_punctuation(c: char) -> bool {
    matches!(get_general_category(c), G::OpenPunctuation | G::InitialPunctuation)
}

pub fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x323AF)
}

pub fn contains_han(text: &str) -> bool {
    text.chars().any(|c| matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F | 0x30000..=0x3347F))
}

/// Approximates Swift's `localizedCaseInsensitiveContains`.
pub fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

pub fn prefix(text: &str, count: usize) -> String { text.chars().take(count).collect() }

pub fn suffix(text: &str, count: usize) -> String {
    let total = text.chars().count();
    text.chars().skip(total.saturating_sub(count)).collect()
}

pub fn char_count(text: &str) -> usize { text.chars().count() }

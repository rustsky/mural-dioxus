use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub struct ConversationTheme {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub symbol: String,
    pub category: String,
    pub situation: String,
    pub color_index: usize,
}

impl ConversationTheme {
    pub fn new(id: &str, title: &str, subtitle: &str, symbol: &str, category: &str, situation: &str, color_index: usize) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: subtitle.into(),
            symbol: symbol.into(),
            category: category.into(),
            situation: situation.into(),
            color_index,
        }
    }
}

/// A target language's content and teaching policy. IDs are stable storage keys.
#[derive(Clone, Debug)]
pub struct LanguageModule {
    pub id: &'static str,
    pub name: &'static str,
    pub native_name: &'static str,
    pub variety: &'static str,
    #[allow(dead_code)]
    pub locale: &'static str,
    pub greeting: &'static str,
    #[allow(dead_code)]
    pub greeting_word: &'static str,
    pub speech_guidance: &'static str,
    pub writing_guidance: &'static str,
    pub lemma_guidance: &'static str,
    pub teaching_focus: [&'static str; 6],
    pub topic_placeholder: &'static str,
    pub lookup_unavailable_reply: &'static str,
    pub theme_overrides: Vec<ConversationTheme>,
}

impl LanguageModule {
    pub fn themes(&self) -> Vec<ConversationTheme> {
        super::content::shared_themes()
            .into_iter()
            .map(|t| self.theme_overrides.iter().find(|o| o.id == t.id).cloned().unwrap_or(t))
            .collect()
    }
    pub fn default_title(&self) -> String { format!("A little {}", self.name) }
    pub fn talk_title(&self) -> String { format!("A little everyday {}", self.name) }
    pub fn settings_title(&self) -> String { format!("{} · {}", self.name, self.variety) }
}

pub const DEFAULT_ID: &str = "nb";

pub fn all() -> &'static [LanguageModule] {
    static ALL: OnceLock<Vec<LanguageModule>> = OnceLock::new();
    ALL.get_or_init(super::content::all)
}

pub fn module(id: &str) -> Option<&'static LanguageModule> {
    all().iter().find(|m| m.id == id)
}

pub const MEANING_LANGUAGES: [&str; 11] = [
    "English", "French", "German", "Spanish", "Norwegian", "Portuguese", "Italian",
    "Chinese (Simplified)", "Polish", "Arabic", "Ukrainian",
];

pub fn meaning_greeting(language: &str) -> &'static str {
    match language {
        "English" => "Hi!",
        "French" => "Salut !",
        "German" => "Hallo!",
        "Spanish" => "¡Hola!",
        "Norwegian" => "Hei!",
        "Portuguese" => "Olá!",
        "Italian" => "Ciao!",
        "Chinese (Simplified)" | "Chinese" => "你好！",
        "Polish" => "Cześć!",
        "Arabic" => "مرحبًا!",
        "Ukrainian" => "Привіт!",
        _ => "Hi!",
    }
}

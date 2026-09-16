use dioxus::prelude::*;

use super::components::{Dialog, DialogButton, Markdown, PageHeading, PinyinHelp, RecallBars, SheetFrame, SourcesView};
use super::icons::Icon;
use super::state::{Mural, Sheet, Tab};
use crate::engine::meaning_cache_key;
use crate::engine::models::{SessionRecord, Speaker, TopicBrief};
use crate::engine::text::contains_ci;

#[component]
pub fn ThemesView() -> Element {
    let app = use_context::<Mural>();
    let mut search = use_signal(String::new);
    let mut category = use_signal(|| "All".to_string());
    let language = app.store.read().language();
    let all = language.themes();
    let mut categories = vec!["All".to_string()];
    for t in &all { if !categories.contains(&t.category) { categories.push(t.category.clone()); } }
    let query = search.read().clone();
    let themes: Vec<_> = all.into_iter().filter(|t| {
        (category() == "All" || t.category == category())
            && (query.is_empty() || contains_ci(&t.title, &query) || contains_ci(&t.category, &query))
    }).collect();
    let choose = move |theme| {
        app.choose_theme(theme);
        app.ui.write_unchecked().tab = Tab::Talk;
    };
    rsx! {
        div { class: "page stack",
            PageHeading { eyebrow: "A place to begin", title: "What’s on\nyour mind?", subtitle: "Same friend. Somewhere new." }
            div { class: "search",
                Icon { name: "magnifyingglass", size: 15 }
                input { r#type: "search", placeholder: "Find a conversation", value: "{search}", oninput: move |e| search.set(e.value()) }
            }
            button { class: "just-talk", onclick: move |_| choose(None),
                Icon { name: "waveform" } span { "Just talk" } span { class: "spacer" } Icon { name: "arrow.up.right" }
            }
            div { class: "chips",
                for c in categories {
                    button { key: "{c}", class: if category() == c { "chip selected" } else { "chip" },
                        aria_pressed: "{category() == c}",
                        onclick: { let c = c.clone(); move |_| category.set(c.clone()) },
                        "{c}"
                    }
                }
            }
            div { class: "theme-grid",
                for theme in themes.iter().cloned() {
                    button { key: "{theme.id}", class: "theme-card panel-{theme.color_index % 4}",
                        onclick: {
                            let theme = theme.clone();
                            move |_| if theme.id == "today" { app.open_sheet(Sheet::CurrentTopic) } else { choose(Some(theme.clone())) }
                        },
                        span { class: "secondary", Icon { name: theme.symbol.clone(), size: 28, weight: 1.2 } }
                        div {
                            div { class: "title", "{theme.title}" }
                            div { class: "subtitle", "{theme.subtitle}" }
                        }
                    }
                }
            }
            if themes.is_empty() {
                div { class: "empty-state",
                    h3 { "No Results for “{query}”" }
                    div { "Check the spelling or try a new search." }
                }
            }
        }
    }
}

#[component]
pub fn CurrentTopicSheet() -> Element {
    let app = use_context::<Mural>();
    let mut query = use_signal(String::new);
    let mut brief = use_signal(|| None::<TopicBrief>);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let placeholder = app.store.read().language().topic_placeholder;
    let mut find = move || {
        if loading() || query.read().trim().is_empty() { return; }
        loading.set(true);
        error.set(None);
        let q = query.read().clone();
        spawn(async move {
            match app.current_topic(q).await {
                Ok(b) => brief.set(Some(b)),
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    };
    rsx! {
        SheetFrame { leading: Some(("Close".to_string(), EventHandler::new(move |_| app.close_sheet()))),
            PageHeading { eyebrow: "The world today", title: "A fresh conversation.", subtitle: "What would you like to talk about?" }
            textarea { class: "field", rows: 2, placeholder, value: "{query}", autofocus: true,
                oninput: move |e| query.set(e.value()),
                onkeydown: move |e: Event<KeyboardData>| if e.key() == Key::Enter && !e.modifiers().shift() { e.prevent_default(); find(); },
            }
            button { class: "pill-button peach", disabled: loading() || query.read().trim().is_empty(), onclick: move |_| find(),
                span { if loading() { "Finding something interesting…" } else { "Find a topic" } }
                if loading() { span { class: "spinner" } } else { Icon { name: "magnifyingglass", size: 18 } }
            }
            if let Some(e) = error() { div { class: "footnote secondary", "{e}" } }
            if let Some(b) = brief() {
                Markdown { text: b.text.clone() }
                SourcesView { sources: b.sources.clone(), date: b.retrieved_at }
                button { class: "pill-button center",
                    onclick: move |_| {
                        if let Some(b) = brief() {
                            app.discuss(b);
                            app.close_sheet();
                            app.ui.write_unchecked().tab = Tab::Talk;
                        }
                    },
                    Icon { name: "waveform", size: 18 } "Talk about this"
                }
            }
            div { class: "footnote secondary",
                if crate::services::provider::current().teacher.is_ollama() { "Search uses your Ollama account. Sources stay attached to the topic." }
                else { "Search uses your OpenAI API account. Sources stay attached to the topic." }
            }
        }
    }
}

#[component]
pub fn WordsView() -> Element {
    let app = use_context::<Mural>();
    let mut search = use_signal(String::new);
    let store = app.store.read();
    let language = store.language();
    let learner = store.learner();
    let query = search.read().clone();
    let words: Vec<_> = learner.words.iter().filter(|w| query.is_empty() || contains_ci(&w.lemma, &query) || contains_ci(&w.meaning, &query)).cloned().collect();
    rsx! {
        div { class: "page stack",
            PageHeading { eyebrow: "Little by little · {language.name}", title: "Your words.", subtitle: "Familiar words, ready for another conversation." }
            div { class: "search",
                Icon { name: "magnifyingglass", size: 15 }
                input { r#type: "search", placeholder: "Find a word", value: "{search}", oninput: move |e| search.set(e.value()) }
            }
            if words.is_empty() {
                div { class: "card panel-2",
                    Icon { name: "leaf", size: 34, weight: 1.2 }
                    h2 { if query.is_empty() { "They’ll grow from here." } else { "No matching words yet." } }
                    div { class: "sub secondary",
                        if query.is_empty() { "As we talk, useful words and phrases find a home here. Their strength grows when you recall them over time." }
                        else { "Try another {language.name} word or English meaning." }
                    }
                }
            } else {
                div {
                    for word in words {
                        button { key: "{word.id}", class: "word-row",
                            onclick: { let id = word.id.clone(); move |_| app.open_sheet(Sheet::WordDetail(id.clone())) },
                            div { style: "flex: 1; display: flex; flex-direction: column; gap: 6px",
                                span { class: "lemma", "{word.lemma}" }
                                span { class: "sub secondary", "{word.meaning}" }
                            }
                            div { style: "display: flex; flex-direction: column; align-items: flex-end; gap: 8px",
                                RecallBars { count: word.bars }
                                span { class: "caption2 secondary", "{word.label()}" }
                            }
                        }
                    }
                }
            }
            div { class: "legend", span { "1 · Fragile" } span { "2 · Growing" } span { "3 · Steady" } }
            div { class: "footnote secondary", "The bars estimate spoken recall, not permanent mastery. Using a word with visible meanings counts as supported practice." }
            if !learner.capabilities.is_empty() {
                div { class: "capabilities",
                    h3 { "Finding your voice" }
                    for c in learner.capabilities.iter() { div { key: "{c}", class: "sub", "{c}" } }
                    div { class: "footnote secondary", "Observed across conversations. These are provisional, not formal level certificates." }
                }
            }
            div {
                button { class: "plain-action", onclick: move |_| app.open_sheet(Sheet::History),
                    Icon { name: "clock.arrow.circlepath", size: 16 } "Past conversations"
                }
            }
        }
    }
}

#[component]
pub fn WordDetailSheet(id: String) -> Element {
    let app = use_context::<Mural>();
    let store = app.store.read();
    let is_zh = store.language().id == "zh";
    let Some(word) = store.learner().words.into_iter().find(|w| w.id == id) else {
        return rsx! { SheetFrame { div { class: "secondary", "This word is no longer in your list." } } };
    };
    rsx! {
        SheetFrame {
            div { class: "big-word selectable", "{word.lemma}" }
            if is_zh { PinyinHelp { text: word.lemma.clone() } }
            div { class: "secondary", style: "font-size: 19px", "{word.meaning}" }
            div { class: "row", RecallBars { count: word.bars } span { class: "sub", "{word.label()}" } }
            div { style: "font-size: 15px", "{word.explanation()}" }
            div { class: "quote selectable", "“{word.example}”" }
            div { class: "footnote secondary", "{word.independent_count} independent uses · Last seen {word.last_seen.abbreviated()}" }
            div {
                button { class: "footnote danger",
                    onclick: move |_| { app.store.write_unchecked().hide_word(&id); app.close_sheet(); },
                    "Remove from my words"
                }
            }
        }
    }
}

#[component]
fn TranscriptBody(session: SessionRecord, meaning_language: String, #[props(default)] on_edit: Option<EventHandler<(String, String)>>) -> Element {
    let is_zh = session.language_id == "zh";
    let editable = session.ended_at.is_some();
    rsx! {
        for passage in session.passages() {
            div { key: "{passage.id}", class: "transcript-entry",
                div { class: "who",
                    span { if passage.speaker == Speaker::Assistant { "MURAL" } else { "YOU" } }
                    span { class: "spacer" }
                    if let (Some(handler), Speaker::User, true) = (on_edit, passage.speaker, editable) {
                        button { class: "caption",
                            onclick: { let (id, text) = (passage.id.clone(), passage.text()); move |_| handler.call((id.clone(), text.clone())) },
                            "Edit"
                        }
                    }
                }
                div { class: "said selectable", "{passage.text()}" }
                if is_zh { PinyinHelp { text: passage.text() } }
                if on_edit.is_none() {
                    if let Some(t) = session.translations.get(&meaning_cache_key(&passage.revision_key(), &meaning_language))
                        .or_else(|| session.translations.get(&passage.revision_key())) {
                        div { class: "sub secondary", "{t}" }
                    }
                }
            }
        }
        for topic in session.topics.iter() {
            div { key: "{topic.id}", class: "stack", style: "gap: 12px",
                Markdown { text: topic.text.clone() }
                SourcesView { sources: topic.sources.clone(), date: topic.retrieved_at }
            }
        }
    }
}

#[component]
pub fn TranscriptSheet() -> Element {
    let app = use_context::<Mural>();
    let session = app.conv.read().session.clone();
    let meaning_language = app.store.read().preferences().meaning_language.clone();
    rsx! {
        SheetFrame { title: "Our conversation",
            match session {
                Some(session) => {
                    let empty = session.fragments.is_empty() && session.topics.is_empty();
                    rsx! {
                        TranscriptBody { session, meaning_language }
                        if empty { div { class: "secondary", "Your conversation will appear here." } }
                    }
                }
                None => rsx! { div { "Start a conversation and your words will appear here." } },
            }
        }
    }
}

#[component]
pub fn HistorySheet() -> Element {
    let app = use_context::<Mural>();
    let mut deleting = use_signal(|| None::<String>);
    let store = app.store.read();
    let sessions = store.learning_sessions();
    let name = store.language().name;
    rsx! {
        SheetFrame { title: "Past conversations",
            if sessions.is_empty() { div { class: "secondary", "Your {name} conversations will appear here." } }
            div {
                for session in sessions {
                    div { key: "{session.id}", class: "history-item",
                        button { class: "history-row",
                            onclick: { let id = session.id.clone(); move |_| app.open_sheet(Sheet::EditableTranscript(id.clone())) },
                            span { style: "font-weight: 600; font-size: 15px", "{session.title}" }
                            span { class: "caption secondary", "{session.started_at.abbreviated_time()}" }
                        }
                        button { class: "glass-button danger", title: "Delete", aria_label: "Delete conversation",
                            disabled: session.ended_at.is_none(),
                            onclick: { let id = session.id.clone(); move |_| deleting.set(Some(id.clone())) },
                            Icon { name: "trash", size: 16 }
                        }
                    }
                }
            }
        }
        if let Some(id) = deleting() {
            Dialog {
                title: "Delete this conversation and its learning evidence?",
                buttons: vec![DialogButton { label: "Delete conversation".into(), role: "destructive", action: EventHandler::new(move |_| {
                    app.delete_session(&id);
                    deleting.set(None);
                }) }],
                on_cancel: move |_| deleting.set(None),
            }
        }
    }
}

#[component]
pub fn EditableTranscriptSheet(session_id: String) -> Element {
    let app = use_context::<Mural>();
    let mut editing = use_signal(|| None::<String>);
    let mut edited = use_signal(String::new);
    let session = app.store.read().session(&session_id).cloned();
    let meaning_language = app.store.read().preferences().meaning_language.clone();
    if let Some(passage_id) = editing() {
        let sid = session_id.clone();
        return rsx! {
            SheetFrame { title: "What you said",
                leading: Some(("Cancel".to_string(), EventHandler::new(move |_| editing.set(None)))),
                trailing: Some(("Save".to_string(), EventHandler::new(move |_| {
                    app.correct_passage(&sid, &passage_id, &edited.read());
                    editing.set(None);
                }))),
                textarea { class: "field", rows: 6, autofocus: true, value: "{edited}", oninput: move |e| edited.set(e.value()) }
                div { class: "footnote secondary", "Correct a misheard phrase. Learning evidence from the old wording will be removed; the original remains in your backup history." }
            }
        };
    }
    rsx! {
        SheetFrame { title: "Our conversation",
            if let Some(session) = session {
                TranscriptBody { session, meaning_language,
                    on_edit: EventHandler::new(move |(id, text): (String, String)| { edited.set(text); editing.set(Some(id)); }),
                }
            }
        }
    }
}

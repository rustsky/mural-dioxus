use dioxus::prelude::*;

use super::components::{Dialog, DialogButton, Markdown, PageHeading, PinyinHelp, RecallBars, SheetFrame, SourcesView};
use super::icons::Icon;
use super::state::{Mural, Sheet, Tab};
use crate::engine::flashcards::{self, Rating, Schedule};
use crate::engine::meaning_cache_key;
use crate::engine::time::Date;
use crate::services::flashcard_store;
use crate::engine::models::{SessionRecord, Speaker, TopicBrief};
use crate::engine::text::contains_ci;

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
    let mut reviewing = use_signal(|| false);
    if reviewing() {
        return rsx! { Review { on_close: move |_| reviewing.set(false) } };
    }
    let store = app.store.read();
    let language = store.language();
    let learner = store.learner();
    let record = flashcard_store::current();
    let now = Date::now();
    let cards: Vec<(&str, Option<&Schedule>)> = learner.words.iter().map(|w| (w.id.as_str(), record.get(&w.id))).collect();
    let queue = flashcards::queue(&cards, now);
    let new_count = queue.iter().filter(|id| record.get(id).is_none()).count();
    let due_count = queue.iter().filter(|id| record.get(id).is_some_and(|c| c.due <= now)).count();
    // Learning cards coming back within minutes; the session shows them at the end.
    let learning_count = queue.len() - new_count - due_count;
    let next = flashcards::next_due(&cards).filter(|d| *d > now);
    let query = search.read().clone();
    let words: Vec<_> = learner.words.iter().filter(|w| query.is_empty() || contains_ci(&w.lemma, &query) || contains_ci(&w.meaning, &query)).cloned().collect();
    rsx! {
        div { class: "page stack",
            PageHeading { eyebrow: "Little by little · {language.name}", title: "Your words.", subtitle: "Familiar words, ready for another conversation." }
            if !learner.words.is_empty() {
                div { class: "review-panel",
                    div { class: "review-summary",
                        if queue.is_empty() {
                            span { class: "review-count", "All caught up" }
                            span { class: "sub secondary",
                                if let Some(next) = next { "The next card is due in {flashcards::wait_label(next.since(now))}." }
                                else { "Review your words as flash cards." }
                            }
                        } else {
                            span { class: "review-count",
                                "{due_count} due · {new_count} new"
                                if learning_count > 0 { " · {learning_count} learning" }
                            }
                            span { class: "sub secondary", "See the word, recall its meaning, say how it went." }
                        }
                    }
                    button { class: "pill-button center review-start", disabled: queue.is_empty(), onclick: move |_| reviewing.set(true),
                        Icon { name: "leaf", size: 16 } "Review"
                    }
                }
            }
            div { class: "row",
                div { class: "search", style: "flex: 1",
                    Icon { name: "magnifyingglass", size: 15 }
                    input { r#type: "search", placeholder: "Find a word", value: "{search}", oninput: move |e| search.set(e.value()) }
                }
                button { class: "plain-action", onclick: move |_| app.open_sheet(Sheet::AddWord), Icon { name: "plus", size: 15 } "Add a word" }
            }
            if words.is_empty() {
                div { class: "card panel-2",
                    Icon { name: "leaf", size: 34, weight: 1.2 }
                    h2 { if query.is_empty() { "They’ll grow from here." } else { "No matching words yet." } }
                    div { class: "sub secondary",
                        if query.is_empty() { "As we talk, useful words and phrases find a home here, and each becomes a flash card. You can also add your own." }
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
                                span { class: "caption2 secondary", "{card_status(record.get(&word.id), now)}" }
                            }
                        }
                    }
                }
            }
            div { class: "legend", span { "1 · Fragile" } span { "2 · Growing" } span { "3 · Steady" } }
            div { class: "footnote secondary", "The bars estimate spoken recall in conversation, not permanent mastery, and using a word with visible meanings counts as supported practice. Flash cards are scheduled on their own." }
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

/// A word's flash card in a few words: new, due, or when it comes back.
fn card_status(card: Option<&Schedule>, now: Date) -> String {
    match card {
        None => "New card".into(),
        Some(card) if card.due <= now => "Card due".into(),
        Some(card) => format!("Card in {}", flashcards::wait_label(card.due.since(now))),
    }
}

/// One review session: the word, then its meaning, then how it went. Space shows the answer and
/// 1–4 answer it, as in Anki; Escape leaves.
#[component]
fn Review(on_close: EventHandler<()>) -> Element {
    let app = use_context::<Mural>();
    let mut revealed = use_signal(|| false);
    let mut reviewed = use_signal(|| 0usize);
    let mut error = use_signal(|| None::<String>);
    // The card store isn't a signal; this re-reads it after each answer.
    let mut answered = use_signal(|| 0u32);
    let _ = answered();

    let store = app.store.read();
    let is_zh = store.language().id == "zh";
    let words = store.learner().words;
    let record = flashcard_store::current();
    let now = Date::now();
    let cards: Vec<(&str, Option<&Schedule>)> = words.iter().map(|w| (w.id.as_str(), record.get(&w.id))).collect();
    let queue = flashcards::queue(&cards, now);
    let left = queue.len();
    let current = queue.first().and_then(|id| words.iter().find(|w| w.id == *id)).cloned();
    let next = flashcards::next_due(&cards).filter(|d| *d > now);

    let mut answer = move |id: String, rating: Rating| {
        match flashcard_store::update(|r| { r.review(&id, rating, Date::now()); }) {
            Ok(_) => { revealed.set(false); reviewed += 1; error.set(None); }
            Err(e) => error.set(Some(e)),
        }
        answered += 1;
    };

    let Some(word) = current else {
        return rsx! {
            div { class: "page stack drill",
                button { class: "text-button row", onclick: move |_| on_close.call(()), Icon { name: "chevron.left", size: 14 } "Your words" }
                div { class: "drill-card",
                    Icon { name: "leaf", size: 34, weight: 1.2 }
                    div { class: "drill-phrase", "All done for now." }
                    div { class: "meaning",
                        if reviewed() > 0 { "{reviewed} cards reviewed. " }
                        if let Some(next) = next { "The next card is due in {flashcards::wait_label(next.since(now))}." }
                    }
                }
                button { class: "pill-button center", onclick: move |_| on_close.call(()), "Back to your words" }
            }
        };
    };

    let schedule = record.get(&word.id).cloned();
    let choices: Vec<(Rating, String)> = Rating::ALL.iter()
        .map(|r| (*r, flashcards::wait_label(flashcards::review(schedule.as_ref(), *r, now).due.since(now))))
        .collect();
    let state = match &schedule { None => "New", Some(s) if s.is_learning() => "Learning", Some(_) => "Review" };
    let id = word.id.clone();

    rsx! {
        div { class: "page stack drill review", tabindex: "0",
            onmounted: move |e| async move { let _ = e.set_focus(true).await; },
            onkeydown: {
                let id = id.clone();
                move |e: Event<KeyboardData>| {
                    let key = e.key();
                    if key == Key::Escape { e.stop_propagation(); on_close.call(()); return; }
                    if !revealed() {
                        if key == Key::Enter || key == Key::Character(" ".into()) { e.prevent_default(); revealed.set(true); }
                        return;
                    }
                    if let Key::Character(c) = &key {
                        if let Some(rating) = Rating::ALL.into_iter().find(|r| r.key() == c) { e.prevent_default(); answer(id.clone(), rating); }
                    }
                }
            },
            div { class: "review-top",
                button { class: "text-button row", onclick: move |_| on_close.call(()), Icon { name: "chevron.left", size: 14 } "Your words" }
                span { class: "caption2 secondary", "{left} left" }
            }
            div { class: "drill-card flash-card",
                span { class: "band-badge", "{state}" }
                div { class: "drill-phrase selectable", "{word.lemma}" }
                if revealed() {
                    div { class: "flash-answer",
                        if is_zh { PinyinHelp { text: word.lemma.clone() } }
                        div { class: "flash-meaning selectable", "{word.meaning}" }
                        if !word.example.is_empty() { div { class: "quote selectable", "“{word.example}”" } }
                    }
                }
            }
            if revealed() {
                div { class: "grade-buttons",
                    for (rating, wait) in choices {
                        button { key: "{rating.label()}", class: "grade grade-{rating.key()}",
                            onclick: { let id = id.clone(); move |_| answer(id.clone(), rating) },
                            span { class: "grade-wait", "{wait}" }
                            span { "{rating.label()}" }
                        }
                    }
                }
                div { class: "caption2 secondary center", "Keys 1–4" }
            } else {
                button { class: "pill-button center", onclick: move |_| revealed.set(true), "Show answer" }
                div { class: "caption2 secondary center", "Space" }
            }
            if let Some(e) = error() { div { class: "footnote danger", "{e}" } }
        }
    }
}

/// Adds a word of the learner's own. It joins the list, and the flash cards, like a kept word.
#[component]
pub fn AddWordSheet() -> Element {
    let app = use_context::<Mural>();
    let mut lemma = use_signal(String::new);
    let mut meaning = use_signal(String::new);
    let mut example = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let language = app.store.read().language();
    let meaning_language = app.store.read().preferences().meaning_language.clone();
    let mut save = move || {
        if lemma.read().trim().is_empty() || meaning.read().trim().is_empty() {
            error.set(Some("A word and its meaning are both needed.".into()));
            return;
        }
        if app.store.write_unchecked().keep_word(&lemma.read(), &meaning.read(), &example.read()) {
            app.close_sheet();
        } else {
            error.set(Some("That word, with that meaning, is already in your list.".into()));
        }
    };
    rsx! {
        SheetFrame { title: "Add a word",
            leading: Some(("Cancel".to_string(), EventHandler::new(move |_| app.close_sheet()))),
            trailing: Some(("Add".to_string(), EventHandler::new(move |_| save()))),
            div { class: "stack",
                label { class: "stack",
                    span { class: "sub", "{language.name} word or phrase" }
                    input { class: "field", value: "{lemma}", autofocus: true, oninput: move |e| lemma.set(e.value()) }
                }
                label { class: "stack",
                    span { class: "sub", "Meaning in {meaning_language}" }
                    input { class: "field", value: "{meaning}", oninput: move |e| meaning.set(e.value()),
                        onkeydown: move |e: Event<KeyboardData>| if e.key() == Key::Enter { save() } }
                }
                label { class: "stack",
                    span { class: "sub", "An example sentence (optional)" }
                    textarea { class: "field", rows: 2, value: "{example}", oninput: move |e| example.set(e.value()) }
                }
                if let Some(e) = error() { div { class: "footnote danger", "{e}" } }
                div { class: "footnote secondary", "It joins your words and your flash cards. Its bars still grow only from using it in conversation." }
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
    let card_line = match flashcard_store::current().get(&word.id) {
        None => "Flash card: new, not reviewed yet".to_string(),
        Some(card) => format!("Flash card: {} · reviewed {} times{}", card_status(Some(card), Date::now()).to_lowercase().replace("card ", ""),
                              card.reviews, if card.lapses > 0 { format!(", forgotten {}", card.lapses) } else { String::new() }),
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
            div { class: "footnote secondary", "{card_line}" }
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

/// Keeps a live transcript at the newest line, unless the learner has scrolled back to read.
/// Scrolling within 40px of the bottom resumes following.
const FOLLOW: &str = "requestAnimationFrame(() => {
  const b = document.querySelector('.sheet-body');
  if (!b) return;
  if (!b.dataset.wired) {
    b.dataset.wired = '1';
    b.dataset.follow = 'on';
    b.addEventListener('scroll', () => {
      b.dataset.follow = b.scrollHeight - b.scrollTop - b.clientHeight < 40 ? 'on' : 'off';
    });
  }
  if (b.dataset.follow !== 'off') b.scrollTop = b.scrollHeight;
});";

#[component]
pub fn TranscriptSheet() -> Element {
    let app = use_context::<Mural>();
    let session = app.conv.read().session.clone();
    let meaning_language = app.store.read().preferences().meaning_language.clone();
    let conv = app.conv;
    use_effect(move || {
        // Reading the conversation here is what re-runs this as new lines arrive.
        let live = {
            let c = conv.read();
            let Some(session) = c.session.as_ref() else { return };
            let _lines = session.fragments.len();
            session.ended_at.is_none()
        };
        if !live { return; }
        spawn(async move { let _ = document::eval(FOLLOW).await; });
    });
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

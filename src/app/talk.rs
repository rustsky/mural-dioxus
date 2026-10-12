use dioxus::prelude::*;

use super::components::{Orb, PinyinHelp, SheetFrame};
use super::icons::Icon;
use super::state::{ConnectionState, Mural, Sheet};
use crate::engine::captions;
use crate::engine::languages::meaning_greeting;
use crate::engine::meaning_cache_key;
use crate::engine::models::Speaker;
use crate::engine::text::prefix;

/// Keeps the dialogue on the newest line, unless the learner has scrolled back to read earlier
/// ones. Scrolling within 48px of the bottom follows again.
const FOLLOW: &str = "requestAnimationFrame(() => {
  const d = document.querySelector('.dialogue');
  if (!d) return;
  if (!d.dataset.wired) {
    d.dataset.wired = '1';
    d.dataset.follow = 'on';
    d.addEventListener('scroll', () => {
      d.dataset.follow = d.scrollHeight - d.scrollTop - d.clientHeight < 48 ? 'on' : 'off';
    });
  }
  if (d.dataset.follow !== 'off') d.scrollTop = d.scrollHeight;
});";

fn translation(conv: &super::state::Conv, revision_key: &str, meaning_language: &str) -> Option<String> {
    let session = conv.session.as_ref()?;
    session.translations.get(&meaning_cache_key(revision_key, meaning_language))
        .or_else(|| session.translations.get(revision_key))
        .cloned()
}

#[component]
pub fn TalkView() -> Element {
    let app = use_context::<Mural>();
    let conv = app.conv.read();
    let store = app.store.read();
    let language = store.language();
    let prefs = store.preferences();
    let state = conv.state;
    let running = conv.is_running();
    let assistant = conv.assistant_passage();
    let caption = conv.caption(language);
    let title = conv.scenario.map(|s| s.title.clone())
        .or_else(|| conv.selected_theme.as_ref().map(|t| t.title.clone()))
        .unwrap_or_else(|| language.talk_title());
    let energy = conv.output_level.max(conv.input_level * 0.45);
    let listening = state == ConnectionState::Active && !conv.is_muted;
    let meaning_visible = prefs.meaning_visible;
    let meaning_text = if assistant.is_none() {
        meaning_greeting(&prefs.meaning_language).to_string()
    } else if !conv.meaning.text.is_empty() {
        conv.meaning.text.clone()
    } else if conv.meaning.loading {
        "Finding the meaning…".into()
    } else {
        String::new()
    };
    let segments = captions::segments(&caption, language.id);
    let has_assistant = assistant.is_some();
    let passages = conv.session.as_ref().map(|s| s.passages()).unwrap_or_default();
    let newest_assistant = passages.iter().rev().find(|p| p.speaker == Speaker::Assistant).map(|p| p.id.clone());
    let meaning_language = prefs.meaning_language.clone();
    let has_sources = conv.session.as_ref().and_then(|s| s.topics.last()).map(|t| !t.sources.is_empty()).unwrap_or(false);
    let has_session = conv.session.is_some();
    let busy = matches!(state, ConnectionState::Connecting | ConnectionState::Closing);
    let hold_to_talk = conv.hold_to_talk;
    let holding = conv.holding;
    let mic_icon = if conv.is_muted && state == ConnectionState::Active { "mic.slash" } else { "mic" };
    let mic_label = match state {
        ConnectionState::Active if hold_to_talk && holding => "Release to finish speaking",
        ConnectionState::Active if hold_to_talk => "Hold to speak",
        ConnectionState::Active if conv.is_muted => "Unmute microphone",
        ConnectionState::Active => "Mute microphone",
        _ => "Start conversation",
    };

    let mut peek = use_signal(|| None::<WordPeek>);

    // Reading the conversation here is what re-runs this as new lines arrive.
    let conv_signal = app.conv;
    use_effect(move || {
        let _ = conv_signal.read().session.as_ref().map(|s| s.fragments.len());
        spawn(async move { let _ = document::eval(FOLLOW).await; });
    });

    rsx! {
        div { class: "talk",
            div { class: "talk-pill", "{title}" }
            if let Some(scenario) = conv.scenario {
                div { class: "beat-strip",
                    for (i, beat) in scenario.beats.iter().enumerate() {
                        div { key: "{i}", class: if conv.beats_covered.contains(&i) { "beat-dot done" } else { "beat-dot" },
                            title: "{beat.en}",
                            if conv.beats_covered.contains(&i) { Icon { name: "checkmark.circle.fill", size: 13 } }
                            span { "{beat.de}" }
                        }
                    }
                }
            }
            div { class: if has_assistant { "orb-wrap compact" } else { "orb-wrap" },
                Orb { energy, listening, active: state != ConnectionState::Closing }
            }
            div { class: "status", role: "status", aria_live: "polite",
                if let (ConnectionState::Active, Some(seconds)) = (state, conv.inactivity_seconds) {
                    strong { "Ending in {seconds}s" }
                    div { class: "caption2", "Reply to continue" }
                } else {
                    "{conv.status()}"
                }
            }
            div { class: "dialogue",
                if !has_assistant {
                    div { class: "target-caption greeting",
                        for (i, segment) in segments.into_iter().enumerate() {
                            span { key: "{i}", "{segment.text}" }
                        }
                    }
                    if meaning_visible { div { class: "meaning", "{meaning_text}" } }
                }
                for passage in passages.iter() {
                    div { key: "{passage.id}", class: if passage.speaker == Speaker::Assistant { "line mural" } else { "line you" },
                        if passage.speaker == Speaker::User { div { class: "who", "YOU" } }
                        div { class: "said selectable",
                            for (i, segment) in captions::segments(&passage.text(), language.id).into_iter().enumerate() {
                                if let (Speaker::Assistant, Some(word)) = (passage.speaker, segment.lookup.clone()) {
                                    span { key: "{i}", class: "word-link", title: "Look up “{word}”",
                                        onclick: {
                                            let sentence = passage.text();
                                            let word = word.clone();
                                            move |e: Event<MouseData>| {
                                                let at = e.client_coordinates();
                                                peek.set(Some(WordPeek {
                                                    word: word.clone(),
                                                    sentence: sentence.clone(),
                                                    x: at.x, y: at.y,
                                                }));
                                            }
                                        },
                                        "{segment.text}"
                                    }
                                } else {
                                    span { key: "{i}", "{segment.text}" }
                                }
                            }
                        }
                        if language.id == "zh" && passage.speaker == Speaker::Assistant && Some(&passage.id) == newest_assistant.as_ref() {
                            PinyinHelp { text: passage.text() }
                        }
                        if meaning_visible && passage.speaker == Speaker::Assistant {
                            // Stored meanings stay with their line; the newest one may still be arriving.
                            if let Some(t) = translation(&conv, &passage.revision_key(), &meaning_language) {
                                div { class: "meaning", "{t}" }
                            } else if Some(&passage.id) == newest_assistant.as_ref() && !meaning_text.is_empty() {
                                div { class: "meaning", "{meaning_text}" }
                            }
                        }
                    }
                }
                if meaning_visible {
                    if let Some(error) = conv.meaning.error.clone() {
                        div { class: "caption secondary",
                            div { "{error}" }
                            button { class: "text-button", onclick: move |_| app.retry_meaning(), "Try meaning again" }
                        }
                    }
                }
                if conv.working {
                    div { class: "working", span { class: "spinner" } "Checking that for you…" }
                }
                if has_sources {
                    button { class: "text-button row", onclick: move |_| app.open_sheet(Sheet::Transcript),
                        Icon { name: "link", size: 14 } "Sources"
                    }
                }
            }
            div { class: "controls",
                div { class: "control",
                    button {
                        class: if meaning_visible { "glass-button on" } else { "glass-button" },
                        aria_label: if meaning_visible { "Hide meaning subtitles" } else { "Show meaning subtitles" },
                        aria_pressed: "{meaning_visible}",
                        onclick: move |_| app.toggle_meaning(),
                        Icon { name: "captions", size: 22 }
                    }
                    "Meaning"
                }
                button {
                    class: if holding { "mic-button holding" } else { "mic-button" },
                    disabled: busy, aria_label: mic_label, title: mic_label,
                    onclick: move |_| {
                        let state = app.conv.peek().state;
                        if state == ConnectionState::Active {
                            // While holding to talk, the press and release do the work instead.
                            if !hold_to_talk { app.toggle_mute() }
                        } else if !app.conv.peek().is_running() { app.start() }
                    },
                    onpointerdown: move |_| app.set_holding(true),
                    onpointerup: move |_| app.set_holding(false),
                    onpointerleave: move |_| app.set_holding(false),
                    onkeydown: move |e| if matches!(e.key(), Key::Character(ref c) if c == " ") { app.set_holding(true) },
                    onkeyup: move |e| if matches!(e.key(), Key::Character(ref c) if c == " ") { app.set_holding(false) },
                    if busy { span { class: "spinner dark" } } else { Icon { name: mic_icon, size: 30, weight: 1.5 } }
                }
                div { class: "control",
                    button { class: "glass-button", disabled: !has_session,
                        aria_label: if running { "End conversation" } else { "Conversation transcript" },
                        onclick: move |_| {
                            if app.conv.peek().is_running() { app.end("Ended by you") } else { app.open_sheet(Sheet::Transcript) }
                        },
                        Icon { name: if running { "phone.down" } else { "text.bubble" }, size: 22 }
                    }
                    if running { "End" } else { "Transcript" }
                }
            }
            div { class: "mic-label", "{conv.microphone_label()}" }
            div { class: "secondary-actions",
                if state == ConnectionState::Active {
                    button { onclick: move |_| app.open_sheet(Sheet::TypedReply), Icon { name: "keyboard", size: 15 } "Type instead" }
                    button { onclick: move |_| app.help(), Icon { name: "sparkles", size: 15 } "A little help" }
                } else if !has_session {
                    span { class: "secondary", "Reply in whichever language comes to you." }
                } else if !running {
                    button { onclick: move |_| app.reset_conversation(), Icon { name: "arrow.counterclockwise", size: 15 } "New conversation" }
                }
            }
            if let Some(open) = peek() {
                WordCard { peek: open, on_close: move |_| peek.set(None) }
            }
            if let Some(notice) = conv.notice.clone() {
                div { class: "notice", "{notice}" }
            }
        }
    }
}

/// Splits "dictionary form — meaning" into its two halves, falling back to the word as tapped
/// when the answer does not arrive in that shape.
fn split_gloss(answer: &str, word: &str) -> (String, String) {
    let line = answer.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    let split = line.split_once('—').or_else(|| line.split_once(" - ")).or_else(|| line.split_once(':'));
    match split {
        Some((lemma, meaning)) if !lemma.trim().is_empty() && !meaning.trim().is_empty() => {
            (prefix(lemma.trim(), 60), prefix(meaning.trim(), 80))
        }
        // No separator: whatever came back is the meaning, and the word stands as tapped.
        _ if !line.is_empty() => (word.to_string(), prefix(line, 80)),
        _ => (word.to_string(), String::new()),
    }
}

/// A word the learner tapped, and where on screen it was.
#[derive(Clone, Debug, PartialEq)]
pub struct WordPeek {
    pub word: String,
    pub sentence: String,
    pub x: f64,
    pub y: f64,
}

/// The meaning of one word, shown beside it, with the option to keep it.
#[component]
fn WordCard(peek: WordPeek, on_close: EventHandler<()>) -> Element {
    let app = use_context::<Mural>();
    let mut kept = use_signal(|| false);
    let mut keeping = use_signal(|| false);
    let is_zh = app.store.read().language().id == "zh";
    let (word, sentence) = (peek.word.clone(), peek.sentence.clone());
    let meaning = use_resource(use_reactive!(|(word, sentence)| async move { app.lookup(word, sentence).await }));
    // Keep the card on screen: beside the word, above it when there is no room below, clear of the
    // toolbar, and in the middle when the event carried no position at all.
    let placed = peek.x > 1.0 || peek.y > 1.0;
    let left = if placed { peek.x.clamp(20.0, 760.0) } else { 330.0 };
    let top = if !placed { 180.0 } else if peek.y < 520.0 { peek.y + 26.0 } else { peek.y - 250.0 };
    let top = top.max(70.0);

    rsx! {
        div { class: "peek-catcher", onclick: move |_| on_close.call(()) }
        div { class: "word-card", style: "left: {left}px; top: {top}px",
            div { class: "word-card-head",
                span { class: "big", "{peek.word}" }
                span { class: "spacer" }
                button { class: "text-button", onclick: move |_| on_close.call(()), "Close" }
            }
            if is_zh { PinyinHelp { text: peek.word.clone() } }
            match &*meaning.read() {
                Some(Ok(text)) => rsx! { div { class: "word-card-meaning selectable", "{text}" } },
                Some(Err(error)) => rsx! { div { class: "secondary lookup-error", "{error}" } },
                None => rsx! { div { class: "working", span { class: "spinner" } "Finding the meaning…" } },
            }
            div { class: "word-card-foot",
                if kept() {
                    span { class: "caption2 good", "Kept in your words" }
                } else if keeping() {
                    div { class: "working", span { class: "spinner" } "Keeping…" }
                } else {
                    button { class: "action row",
                        onclick: {
                            let (w, s) = (peek.word.clone(), peek.sentence.clone());
                            move |_| {
                                let (w, s) = (w.clone(), s.clone());
                                keeping.set(true);
                                spawn(async move {
                                    // A line of its own, not the prose on screen: the list has to
                                    // read as a list. If the request fails the word still goes in.
                                    let answer = app.gloss(w.clone(), s.clone()).await.unwrap_or_default();
                                    let (lemma, sense) = split_gloss(&answer, &w);
                                    app.store.write_unchecked().keep_word(&lemma, &sense, &s);
                                    keeping.set(false);
                                    kept.set(true);
                                });
                            }
                        },
                        Icon { name: "book", size: 15 } "Keep this word"
                    }
                }
            }
        }
    }
}

#[component]
pub fn TypedReplySheet() -> Element {
    let app = use_context::<Mural>();
    let mut text = use_signal(String::new);
    let mut sending = use_signal(|| false);
    let language = app.store.read().language();
    let error = app.conv.read().typed_reply_error.clone();
    use_hook(|| app.note_typing_activity());
    let empty = text.read().trim().is_empty();
    rsx! {
        SheetFrame { leading: Some(("Close".to_string(), EventHandler::new(move |_| app.close_sheet()))),
            h1 { "Say it your way." }
            textarea { class: "field", rows: 4, autofocus: true,
                placeholder: "Reply in {language.name} or another language",
                value: "{text}",
                oninput: move |e| { text.set(e.value()); app.note_typing_activity(); },
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.key() == Key::Enter && e.modifiers().meta() { e.prevent_default(); }
                },
            }
            if let Some(error) = error { div { class: "footnote secondary", "{error}" } }
            button { class: "pill-button", disabled: sending() || empty,
                onclick: move |_| {
                    sending.set(true);
                    let value = text.read().clone();
                    spawn(async move {
                        let ok = app.send_typed(value).await;
                        sending.set(false);
                        if ok { app.close_sheet(); }
                    });
                },
                span { if sending() { "Sending…" } else { "Send reply" } }
                Icon { name: "arrow.up", size: 18 }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split_gloss;

    #[test]
    fn a_gloss_splits_into_dictionary_form_and_meaning() {
        assert_eq!(split_gloss("la sobremesa — conversation after a meal", "sobremesa"),
            ("la sobremesa".into(), "conversation after a meal".into()));
        // Some models answer with a hyphen or a colon instead.
        assert_eq!(split_gloss("der Termin - the appointment", "Termin"),
            ("der Termin".into(), "the appointment".into()));
        assert_eq!(split_gloss("die Quittung: the receipt", "Quittung"),
            ("die Quittung".into(), "the receipt".into()));
    }

    #[test]
    fn an_answer_without_a_separator_is_treated_as_the_meaning() {
        assert_eq!(split_gloss("the receipt", "Quittung"), ("Quittung".into(), "the receipt".into()));
        // Chatter before the answer is ignored in favour of the first real line.
        assert_eq!(split_gloss("\n\nder Zug — the train", "Zug"), ("der Zug".into(), "the train".into()));
    }

    #[test]
    fn nothing_usable_still_keeps_the_word_as_tapped() {
        assert_eq!(split_gloss("", "Quittung"), ("Quittung".into(), String::new()));
        assert_eq!(split_gloss("   ", "Quittung"), ("Quittung".into(), String::new()));
    }
}

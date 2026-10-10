use dioxus::prelude::*;

use super::components::{Orb, PinyinHelp, SheetFrame};
use super::icons::Icon;
use super::state::{ConnectionState, Mural, Sheet};
use crate::engine::captions;
use crate::engine::languages::meaning_greeting;
use crate::engine::meaning_cache_key;
use crate::engine::models::Speaker;

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
    let title = conv.selected_theme.as_ref().map(|t| t.title.clone()).unwrap_or_else(|| language.talk_title());
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

    // Reading the conversation here is what re-runs this as new lines arrive.
    let conv_signal = app.conv;
    use_effect(move || {
        let _ = conv_signal.read().session.as_ref().map(|s| s.fragments.len());
        spawn(async move { let _ = document::eval(FOLLOW).await; });
    });

    rsx! {
        div { class: "talk",
            div { class: "talk-pill", "{title}" }
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
                                            move |_| app.open_sheet(Sheet::Lookup { word: word.clone(), sentence: sentence.clone() })
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
            if let Some(notice) = conv.notice.clone() {
                div { class: "notice", "{notice}" }
            }
        }
    }
}

#[component]
pub fn LookupSheet(word: String, sentence: String) -> Element {
    let app = use_context::<Mural>();
    let is_zh = app.store.read().language().id == "zh";
    let lookup = use_resource(use_reactive!(|word, sentence| async move { app.lookup(word, sentence).await }));
    rsx! {
        SheetFrame { title: "A little meaning",
            div { class: "big-word selectable", "{word}" }
            if is_zh { PinyinHelp { text: word.clone() } }
            div { class: "secondary selectable", style: "font-size: 19px", "{sentence}" }
            match &*lookup.read() {
                Some(Ok(text)) => rsx! { div { class: "selectable", style: "font-size: 15px; line-height: 1.5", "{text}" } },
                Some(Err(error)) => rsx! { div { class: "secondary lookup-error", "{error}" } },
                None => rsx! { div { class: "working", span { class: "spinner" } "Finding the meaning…" } },
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

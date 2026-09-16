use dioxus::prelude::*;

use super::components::{Orb, PinyinHelp, SheetFrame};
use super::icons::Icon;
use super::state::{ConnectionState, Mural, Sheet};
use crate::engine::captions;
use crate::engine::languages::meaning_greeting;
use crate::engine::text::suffix;

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
    let user_text = conv.user_passage().map(|p| suffix(&p.text(), 160));
    let has_sources = conv.session.as_ref().and_then(|s| s.topics.last()).map(|t| !t.sources.is_empty()).unwrap_or(false);
    let has_session = conv.session.is_some();
    let busy = matches!(state, ConnectionState::Connecting | ConnectionState::Closing);
    let mic_icon = if conv.is_muted && state == ConnectionState::Active { "mic.slash" } else { "mic" };
    let mic_label = match state {
        ConnectionState::Active if conv.is_muted => "Unmute microphone",
        ConnectionState::Active => "Mute microphone",
        _ => "Start conversation",
    };

    rsx! {
        div { class: "talk",
            div { class: "talk-pill", "{title}" }
            div { class: "grow" }
            div { class: "orb-wrap", Orb { energy, listening, active: state != ConnectionState::Closing } }
            div { class: "status", role: "status", aria_live: "polite",
                if let (ConnectionState::Active, Some(seconds)) = (state, conv.inactivity_seconds) {
                    strong { "Ending in {seconds}s" }
                    div { class: "caption2", "Reply to continue" }
                } else {
                    "{conv.status()}"
                }
            }
            div { class: "captions",
                div { class: if has_assistant { "target-caption selectable" } else { "target-caption greeting" },
                    for (i, segment) in segments.into_iter().enumerate() {
                        if let (true, Some(word)) = (has_assistant, segment.lookup.clone()) {
                            span { key: "{i}", class: "word-link", title: "Look up “{word}”",
                                onclick: {
                                    let sentence = caption.clone();
                                    move |_| app.open_sheet(Sheet::Lookup { word: word.clone(), sentence: sentence.clone() })
                                },
                                "{segment.text}"
                            }
                        } else {
                            span { key: "{i}", "{segment.text}" }
                        }
                    }
                }
                if language.id == "zh" { PinyinHelp { text: caption.clone() } }
                if meaning_visible {
                    div { class: "meaning", "{meaning_text}" }
                    if let Some(error) = conv.meaning.error.clone() {
                        div { class: "caption secondary",
                            div { "{error}" }
                            button { class: "text-button", onclick: move |_| app.retry_meaning(), "Try meaning again" }
                        }
                    }
                }
                if let Some(text) = user_text {
                    div { class: "you-line", b { "YOU" } span { "{text}" } }
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
            div { class: "grow" }
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
                button { class: "mic-button", disabled: busy, aria_label: mic_label, title: mic_label,
                    onclick: move |_| {
                        let state = app.conv.peek().state;
                        if state == ConnectionState::Active { app.toggle_mute() }
                        else if !app.conv.peek().is_running() { app.start() }
                    },
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

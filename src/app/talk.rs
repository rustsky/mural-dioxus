use dioxus::prelude::*;

use super::components::{LookupText, Orb, PinyinHelp, SheetFrame};
use super::icons::Icon;
use super::state::{ConnectionState, Mural, Sheet};
use crate::engine::languages::meaning_greeting;
use crate::engine::meaning_cache_key;
use crate::engine::models::Speaker;
use crate::engine::scenarios::beat_window;

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

    let mut tasks_open = use_signal(|| true);
    let mut hovered = use_signal(|| None::<usize>);
    let next: Vec<usize> = conv.scenario
        .map(|s| beat_window(s.beats.len(), &conv.beats_covered).into_iter().take(3).collect())
        .unwrap_or_default();

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
                // Every step, ticked as it happens, with the next few marked. The meaning of the one
                // under the pointer shows beneath: a native tooltip doesn't appear in this webview.
                div { class: "beat-panel",
                    button { class: "beat-toggle", onclick: move |_| tasks_open.toggle(),
                        Icon { name: "checkmark.circle.fill", size: 13 }
                        span { "{conv.beats_covered.len()}/{scenario.beats.len()} tasks" }
                        Icon { name: if tasks_open() { "chevron.up" } else { "chevron.down" }, size: 11 }
                    }
                    if tasks_open() {
                        div { class: "beat-strip",
                            for (i, beat) in scenario.beats.iter().enumerate() {
                                div { key: "{i}",
                                    class: if conv.beats_covered.contains(&i) { "beat-dot done" }
                                        else if next.contains(&i) { "beat-dot next" } else { "beat-dot" },
                                    onmouseenter: move |_| hovered.set(Some(i)),
                                    onmouseleave: move |_| hovered.set(None),
                                    if conv.beats_covered.contains(&i) { Icon { name: "checkmark.circle.fill", size: 12 } }
                                    span { "{beat.text}" }
                                }
                            }
                        }
                        div { class: "beat-meaning",
                            match hovered().and_then(|i| scenario.beats.get(i)) {
                                Some(beat) => rsx! { "{beat.en}" },
                                None => rsx! { span { class: "secondary", "Point at a task to see what it means." } },
                            }
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
                        LookupText { text: caption.clone(), language_id: language.id.to_string() }
                    }
                    if meaning_visible { div { class: "meaning", "{meaning_text}" } }
                }
                for passage in passages.iter() {
                    div { key: "{passage.id}", class: if passage.speaker == Speaker::Assistant { "line mural" } else { "line you" },
                        if passage.speaker == Speaker::User { div { class: "who", "YOU" } }
                        div { class: "said selectable",
                            LookupText { text: passage.text(), language_id: language.id.to_string() }
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

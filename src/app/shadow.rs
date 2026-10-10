//! The Shadow tab: hear a short phrase, repeat it, and be told how it came out.
//!
//! The audio is Mural's own on-device voice, and the scoring is the on-device recogniser, so a
//! drill works offline. Film clips of a phrase live on PlayPhrase, which opens in the browser.

use dioxus::prelude::*;

use super::components::PageHeading;
use super::icons::Icon;
use super::state::{Mural, Tab};
use crate::engine::phrases::{self, Phrase};
use crate::engine::shadow::Verdict;
use crate::engine::text::contains_ci;
use crate::engine::time::Date;
use crate::services::shadow_store;

#[component]
pub fn ShadowView() -> Element {
    let app = use_context::<Mural>();
    let language = app.store.read().language();
    let drilling = app.shadow.read().phrase_id.is_some();

    // Leaving the tab hands the microphone and the models back.
    use_drop(move || app.shadow_close());

    if !phrases::available(language.id) {
        return rsx! {
            div { class: "page stack",
                PageHeading { eyebrow: "Shadowing", title: "Not in {language.name}\nyet",
                    subtitle: "Phrase drills are written per language. German has a list; the others follow." }
            }
        };
    }
    if app.conv.read().is_running() {
        return rsx! {
            div { class: "page stack",
                PageHeading { eyebrow: "Shadowing", title: "One at a time",
                    subtitle: "Finish your conversation first. Talking and shadowing both want the microphone." }
                button { class: "action", onclick: move |_| app.ui.write_unchecked().tab = Tab::Talk, "Back to the conversation" }
            }
        };
    }
    if drilling { rsx! { Drill {} } } else { rsx! { PhraseList {} } }
}

#[component]
fn PhraseList() -> Element {
    let app = use_context::<Mural>();
    let mut search = use_signal(String::new);
    let mut category = use_signal(|| "All".to_string());
    let language = app.store.read().language();
    let meaning_visible = app.store.read().preferences().meaning_visible;
    let record = shadow_store::current();
    let now = Date::now();

    let query = search.read().clone();
    let chosen = category();
    let mut list: Vec<&Phrase> = phrases::all(language.id).iter().filter(|p| {
        (chosen == "All" || p.category == chosen)
            && (query.is_empty() || contains_ci(&p.text, &query) || contains_ci(&p.meaning, &query))
    }).collect();
    // Due first, then the ones never tried, so the list opens on what is worth saying today.
    list.sort_by_key(|p| {
        let progress = record.get(&p.id);
        (!progress.is_due(now), progress.attempts > 0, p.level)
    });

    let practised = record.practised();
    let settled = record.settled();
    let total = phrases::all(language.id).len();

    rsx! {
        div { class: "page stack",
            PageHeading { eyebrow: "Shadowing", title: "Say it after\nthe voice",
                subtitle: "Short phrases you will actually use. Listen, repeat, hear how close you were." }
            div { class: "shadow-tally",
                span { "{practised} of {total} tried" }
                span { class: "spacer" }
                span { "{settled} settled" }
            }
            div { class: "search",
                Icon { name: "magnifyingglass", size: 15 }
                input { r#type: "search", placeholder: "Find a phrase", value: "{search}", oninput: move |e| search.set(e.value()) }
            }
            div { class: "chips",
                button { class: if chosen == "All" { "chip selected" } else { "chip" },
                    onclick: move |_| category.set("All".into()), "All"
                }
                for c in phrases::categories(language.id) {
                    button { key: "{c}", class: if chosen == c { "chip selected" } else { "chip" },
                        aria_pressed: "{chosen == c}",
                        onclick: move |_| category.set(c.to_string()), "{c}"
                    }
                }
            }
            if list.is_empty() {
                div { class: "sub secondary", "No phrase matches that. Try another word, in {language.name} or English." }
            } else {
                div {
                    for phrase in list {
                        {
                            let progress = record.get(&phrase.id);
                            let id = phrase.id.clone();
                            rsx! {
                                button { key: "{phrase.id}", class: "phrase-row",
                                    onclick: move |_| app.shadow_open(id.clone()),
                                    div { class: "phrase-text",
                                        span { class: "said", "{phrase.text}" }
                                        if meaning_visible { span { class: "sub secondary", "{phrase.meaning}" } }
                                    }
                                    div { class: "phrase-mark",
                                        if progress.attempts == 0 {
                                            span { class: "caption2 secondary", "New" }
                                        } else if progress.is_settled() {
                                            span { class: "caption2 good", "Settled" }
                                        } else {
                                            span { class: "caption2 secondary", "{(progress.best * 100.0).round()}%" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Drill() -> Element {
    let app = use_context::<Mural>();
    let shadow = app.shadow.read();
    let language = app.store.read().language();
    let meaning_visible = app.store.read().preferences().meaning_visible;
    let Some(phrase) = app.shadow_phrase() else {
        return rsx! { div { class: "page stack", "That phrase has gone." } };
    };
    let progress = shadow_store::current().get(&phrase.id);
    let ready = shadow.ready;
    let holding = shadow.holding;
    let speaking = shadow.speaking;
    let preparing = shadow.preparing.clone();
    let notice = shadow.notice.clone();
    let result = shadow.result.clone();
    drop(shadow);

    let next = {
        let all = phrases::all(language.id);
        let here = all.iter().position(|p| p.id == phrase.id).unwrap_or(0);
        all.get(here + 1).or_else(|| all.first()).map(|p| p.id.clone())
    };

    rsx! {
        div { class: "page stack drill",
            button { class: "text-button row", onclick: move |_| app.shadow_open_none(),
                Icon { name: "chevron.left", size: 14 } "All phrases"
            }
            div { class: "drill-card",
                div { class: "drill-phrase selectable", "{phrase.text}" }
                if meaning_visible { div { class: "meaning", "{phrase.meaning}" } }
                div { class: "caption2 secondary", "{phrase.category} · tried {progress.attempts} times" }
            }

            if let Some(step) = preparing {
                div { class: "drill-status", span { class: "spinner" } "{step}" }
            } else if !ready {
                div { class: "drill-status", "The voice isn’t ready yet." }
            } else {
                div { class: "drill-controls",
                    button { class: "action row", disabled: speaking || holding, onclick: move |_| app.shadow_say(false),
                        Icon { name: "waveform", size: 16 } "Listen"
                    }
                    button { class: "action row", disabled: speaking || holding, onclick: move |_| app.shadow_say(true),
                        Icon { name: "clock.arrow.circlepath", size: 16 } "Slowly"
                    }
                }
                button {
                    class: if holding { "repeat-button holding" } else { "repeat-button" },
                    disabled: speaking,
                    aria_label: if holding { "Release when you have said it" } else { "Hold to repeat" },
                    onpointerdown: move |_| app.shadow_hold(true),
                    onpointerup: move |_| app.shadow_hold(false),
                    onpointerleave: move |_| app.shadow_hold(false),
                    onkeydown: move |e| if matches!(e.key(), Key::Character(ref c) if c == " ") { app.shadow_hold(true) },
                    onkeyup: move |e| if matches!(e.key(), Key::Character(ref c) if c == " ") { app.shadow_hold(false) },
                    Icon { name: "mic", size: 26, weight: 1.5 }
                    span { if holding { "Release when you’re done" } else { "Hold and say it" } }
                }
            }

            if let Some(attempt) = result {
                {
                    let verdict = attempt.verdict();
                    rsx! {
                        div { class: "drill-result {verdict.class()}",
                            div { class: "drill-verdict",
                                span { "{verdict.label()}" }
                                span { class: "spacer" }
                                span { "{(attempt.score * 100.0).round()}%" }
                            }
                            div { class: "drill-words",
                                for (i, word) in attempt.words.iter().enumerate() {
                                    span { key: "{i}", class: if word.matched { "word" } else { "word missed" }, "{word.word}" }
                                }
                            }
                            if verdict != Verdict::Good {
                                div { class: "caption2 secondary", "Marked words didn’t come through. Listen once more, then say it a little slower." }
                            }
                        }
                    }
                }
            }

            div { class: "drill-footer",
                button { class: "text-button row", onclick: move |_| app.shadow_in_films(),
                    Icon { name: "film", size: 15 } "Hear it in films"
                }
                if let Some(id) = next {
                    button { class: "text-button row", onclick: move |_| app.shadow_open(id.clone()),
                        "Next phrase" Icon { name: "arrow.up.right", size: 14 }
                    }
                }
            }
            if let Some(notice) = notice { div { class: "notice", "{notice}" } }
        }
    }
}

//! The Dialog tab: everyday situations played out from hello to goodbye.
//!
//! A scenario is a brief rather than a script. The learner reads who they are and what they want,
//! then holds the conversation in Talk; the beats tick off as the exchange reaches them.

use dioxus::prelude::*;

use super::components::PageHeading;
use super::icons::Icon;
use super::state::{Mural, Sheet, Tab};
use crate::engine::phrases::Band;
use crate::engine::scenarios::{self, Scenario};
use crate::engine::text::contains_ci;
use crate::services::dialog_store;

#[component]
pub fn DialogView() -> Element {
    let app = use_context::<Mural>();
    let language = app.store.read().language();
    let mut chosen_id = use_signal(|| None::<String>);

    if !scenarios::available(language.id) {
        return rsx! {
            div { class: "page stack",
                PageHeading { eyebrow: "Dialogues", title: "Not in {language.name}\nyet",
                    subtitle: "Scenarios are written per language, and this one has none yet." }
            }
        };
    }
    let picked = chosen_id().and_then(|id| scenarios::find(language.id, &id));
    match picked {
        Some(scenario) => rsx! { Brief { scenario: scenario.id.clone(), on_back: move |_| chosen_id.set(None) } },
        None => rsx! { ScenarioList { on_pick: move |id: String| chosen_id.set(Some(id)) } },
    }
}

#[component]
fn ScenarioList(on_pick: EventHandler<String>) -> Element {
    let app = use_context::<Mural>();
    let mut search = use_signal(String::new);
    let mut band = use_signal(|| None::<Band>);
    let language = app.store.read().language();
    let meaning_visible = app.store.read().preferences().meaning_visible;
    let record = dialog_store::current();

    let query = search.read().clone();
    let chosen_band = band();
    let list: Vec<&Scenario> = scenarios::all(language.id).iter().filter(|s| {
        chosen_band.map(|b| s.cefr == b).unwrap_or(true)
            && (query.is_empty() || contains_ci(&s.title, &query) || contains_ci(&s.title_en, &query)
                || contains_ci(&s.category, &query) || contains_ci(&s.goal_en, &query))
    }).collect();
    let grouped: Vec<(&str, Vec<&Scenario>)> = scenarios::categories(language.id).into_iter()
        .map(|c| (c, list.iter().copied().filter(|s| s.category == c).collect::<Vec<_>>()))
        .filter(|(_, rows)| !rows.is_empty())
        .collect();

    let played = record.played();
    let completed = record.completed();
    let total = scenarios::all(language.id).len();

    rsx! {
        div { class: "page stack",
            PageHeading { eyebrow: "Dialogues", title: "A conversation\nwith a plan",
                subtitle: "Everyday situations played from hello to goodbye. You know where it is going." }
            div { class: "shadow-tally",
                span { "{played} of {total} played" }
                span { class: "spacer" }
                span { "{completed} finished" }
            }
            div { class: "search",
                Icon { name: "magnifyingglass", size: 15 }
                input { r#type: "search", placeholder: "Find a situation", value: "{search}", oninput: move |e| search.set(e.value()) }
            }
            button { class: "action row", onclick: move |_| app.open_sheet(Sheet::CurrentTopic),
                Icon { name: "newspaper", size: 16 } "Something in the news instead"
            }
            div { class: "chips",
                button { class: if chosen_band.is_none() { "chip selected" } else { "chip" },
                    onclick: move |_| band.set(None), "All levels"
                }
                for b in scenarios::bands(language.id) {
                    button { key: "{b.label()}", class: if chosen_band == Some(b) { "chip selected" } else { "chip" },
                        aria_pressed: "{chosen_band == Some(b)}", title: "{b.gist()}",
                        onclick: move |_| band.set(Some(b)), "{b.label()}"
                    }
                }
            }
            if grouped.is_empty() {
                div { class: "sub secondary", "No situation matches that. Try another word." }
            } else {
                for (category, rows) in grouped {
                    div { key: "{category}", class: "band-group",
                        div { class: "band-head",
                            span { class: "caption2 secondary", "{category}" }
                            span { class: "spacer" }
                            span { class: "caption2 secondary", "{rows.len()}" }
                        }
                        for scenario in rows {
                            {
                                let progress = record.get(&scenario.id);
                                let id = scenario.id.clone();
                                rsx! {
                                    button { key: "{scenario.id}", class: "phrase-row",
                                        onclick: move |_| on_pick.call(id.clone()),
                                        div { class: "phrase-text",
                                            span { class: "said", "{scenario.title}" }
                                            if meaning_visible { span { class: "sub secondary", "{scenario.title_en} · {scenario.minutes} min" } }
                                            else { span { class: "sub secondary", "{scenario.minutes} min" } }
                                        }
                                        div { class: if progress.completed { "phrase-mark settled" } else { "phrase-mark" },
                                            span { class: "band-badge", "{scenario.cefr.label()}" }
                                            if progress.completed { Icon { name: "checkmark.circle.fill", size: 17 } }
                                            else if progress.runs > 0 { span { class: "caption2 secondary", "{progress.best_beats}/{scenario.beats.len()}" } }
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
fn Brief(scenario: String, on_back: EventHandler<()>) -> Element {
    let app = use_context::<Mural>();
    let language = app.store.read().language();
    let meaning_visible = app.store.read().preferences().meaning_visible;
    let Some(scenario) = scenarios::find(language.id, &scenario) else {
        return rsx! { div { class: "page stack", "That situation has gone." } };
    };
    let progress = dialog_store::current().get(&scenario.id);
    let running = app.conv.read().is_running();

    let start = move |_| {
        app.choose_scenario(Some(scenario));
        app.ui.write_unchecked().tab = Tab::Talk;
        if !app.conv.peek().is_running() { app.start(); }
    };

    rsx! {
        div { class: "page stack drill",
            button { class: "text-button row", onclick: move |_| on_back.call(()),
                Icon { name: "chevron.left", size: 14 } "All situations"
            }
            div { class: "drill-card",
                div { class: "drill-phrase selectable", "{scenario.title}" }
                if meaning_visible { div { class: "meaning", "{scenario.title_en}" } }
                div { class: "drill-meta",
                    span { class: "band-badge", "{scenario.cefr.label()}" }
                    span { class: "caption2 secondary", "{scenario.category} · about {scenario.minutes} minutes" }
                }
            }
            div { class: "brief-card",
                div { class: "brief-row",
                    span { class: "brief-label", "Where" }
                    span { "{scenario.setting}" }
                }
                div { class: "brief-row",
                    span { class: "brief-label", "Who" }
                    span { "{scenario.partner}" }
                }
                div { class: "brief-row",
                    span { class: "brief-label", "You want" }
                    span {
                        div { "{scenario.goal}" }
                        if meaning_visible { div { class: "sub secondary", "{scenario.goal_en}" } }
                    }
                }
            }
            div { class: "beats",
                div { class: "caption2 secondary", "The way through" }
                for (i, beat) in scenario.beats.iter().enumerate() {
                    div { key: "{i}", class: "beat",
                        span { class: "beat-number", "{i + 1}" }
                        span {
                            div { "{beat.text}" }
                            if meaning_visible { div { class: "sub secondary", "{beat.en}" } }
                        }
                    }
                }
            }
            button { class: "repeat-button", onclick: start,
                Icon { name: "waveform", size: 22, weight: 1.5 }
                span { if running { "Switch this conversation to the situation" } else { "Start the conversation" } }
            }
            if progress.runs > 0 {
                div { class: "caption2 secondary",
                    if progress.completed { "You have been through this one." }
                    else { "Played {progress.runs} times, {progress.best_beats} of {scenario.beats.len()} beats so far." }
                }
            }
        }
    }
}

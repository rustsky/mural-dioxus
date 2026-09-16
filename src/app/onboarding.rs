use std::time::Duration;

use dioxus::prelude::*;

use super::components::{Brand, ExternalLink, Orb};
use super::icons::Icon;
use super::state::Mural;
use crate::engine::languages::{self, meaning_greeting, MEANING_LANGUAGES};
use crate::engine::models::Preferences;
use crate::engine::{pinyin, AI_CONSENT_SUMMARY};

/// The system's preferred languages, as BCP 47 identifiers.
#[cfg(target_os = "macos")]
fn preferred_languages() -> Vec<String> {
    objc2_foundation::NSLocale::preferredLanguages().iter().map(|s| s.to_string()).collect()
}

#[cfg(not(target_os = "macos"))]
fn preferred_languages() -> Vec<String> {
    std::env::var("LANG").map(|l| vec![l.replace('_', "-")]).unwrap_or_default()
}

fn meaning_name(identifier: &str) -> Option<&'static str> {
    let code = identifier.split(['-', '_']).next().unwrap_or(identifier).to_lowercase();
    Some(match code.as_str() {
        "zh" => "Chinese (Simplified)",
        "no" | "nn" => "Norwegian",
        "pl" => "Polish",
        "ar" => "Arabic",
        "uk" => "Ukrainian",
        other => return languages::module(other).map(|m| m.name),
    })
}

#[component]
pub fn OnboardingView() -> Element {
    let app = use_context::<Mural>();
    let mut step = use_signal(|| 0);
    let mut target_id = use_signal(|| app.store.peek().language().id.to_string());
    let initial_meaning = app.store.peek().preferences().meaning_language.clone();
    let mut has_chosen_meaning = use_signal(|| initial_meaning != Preferences::default().meaning_language);
    let mut meaning_language = use_signal(|| initial_meaning);
    let mut greeting_index = use_signal(|| 0usize);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(3800)).await;
            greeting_index.set((greeting_index() + 1) % languages::all().len());
        }
    });

    let target = languages::module(&target_id()).unwrap_or(&languages::all()[0]);
    let greeting = languages::all()[greeting_index()].greeting;
    let advance = move |_| {
        if step() == 0 {
            let target = languages::module(&target_id()).unwrap_or(&languages::all()[0]);
            if !has_chosen_meaning() && meaning_language() == target.name {
                let preferred = preferred_languages().iter().filter_map(|l| meaning_name(l))
                    .find(|n| MEANING_LANGUAGES.contains(n) && *n != target.name)
                    .or_else(|| MEANING_LANGUAGES.iter().copied().find(|n| *n != target.name))
                    .unwrap_or("English");
                meaning_language.set(preferred.to_string());
            }
            step.set(1);
        } else {
            app.complete_onboarding(&target_id(), &meaning_language());
        }
    };

    rsx! {
        div { class: "onboarding",
            div { class: "onboarding-glow" }
            div { class: "onboarding-top",
                if step() == 1 {
                    button { class: "glass-button", aria_label: "Back to learning language", onclick: move |_| step.set(0),
                        Icon { name: "chevron.left", size: 20 }
                    }
                } else { Brand {} }
                div { class: "steps", aria_label: "Step {step() + 1} of 2",
                    for i in 0..2 { span { key: "{i}", class: if i == step() { "step current" } else { "step" } } }
                }
            }
            div { class: "onboarding-scroll",
                div { key: "{step()}", class: "onboarding-inner",
                    div { aria_label: "Welcome to Mural",
                        div { style: if step() == 0 { "height: 134px; margin-top: 8px" } else { "height: 74px" }, Orb {} }
                        div { key: "{greeting}", class: if step() == 0 { "greeting-big" } else { "greeting-big small" }, "{greeting}" }
                    }
                    if step() == 0 {
                        h2 { "What would you\nlike to speak?" }
                        div { class: "language-list", role: "radiogroup",
                            for language in languages::all() {
                                button { key: "{language.id}", role: "radio",
                                    aria_checked: "{target_id() == language.id}", aria_label: "{language.settings_title()}",
                                    class: if target_id() == language.id { "language-option selected" } else { "language-option" },
                                    onclick: move |_| target_id.set(language.id.to_string()),
                                    div { style: "flex: 1; display: flex; flex-direction: column; gap: 3px",
                                        span { class: "name", "{language.native_name}" }
                                        span { class: "caption secondary", "{language.settings_title()}" }
                                    }
                                    span { class: "check",
                                        Icon { name: if target_id() == language.id { "checkmark.circle.fill" } else { "circle" }, size: 22 }
                                    }
                                }
                            }
                        }
                    } else {
                        div {
                            h2 { "A little help,\nin your language." }
                            p { class: "sub secondary", "Mural speaks {target.name}. Choose the language you read most easily for meanings." }
                        }
                        div { class: "picker-card",
                            select { aria_label: "Subtitle language",
                                onchange: move |e| { meaning_language.set(e.value()); has_chosen_meaning.set(true); },
                                for l in MEANING_LANGUAGES {
                                    option { key: "{l}", value: l, selected: l == meaning_language(), "{l}" }
                                }
                            }
                        }
                        div { style: "display: flex; flex-direction: column; gap: 8px; padding: 12px 0",
                            div { class: "rounded", style: "font-size: 22px; font-weight: 500", "{target.greeting}" }
                            if target.id == "zh" {
                                if let Some(reading) = pinyin::reading(target.greeting) { div { class: "secondary", "{reading}" } }
                            }
                            div { class: "secondary", style: "font-size: 15px", "{meaning_greeting(&meaning_language())}" }
                            div { class: "caption secondary", style: "padding-top: 8px", "Turn meanings on whenever you need a hand." }
                        }
                    }
                }
            }
            div { class: "onboarding-bottom",
                if step() == 1 {
                    div { class: "footnote secondary", style: "text-align: center", "{AI_CONSENT_SUMMARY}" }
                    ExternalLink { label: "Privacy policy", url: "https://mural.chat/privacy/", class: "footnote" }
                }
                button { class: "pill-button", onclick: advance, autofocus: true,
                    if step() == 0 { "Continue" } else { "Agree and continue" }
                }
                div { class: "caption secondary", style: "text-align: center",
                    if step() == 0 { "We’ll find your pace through conversation." } else { "You can change both languages in Settings." }
                }
            }
        }
    }
}

#[component]
pub fn AiConsentSheet() -> Element {
    let app = use_context::<Mural>();
    rsx! {
        div { class: "overlay",
            div { class: "sheet", role: "dialog",
                div { class: "sheet-body", style: "padding-top: 28px",
                    span { style: "color: var(--orange)", Icon { name: "waveform.bubble", size: 34, weight: 1.2 } }
                    h1 { "Before we talk." }
                    div { style: "font-size: 15px; line-height: 1.45", "{AI_CONSENT_SUMMARY}" }
                    div { class: "sub secondary", "Your learning record is stored on this Mac. Mural does not save raw audio. You can keep browsing your saved words and conversations without agreeing." }
                    ExternalLink { label: "Privacy policy", url: "https://mural.chat/privacy/", class: "sub" }
                    button { class: "pill-button center", onclick: move |_| app.accept_ai_consent(), "Agree and continue" }
                    button { class: "sub", onclick: move |_| app.decline_ai_consent(), "Not now" }
                }
            }
        }
    }
}

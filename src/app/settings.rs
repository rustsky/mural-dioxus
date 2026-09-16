use dioxus::prelude::*;

use super::components::{Dialog, DialogButton, ExternalLink, SheetFrame};
use super::icons::Icon;
use super::state::{Mural, Sheet};
use crate::engine::languages::{self, MEANING_LANGUAGES};
use crate::engine::models::Archive;
use crate::services::credentials;

#[component]
pub fn SettingsSheet() -> Element {
    let app = use_context::<Mural>();
    let mut key = use_signal(String::new);
    let mut has_key = use_signal(credentials::has_key);
    let mut message = use_signal(|| None::<String>);
    let mut deleting = use_signal(|| false);
    let mut showing_key = use_signal(|| !credentials::has_key());
    let running = app.conv.read().is_running();
    let store = app.store.read();
    let prefs = store.preferences().clone();
    let language_id = store.language().id;
    let total_voice = store.archive.sessions.iter().fold(0.0, |sum, s| sum + s.voice_seconds);
    let searches: i64 = store.archive.sessions.iter().map(|s| s.search_calls).sum();
    drop(store);
    let voice_time = format!("{} min {} sec", (total_voice / 60.0) as i64, total_voice as i64 % 60);
    let estimate = format!("${:.2} USD", total_voice / 60.0 * 0.05);

    let mut save_key = move |_| {
        let value = key.read().clone();
        match credentials::save(&value) {
            Ok(()) => { key.set(String::new()); has_key.set(true); message.set(Some("Saved securely. Start a conversation to connect.".into())); }
            Err(e) => message.set(Some(e.to_string())),
        }
    };
    let export = move |_| {
        let data = match app.store.peek().export_data() { Ok(d) => d, Err(e) => { message.set(Some(e)); return; } };
        let Some(path) = rfd::FileDialog::new().set_file_name("Mural-learning-backup.json").add_filter("JSON", &["json"]).save_file() else { return };
        match std::fs::write(&path, data) {
            Ok(()) => message.set(Some("Your backup has been exported.".into())),
            Err(e) => message.set(Some(e.to_string())),
        }
    };
    let import = move |_| {
        let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() else { return };
        let result = Archive::read_import_data(&path).map_err(|e| e.to_string())
            .and_then(|data| app.store.write_unchecked().import_data(&data));
        message.set(Some(match result { Ok(()) => "Your backup has been imported.".into(), Err(e) => e }));
    };

    rsx! {
        SheetFrame { title: "Make yourself comfortable",
            trailing: Some(("Done".to_string(), EventHandler::new(move |_| { key.set(String::new()); app.close_sheet(); }))),
            div { class: "form",
                section {
                    div { class: "section-title", "Just your pace" }
                    div { class: "group",
                        label { class: "form-row",
                            span { class: "label", "Learning language" }
                            select { disabled: running, aria_label: "Learning language",
                                onchange: move |e| app.select_language(&e.value()),
                                for m in languages::all() {
                                    option { key: "{m.id}", value: m.id, selected: m.id == language_id, "{m.settings_title()}" }
                                }
                            }
                        }
                        div { class: "form-row",
                            span { class: "label", "Meaning subtitles" }
                            button { class: if prefs.meaning_visible { "switch on" } else { "switch" }, role: "switch",
                                aria_checked: "{prefs.meaning_visible}", aria_label: "Meaning subtitles",
                                onclick: move |_| app.toggle_meaning(),
                            }
                        }
                        label { class: "form-row",
                            span { class: "label", "Meaning language" }
                            select { aria_label: "Meaning language",
                                onchange: move |e| app.select_meaning_language(&e.value()),
                                for l in MEANING_LANGUAGES {
                                    option { key: "{l}", value: l, selected: l == prefs.meaning_language, "{l}" }
                                }
                            }
                        }
                        div { class: "form-row", span { class: "label", "Corrections" } span { class: "value", "Gently, as we talk" } }
                        div { class: "form-row",
                            textarea { rows: 2, placeholder: "A few things you enjoy", value: "{prefs.interests}",
                                oninput: move |e| {
                                    let value: String = e.value().chars().take(500).collect();
                                    app.store.write_unchecked().update_preferences(|p| p.interests = value);
                                },
                            }
                        }
                    }
                    div { class: "section-footer",
                        if running { "End this conversation to switch languages. Each language keeps its own words and progress." }
                        else { "Each language keeps its own words and progress. Mural finds your pace through conversation." }
                    }
                }
                section {
                    div { class: "section-title", "Advanced" }
                    div { class: "group",
                        div { class: "form-row",
                            button { class: "disclosure", aria_expanded: "{showing_key()}", onclick: move |_| showing_key.toggle(),
                                Icon { name: "key", size: 16 }
                                span { class: "label", "Use your own API key" }
                                Icon { name: if showing_key() { "chevron.up" } else { "chevron.down" }, size: 14 }
                            }
                        }
                        if showing_key() {
                            if has_key() {
                                div { class: "form-row", Icon { name: "checkmark.shield", size: 16 } span { "Your key is saved on this Mac" } }
                            }
                            div { class: "form-row",
                                input { r#type: "password", autocomplete: "off", spellcheck: false,
                                    placeholder: if has_key() { "Replace OpenAI key" } else { "OpenAI API key" },
                                    value: "{key}", oninput: move |e| key.set(e.value()),
                                    onkeydown: move |e: Event<KeyboardData>| if e.key() == Key::Enter && !key.read().is_empty() && !running { save_key(()) },
                                }
                            }
                            div { class: "form-row",
                                button { class: "action", disabled: key.read().is_empty() || running, onclick: move |_| save_key(()),
                                    if has_key() { "Save replacement key" } else { "Save key" }
                                }
                            }
                            div { class: "form-row", ExternalLink { label: "Open OpenAI API keys", url: "https://platform.openai.com/api-keys" } }
                            if has_key() {
                                div { class: "form-row",
                                    button { class: "action danger", disabled: running,
                                        onclick: move |_| match credentials::delete() {
                                            Ok(()) => { has_key.set(false); message.set(Some("Your key has been removed.".into())); }
                                            Err(e) => message.set(Some(e.to_string())),
                                        },
                                        "Remove key"
                                    }
                                }
                            }
                            div { class: "form-row footnote secondary", "Your OpenAI account pays for usage. The key stays in this Mac’s Keychain and is sent only to OpenAI." }
                        }
                        if let Some(m) = message() { div { class: "form-row footnote secondary", "{m}" } }
                    }
                    if !has_key() { div { class: "section-footer", "This version uses your OpenAI API key to start a conversation." } }
                }
                section {
                    div { class: "section-title", "Keep it comfortable" }
                    div { class: "group",
                        label { class: "form-row",
                            span { class: "label", "Conversation limit" }
                            select { onchange: move |e| {
                                    if let Ok(v) = e.value().parse::<i64>() { app.store.write_unchecked().update_preferences(|p| p.session_minutes = v); }
                                },
                                for m in [5i64, 10, 15, 20, 30, 60] {
                                    option { key: "{m}", value: "{m}", selected: m == prefs.session_minutes, "{m} minutes" }
                                }
                            }
                        }
                        div { class: "form-row", span { class: "label", "Recorded voice time" } span { class: "value", "{voice_time}" } }
                        div { class: "form-row", span { class: "label", "Voice estimate" } span { class: "value", "{estimate}" } }
                        div { class: "form-row", span { class: "label", "Search calls recorded" } span { class: "value", "{searches}" } }
                        div { class: "form-row", ExternalLink { label: "OpenAI usage and billing", url: "https://platform.openai.com/usage" } }
                    }
                    div { class: "section-footer", "Voice estimate uses $0.05/min as of 11 September 2026. Translation, teaching and search cost extra. Interrupted requests can be billed without a usage record here. Your OpenAI dashboard is authoritative. The time limit is local, not a billing cap." }
                }
                section {
                    div { class: "section-title", "Your words belong to you" }
                    div { class: "group",
                        div { class: "form-row", button { class: "action row", onclick: export, Icon { name: "square.and.arrow.up", size: 16 } "Export learning backup" } }
                        div { class: "form-row", button { class: "action row", disabled: running, onclick: import, Icon { name: "square.and.arrow.down", size: 16 } "Import learning backup" } }
                        div { class: "form-row", button { class: "action danger", disabled: running, onclick: move |_| deleting.set(true), "Delete all conversations and learning" } }
                    }
                    div { class: "section-footer", "Backups include transcripts and learning evidence, never your API key. Import adds conversations with new IDs. Existing conversations stay unchanged. There is no cloud sync." }
                }
                section {
                    div { class: "section-title", "Help and privacy" }
                    div { class: "group",
                        div { class: "form-row", ExternalLink { label: "Privacy policy", url: "https://mural.chat/privacy/" } }
                        div { class: "form-row", ExternalLink { label: "Terms of use", url: "https://mural.chat/terms/" } }
                        div { class: "form-row", ExternalLink { label: "Contact support", url: "https://mural.chat/support/" } }
                    }
                }
                section {
                    div { class: "group",
                        div { class: "form-row footnote", "Mural {env!(\"CARGO_PKG_VERSION\")} · macOS build" }
                        div { class: "form-row footnote", "Voice: GPT-Live-1 · Teacher: GPT-5.6 Luna" }
                        div { class: "form-row", ExternalLink { label: "OpenAI data controls", url: "https://developers.openai.com/api/docs/guides/your-data" } }
                        div { class: "form-row footnote", "Audio and selected text go to OpenAI while you practise. Requests disable provider storage where supported; abuse-monitoring retention may still apply. Raw audio is not saved by Mural." }
                        div { class: "form-row", button { class: "action", onclick: move |_| app.open_sheet(Sheet::Notices), "Open-source notices" } }
                    }
                }
            }
        }
        if deleting() {
            Dialog {
                title: "Delete all learning data on this Mac?",
                message: "This removes conversations, vocabulary and progress. Export a backup first if you want to keep them. Your API key and preferences remain.",
                buttons: vec![DialogButton { label: "Delete all learning data".into(), role: "destructive", action: EventHandler::new(move |_| {
                    app.delete_learning_data();
                    deleting.set(false);
                }) }],
                on_cancel: move |_| deleting.set(false),
            }
        }
    }
}

#[component]
pub fn NoticesSheet() -> Element {
    rsx! {
        SheetFrame { title: "Open-source notices", wide: true,
            pre { class: "footnote selectable", style: "white-space: pre-wrap; font-family: ui-monospace, monospace; margin: 0",
                {concat!(include_str!("../../assets/ThirdPartyNotices.txt"), "\n\n", include_str!("../../assets/RustNotices.txt"))}
            }
        }
    }
}

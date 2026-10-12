use dioxus::prelude::*;

use super::components::{Dialog, DialogButton, ExternalLink, SheetFrame};
use super::icons::Icon;
use super::state::{Mural, Sheet};
use crate::engine::languages::{self, MEANING_LANGUAGES};
use crate::engine::models::Archive;
use crate::services::api;
use crate::services::credentials::{OLLAMA, OPENAI};
use crate::services::provider::{self, Teacher, VoiceStyle};
use crate::services::voice;

const PLATFORM: &str = if cfg!(target_os = "macos") { "macOS" } else if cfg!(windows) { "Windows" } else { "Linux" };

#[component]
pub fn SettingsSheet() -> Element {
    let app = use_context::<Mural>();
    let mut key = use_signal(String::new);
    let mut has_key = use_signal(|| OPENAI.has_key());
    let mut message = use_signal(|| None::<String>);
    let mut deleting = use_signal(|| false);
    let mut showing_key = use_signal(|| !OPENAI.has_key());
    let mut showing_legal = use_signal(|| false);
    let mut held = use_signal(|| provider::current().push_to_talk);
    let running = app.conv.read().is_running();
    let store = app.store.read();
    let prefs = store.preferences().clone();
    let language_id = store.language().id;
    let total_voice = store.archive.sessions.iter().fold(0.0, |sum, s| sum + s.voice_seconds);
    let searches: i64 = store.archive.sessions.iter().map(|s| s.search_calls).sum();
    drop(store);
    // Reading the revision subscribes this sheet to teacher changes.
    let _ = crate::app::state::PROVIDER_REVISION();
    let uses_openai = provider::current().teacher == Teacher::OpenAi;
    // An Ollama teacher needs no OpenAI key, but one already saved must stay removable.
    let show_openai_key = uses_openai || has_key();
    // Voice time and its cost only accrue with OpenAI live voice; past conversations still count.
    let show_usage = uses_openai || total_voice > 0.0 || searches > 0;
    let voice_time = format!("{} min {} sec", (total_voice / 60.0) as i64, total_voice as i64 % 60);
    let estimate = format!("${:.2} USD", total_voice / 60.0 * 0.05);

    let mut save_key = move |_| {
        let value = key.read().clone();
        match OPENAI.save(&value) {
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
                    div { class: "section-title", "Conversations" }
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
                            div { class: "stack",
                                span { class: "label", "Meaning subtitles" }
                                span { class: "desc", "Show what Mural just said in your own language." }
                            }
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
                        div { class: "form-row",
                            div { class: "stack",
                                span { class: "label", "Things you enjoy" }
                                textarea { rows: 2, placeholder: "Design, space, long walks\u{2026}", value: "{prefs.interests}",
                                    oninput: move |e| {
                                        let value: String = e.value().chars().take(500).collect();
                                        app.store.write_unchecked().update_preferences(|p| p.interests = value);
                                    },
                                }
                            }
                        }
                    }
                    div { class: "section-footer",
                        if running { "End this conversation to switch languages. Each language keeps its own words and progress, and Mural corrects gently as you talk." }
                        else { "Each language keeps its own words and progress. Mural finds your pace through conversation and corrects gently as you talk." }
                    }
                }
                section {
                    div { class: "section-title", "Speaking" }
                    div { class: "group",
                        div { class: "form-row",
                            div { class: "stack",
                                span { class: "label", "Hold to speak" }
                                span { class: "desc", "Mural normally decides when your turn has ended. Hold the microphone instead, and pause as long as you like." }
                            }
                            button { class: if held() { "switch on" } else { "switch" }, role: "switch",
                                disabled: running, aria_checked: "{held}", aria_label: "Hold to speak",
                                onclick: move |_| held.set(app.toggle_push_to_talk()),
                            }
                        }
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
                    }
                    div { class: "section-footer",
                        if provider::current().teacher == Teacher::OllamaLocal { "With Ollama on this computer, conversations have no time limit and don\u{2019}t end when you go quiet. End them yourself." }
                        else { "The limit is local, not a billing cap. A quiet conversation ends on its own." }
                    }
                }
                TeacherSection { running }
                if show_openai_key {
                section {
                    div { class: "section-title", if uses_openai { "OpenAI key" } else { "Saved OpenAI key" } }
                    div { class: "group",
                        div { class: "form-row",
                            button { class: "disclosure", aria_expanded: "{showing_key()}", onclick: move |_| showing_key.toggle(),
                                Icon { name: "key", size: 16 }
                                span { class: "label", if has_key() { "Your key is saved" } else { "Use your own OpenAI API key" } }
                                Icon { name: if showing_key() { "chevron.up" } else { "chevron.down" }, size: 14 }
                            }
                        }
                        if showing_key() {
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
                                        onclick: move |_| match OPENAI.delete() {
                                            Ok(()) => { has_key.set(false); message.set(Some("Your key has been removed.".into())); }
                                            Err(e) => message.set(Some(e.to_string())),
                                        },
                                        "Remove key"
                                    }
                                }
                            }
                        }
                        if let Some(m) = message() { div { class: "form-row footnote secondary", "{m}" } }
                    }
                    div { class: "section-footer",
                        if !uses_openai { "Your teacher is Ollama, so this key is unused. It stays in this computer\u{2019}s keychain until you remove it." }
                        else if has_key() { "Your OpenAI account pays for usage. The key stays in this computer\u{2019}s keychain and is sent only to OpenAI." }
                        else { "Live voice and teaching use this key." }
                    }
                }
                }
                if show_usage {
                section {
                    div { class: "section-title", "Usage" }
                    div { class: "group",
                        div { class: "form-row", span { class: "label", "Recorded voice time" } span { class: "value", "{voice_time}" } }
                        div { class: "form-row", span { class: "label", "Voice estimate" } span { class: "value", "{estimate}" } }
                        div { class: "form-row", span { class: "label", "Search calls recorded" } span { class: "value", "{searches}" } }
                        div { class: "form-row", ExternalLink { label: "OpenAI usage and billing", url: "https://platform.openai.com/usage" } }
                    }
                    div { class: "section-footer", "Estimated at $0.05/min as of 11 September 2026. Teaching, translation and search cost extra, and interrupted requests can be billed without appearing here. Your OpenAI dashboard is authoritative." }
                }
                }
                section {
                    div { class: "section-title", "Your words belong to you" }
                    div { class: "group",
                        div { class: "form-row", button { class: "action row", onclick: export, Icon { name: "square.and.arrow.up", size: 16 } "Export learning backup" } }
                        div { class: "form-row", button { class: "action row", disabled: running, onclick: import, Icon { name: "square.and.arrow.down", size: 16 } "Import learning backup" } }
                        div { class: "form-row", button { class: "action danger", disabled: running, onclick: move |_| deleting.set(true), "Delete all conversations and learning" } }
                    }
                    div { class: "section-footer", "Backups hold transcripts and learning evidence, never your API key. Importing adds conversations with new IDs and leaves existing ones alone. There is no cloud sync." }
                }
                section {
                    div { class: "section-title", "About" }
                    div { class: "group",
                        div { class: "form-row", span { class: "label", "Version" } span { class: "value", "{env!(\"CARGO_PKG_VERSION\")} · {PLATFORM}" } }
                        div { class: "form-row", ExternalLink { label: "Privacy policy", url: "https://mural.chat/privacy/" } }
                        div { class: "form-row", ExternalLink { label: "Terms of use", url: "https://mural.chat/terms/" } }
                        div { class: "form-row", ExternalLink { label: "Contact support", url: "https://mural.chat/support/" } }
                        div { class: "form-row",
                            button { class: "disclosure", aria_expanded: "{showing_legal()}", onclick: move |_| showing_legal.toggle(),
                                Icon { name: "checkmark.shield", size: 16 }
                                span { class: "label", "How your data and licence work" }
                                Icon { name: if showing_legal() { "chevron.up" } else { "chevron.down" }, size: 14 }
                            }
                        }
                        if showing_legal() {
                            div { class: "form-row footnote", "Audio and selected text go to OpenAI while you practise. Requests disable provider storage where supported; abuse-monitoring retention may still apply. Mural never saves raw audio." }
                            div { class: "form-row", ExternalLink { label: "OpenAI data controls", url: "https://developers.openai.com/api/docs/guides/your-data" } }
                            div { class: "form-row footnote", "Mural is free software under the GNU General Public License, version 3 or later, and comes with absolutely no warranty. You may share and change it under that licence." }
                        }
                        div { class: "form-row", button { class: "action", onclick: move |_| app.open_sheet(Sheet::Notices), "Licences and open-source notices" } }
                    }
                }
            }
        }
        if deleting() {
            Dialog {
                title: "Delete all learning data on this computer?",
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
        SheetFrame { title: "Licences and open-source notices", wide: true,
            pre { class: "footnote selectable", style: "white-space: pre-wrap; font-family: ui-monospace, monospace; margin: 0",
                {concat!(
                    include_str!("../../THIRD_PARTY_NOTICES.md"),
                    "\n\n---\nLICENSE-MIT (original Mural project)\n\n", include_str!("../../LICENSE-MIT"),
                    "\n\n---\nNotices carried over from the original Mural apps\n\n", include_str!("../../assets/ThirdPartyNotices.txt"),
                    "\n\n---\n", include_str!("../../THIRD_PARTY_RUST.txt"),
                    "\n\n---\n", include_str!("../../LICENSE"),
                )}
            }
        }
    }
}

#[component]
fn TeacherSection(running: bool) -> Element {
    let app = use_context::<Mural>();
    let language_id = app.store.read().language().id;
    let mut models_changed = use_signal(|| 0u32);
    let mut settings = use_signal(provider::current);
    let mut key = use_signal(String::new);
    let mut has_key = use_signal(|| OLLAMA.has_key());
    let mut message = use_signal(|| None::<String>);
    let mut models = use_signal(Vec::<String>::new);
    let mut loading = use_signal(|| false);
    let current = settings();
    let teacher = current.teacher;
    let model = current.model().to_string();

    let mut apply = move |change: Box<dyn FnOnce(&mut provider::ProviderSettings)>| {
        match crate::app::state::update_provider(change) {
            Ok(updated) => settings.set(updated),
            Err(e) => message.set(Some(e)),
        }
    };
    let mut refresh = move || {
        let teacher = settings.peek().teacher;
        if !teacher.is_ollama() { return; }
        loading.set(true);
        spawn(async move {
            match api::ollama_models(teacher).await {
                Ok(list) => { message.set(if list.is_empty() { Some("No models found that fit in this computer’s memory. Run “ollama pull llama3.2” first.".into()) } else { None }); models.set(list); }
                Err(e) => { models.set(vec![]); message.set(Some(e.to_string())); }
            }
            loading.set(false);
        });
    };
    let mut save_key = move || {
        let value = key.read().clone();
        match OLLAMA.save(&value) {
            Ok(()) => { key.set(String::new()); has_key.set(true); message.set(Some("Ollama key saved to this computer’s keychain.".into())); }
            Err(e) => message.set(Some(e.to_string())),
        }
    };
    let destination = match teacher {
        Teacher::OpenAi => "OpenAI",
        Teacher::OllamaCloud => "Ollama Cloud",
        Teacher::OllamaLocal => "the Ollama server on this computer",
    };

    rsx! {
        section {
            div { class: "section-title", "Teacher" }
            div { class: "group",
                label { class: "form-row",
                    span { class: "label", "Meanings and feedback" }
                    select { disabled: running, aria_label: "Teacher",
                        onchange: move |e| {
                            if let Some(t) = Teacher::from_id(&e.value()) {
                                models.set(vec![]);
                                message.set(None);
                                apply(Box::new(move |s| s.teacher = t));
                                if t.is_ollama() { refresh(); }
                            }
                        },
                        for t in Teacher::ALL {
                            option { key: "{t.id()}", value: t.id(), selected: t == teacher, "{t.label()}" }
                        }
                    }
                }
                if teacher.is_ollama() {
                    div { class: "form-row",
                        span { class: "label", "Model" }
                        input { r#type: "text", list: "ollama-models", spellcheck: false, disabled: running,
                            style: "text-align: right; color: var(--secondary)",
                            value: "{model}",
                            onchange: move |e| {
                                let value = e.value().trim().to_string();
                                if value.is_empty() { return; }
                                apply(Box::new(move |s| match s.teacher {
                                    Teacher::OllamaLocal => s.local_model = value,
                                    _ => s.cloud_model = value,
                                }));
                            },
                        }
                        datalist { id: "ollama-models",
                            for m in models() { option { key: "{m}", value: "{m}" } }
                        }
                    }
                    div { class: "form-row",
                        button { class: "action", disabled: loading(), onclick: move |_| refresh(),
                            if loading() { "Checking models…" } else if models().is_empty() { "Show available models" } else { "{models().len()} models available · refresh" }
                        }
                    }
                }
                if teacher.is_ollama() {
                    label { class: "form-row",
                        span { class: "label", "Voice" }
                        select { disabled: running, aria_label: "Voice",
                            onchange: move |e| {
                                let style = if e.value() == "system" { VoiceStyle::System } else { VoiceStyle::Natural };
                                apply(Box::new(move |s| s.voice = style));
                            },
                            option { value: "natural", selected: current.voice == VoiceStyle::Natural, "Natural (on this computer)" }
                            option { value: "system", selected: current.voice == VoiceStyle::System, "System speech" }
                        }
                    }
                    if current.voice == VoiceStyle::Natural {
                        {
                            let _ = models_changed();
                            let missing = voice::missing_megabytes(language_id);
                            rsx! {
                                div { class: "form-row footnote secondary",
                                    if !voice::supported(language_id) { "This language uses system speech." }
                                    else if missing > 0 { "The first conversation downloads about {missing} MB of voice models for this language." }
                                    else { "Voice models for this language are ready." }
                                }
                                if voice::directory().exists() {
                                    div { class: "form-row",
                                        button { class: "action danger", disabled: running,
                                            onclick: move |_| {
                                                if let Err(e) = voice::remove_all() { message.set(Some(e)); }
                                                models_changed += 1;
                                            },
                                            "Delete downloaded voice models"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if teacher == Teacher::OllamaCloud || (teacher == Teacher::OllamaLocal && has_key()) {
                    if has_key() {
                        div { class: "form-row", Icon { name: "checkmark.shield", size: 16 } span { class: "label", "Your Ollama key is saved on this computer" }
                            button { class: "action danger", disabled: running,
                                onclick: move |_| match OLLAMA.delete() {
                                    Ok(()) => { has_key.set(false); message.set(Some("Your Ollama key has been removed.".into())); }
                                    Err(e) => message.set(Some(e.to_string())),
                                },
                                "Remove"
                            }
                        }
                    }
                }
                if teacher.is_ollama() {
                    div { class: "form-row",
                        input { r#type: "password", autocomplete: "off", spellcheck: false,
                            placeholder: if has_key() { "Replace Ollama API key" } else if teacher == Teacher::OllamaCloud { "Ollama API key" } else { "Ollama API key (optional, for topic search)" },
                            value: "{key}", oninput: move |e| key.set(e.value()),
                            onkeydown: move |e: Event<KeyboardData>| if e.key() == Key::Enter && !key.read().is_empty() { save_key() },
                        }
                        button { class: "action", disabled: key.read().is_empty(), onclick: move |_| save_key(), "Save" }
                    }
                    div { class: "form-row",
                        if teacher == Teacher::OllamaCloud {
                            ExternalLink { label: "Create a free Ollama API key", url: "https://ollama.com/settings/keys" }
                        } else {
                            ExternalLink { label: "Download Ollama", url: "https://ollama.com/download" }
                        }
                    }
                }
                if let Some(m) = message() { div { class: "form-row footnote secondary", "{m}" } }
            }
            div { class: "section-footer",
                "Meanings, lookups, typed replies and topic searches go to {destination}. "
                if teacher.is_ollama() && current.voice == VoiceStyle::Natural { "Natural voice listens and speaks with open models on this computer. " }
                if teacher.is_ollama() && current.voice == VoiceStyle::System { "System speech uses this computer’s own recognition and voices, so Dictation must be on. " }
                else { "Live voice uses OpenAI. " }
                if teacher == Teacher::OllamaCloud { "Ollama’s free plan has usage limits. " }
                if teacher == Teacher::OllamaLocal { "Local models may be slower, and topic search needs an Ollama key. " }
                if teacher.is_ollama() { "Smaller models log fewer words." }
            }
        }
    }
}

#[cfg(debug_assertions)]
mod automation;
mod components;
mod icons;
mod library;
mod onboarding;
mod settings;
mod state;
mod talk;

use dioxus::desktop::tao::event::{Event as TaoEvent, WindowEvent};
use dioxus::desktop::use_wry_event_handler;
use dioxus::prelude::*;

use components::{Brand, Dialog};
use icons::Icon;
use state::{Mural, Sheet, Tab};

pub const STYLE: &str = include_str!("style.css");

/// Set before launch: Ok(store) or a startup error message.
pub static STARTUP: std::sync::Mutex<Option<Result<crate::services::store::LearningStore, String>>> = std::sync::Mutex::new(None);

#[component]
pub fn App() -> Element {
    let startup = use_hook(|| STARTUP.lock().ok().and_then(|mut s| s.take())
        .unwrap_or_else(|| Err("The learning record is unavailable.".into())));
    match startup {
        Ok(store) => rsx! { Main { store: Signal::new(Some(store)) } },
        Err(_) => rsx! {
            div { class: "startup-error",
                Icon { name: "leaf", size: 40 }
                h1 { "Let’s try again" }
                p { class: "secondary", "Mural couldn’t open its learning record. Your existing data has not been replaced." }
            }
        },
    }
}

#[component]
fn Main(store: Signal<Option<crate::services::store::LearningStore>>) -> Element {
    let app = use_hook(|| Mural::new(store.write().take().expect("store")));
    use_context_provider(|| app);
    use_hook(|| app.attach_bridge());
    #[cfg(debug_assertions)]
    use_hook(|| { automation::start(app); app.prepare_preview(); });
    use_wry_event_handler(move |event, _| {
        if let TaoEvent::WindowEvent { event: WindowEvent::Focused(true), .. } = event { app.resume(); }
    });

    let ui = app.ui.read();
    let tab = ui.tab;
    let sheets = ui.sheets.clone();
    let onboarding = ui.onboarding;
    drop(ui);
    let error = app.conv.read().error.clone().or_else(|| app.store.read().error.clone());

    let tab_button = move |value: Tab, label: &'static str, icon: &'static str| rsx! {
        button { class: if tab == value { "tab selected" } else { "tab" }, role: "tab", aria_selected: "{tab == value}",
            onclick: move |_| app.ui.write_unchecked().tab = value,
            Icon { name: icon, size: 16 } "{label}"
        }
    };

    rsx! {
        div { class: "shell", tabindex: "-1",
            onkeydown: move |e: Event<KeyboardData>| {
                if e.key() == Key::Escape && !app.ui.peek().sheets.is_empty() {
                    let top = app.ui.peek().sheets.last().cloned();
                    if top != Some(Sheet::AiConsent) { app.close_sheet(); }
                }
                if e.modifiers().meta() && e.key() == Key::Character(",".into()) { app.open_sheet(Sheet::Settings); }
            },
            header { class: "toolbar",
                Brand {}
                nav { class: "tabs", role: "tablist",
                    {tab_button(Tab::Talk, "Talk", "waveform")}
                    {tab_button(Tab::Themes, "Themes", "square.grid.2x2")}
                    {tab_button(Tab::Words, "Words", "book")}
                }
                button { class: "glass-button", aria_label: "Settings", title: "Settings", onclick: move |_| app.open_sheet(Sheet::Settings),
                    Icon { name: "slider.horizontal.3", size: 18 }
                }
            }
            main { class: "content",
                match tab {
                    Tab::Talk => rsx! { talk::TalkView {} },
                    Tab::Themes => rsx! { library::ThemesView {} },
                    Tab::Words => rsx! { library::WordsView {} },
                }
            }
            for (i, sheet) in sheets.into_iter().enumerate() {
                div { key: "{i}-{sheet:?}",
                    match sheet {
                        Sheet::Settings => rsx! { settings::SettingsSheet {} },
                        Sheet::AiConsent => rsx! { onboarding::AiConsentSheet {} },
                        Sheet::TypedReply => rsx! { talk::TypedReplySheet {} },
                        Sheet::Transcript => rsx! { library::TranscriptSheet {} },
                        Sheet::Lookup { word, sentence } => rsx! { talk::LookupSheet { word, sentence } },
                        Sheet::CurrentTopic => rsx! { library::CurrentTopicSheet {} },
                        Sheet::WordDetail(id) => rsx! { library::WordDetailSheet { id } },
                        Sheet::History => rsx! { library::HistorySheet {} },
                        Sheet::EditableTranscript(session_id) => rsx! { library::EditableTranscriptSheet { session_id } },
                        Sheet::Notices => rsx! { settings::NoticesSheet {} },
                    }
                }
            }
            if onboarding { onboarding::OnboardingView {} }
            if let Some(error) = error {
                Dialog { title: "A little interruption", message: error, buttons: vec![], cancel_label: "OK", on_cancel: move |_| app.clear_errors() }
            }
        }
    }
}

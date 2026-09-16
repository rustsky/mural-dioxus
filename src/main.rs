#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod engine;
mod services;

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{Config, WindowBuilder};

fn main() {
    let in_memory = std::env::args().any(|a| a == "--preview");
    let store = services::store::LearningStore::open(in_memory);
    if let Err(e) = &store { eprintln!("Mural couldn't open its learning record: {e}"); }
    *app::STARTUP.lock().unwrap() = Some(store);

    let window = WindowBuilder::new()
        .with_title("Mural")
        .with_inner_size(LogicalSize::new(900.0, 860.0))
        .with_min_inner_size(LogicalSize::new(520.0, 640.0));
    let config = Config::new()
        .with_window(window)
        .with_background_color((255, 249, 238, 255))
        .with_data_directory(services::store::LearningStore::directory().join("WebView"))
        .with_custom_head(format!("<meta name=\"color-scheme\" content=\"light\"><style>{}</style>", app::STYLE))
        .with_disable_context_menu(!cfg!(debug_assertions));
    dioxus::LaunchBuilder::desktop().with_cfg(config).launch(app::App);
}

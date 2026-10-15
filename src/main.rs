#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod engine;
mod services;

/// The webview renderer. On Android and iOS it is the same crate under its mobile name.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use dioxus::desktop as renderer;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub use dioxus::mobile as renderer;

use crate::renderer::tao::dpi::LogicalSize;
use crate::renderer::{Config, WindowBuilder};

/// A conversation is worth reading, so the window opens as tall as the screen comfortably allows
/// rather than at one fixed guess. The screen is measured in points, like the window itself.
fn opening_size() -> LogicalSize<f64> {
    let (width, height) = match screen_size() {
        Some((w, h)) => ((w - 120.0).clamp(520.0, 1000.0), (h - 60.0).clamp(640.0, 1100.0)),
        None => (900.0, 860.0),
    };
    LogicalSize::new(width, height)
}

/// The usable area of the main screen, with the menu bar and Dock already taken out.
#[cfg(target_os = "macos")]
fn screen_size() -> Option<(f64, f64)> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;
    let screen = NSScreen::mainScreen(MainThreadMarker::new()?)?;
    let frame = screen.visibleFrame();
    Some((frame.size.width, frame.size.height))
}

#[cfg(not(target_os = "macos"))]
fn screen_size() -> Option<(f64, f64)> { None }

fn main() {
    // Android has no environment to set RUST_BACKTRACE in; debug builds print one regardless.
    #[cfg(all(target_os = "android", debug_assertions))]
    std::panic::set_hook(Box::new(|info| eprintln!("{info}\n{}", std::backtrace::Backtrace::force_capture())));
    // On Android the app is a library inside the Java process, and reading its arguments panics.
    let in_memory = cfg!(not(target_os = "android")) && std::env::args().any(|a| a == "--preview");
    let store = services::store::LearningStore::open(in_memory);
    if let Err(e) = &store { eprintln!("Mural couldn't open its learning record: {e}"); }
    *app::STARTUP.lock().unwrap() = Some(store);

    let window = WindowBuilder::new().with_title("Mural");
    // A phone's window is its screen; a size from here would make the page wider than it.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let window = window.with_inner_size(opening_size()).with_min_inner_size(LogicalSize::new(520.0, 640.0));
    let config = Config::new()
        .with_window(window)
        .with_background_color((255, 249, 238, 255))
        .with_data_directory(services::store::LearningStore::directory().join("WebView"))
        .with_custom_head(format!("<meta name=\"color-scheme\" content=\"light\"><style>{}</style>", app::STYLE))
        .with_disable_context_menu(!cfg!(debug_assertions));
    // On Windows, Dioxus serves the page from plain http://dioxus.index.html/, which is not a
    // secure context, so WebView2 hides navigator.mediaDevices and voice can't reach the
    // microphone. Mark that origin as secure. Setting args replaces wry's defaults, so keep them.
    #[cfg(target_os = "windows")]
    let config = config.with_windows_browser_args(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
         --unsafely-treat-insecure-origin-as-secure=http://dioxus.index.html",
    );
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    dioxus::LaunchBuilder::desktop().with_cfg(config).launch(app::App);
    #[cfg(any(target_os = "android", target_os = "ios"))]
    dioxus::LaunchBuilder::mobile().with_cfg(config).launch(app::App);
}

//! Debug-only UI automation for verifying the real WebKit rendering without screen-recording access.
//! Set MURAL_AUTOMATION=<dir>; write commands to <dir>/commands (one per line):
//!   js <code>        evaluate JavaScript in the page
//!   snap <name>      save <dir>/<name>.tiff from WKWebView
//!   jsout <name> <code>  save the returned JSON to <dir>/<name>.txt
//!   event <json>     handle a provider event (debug.start opens an offline conversation)
//!   dom <name>       save <dir>/<name>.html with the current DOM
use std::path::PathBuf;
use std::time::Duration;

use dioxus::desktop::window;
use dioxus::prelude::*;

pub fn start(app: super::state::Mural) {
    let Ok(dir) = std::env::var("MURAL_AUTOMATION") else { return };
    let dir = PathBuf::from(dir);
    spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let path = dir.join("commands");
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let _ = std::fs::remove_file(&path);
            for line in text.lines() {
                let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));
                match cmd {
                    "js" => { let _ = document::eval(arg).await; }
                    "jsout" => {
                        let (name, code) = arg.split_once(' ').unwrap_or((arg, ""));
                        let value = document::eval(code).join::<serde_json::Value>().await.map(|v| v.to_string()).unwrap_or_else(|e| format!("error: {e:?}"));
                        let _ = std::fs::write(dir.join(format!("{name}.txt")), value);
                    }
                    "event" => if let Ok(value) = serde_json::from_str(arg) { app.debug_event(value) },
                    "wait" => tokio::time::sleep(Duration::from_millis(arg.parse().unwrap_or(500))).await,
                    "dom" => {
                        if let Ok(html) = document::eval("return document.documentElement.outerHTML").join::<String>().await {
                            let _ = std::fs::write(dir.join(format!("{arg}.html")), html);
                        }
                    }
                    "snap" => snapshot(dir.join(format!("{arg}.tiff"))),
                    _ => {}
                }
            }
            let _ = std::fs::write(dir.join("done"), "");
        }
    });
}

#[cfg(target_os = "macos")]
fn snapshot(path: PathBuf) {
    use block2::RcBlock;
    use dioxus::desktop::wry::WebViewExtMacOS;
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSError;
    let webview = window().webview.webview();
    let block = RcBlock::new(move |image: *mut NSImage, _error: *mut NSError| {
        let Some(image) = (unsafe { image.as_ref() }) else { return };
        if let Some(data) = image.TIFFRepresentation() {
            let _ = std::fs::write(&path, data.to_vec());
        }
    });
    unsafe { webview.takeSnapshotWithConfiguration_completionHandler(None, &block) };
}

#[cfg(not(target_os = "macos"))]
fn snapshot(_path: PathBuf) {}

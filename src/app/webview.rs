//! Platform webview setup that wry does not do for us.

/// WebKitGTK ships with media capture off and denies permission requests nobody answers, so the
/// microphone would never open on Linux. Grant microphone-only requests; everything else keeps
/// WebKitGTK's default. macOS (WKWebView) and Windows (WebView2) ask the user themselves.
#[cfg(target_os = "linux")]
pub fn prepare() {
    use dioxus::desktop::wry::WebViewExtUnix;
    use webkit2gtk::glib::Cast;
    use webkit2gtk::{PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, UserMediaPermissionRequestExt, WebViewExt};

    let webview = dioxus::desktop::window().webview.webview();
    if let Some(settings) = WebViewExt::settings(&webview) {
        settings.set_enable_media_stream(true);
        settings.set_enable_webrtc(true);
    }
    webview.connect_permission_request(|_, request| {
        match request.downcast_ref::<UserMediaPermissionRequest>() {
            Some(media) if media.is_for_audio_device() && !media.is_for_video_device() => {
                request.allow();
                true
            }
            _ => false,
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn prepare() {}

//! Line icons standing in for the SF Symbols used on iPhone (drawn in the style of Lucide, ISC).
use dioxus::prelude::*;

fn paths(name: &str) -> &'static str {
    match name {
        "waveform" => r#"<path d="M2 12h2M6 8v8M10 4v16M14 7v10M18 10v4M22 12h-2"/>"#,
        "square.grid.2x2" => r#"<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>"#,
        "book" => r#"<path d="M4 19.5V5a2 2 0 0 1 2-2h14v16H6.5A2.5 2.5 0 0 0 4 21.5"/><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>"#,
        "slider.horizontal.3" => r#"<path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1"/><circle cx="15" cy="6" r="2"/><circle cx="9" cy="12" r="2"/><circle cx="17" cy="18" r="2"/>"#,
        "mic" => r#"<rect x="9" y="2" width="6" height="12" rx="3"/><path d="M19 10v1a7 7 0 0 1-14 0v-1M12 18v4M8 22h8"/>"#,
        "mic.slash" => r#"<path d="M2 2l20 20M18.9 13.1A7 7 0 0 0 19 11v-1M5 10v1a7 7 0 0 0 11.2 5.6M15 9.3V5a3 3 0 0 0-5.7-1.3M9 9v2a3 3 0 0 0 5 2.2M12 18v4M8 22h8"/>"#,
        "phone.down" => r#"<path d="M3 14.5c5-5 13-5 18 0l-2.2 2.6a1.2 1.2 0 0 1-1.5.3l-2.3-1.2a1.2 1.2 0 0 1-.6-1.2l.2-1.4a10 10 0 0 0-5.2 0l.2 1.4a1.2 1.2 0 0 1-.6 1.2L6.7 17.4a1.2 1.2 0 0 1-1.5-.3z"/>"#,
        "text.bubble" => r#"<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M7 8h10M7 12h6"/>"#,
        "captions" => r#"<rect x="3" y="5" width="18" height="14" rx="3"/><path d="M7 15h4M15 15h2M7 11h2M13 11h4"/>"#,
        "keyboard" => r#"<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M18 13h.01M10 13h4M7 16h10"/>"#,
        "sparkles" => r#"<path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9z"/><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"/>"#,
        "arrow.counterclockwise" => r#"<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>"#,
        "link" => r#"<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>"#,
        "arrow.up.right" => r#"<path d="M7 17L17 7M7 7h10v10"/>"#,
        "arrow.up" => r#"<path d="M12 19V5M5 12l7-7 7 7"/>"#,
        "chevron.left" => r#"<path d="M15 18l-6-6 6-6"/>"#,
        "chevron.down" => r#"<path d="M6 9l6 6 6-6"/>"#,
        "chevron.up" => r#"<path d="M18 15l-6-6-6 6"/>"#,
        "xmark" => r#"<path d="M18 6L6 18M6 6l12 12"/>"#,
        "checkmark.circle.fill" => r#"<circle cx="12" cy="12" r="10" fill="currentColor"/><path d="M8 12.5l2.7 2.7L16.5 9" stroke="white"/>"#,
        "circle" => r#"<circle cx="12" cy="12" r="10"/>"#,
        "leaf" => r#"<path d="M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.5 19 2c1 2 2 4.2 2 8 0 5.5-4.8 10-10 10z"/><path d="M2 21c0-3 1.9-5.4 5.1-6C9.5 14.5 12 13 13 12"/>"#,
        "clock.arrow.circlepath" => r#"<path d="M3 12a9 9 0 1 0 2.6-6.4L3 8"/><path d="M3 3v5h5M12 7v5l3 2"/>"#,
        "key" => r#"<circle cx="7.5" cy="15.5" r="5.5"/><path d="M11.5 11.5L21 2M16 7l3 3M18.5 4.5l2 2"/>"#,
        "checkmark.shield" => r#"<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="M9 12l2 2 4-4"/>"#,
        "waveform.bubble" => r#"<path d="M21 11.5a8.4 8.4 0 0 1-9 8.4 9 9 0 0 1-3.8-.9L3 21l1.9-5.2A8.4 8.4 0 0 1 12 3a8.4 8.4 0 0 1 9 8.5z"/><path d="M8 11v2M11 9v6M14 10v4M17 11v2"/>"#,
        "magnifyingglass" => r#"<circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/>"#,
        "square.and.arrow.up" => r#"<path d="M4 12v7a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7M16 6l-4-4-4 4M12 2v13"/>"#,
        "square.and.arrow.down" => r#"<path d="M4 12v7a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7M8 11l4 4 4-4M12 3v12"/>"#,
        "trash" => r#"<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/>"#,
        // Theme symbols
        "cup.and.saucer" => r#"<path d="M17 8h1a4 4 0 0 1 0 8h-1M3 8h14v7a5 5 0 0 1-5 5H8a5 5 0 0 1-5-5z"/><path d="M7 2v2M11 2v2"/>"#,
        "sun.horizon" => r#"<path d="M17 18a5 5 0 0 0-10 0M12 2v7M4.2 10.2l1.4 1.4M1 18h2M21 18h2M18.4 11.6l1.4-1.4M23 22H1M8 6l4-4 4 4"/>"#,
        "tree" => r#"<path d="M12 22v-7M8 15h8l-4-5h3l-3-4h2l-4-4-4 4h2l-3 4h3z"/>"#,
        "fork.knife" => r#"<path d="M3 2v7c0 1.1.9 2 2 2h4a2 2 0 0 0 2-2V2M7 2v20M21 15V2a5 5 0 0 0-5 5v6c0 1.1.9 2 2 2h3zm0 0v7"/>"#,
        "hand.wave" => r#"<path d="M18 11V6a2 2 0 0 0-4 0v1M14 10V4a2 2 0 0 0-4 0v2M10 10.5V6a2 2 0 0 0-4 0v8"/><path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.9-6-2.4l-3.6-3.6a2 2 0 0 1 2.8-2.8L7 15"/>"#,
        "basket" => r#"<path d="M15 11l-1 9M19 11l-4-7M2 11h20M3.5 11l1.6 7.4a2 2 0 0 0 2 1.6h9.8a2 2 0 0 0 2-1.6l1.7-7.4M4.5 15.5h15M5 11l4-7M9 11l1 9"/>"#,
        "tram" => r#"<rect x="4" y="6" width="16" height="12" rx="2"/><path d="M4 12h16M8 18l-2 4M16 18l2 4M9 2h6M12 2v4M8 15h.01M16 15h.01"/>"#,
        "house" => r#"<path d="M3 10l9-7 9 7v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M9 22V12h6v10"/>"#,
        "person.2" => r#"<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.9M16 3.1a4 4 0 0 1 0 7.8"/>"#,
        "briefcase" => r#"<rect x="2" y="7" width="20" height="14" rx="2"/><path d="M16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16"/>"#,
        "cloud.rain" => r#"<path d="M4 14.9A7 7 0 1 1 15.7 8h1.8a4.5 4.5 0 0 1 2.5 8.2M16 14v6M8 14v6M12 16v6"/>"#,
        "mountain.2" => r#"<path d="M8 3l4 8 5-5 5 15H2z"/>"#,
        "music.note" => r#"<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>"#,
        "film" => r#"<rect x="2" y="2" width="20" height="20" rx="2.2"/><path d="M7 2v20M17 2v20M2 12h20M2 7h5M2 17h5M17 17h5M17 7h5"/>"#,
        "pencil.and.outline" => r#"<path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>"#,
        "globe.europe.africa" => r#"<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15 15 0 0 1 4 10 15 15 0 0 1-4 10 15 15 0 0 1-4-10 15 15 0 0 1 4-10z"/>"#,
        "wineglass" => r#"<path d="M8 22h8M7 10h10M12 15v7M12 15a5 5 0 0 0 5-5c0-2-.5-4-2-8H9c-1.5 4-2 6-2 8a5 5 0 0 0 5 5z"/>"#,
        "building.2" => r#"<path d="M6 22V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v18zM6 12H4a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2M18 9h2a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-2M10 6h4M10 10h4M10 14h4M10 18h4"/>"#,
        "flag" => r#"<path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1zM4 22v-7"/>"#,
        "quote.bubble" => r#"<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M8 9h2v2l-1 2M13 9h2v2l-1 2"/>"#,
        "paperplane" => r#"<path d="M22 2L11 13M22 2l-7 20-4-9-9-4z"/>"#,
        "newspaper" => r#"<path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-4 0v-9c0-1.1.9-2 2-2h2"/><path d="M18 14h-8M15 18h-5M10 6h8v4h-8z"/>"#,
        "figure.walk" => r#"<circle cx="13" cy="4" r="2"/><path d="M7 22l3-6 3 2v4M10 16l1-6 4 3h3M11 10l-3 1-2 3"/>"#,
        _ => r#"<circle cx="12" cy="12" r="9"/>"#,
    }
}

#[component]
pub fn Icon(name: String, #[props(default = 20)] size: u32, #[props(default = 1.6)] weight: f64) -> Element {
    let svg = format!(
        r#"<svg width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="{weight}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{}</svg>"#,
        paths(&name)
    );
    rsx! { span { class: "icon", dangerous_inner_html: svg } }
}

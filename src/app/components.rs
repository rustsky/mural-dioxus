use dioxus::prelude::*;

use super::icons::Icon;
use super::state::Mural;
use crate::engine::models::SourceLink;
use crate::engine::pinyin;
use crate::engine::time::{new_id, Date};

pub fn open_external(url: &str) {
    let safe = SourceLink { title: String::new(), url: url.into() }.safe_url();
    if let Some(url) = safe { let _ = open::that_detached(url); }
}

#[component]
pub fn Brand() -> Element {
    rsx! {
        div { class: "brand", aria_label: "Mural",
            span { class: "brand-dot" }
            "mural"
        }
    }
}

#[component]
pub fn Orb(#[props(default = 0.0)] energy: f64, #[props(default = false)] listening: bool, #[props(default = true)] active: bool) -> Element {
    let id = use_hook(|| new_id().replace('-', ""));
    let svg = use_hook(|| format!(r##"<svg viewBox="-12 -12 124 124" aria-hidden="true">
<defs>
  <radialGradient id="o{id}" class="orb-hot" gradientUnits="userSpaceOnUse" cx="50" cy="50" r="62">
    <stop offset="0" stop-color="#ffeac8"/><stop offset="0.28" stop-color="#ffb46b"/>
    <stop offset="0.62" stop-color="#ff8a4d"/><stop offset="1" stop-color="#f56b59"/>
  </radialGradient>
  <linearGradient id="l{id}" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#fff7d1" stop-opacity="0.95"/><stop offset="0.45" stop-color="#fff1c7" stop-opacity="0"/>
    <stop offset="0.7" stop-color="#cdaded" stop-opacity="0"/><stop offset="1" stop-color="#dcbff2" stop-opacity="0.95"/>
  </linearGradient>
  <filter id="b{id}" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="4"/></filter>
  <filter id="s{id}" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="3"/></filter>
  <clipPath id="c{id}"><path class="orb-clip-path" d="M3 50a47 47 0 1 0 94 0a47 47 0 1 0 -94 0"/></clipPath>
  <radialGradient id="d{id}" cx="0.3" cy="0.3" r="0.8"><stop offset="0" stop-color="#fff"/><stop offset="0.5" stop-color="#ffe3cf"/><stop offset="1" stop-color="#ff8a4d" stop-opacity="0.5"/></radialGradient>
</defs>
<ellipse cx="50" cy="98" rx="28" ry="3.7" fill="#ff8a4d" fill-opacity="0.14" filter="url(#s{id})"/>
<circle class="orb-ring ring1" cx="50" cy="50" r="52" fill="none" stroke="#ff8a4d" stroke-width="0.5"/>
<circle class="orb-ring ring2" cx="50" cy="50" r="57" fill="none" stroke="#ff8a4d" stroke-width="0.5"/>
<g class="orb-body" filter="drop-shadow(0 5px 8px rgba(255,138,77,0.12))">
  <g clip-path="url(#c{id})">
    <rect x="-10" y="-10" width="120" height="120" fill="url(#o{id})"/>
    <rect x="-10" y="-10" width="120" height="120" fill="url(#l{id})"/>
    <ellipse cx="0" cy="0" rx="24" ry="7.5" fill="#fff" fill-opacity="0.65" filter="url(#b{id})" transform="translate(33 22) rotate(-28)"/>
    <ellipse cx="0" cy="0" rx="60" ry="25" fill="none" stroke="#fff1c7" stroke-opacity="0.48" stroke-width="8" filter="url(#b{id})" transform="translate(50 104) rotate(-15)"/>
  </g>
</g>
<circle cx="105" cy="26" r="3" fill="url(#d{id})"/>
<circle cx="-4" cy="63" r="1.8" fill="#ffe3cf"/>
</svg>"##));
    let ring = if listening { "listening" } else { "" };
    rsx! {
        div {
            class: "mural-orb {ring}",
            "data-energy": "{energy:.3}",
            "data-active": "{active}",
            dangerous_inner_html: svg,
        }
    }
}

#[component]
pub fn RecallBars(count: usize) -> Element {
    rsx! {
        div { class: "bars", aria_label: "{count} of 3 recall bars",
            for i in 0..3usize {
                span { key: "{i}", class: if i < count { "bar on" } else { "bar" } }
            }
        }
    }
}

#[component]
pub fn PageHeading(eyebrow: String, title: String, #[props(default)] subtitle: String) -> Element {
    rsx! {
        div { class: "heading",
            div { class: "eyebrow", "{eyebrow}" }
            h1 { "{title}" }
            if !subtitle.is_empty() { p { "{subtitle}" } }
        }
    }
}

/// Keeps Han text selectable, with an optional reading below it.
#[component]
pub fn PinyinHelp(text: String) -> Element {
    let mut expanded = use_signal(|| true);
    let reading = use_memo(use_reactive!(|text| pinyin::reading(&text)));
    let Some(reading) = reading() else { return rsx! {} };
    rsx! {
        div { class: "pinyin",
            button { onclick: move |_| expanded.toggle(),
                if expanded() { "Hide pinyin" } else { "Show pinyin" }
                Icon { name: if expanded() { "chevron.up" } else { "chevron.down" }, size: 12 }
            }
            if expanded() { div { class: "reading selectable", "{reading}" } }
        }
    }
}

#[component]
pub fn SheetFrame(
    #[props(default)] title: String,
    #[props(default)] leading: Option<(String, EventHandler<()>)>,
    #[props(default)] trailing: Option<(String, EventHandler<()>)>,
    #[props(default = false)] wide: bool,
    children: Element,
) -> Element {
    let app = use_context::<Mural>();
    let dismissable = leading.is_none() && trailing.is_none();
    rsx! {
        div { class: "overlay",
            onclick: move |_| if dismissable { app.close_sheet() },
            div { class: if wide { "sheet wide" } else { "sheet" },
                role: "dialog",
                onclick: move |e| e.stop_propagation(),
                div { class: "sheet-bar",
                    div { class: "left",
                        if let Some((label, action)) = leading.clone() {
                            button { class: "bar-button plain", onclick: move |_| action.call(()), "{label}" }
                        }
                    }
                    div { class: "title", "{title}" }
                    div { class: "right",
                        if let Some((label, action)) = trailing.clone() {
                            button { class: "bar-button", onclick: move |_| action.call(()), "{label}" }
                        } else if dismissable {
                            button { class: "bar-button", onclick: move |_| app.close_sheet(), "Done" }
                        }
                    }
                }
                div { class: "sheet-body", {children} }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct DialogButton { pub label: String, pub role: &'static str, pub action: EventHandler<()> }

#[component]
pub fn Dialog(title: String, #[props(default)] message: String, buttons: Vec<DialogButton>, on_cancel: EventHandler<()>, #[props(default = "Cancel".to_string())] cancel_label: String) -> Element {
    let buttons_empty = buttons.is_empty();
    rsx! {
        div { class: "overlay", style: "z-index: 40", onclick: move |_| on_cancel.call(()),
            div { class: "dialog", role: "alertdialog", onclick: move |e| e.stop_propagation(),
                h3 { "{title}" }
                if !message.is_empty() { p { "{message}" } }
                div { class: "buttons",
                    for (i, b) in buttons.into_iter().enumerate() {
                        button { key: "{i}", class: b.role, onclick: move |_| b.action.call(()), "{b.label}" }
                    }
                    button { class: if buttons_empty { "primary" } else { "" }, autofocus: true, onclick: move |_| on_cancel.call(()), "{cancel_label}" }
                }
            }
        }
    }
}

enum Inline { Text(String), Link(String, String), Bold(String) }

fn inline(text: &str) -> Vec<Inline> {
    let mut out = vec![];
    let mut rest = text;
    while !rest.is_empty() {
        let link = rest.find('[');
        let bold = rest.find("**");
        match (link, bold) {
            (Some(l), b) if b.map(|b| l < b).unwrap_or(true) => {
                if let Some((label, url, len)) = parse_link(&rest[l..]) {
                    if l > 0 { out.push(Inline::Text(rest[..l].into())); }
                    out.push(Inline::Link(label, url));
                    rest = &rest[l + len..];
                } else {
                    out.push(Inline::Text(rest[..=l].into()));
                    rest = &rest[l + 1..];
                }
            }
            (_, Some(b)) => {
                if let Some(end) = rest[b + 2..].find("**") {
                    if b > 0 { out.push(Inline::Text(rest[..b].into())); }
                    out.push(Inline::Bold(rest[b + 2..b + 2 + end].into()));
                    rest = &rest[b + 4 + end..];
                } else {
                    out.push(Inline::Text(rest.into()));
                    rest = "";
                }
            }
            _ => { out.push(Inline::Text(rest.into())); rest = ""; }
        }
    }
    out
}

fn parse_link(s: &str) -> Option<(String, String, usize)> {
    let close = s.find("](")?;
    let end = s[close + 2..].find(')')?;
    let label = s[1..close].to_string();
    let url = s[close + 2..close + 2 + end].to_string();
    if label.contains('[') || url.contains(char::is_whitespace) { return None; }
    Some((label, url, close + 3 + end))
}

/// Inline Markdown as SwiftUI renders it: paragraphs, bold and links.
#[component]
pub fn Markdown(text: String) -> Element {
    let paragraphs: Vec<Vec<Inline>> = text.split("\n\n").filter(|p| !p.trim().is_empty()).map(|p| inline(p.trim())).collect();
    rsx! {
        div { class: "markdown selectable",
            for (i, parts) in paragraphs.into_iter().enumerate() {
                p { key: "{i}",
                    for (j, part) in parts.into_iter().enumerate() {
                        match part {
                            Inline::Text(t) => rsx! { span { key: "{j}", "{t}" } },
                            Inline::Bold(t) => rsx! { strong { key: "{j}", "{t}" } },
                            Inline::Link(label, url) => rsx! {
                                a { key: "{j}", href: "#", class: "linkish", onclick: move |e: Event<MouseData>| { e.prevent_default(); open_external(&url); }, "{label}" }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn SourcesView(sources: Vec<SourceLink>, date: Date) -> Element {
    rsx! {
        div { class: "sources",
            div { class: "caption secondary", "Sources · {date.abbreviated()}" }
            for (i, source) in sources.into_iter().enumerate() {
                if let Some(url) = source.safe_url() {
                    a { key: "{i}", href: "#", onclick: move |e: Event<MouseData>| { e.prevent_default(); open_external(&url); },
                        Icon { name: "arrow.up.right", size: 14 }
                        "{source.title}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn ExternalLink(label: String, url: String, #[props(default)] class: String) -> Element {
    rsx! {
        a { class: "{class}", href: "#", onclick: move |e: Event<MouseData>| { e.prevent_default(); open_external(&url); }, "{label}" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_markdown_finds_links_and_bold() {
        let parts = inline("See **this** and [NRK](https://www.nrk.no/) now [not a link");
        let kinds: Vec<&str> = parts.iter().map(|p| match p { Inline::Text(_) => "t", Inline::Bold(_) => "b", Inline::Link(..) => "l" }).collect();
        assert_eq!(kinds, ["t", "b", "t", "l", "t", "t"]);
        assert!(matches!(&parts[3], Inline::Link(l, u) if l == "NRK" && u == "https://www.nrk.no/"));
    }
}

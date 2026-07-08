//! Share: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn SharePage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Beyond the window",
            title: "Share",
            lead: "The app boundary in both directions. ExternalDragSource writes real DataTransfer formats that other tabs and applications understand; ExternalDropZone classifies whatever the outside world drags in. This is the only place your content meets the native drag protocol.",
        }
        ShareDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Outbound: write every format that fits.",
                        "OutboundContent describes what leaves. A url is written as text/uri-list, text/plain, and (when titled) a text/html anchor, so URL bars, editors and rich targets each find a representation they like. Text, HTML with a plain fallback, and raw custom pairs cover the rest.",
                    ),
                    (
                        "Inbound: classify what arrives.",
                        "ExternalDropZone decodes the drop into ExternalPayload values, most specific first: urls parsed out of uri-lists, HTML, plain text, plus any files. The url(), text() and best() helpers pull out what you want in one call.",
                    ),
                    (
                        "Treat inbound as hostile.",
                        "External HTML and URLs are attacker-controlled. Sanitize markup before rendering it and check URL schemes before navigating. Outbound is guarded for you: generated anchors escape their content and drop the href entirely for javascript:, data: and vbscript: schemes.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Neither component needs a provider: the browser is the transport here, not the shared context. Try dragging the card into your URL bar, a text editor, or another browser entirely."
                }
            }
            DioxusNote {
                p {
                    "This is deliberately the opposite of the in-app pages: payloads are strings in standard formats because the receiving application is not yours. Everywhere inside your app, skip the serialization and let typed Rust values travel through context instead."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "ExternalDragSource props",
                rows: vec![
                    ("content", "OutboundContent, required", "What to place on the outbound DataTransfer."),
                    ("effect", "DropEffect = Copy", "Advertised to the receiving application; Copy is what drag-out almost always means."),
                    ("disabled", "bool = false", "Turn the source off without unmounting."),
                ],
            }
            PropsTable {
                title: "OutboundContent variants",
                rows: vec![
                    ("Text(String)", "", "Plain text."),
                    ("Url or url(url, title)", "", "A link, written in three formats; the constructor takes an optional human title for the HTML anchor."),
                    ("Html", "html, fallback_text", "Rich content plus the plain-text version for targets that don't take HTML."),
                    ("Custom(Vec<(format, data)>)", "", "Raw pairs written verbatim, in order, for proprietary formats."),
                ],
            }
            PropsTable {
                title: "ExternalDropZone and ExternalDrop",
                rows: vec![
                    ("on_drop", "EventHandler<ExternalDrop>, required", "Fired with the classified payloads and files; empty drops are swallowed."),
                    ("on_hover", "EventHandler<bool>", "Enter/leave signal beyond the data-over attribute."),
                    ("ExternalDrop", "payloads, files, client, element", "All representations offered (most specific first), any files, and where the drop landed."),
                    ("url() / text() / best()", "-> Option", "Convenience accessors for the first URL, first plain text, or the most specific payload."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Boundary components stay native:",
                        "files, outside content and drag-out require DataTransfer, which pointer events cannot reach. Everything inside your app should use the typed context instead.",
                    ),
                    (
                        "Typed payloads across windows:",
                        "the serde feature adds external::typed::store and retrieve for JSON payloads between two of your own windows, wire-compatible with dioxus-html's own helpers.",
                    ),
                    (
                        "classify is public",
                        "when you need the same DataTransfer decoding in a custom handler.",
                    ),
                    (
                        "Dangerous schemes lose their href:",
                        "an outbound javascript: url still ships as plain text, but the generated HTML anchor refuses to make it clickable.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"ExternalDragSource {
    content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
    "Drag this link to another tab"
}
ExternalDropZone {
    on_drop: move |d: ExternalDrop| {
        for p in d.payloads { /* ExternalPayload::Url, Text, Html */ }
    },
    "Drop a link or text here"
}"#;

// --- 14. share (drag out / external in, native) ------------------------------

#[component]
fn ShareDemo() -> Element {
    let mut dropped = use_signal(String::new);
    rsx! {
        Section {
            title: "Share",
            note: "Drag the link out to another tab or app, or drop a link or text back in. (Native drag across the app boundary.)",
            tag: "ExternalDragSource",
            div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                ExternalDragSource {
                    content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
                    class: "flex cursor-grab items-center justify-between gap-3 rounded-xl bg-gradient-to-b from-[#FBFAF6] to-[#F6F3EC] px-3.5 py-4 text-[13px] text-[#2C2A25] shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.05),0_1px_2px_rgba(26,24,21,0.10),0_4px_12px_-4px_rgba(26,24,21,0.08)] transition hover:-translate-y-px hover:brightness-[1.06]",
                    div { class: "min-w-0",
                        div { class: "font-medium text-[#1A1815]", "Dioxus" }
                        div { class: "truncate text-[11px] text-[#7A776C]", "dioxuslabs.com" }
                    }
                    span { class: "text-[#B88B2F]", "↗" }
                }
                ExternalDropZone {
                    on_drop: move |d: ExternalDrop| {
                        dropped
                            .set(format!("{} payload(s), {} file(s)", d.payloads.len(), d.files.len()));
                    },
                    class: "flex min-h-24 items-center justify-center rounded-xl border-2 border-dashed border-[#7A776C]/30 p-3 text-center text-sm text-[#7A776C] transition data-over:border-[#1C4A38] data-over:bg-[#1C4A38]/15 data-over:text-[#45423B]",
                    if dropped.read().is_empty() {
                        "Drop a link or text here"
                    } else {
                        "{dropped}"
                    }
                }
            }
        }
    }
}

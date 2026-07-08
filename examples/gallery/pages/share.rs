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
            lead: "The app boundary, both directions: ExternalDragSource writes real DataTransfer formats other applications understand, and ExternalDropZone classifies whatever arrives from outside.",
        }
        ShareDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "OutboundContent covers text, links (written as text/uri-list plus text/plain plus text/html so every receiver finds a format it likes), rich HTML with a plain-text fallback, and raw custom format pairs."
                }
                p {
                    "Inbound, ExternalDrop hands you classified payloads (urls, text, html) plus any files; the classify helper is public if you want the same logic elsewhere."
                }
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Boundary components stay native:",
                        "this is DataTransfer territory, where pointer events cannot reach.",
                    ),
                    (
                        "Outbound defaults to Copy,",
                        "which is what dragging out of an app almost always means.",
                    ),
                    (
                        "Typed payloads across windows:",
                        "the serde feature adds external::typed::store and retrieve, wire-compatible with dioxus-html's own.",
                    ),
                    (
                        "Firefox quirk handled:",
                        "drags always set data so the gesture actually starts.",
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
                    class: "flex cursor-grab items-center justify-between gap-3 rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] px-3.5 py-4 text-[13px] text-[#d9cfbc] shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.03),0_1px_2px_rgba(0,0,0,0.5),0_4px_12px_-4px_rgba(0,0,0,0.4)] transition hover:-translate-y-px hover:brightness-[1.06]",
                    div { class: "min-w-0",
                        div { class: "font-medium text-[#f4e9d7]", "Dioxus" }
                        div { class: "truncate text-[11px] text-[#9c8f77]", "dioxuslabs.com" }
                    }
                    span { class: "text-[#B8A98C]", "↗" }
                }
                ExternalDropZone {
                    on_drop: move |d: ExternalDrop| {
                        dropped
                            .set(format!("{} payload(s), {} file(s)", d.payloads.len(), d.files.len()));
                    },
                    class: "flex min-h-24 items-center justify-center rounded-xl border-2 border-dashed border-white/15 p-3 text-center text-sm text-[#9c8f77] transition data-over:border-[#D97D55] data-over:bg-[#D97D55]/15 data-over:text-[#b8ab93]",
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

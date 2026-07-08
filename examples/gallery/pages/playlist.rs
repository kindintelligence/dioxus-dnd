//! Playlist: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn PlaylistPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Reorder",
            title: "Playlist",
            lead: "SortableList owns the whole reorder gesture: grab a row and the others slide out of the way in real time, showing exactly the order you would get if you released.",
        }
        PlaylistDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "You give it three things: len, a render callback for the row at an index, and on_sort receiving a SortEvent carrying from and to when a drop commits. apply_sort applies that event to a Vec. The list renders its own row wrappers; those wrappers carry data-dragging and data-drop-target, so you style them from the root class with child selectors."
                }
                p {
                    "The live preview translates rows by the measured slot pitch, which includes your margins and gaps, so spacing never squashes mid-drag. Set live_preview: false for plain highlight-only behavior."
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
                        "Pointer events by default:",
                        "mouse, touch and pen all avoid the browser's native drag image; native DataTransfer is reserved for app-boundary APIs.",
                    ),
                    (
                        "axis: Axis::Horizontal",
                        "turns the same component into a horizontal strip; the preview shifts along X instead of Y.",
                    ),
                    (
                        "The overlay prop",
                        "renders a floating ghost pinned to the pointer while the in-flow row becomes the gap, when you want the dnd-kit look.",
                    ),
                    (
                        "Rows are measured at drag start,",
                        "so hit-testing stays honest after scrolling; keep row geometry stable during the drag.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"SortableList {
    len: items.read().len(),
    on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
    class: "rounded-xl bg-white/[0.03]
            [&>[data-dragging]]:shadow-xl
            [&>[data-drop-target]]:bg-white/5",
    render: move |ix: usize| rsx! {
        TrackRow { track: items.read()[ix].clone() }
    },
}"#;

// --- 4. playlist (sortable list) ---------------------------------------------

#[derive(Clone, PartialEq)]
struct Track {
    title: &'static str,
    artist: &'static str,
    dur: &'static str,
}

#[component]
fn PlaylistDemo() -> Element {
    let mut items = use_signal(|| {
        vec![
            Track {
                title: "Nightcall",
                artist: "Kavinsky",
                dur: "4:18",
            },
            Track {
                title: "Redbone",
                artist: "Childish Gambino",
                dur: "5:27",
            },
            Track {
                title: "Midnight City",
                artist: "M83",
                dur: "4:03",
            },
            Track {
                title: "Teardrop",
                artist: "Massive Attack",
                dur: "5:29",
            },
            Track {
                title: "Weird Fishes",
                artist: "Radiohead",
                dur: "5:18",
            },
        ]
    });
    rsx! {
        Section {
            title: "Playlist",
            note: "Reorder tonight's set. Grab a track and the others slide to make room; drop it and the ghost settles into its slot.",
            tag: "SortableList",
            SortableList {
                len: items.read().len(),
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                // No floating overlay: the grabbed row lifts in place and the
                // others slide to make room. No ghost, no ring.
                // The mailbox's list language: one contained well, hairline
                // dividers, and the grabbed row lifts out of it as a card.
                class: "relative rounded-xl bg-white/[0.03] ring-1 ring-white/5 [&>*]:flex [&>*]:items-center [&>*]:gap-3 [&>*]:px-3.5 [&>*]:py-2.5 [&>*]:text-[13px] [&>*]:cursor-grab [&>*]:select-none [&>*]:transition [&>*+*]:border-t [&>*+*]:border-white/5 [&>*:first-child]:rounded-t-xl [&>*:last-child]:rounded-b-xl [&>*:hover]:bg-white/[0.04] [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:rounded-lg [&>[data-dragging]]:bg-[#3d352a] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.04),0_16px_34px_-12px_rgba(0,0,0,0.65)]",
                render: move |ix: usize| {
                    let t = items.read()[ix].clone();
                    rsx! {
                        span { class: "w-4 shrink-0 text-center text-[11px] font-semibold tabular-nums text-[#6d6150]",
                            "{ix + 1}"
                        }
                        div { class: "min-w-0 flex-1",
                            div { class: "truncate font-medium text-[#f4e9d7]", "{t.title}" }
                            div { class: "truncate text-[11px] text-[#9c8f77]", "{t.artist}" }
                        }
                        span { class: "shrink-0 text-[11px] tabular-nums text-[#9c8f77]", "{t.dur}" }
                    }
                },
            }
        }
    }
}

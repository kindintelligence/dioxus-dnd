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
            lead: "SortableList is self-contained reordering: no provider, no payload type, no zone ids. Hand it a length and a way to render row N; when the user drops, it hands you back \"move index 3 to index 0\" and you apply it.",
        }
        PlaylistDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Index in, index out.",
                        "The list never sees your data. len says how many rows there are, render draws the row at a given index, and on_sort receives a SortEvent carrying from and to when a drop commits. apply_sort applies that event to any Vec in one call.",
                    ),
                    (
                        "Rows slide in real time.",
                        "While you drag, every row translates toward the order you would get on release, so the gap under the pointer is exactly where the row will land. The slide distance is the measured slot pitch, margins and gaps included, so spacing never squashes mid-drag.",
                    ),
                    (
                        "You style the wrappers.",
                        "The list renders one wrapper div per row. The grabbed one carries data-dragging and the row it would displace carries data-drop-target; style both from the root class with child selectors, as the snippet does.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "That is the entire integration: the component owns the gesture, the preview and the hit-testing, and your model changes in exactly one place. Because rows are addressed by index, the same list works for strings, structs, or anything else you keep in a Vec."
                }
            }
            DioxusNote {
                p {
                    "render is a callback from row index to rsx: a move closure taking ix and returning markup. It reads items.read()[ix], and because the closure reads the signal, applying a SortEvent to the Vec re-renders the rows in their new order automatically."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "SortableList props",
                rows: vec![
                    ("len", "usize, required", "Number of rows."),
                    ("render", "Callback<usize, Element>, required", "Draws the row at the given index."),
                    ("on_sort", "EventHandler<SortEvent>, required", "Fired when a drop commits a new position."),
                    ("axis", "Axis = Vertical", "Layout direction. Horizontal turns the same component into a strip that previews along X."),
                    ("live_preview", "bool = true", "Rows slide to preview the final order. Set false for highlight-only feedback."),
                    ("transition_ms", "u32 = 160", "Duration of the row-slide transition."),
                    ("overlay", "Callback<usize, Element>", "Opt-in floating ghost pinned to the pointer; the in-flow row hides and becomes the gap (the dnd-kit look)."),
                    ("touch_handle", "bool = false", "Confine touch drags to a leading grip so rows still finger-scroll. See the Podcast queue page."),
                ],
            }
            PropsTable {
                title: "The event and its helpers",
                rows: vec![
                    ("SortEvent", "from: usize, to: usize", "\"Move the item at from so it ends up at index to.\" The same event drives drags, reorder buttons, and grids."),
                    ("apply_sort(&mut vec, ev)", "", "Remove-and-insert: the standard list reorder. Ignores out-of-range and no-op events."),
                    ("apply_swap(&mut vec, ev)", "", "Exchange the two positions instead, for fixed-slot layouts."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "No provider needed.",
                        "SortableList manages its own drag state internally; drop it into any page as-is.",
                    ),
                    (
                        "Pointer events by default:",
                        "mouse, touch and pen share one gesture, and the browser never paints its own drag image.",
                    ),
                    (
                        "Rows are measured at drag start,",
                        "so hit-testing stays honest after scrolling; keep row geometry stable during the drag.",
                    ),
                    (
                        "A release outside the list cancels",
                        "rather than committing, so dropping a row \"nowhere\" never reorders anything.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"SortableList {
    len: items.read().len(),
    on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
    class: "rounded-xl bg-[#EEEADF]
            [&>[data-dragging]]:shadow-xl
            [&>[data-drop-target]]:bg-[#E1DDCE]/50",
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
                class: "relative rounded-xl bg-[#EEEADF] ring-1 ring-[#E8E5D9] [&>*]:flex [&>*]:items-center [&>*]:gap-3 [&>*]:px-3.5 [&>*]:py-2.5 [&>*]:text-[13px] [&>*]:cursor-grab [&>*]:select-none [&>*]:transition [&>*+*]:border-t [&>*+*]:border-[#E8E5D9] [&>*:first-child]:rounded-t-xl [&>*:last-child]:rounded-b-xl [&>*:hover]:bg-[#E1DDCE]/50 [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:rounded-lg [&>[data-dragging]]:bg-[#FBFAF6] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.06),0_16px_34px_-12px_rgba(26,24,21,0.14)]",
                render: move |ix: usize| {
                    let t = items.read()[ix].clone();
                    rsx! {
                        span { class: "w-4 shrink-0 text-center text-[11px] font-semibold tabular-nums text-[#1C4A38]",
                            "{ix + 1}"
                        }
                        div { class: "min-w-0 flex-1",
                            div { class: "truncate font-medium text-[#1A1815]", "{t.title}" }
                            div { class: "truncate text-[11px] text-[#7A776C]", "{t.artist}" }
                        }
                        span { class: "shrink-0 text-[11px] tabular-nums text-[#7A776C]", "{t.dur}" }
                    }
                },
            }
        }
    }
}

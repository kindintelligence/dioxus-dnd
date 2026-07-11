//! Podcast queue: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn PodcastQueuePage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Reorder",
            title: "Podcast queue",
            lead: "Long lists have two problems at once: a drag can't reach rows that are scrolled out of view, and a touch drag surface stops the list from finger-scrolling at all. AutoScroll solves the first, touch_handle the second.",
        }
        QueueDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Wrap the scroller.",
                        "AutoScroll renders a div; you give it the overflow CSS. While a drag hovers within threshold pixels of an edge (48 by default), the container scrolls itself by up to speed pixels per event (24 by default), ramped by how deep into the edge band the pointer sits.",
                    ),
                    (
                        "It scrolls for both worlds.",
                        "In-app pointer drags and native boundary drags (an OS file hovering on the way to a drop zone) both drive it. Held-button state gates the pointer path, so passively mousing around never scrolls anything.",
                    ),
                    (
                        "Touch drags move to a grip.",
                        "A touch drag surface must set touch-action: none, which would stop the browser from finger-scrolling the rows. touch_handle: true claims only a small leading grip for the drag gesture; the rest of each row scrolls normally under a finger.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "The two features compose but don't require each other: AutoScroll accepts any children (a board, a tree, a grid), and touch_handle is useful in any scrollable list even without auto-scroll."
                }
            }
            DioxusNote {
                p {
                    "The grip is styled through a data attribute selector rather than a prop: the class string targets descendants with data-sort-handle. Attribute selectors are how headless components stay styleable without inventing a styling API."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "AutoScroll props",
                rows: vec![
                    ("threshold", "f64 = 48.0", "Size of the edge band in pixels. Enter it and scrolling begins."),
                    ("speed", "f64 = 24.0", "Maximum scroll per event, reached at the very edge."),
                    ("axis", "ScrollAxis = Y", "Y for lists, X for strips, Both for 2D panes."),
                    ("active", "Option<bool>", "External gate: Some(true) forces scrolling on pointer movement, Some(false) suppresses it, None uses the built-in contact heuristic."),
                ],
            }
            PropsTable {
                title: "SortableList touch props",
                rows: vec![
                    ("touch_handle", "bool = false", "Confine drags to a leading grip so rows keep finger-scrolling. The grip carries data-sort-handle."),
                    ("handle", "Callback<usize, Element>", "Replace the default braille-dots glyph with your own grip content, per row."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "No long-press timers, ever:",
                        "a movement threshold plus an explicit handle is predictable and works identically for pens.",
                    ),
                    (
                        "edge_delta is public and pure,",
                        "so you can unit-test scroll ramps or drive a custom scroller with the same math.",
                    ),
                    (
                        "Pure MountedData:",
                        "no JavaScript eval; the same code works in web and desktop webviews.",
                    ),
                    (
                        "The pointer must stay inside the container",
                        "to scroll it; wandering off the edge stops the scroll rather than pinning it at full speed.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"AutoScroll {
    class: "max-h-52 overflow-y-auto rounded-xl",
    SortableList {
        len: rows.read().len(),
        touch_handle: true,
        on_sort: move |ev: SortEvent| apply_sort(&mut rows.write(), ev),
        class: "[&_[data-sort-handle]]:w-6
                [&_[data-sort-handle]]:cursor-grab",
        render: move |ix: usize| rsx! { "{rows.read()[ix]}" },
    }
}"#;

// --- 7. podcast queue (auto-scrolling container) -----------------------------

#[component]
fn QueueDemo() -> Element {
    let mut rows = use_signal(|| {
        const T: [&str; 8] = [
            "The long game",
            "Small teams",
            "Design debt",
            "Shipping fast",
            "The art of saying no",
            "On craft",
            "Growth loops",
            "Rest as strategy",
        ];
        (1..=18)
            .map(|n| format!("Ep {n:02}  ·  {}", T[(n - 1) as usize % T.len()]))
            .collect::<Vec<_>>()
    });
    rsx! {
        Section {
            title: "Podcast queue",
            note: "A queue longer than the window. Drag toward the top or bottom edge and it scrolls itself. On a phone the dotted grip does the dragging, so a finger on the rows still scrolls the list.",
            tag: "AutoScroll",
            // The scroll container *is* the well: flat rows, hairline
            // dividers, and the grabbed row lifts out of the surface.
            AutoScroll { class: "max-h-52 overflow-y-auto rounded-xl bg-[#EEEADF] ring-1 ring-[#E8E5D9]",
                SortableList {
                    len: rows.read().len(),
                    // Inside a scroll container, claim only the grip for touch
                    // drags - the rows themselves keep scrolling by finger.
                    touch_handle: true,
                    on_sort: move |ev: SortEvent| apply_sort(&mut rows.write(), ev),
                    class: "[&>*]:px-1.5 [&>*]:transition [&>*+*]:border-t [&>*+*]:border-[#E8E5D9] [&>*:hover]:bg-[#E1DDCE]/50 [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:bg-[#FBFAF6] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.06),0_12px_26px_-10px_rgba(26,24,21,0.14)] [&_[data-sort-handle]]:w-6 [&_[data-sort-handle]]:shrink-0 [&_[data-sort-handle]]:cursor-grab [&_[data-sort-handle]]:text-[13px] [&_[data-sort-handle]]:text-[#BBB8AE] [&_[data-sort-handle]]:transition [&_[data-sort-handle]:hover]:text-[#1C4A38]",
                    render: move |ix: usize| rsx! {
                        div {
                            class: "cursor-grab select-none rounded-md px-2 py-2.5 text-[13px] text-[#2C2A25] transition",
                            "{rows.read()[ix]}"
                        }
                    },
                }
            }
        }
    }
}

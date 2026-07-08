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
            lead: "Two long-list problems, solved together: AutoScroll drives the container when a drag nears its edge, and touch_handle confines touch drags to a grip so the rows themselves keep scrolling under a finger.",
        }
        QueueDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "Wrap any scrollable container in AutoScroll and drags hovering within threshold pixels of an edge (default 48) scroll it by up to speed pixels per event (default 24), ramped by proximity. It works for in-app pointer drags and native boundary drags (OS files, external content) alike, using held-button state so passive mouse hover never scrolls."
                }
                p {
                    "A touch drag surface must set touch-action: none, which would stop the browser from scrolling the list by finger. touch_handle: true claims only a leading grip for the gesture; style it via the data-sort-handle attribute."
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
                        "No long-press timers, ever:",
                        "a movement threshold plus an explicit handle is predictable and works the same for pens.",
                    ),
                    (
                        "active: Some(false)",
                        "suppresses scrolling when a parent tracks drag state and wants control.",
                    ),
                    (
                        "The handle prop",
                        "replaces the default braille-dots grip with your own rsx, keyed by row index.",
                    ),
                    (
                        "Pure MountedData:",
                        "no JavaScript eval; the same code works in web and desktop webviews.",
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
    // Index of the row that just landed, so it can flash.
    let mut dropped = use_signal(|| None::<usize>);
    rsx! {
        Section {
            title: "Podcast queue",
            note: "A queue longer than the window. Drag toward the top or bottom edge and it scrolls itself; the episode flashes where it lands. On a phone the dotted grip does the dragging, so a finger on the rows still scrolls the list.",
            tag: "AutoScroll",
            // The scroll container *is* the well: flat rows, hairline
            // dividers, and the grabbed row lifts out of the surface.
            AutoScroll { class: "max-h-52 overflow-y-auto rounded-xl bg-white/[0.03] ring-1 ring-white/5",
                SortableList {
                    len: rows.read().len(),
                    // Inside a scroll container, claim only the grip for touch
                    // drags - the rows themselves keep scrolling by finger.
                    touch_handle: true,
                    on_sort: move |ev: SortEvent| {
                        apply_sort(&mut rows.write(), ev);
                        dropped.set(Some(ev.to));
                    },
                    class: "[&>*]:px-1.5 [&>*]:transition [&>*+*]:border-t [&>*+*]:border-white/5 [&>*:hover]:bg-white/[0.04] [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:bg-[#3d352a] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.04),0_12px_26px_-10px_rgba(0,0,0,0.65)] [&_[data-sort-handle]]:w-6 [&_[data-sort-handle]]:shrink-0 [&_[data-sort-handle]]:cursor-grab [&_[data-sort-handle]]:text-[13px] [&_[data-sort-handle]]:text-[#6d6150] [&_[data-sort-handle]]:transition [&_[data-sort-handle]:hover]:text-[#D97D55]",
                    render: move |ix: usize| {
                        let flash = if dropped() == Some(ix) { "drop-flash" } else { "" };
                        rsx! {
                            div {
                                class: "cursor-grab select-none rounded-md px-2 py-2.5 text-[13px] text-[#d9cfbc] transition {flash}",
                                // Reset once the flash finishes so the same row
                                // can flash again on its next drop.
                                onanimationend: move |_| {
                                    if dropped() == Some(ix) {
                                        dropped.set(None);
                                    }
                                },
                                "{rows.read()[ix]}"
                            }
                        }
                    },
                }
            }
        }
    }
}

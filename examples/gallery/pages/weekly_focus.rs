//! Weekly focus: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn WeeklyFocusPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Reorder",
            title: "Weekly focus",
            lead: "The most robust accessibility fallback is no drag at all. ReorderButtons renders plain move-up and move-down buttons that emit the same SortEvent as the drag gesture, so supporting every user costs one extra component, not a second code path.",
        }
        PriorityDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Same event, second input.",
                        "ReorderButtons takes the row's index, the list total, and an optional accessible label. Pressing up emits SortEvent from ix to ix - 1; pressing down, to ix + 1. Your on_sort handler cannot tell (and does not care) whether a drag or a button produced the event.",
                    ),
                    (
                        "Edges handle themselves.",
                        "The first row's up button and the last row's down button render disabled, so an out-of-range event can never be emitted and the affordance honestly reflects what is possible.",
                    ),
                    (
                        "Buttons never fight the drag.",
                        "The wrapper stops pointer propagation, so a tap on a button stays a tap: it will not start the enclosing row's drag gesture or steal its pointer capture.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Both on_sort props point at the same handler. That is the whole point: when the list logic changes, there is no second code path to forget, and testing the buttons tests the drag semantics too."
                }
            }
            DioxusNote {
                p {
                    "Passing the same closure to two components is idiomatic Dioxus: event handlers are cheap handles, and each call site captures the items signal by copy. Cloning items.read()[ix] out of the read guard keeps the borrow short."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "ReorderButtons props",
                rows: vec![
                    ("index", "usize, required", "This row's position."),
                    ("total", "usize, required", "The list length, for edge detection."),
                    ("label", "Option<String>", "Accessible name: buttons announce as \"Move label up / down\". Falls back to \"item N\"."),
                    ("on_sort", "EventHandler<SortEvent>, required", "Receives the same event shape as drag reordering."),
                ],
            }
            PropsTable {
                title: "Styling hooks",
                rows: vec![
                    ("data-reorder", "\"up\" | \"down\"", "On each button, so the two directions can be styled or iconed independently."),
                    ("class / attributes", "forwarded", "Land on the wrapper span; style the buttons themselves with descendant selectors, as this page does."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Keyboard drag still works too.",
                        "Buttons are the fallback, not the ceiling: focusing the row itself and pressing Space starts a full keyboard drag.",
                    ),
                    (
                        "Headless by design:",
                        "the component ships two unstyled buttons and the behavior; the arrows, sizing and hover states here are all page CSS.",
                    ),
                    (
                        "Works anywhere SortEvent works.",
                        "Pair it with SortableGrid or your own list; nothing about it assumes SortableList.",
                    ),
                    (
                        "Announce results with LiveRegion",
                        "when the reorder matters to screen-reader users beyond focus staying put.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"SortableList {
    len: items.read().len(),
    on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
    render: move |ix: usize| rsx! {
        span { "{items.read()[ix]}" }
        ReorderButtons {
            index: ix,
            total: items.read().len(),
            label: items.read()[ix].clone(),
            on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
        }
    },
}"#;

// --- 5. weekly focus (accessible reorder, headless ReorderButtons) -----------

#[component]
fn PriorityDemo() -> Element {
    let mut items = use_signal(|| {
        [
            "Ship the redesign",
            "Reply to investors",
            "1:1 with Sam",
            "Book the venue",
        ]
        .map(String::from)
        .to_vec()
    });
    rsx! {
        Section {
            title: "Weekly focus",
            note: "Rank your week with the mouse or the arrow buttons. Both emit the same reorder event, so keyboard users are covered too.",
            tag: "ReorderButtons",
            SortableList {
                len: items.read().len(),
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                class: "rounded-xl bg-[#EEEADF] ring-1 ring-[#E8E5D9] [&>*]:flex [&>*]:items-center [&>*]:justify-between [&>*]:gap-3 [&>*]:px-3.5 [&>*]:py-2.5 [&>*]:text-[13px] [&>*]:cursor-grab [&>*]:select-none [&>*]:transition [&>*+*]:border-t [&>*+*]:border-[#E8E5D9] [&>*:first-child]:rounded-t-xl [&>*:last-child]:rounded-b-xl [&>*:hover]:bg-[#E1DDCE]/50 [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:rounded-lg [&>[data-dragging]]:bg-[#FBFAF6] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.06),0_16px_34px_-12px_rgba(26,24,21,0.14)]",
                render: move |ix: usize| rsx! {
                    div { class: "flex min-w-0 items-center gap-2.5",
                        span { class: "grid h-6 w-6 shrink-0 place-items-center rounded-md bg-[#E4ECDD] text-[11px] font-semibold tabular-nums text-[#1C4A38] ring-1 ring-[#A6C1B0]",
                            "{ix + 1}"
                        }
                        span { class: "truncate font-medium text-[#1A1815]", "{items.read()[ix]}" }
                    }
                    ReorderButtons {
                        index: ix,
                        total: items.read().len(),
                        label: items.read()[ix].clone(),
                        on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                        class: "flex shrink-0 gap-1 [&_button]:grid [&_button]:h-6 [&_button]:w-6 [&_button]:place-items-center [&_button]:rounded-md [&_button]:bg-[#7A776C]/10 [&_button]:text-[#7A776C] [&_button]:transition [&_button:not(:disabled)]:hover:bg-[#7A776C]/20 [&_button:not(:disabled)]:hover:text-[#1C4A38] [&_button:disabled]:opacity-30",
                    }
                },
            }
        }
    }
}

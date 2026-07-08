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
            lead: "The most robust accessibility fallback is no drag at all: ReorderButtons renders headless move-up and move-down buttons that emit the same SortEvent as the drag path, so one on_sort serves both inputs.",
        }
        PriorityDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "ReorderButtons takes the row's index, the total, and an optional accessible label; it renders two buttons with aria-labels like Move-item-up and Move-item-down, disables them at the list edges, and stops pointer propagation so a tap on the button never starts the row's drag gesture."
                }
                p {
                    "Because both inputs produce identical events, your model code cannot drift: there is no second code path to forget when the list logic changes."
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
                        "Style hooks:",
                        "the buttons carry data-reorder=\"up\" and \"down\", and the wrapper span forwards class and attributes.",
                    ),
                    (
                        "Edge handling is built in:",
                        "index 0 disables up, the last row disables down; you never emit an out-of-range event.",
                    ),
                    (
                        "Keyboard drag still works too.",
                        "Buttons are the fallback; focusing the row itself and pressing Space starts a full keyboard drag.",
                    ),
                    (
                        "Screen readers get real names",
                        "when you pass label; otherwise buttons fall back to \"item N\".",
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
                class: "rounded-xl bg-white/[0.03] ring-1 ring-white/5 [&>*]:flex [&>*]:items-center [&>*]:justify-between [&>*]:gap-3 [&>*]:px-3.5 [&>*]:py-2.5 [&>*]:text-[13px] [&>*]:cursor-grab [&>*]:select-none [&>*]:transition [&>*+*]:border-t [&>*+*]:border-white/5 [&>*:first-child]:rounded-t-xl [&>*:last-child]:rounded-b-xl [&>*:hover]:bg-white/[0.04] [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:rounded-lg [&>[data-dragging]]:bg-[#3d352a] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.04),0_16px_34px_-12px_rgba(0,0,0,0.65)]",
                render: move |ix: usize| rsx! {
                    div { class: "flex min-w-0 items-center gap-2.5",
                        span { class: "grid h-6 w-6 shrink-0 place-items-center rounded-md bg-[#D97D55] text-[11px] font-semibold tabular-nums text-white",
                            "{ix + 1}"
                        }
                        span { class: "truncate font-medium text-[#f4e9d7]", "{items.read()[ix]}" }
                    }
                    ReorderButtons {
                        index: ix,
                        total: items.read().len(),
                        label: items.read()[ix].clone(),
                        on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                        class: "flex shrink-0 gap-1 [&_button]:grid [&_button]:h-6 [&_button]:w-6 [&_button]:place-items-center [&_button]:rounded-md [&_button]:bg-white/8 [&_button]:text-[#9c8f77] [&_button]:transition [&_button:not(:disabled)]:hover:bg-white/15 [&_button:not(:disabled)]:hover:text-[#D97D55] [&_button:disabled]:opacity-30",
                    }
                },
            }
        }
    }
}

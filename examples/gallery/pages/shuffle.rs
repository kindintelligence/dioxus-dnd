//! Shuffle: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn ShufflePage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Motion",
            title: "Shuffle",
            lead: "FlipItem measures its position before and after a layout change and animates the difference, so any reorder becomes a glide instead of a teleport.",
        }
        ShuffleDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "Two requirements: a stable key per item, so Dioxus reuses the DOM node across the reorder and FlipItem can measure the move, and an epoch you bump whenever the order changes, telling it to re-measure."
                }
                p {
                    "The technique is First-Last-Invert-Play: record the old rect, let the new layout apply, transform back to the old position, then release the transform with a transition."
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
                        "Experimental, honestly labeled:",
                        "this is the one module whose behavior depends on browser paint timing rather than pure logic.",
                    ),
                    (
                        "Compose with drop feedback:",
                        "keep completion effects shadow-only so they never fight the FLIP transform.",
                    ),
                    (
                        "Any layout change qualifies,",
                        "not just drags; anything that moves keyed nodes can glide.",
                    ),
                    ("Bump the epoch once per change;", "FlipItem does nothing between epochs."),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"let mut tiles = use_signal(|| (1..=6).collect::<Vec<u32>>());
let mut epoch = use_signal(|| 0usize);

button { onclick: move |_| { tiles.write().rotate_left(1); epoch += 1; }, "Shuffle" }
div { class: "grid grid-cols-6 gap-2",
    for n in tiles.read().iter().copied() {
        FlipItem { key: "{n}", epoch: epoch(), Tile { n } }
    }
}"#;

// --- 11. shuffle (FLIP reorder transitions) ----------------------------------

/// A translucent accent tint for FLIP tiles, keyed by index: the palette
/// colors at low opacity over the dark card, with a brightened text tone.
fn soft_tint(i: usize) -> &'static str {
    const C: [&str; 6] = [
        "bg-[#D97D55]/25 text-[#eda87f]",
        "bg-[#6FA4AF]/25 text-[#9ecad3]",
        "bg-[#B8C4A9]/25 text-[#c9d4ba]",
        "bg-[#F4E9D7]/15 text-[#e5d5b8]",
        "bg-[#D97D55]/25 text-[#eda87f]",
        "bg-[#6FA4AF]/25 text-[#9ecad3]",
    ];
    C[i % C.len()]
}

#[component]
fn ShuffleDemo() -> Element {
    let mut tiles = use_signal(|| (1..=6).collect::<Vec<u32>>());
    let mut epoch = use_signal(|| 0usize);
    let shuffle = move |_| {
        tiles.write().rotate_left(1);
        epoch += 1;
    };
    rsx! {
        Section {
            title: "Shuffle",
            note: "Change the order and every tile glides from its old slot to the new one. (Experimental; depends on browser paint timing.)",
            tag: "FlipItem",
            div { class: "space-y-3",
                button {
                    class: "rounded-lg bg-[#D97D55] px-3.5 py-1.5 text-[13px] font-medium text-white shadow-[0_2px_6px_-2px_rgba(217,125,85,0.5)] transition hover:bg-[#c96b45] active:scale-[0.98]",
                    onclick: shuffle,
                    "Shuffle"
                }
                div { class: "grid grid-cols-6 gap-2",
                    for n in tiles.read().iter().copied() {
                        // A stable key per tile lets Dioxus reuse the DOM node
                        // across reorders, so FlipItem can measure the move.
                        FlipItem {
                            key: "{n}",
                            epoch: epoch(),
                            class: "flex aspect-square items-center justify-center rounded-xl text-base font-semibold {soft_tint(n as usize - 1)}",
                            "{n}"
                        }
                    }
                }
            }
        }
    }
}

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
            lead: "FlipItem measures where an element was before a layout change and where it ended up after, then animates the difference. Any reorder becomes a glide instead of a teleport, with no per-item animation code.",
        }
        ShuffleDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Give every item a stable key.",
                        "Keys tell Dioxus which DOM node belongs to which item, so a reorder moves nodes instead of rewriting their contents. FlipItem can only measure a move if the node survives it.",
                    ),
                    (
                        "Bump an epoch when order changes.",
                        "FlipItem doesn't watch your data; it watches a counter. Change the order, add one to the epoch, and every wrapped item re-measures.",
                    ),
                    (
                        "FLIP plays the difference.",
                        "First-Last-Invert-Play: record the old rectangle, let the new layout apply, instantly transform the item back to where it was, then release the transform with a CSS transition. The browser animates the release, so items appear to glide to their new slots.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "The wrapper is invisible when idle: no transform, an armed transition, nothing else. All the motion comes from the one-frame inverse transform and its release."
                }
            }
            DioxusNote {
                p {
                    "key is a special attribute, not a prop: it guides Dioxus's diffing the way React keys do. Key by identity (the tile's number), never by position, or the reorder will rewrite contents in place and there will be no move to animate."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "FlipItem props",
                rows: vec![
                    ("epoch", "usize, required", "Bump whenever the surrounding order or layout changes; triggers a re-measure."),
                    ("duration", "f64 = 200.0", "Transition duration in milliseconds."),
                    ("easing", "String = \"ease\"", "CSS easing function for the glide."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Experimental, honestly labeled:",
                        "this is the one module whose behavior depends on browser paint timing rather than pure logic; validate in your target renderer.",
                    ),
                    (
                        "Any layout change qualifies,",
                        "not just drags: filters, sorts, insertions, window resizes; anything that moves keyed nodes can glide. See the Menu page.",
                    ),
                    (
                        "Compose with drop feedback:",
                        "keep completion effects shadow-only (like this gallery's flash) so they never fight the FLIP transform.",
                    ),
                    (
                        "Bump the epoch once per change;",
                        "FlipItem does nothing between epochs, so it costs nothing while the layout is at rest.",
                    ),
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

/// Tint fills for FLIP tiles, keyed by index: the system's forest, clay,
/// ochre and info fills, each with its own 700-level text tone.
fn soft_tint(i: usize) -> &'static str {
    const C: [&str; 6] = [
        "bg-[#E4ECDD] text-[#1C4A38]",
        "bg-[#E8D4BE] text-[#7A3E25]",
        "bg-[#E9DDB8] text-[#8A6A1F]",
        "bg-[#D9E4EC] text-[#2D4F6B]",
        "bg-[#F0F2E3] text-[#2A5E48]",
        "bg-[#F1D9D1] text-[#8B3A2E]",
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
                    class: "rounded-lg bg-[#1C4A38] px-3.5 py-1.5 text-[13px] font-medium text-white shadow-[0_2px_6px_-2px_rgba(28,74,56,0.5)] transition hover:bg-[#12362A] active:scale-[0.98]",
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

//! Newsletter builder: live demo plus how the pattern works.

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn NewsletterBuilderPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Organize",
            title: "Newsletter builder",
            lead: "Users already know this convention from their file manager: a plain drag moves, a drag with Ctrl or Cmd held drops a copy. dioxus-dnd resolves which one happened and hands you the answer; apply_clone_or_move turns it into the right model update.",
        }
        NewsletterDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "The modifier decides the effect.",
                        "While a drag is in flight, the held modifier keys are sampled on every pointer move. At release, Ctrl or Cmd resolves the effect to Copy and Alt to Link; otherwise the Draggable's base effect stands (Move, unless you set the effect prop).",
                    ),
                    (
                        "The answer arrives in the outcome.",
                        "Your on_drop handler reads DropOutcome::effect. At this point nothing has happened to your data: the library reports what the user asked for and leaves the interpretation to you.",
                    ),
                    (
                        "apply_clone_or_move interprets it.",
                        "Give it your HashMap of zone id to Vec model, a key function, and a clone hook. Move removes the matching item from the source zone and appends it to the target. Copy leaves the source alone and passes the item through the clone hook first, which is where you assign a fresh id.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "One handler covers both gestures. The key function tells a move which item to remove (matching by id, not by pointer equality), and the clone hook only runs on copies, so a copied block gets its own identity and the palette keeps the original."
                }
            }
            DioxusNote {
                p {
                    "zones.write() returns a guard that lets you mutate the value inside a signal; when the guard drops, everything reading that signal re-renders. next_id is also a signal, so next_id += 1 works from inside the closure and persists across renders."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "DropEffect variants",
                rows: vec![
                    ("Move", "default", "The item leaves its source and lands in the target. What most drags mean."),
                    ("Copy", "", "The target receives a duplicate; the source keeps the original. Forced by Ctrl or Cmd at release."),
                    ("Link", "", "A reference-style drop, forced by Alt. Rare, but the vocabulary matches the platform convention."),
                    ("None", "", "Drops disabled. Never overridden by modifier keys."),
                ],
            }
            PropsTable {
                title: "apply_clone_or_move arguments",
                rows: vec![
                    ("zones", "&mut HashMap<ZoneId, Vec<T>>", "Your model: one Vec per zone. An unknown target zone is created rather than dropping the item."),
                    ("outcome", "DropOutcome<T>", "The drop as delivered; from, to and effect steer what happens."),
                    ("key", "Fn(&T) -> K", "Extracts each item's identity so a move can find and remove the original. Matches every item with the payload's key."),
                    ("clone_item", "FnMut(T) -> T", "Runs only on Copy. Assign the fresh id here."),
                ],
            }
            PropsTable {
                title: "Related helpers",
                rows: vec![
                    ("effective_effect(base, modifiers)", "-> DropEffect", "The same modifier resolution the library applies, public for custom handlers."),
                    ("apply_list_clone_or_move(source, target, outcome, key, clone_item)", "", "The two-Vec version: pass the source and target lists directly, or None for a source-less palette drop."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Modifiers are sampled on every pointer move,",
                        "so the state held at the moment of release is what wins, not what was held at pickup.",
                    ),
                    (
                        "Dropping back onto the source zone",
                        "removes and re-appends, sending the item to the end of its own list. Use a sortable or board slots when in-place order matters.",
                    ),
                    (
                        "Keys should be unique within a zone.",
                        "A move prunes every source item whose key matches the payload's, so duplicate keys disappear together.",
                    ),
                    (
                        "The keyboard path plays too:",
                        "a keyboard drop resolves with the base effect, and your handler branches on DropOutcome::effect the same way.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"DropZone::<Card> {
    id: STAGE,
    on_drop: move |o: DropOutcome<Card>| {
        apply_clone_or_move(
            &mut zones.write(),
            o,
            |c| c.id,                 // identity: lets a move remove the source
            move |mut c| {            // clone hook: runs only on copy
                c.id = next_id();
                c
            },
        );
    },
    "Your email"
}"#;

// --- 2. newsletter builder (modifier keys + apply_clone_or_move) -------------

const PALETTE: ZoneId = ZoneId(9011);
const STAGE: ZoneId = ZoneId(9012);

#[component]
fn NewsletterDemo() -> Element {
    let mut zones = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Card>> = HashMap::new();
        m.insert(
            PALETTE,
            vec![
                Card::new(1, "Heading", "Big section title"),
                Card::new(2, "Image", "Full-width photo"),
                Card::new(3, "Button", "Call to action"),
                Card::new(4, "Quote", "Pull quote"),
            ],
        );
        m.insert(STAGE, vec![]);
        m
    });
    let mut next_id = use_signal(|| 100u32);
    let on_drop = move |o: DropOutcome<Card>| {
        // Ctrl/Cmd forces a copy (new id, source kept); a plain drag moves.
        apply_clone_or_move(
            &mut zones.write(),
            o,
            |c| c.id,
            move |mut c| {
                c.id = next_id();
                next_id += 1;
                c
            },
        );
    };
    rsx! {
        Section {
            title: "Newsletter builder",
            note: "Drag blocks in to move them. Hold Cmd or Ctrl to drop a copy instead, and build a whole email from a few pieces.",
            tag: "apply_clone_or_move",
            DndProvider::<Card> {
                LiveRegion::<Card> {}
                div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                    for (name, zone) in [("Blocks", PALETTE), ("Your email", STAGE)] {
                        DropZone::<Card> {
                            id: zone,
                            label: name,
                            on_drop,
                            class: ZONE,
                            p { class: "mb-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#7A776C]",
                                "{name}"
                            }
                            for card in zones.read().get(&zone).cloned().unwrap_or_default() {
                                Draggable::<Card> {
                                    key: "{card.id}",
                                    payload: card.clone(),
                                    zone,
                                    label: card.title.clone(),
                                    class: ITEM,
                                    div { class: ROW,
                                        CardFace { card: card.clone() }
                                    }
                                }
                            }
                            if zone == STAGE && zones.read().get(&zone).map(|v| v.is_empty()).unwrap_or(true) {
                                p { class: "py-3 text-center text-[12px] text-[#BBB8AE]",
                                    "Drop blocks to compose your email"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

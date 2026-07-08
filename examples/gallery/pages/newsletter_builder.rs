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
            lead: "The file-manager convention, for free: a plain drag moves a block, holding Cmd or Ctrl drops a copy, and apply_clone_or_move applies whichever happened to your model.",
        }
        NewsletterDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "During any drag the library resolves the effective drop effect from the held modifiers: Ctrl or Cmd forces Copy, Alt forces Link, otherwise the source's base effect applies. The resolved value arrives in DropOutcome::effect."
                }
                p {
                    "apply_clone_or_move takes a HashMap<ZoneId, Vec<T>> model, an identity function so a move can remove the item from its source, and a clone hook that assigns a fresh id when the drop was a copy."
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
                        "effective_effect is public",
                        "if you need the same modifier resolution inside a custom handler.",
                    ),
                    (
                        "Two plain lists instead of a map?",
                        "apply_list_clone_or_move takes the source and target Vecs directly.",
                    ),
                    (
                        "Modifiers are sampled on every pointer move,",
                        "so the state held at the moment of release is what wins.",
                    ),
                    (
                        "A base effect of DropEffect::None",
                        "disables drops entirely and is never overridden by modifiers.",
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
                            p { class: "mb-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9c8f77]",
                                "{name}"
                            }
                            for card in zones.read().get(&zone).cloned().unwrap_or_default() {
                                Draggable::<Card> {
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
                                p { class: "py-3 text-center text-[12px] text-[#6d6150]",
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

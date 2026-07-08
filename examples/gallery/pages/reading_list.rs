//! Reading list: live demo plus how the pattern works.

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn ReadingListPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Organize",
            title: "Reading list",
            lead: "The core pattern: a payload travels from a Draggable to whichever DropZone receives it, through shared context rather than DataTransfer strings. Everything else in the library builds on this.",
        }
        ReadingListDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "DndProvider stores a Store<DragState<T>> in Dioxus context. Picking an item up writes the payload there; every DropZone reads it to decide whether to light up, and the zone you release over receives a DropOutcome with the payload, the source and target zone ids, the resolved effect, and the input mode."
                }
                p {
                    "The floating card that follows your cursor is a DragOverlay: it renders its children pinned to the pointer while a drag is in flight, so the ghost is your own rsx, not a browser screenshot of the element."
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
                        "Presence-based styling.",
                        "data-over and data-active appear on a DropZone while relevant and are absent otherwise, so Tailwind variants like data-over:border-clay work with zero state of your own.",
                    ),
                    (
                        "Keyboard is built in.",
                        "Every draggable is focusable: Space picks up, arrows walk the registered zones, Space drops, Escape cancels. Render LiveRegion once per provider to voice it.",
                    ),
                    (
                        "The zone prop feeds DropOutcome::from,",
                        "so one on_drop handler can tell a move between shelves from a drop that started elsewhere.",
                    ),
                    (
                        "Explicit ids are optional.",
                        "Zones auto-generate ids; hand out your own (any u32-range value) only when handlers need to name zones.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"DndProvider::<Card> {
    Draggable::<Card> {
        payload: card.clone(),
        zone: SHELF_A,
        label: card.title.clone(),
        CardFace { card }
    }
    DropZone::<Card> {
        id: SHELF_B,
        label: "Finished",
        on_drop: move |o: DropOutcome<Card>| shelve(o.payload, o.to),
        "Drop here"
    }
    DragOverlay::<Card> { class: "rotate-2 shadow-xl", Ghost {} }
}"#;

// --- 1. reading list (core Draggable / DropZone + overlay) -------------------

const TODO: ZoneId = ZoneId(9001);
const DONE: ZoneId = ZoneId(9002);

#[component]
fn ReadingListDemo() -> Element {
    let mut bins = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Card>> = HashMap::new();
        m.insert(
            TODO,
            vec![
                Card::new(1, "The Creative Act", "Rick Rubin"),
                Card::new(2, "Piranesi", "Susanna Clarke"),
                Card::new(3, "Klara and the Sun", "Kazuo Ishiguro"),
            ],
        );
        m.insert(
            DONE,
            vec![Card::new(4, "Tomorrow, and Tomorrow", "Gabrielle Zevin")],
        );
        m
    });
    // Book that just landed, so it flashes onto its new shelf.
    let mut flashed = use_signal(|| None::<u32>);
    let move_card = move |o: DropOutcome<Card>| {
        let id = o.payload.id;
        let mut b = bins.write();
        for cards in b.values_mut() {
            cards.retain(|c| c.id != id);
        }
        b.entry(o.to).or_default().push(o.payload);
        drop(b);
        flashed.set(Some(id));
    };

    rsx! {
        Section {
            title: "Reading list",
            note: "Two shelves: what you're reading, and what you've finished. Move a book across and it flashes onto its new shelf.",
            tag: "DropZone",
            DndProvider::<Card> {
                LiveRegion::<Card> {}
                div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                    for (name, zone, hint) in [("Reading", TODO, "In progress"), ("Finished", DONE, "Done")] {
                        DropZone::<Card> {
                            id: zone,
                            label: name,
                            on_drop: move_card,
                            class: ZONE,
                            div { class: "mb-1 flex items-baseline justify-between",
                                p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9c8f77]",
                                    "{name}"
                                }
                                span { class: "text-[10px] text-[#6d6150]", "{hint}" }
                            }
                            for card in bins.read().get(&zone).cloned().unwrap_or_default() {
                                Draggable::<Card> {
                                    payload: card.clone(),
                                    zone,
                                    label: card.title.clone(),
                                    class: if flashed() == Some(card.id) { format!("{ITEM} drop-flash") } else { ITEM.to_string() },
                                    // Clear the flash when any card is picked up, so the
                                    // next drop re-triggers the animation cleanly.
                                    on_drag_start: move |_| flashed.set(None),
                                    div { class: ROW,
                                        CardFace { card: card.clone() }
                                    }
                                }
                            }
                        }
                    }
                }
                DragOverlay::<Card> { class: "pointer-events-none flex items-center gap-2 rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] px-3.5 py-2.5 text-[13px] font-medium text-[#f4e9d7] shadow-[inset_0_1px_0_rgba(255,255,255,0.09),inset_0_0_0_1px_rgba(255,255,255,0.04),0_20px_44px_-12px_rgba(0,0,0,0.65)]",
                    CardGhost {}
                }
            }
        }
    }
}

#[component]
fn CardGhost() -> Element {
    let dnd = use_dnd::<Card>();
    rsx! {
        if let Some(c) = dnd.payload() {
            span { class: "h-4 w-1 shrink-0 rounded-full {swatch(c.id)}" }
            span { "{c.title}" }
        }
    }
}

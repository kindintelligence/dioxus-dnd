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
            lead: "Everything in dioxus-dnd starts here. Wrap part of your app in a provider, mark things as draggable, mark places as drop zones, and a typed Rust value travels from hand to hand. No JavaScript, no serialization, and no browser-generated drag ghost.",
        }
        ReadingListDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "One provider, one payload type.",
                        "DndProvider::<Card> creates a shared drag context for everything inside it. The payload can be any Rust type that implements Clone; here it is a small Card struct with a title and an author.",
                    ),
                    (
                        "Pick up.",
                        "Draggable wraps its children in a drag source. Press and move eight pixels (or focus it and press Space) and the payload is written into the shared context. The wrapper gains data-dragging while its payload is in flight.",
                    ),
                    (
                        "Every zone reacts.",
                        "While a drag is in flight, each DropZone that would accept the payload carries data-active, and the zone under the pointer also carries data-over. Style those two attributes and you have full hover feedback with zero state of your own.",
                    ),
                    (
                        "Drop.",
                        "Release over a zone and its on_drop handler receives a DropOutcome: the payload, the zone it came from, the zone it landed on, and how the drag was driven. What the drop means for your data is entirely your call; the library never touches your model.",
                    ),
                ],
            }
            Prose {
                p {
                    "The floating card that follows your cursor is a DragOverlay. It renders your own rsx pinned to the pointer while a drag is in flight, so the ghost is a real element you style, not a screenshot the browser took of the original. match_source: true dresses it in the grabbed card's exact measured size, which keeps the cursor precisely where you pressed - no shrink, no jump. And settle: true finishes the story: on drop the ghost glides onto the landing card's exact spot instead of vanishing (snapping near-instantly under reduced motion), and the real card takes over the moment it arrives."
                }
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Three components, one shared type parameter. The Draggable declares what it carries and (optionally) which zone it currently lives in; the DropZone declares what happens when something lands. The overlay is optional: without it, the original element simply fades via data-dragging styling."
                }
            }
            DioxusNote {
                p {
                    "rsx! builds the UI tree, like JSX with Rust syntax: components are capitalized, plain elements are lowercase, and braces hold Rust expressions. A #[component] function returns Element and re-runs whenever state it reads changes."
                }
                p {
                    "use_signal creates that state. Reading a signal inside a component subscribes it; writing (bins.write(), landing.set(...)) re-renders every subscriber. The turbofish ::<Card> just pins the generic payload type."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "Draggable props",
                rows: vec![
                    ("payload", "T, required", "The value delivered to whichever zone receives this drag. Cloned into the shared context on pickup."),
                    ("zone", "Option<ZoneId>", "The zone this item currently lives in. Arrives in DropOutcome::from so handlers can tell a move from an arrival."),
                    ("effect", "DropEffect = Move", "The drop's meaning: Move, Copy, Link, or None to advertise that drops are disabled."),
                    ("disabled", "bool = false", "Turn dragging off without unmounting. Adds data-disabled for styling."),
                    ("threshold", "f64 = 8.0", "Movement in CSS pixels before a press becomes a drag, so clicks stay clicks."),
                    ("label", "Option<String>", "Human name used in screen-reader announcements (\"Picked up Piranesi\")."),
                    ("on_drag_start / on_drag_end", "EventHandler", "Lifecycle hooks. on_drag_end reports true when a zone consumed the payload, false on cancel."),
                ],
            }
            PropsTable {
                title: "DropZone props",
                rows: vec![
                    ("on_drop", "EventHandler<DropOutcome<T>>, required", "Called with the full outcome when an acceptable payload is released here."),
                    ("id", "Option<ZoneId>", "Stable identity. Auto-generated when omitted; pass your own (any u32-range value) when handlers need to name zones."),
                    ("label", "Option<String>", "Screen-reader name announced during keyboard navigation (\"Over Finished\")."),
                    ("accepts", "Callback<T, bool>", "Return false to refuse a payload: the zone won't highlight, and drops pass through to whatever is beneath."),
                ],
            }
            PropsTable {
                title: "DragOverlay props",
                rows: vec![
                    ("settle", "bool = false", "Glide the ghost from the release point into the receiving zone on drop, instead of vanishing. Honors prefers-reduced-motion; cancelled drags and keyboard drops never settle."),
                    ("match_source", "bool = false", "Dress the ghost in the grabbed element's measured rect. The pointer stays exactly where you pressed, whatever your ghost rsx renders - no size jump at pickup."),
                    ("duration", "f64 = 200.0", "Settle transition duration in milliseconds."),
                    ("easing", "String = \"ease\"", "CSS easing function for the settle glide."),
                    ("on_settled", "EventHandler<()>", "Fired when the settle glide lands - the hook for anything that should wait for the ghost (cleanup, sounds, state)."),
                ],
            }
            PropsTable {
                title: "DropOutcome<T> fields",
                rows: vec![
                    ("payload", "T", "The dragged value, handed back to you owned."),
                    ("from / to", "Option<ZoneId> / ZoneId", "Where the drag started (if the Draggable declared a zone) and the zone that received it."),
                    ("effect", "DropEffect", "The resolved effect, including any modifier keys held at release."),
                    ("mode", "DragMode", "Pointer or Keyboard: which input drove the completed drag."),
                    ("client / element / grab", "Point", "Where the drop happened: viewport coordinates, zone-relative coordinates, and where inside the element it was originally grabbed."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Presence-based styling.",
                        "data-over and data-active appear while relevant and are absent otherwise, so Tailwind variants like data-over:border-orange-400 and plain CSS [data-over] selectors work with no state of your own.",
                    ),
                    (
                        "Keyboard is built in.",
                        "Every draggable is focusable: Space picks up, arrow keys walk the registered zones, Space drops, Escape cancels. Render LiveRegion once per provider to voice it to screen readers.",
                    ),
                    (
                        "One provider per payload type.",
                        "Draggables and zones find each other through the nearest DndProvider with a matching type. Two independent drag scopes are just two providers.",
                    ),
                    (
                        "Touch works out of the box.",
                        "The same pointer gesture serves mouse, touch and pen, and near-miss touch drops snap to the closest acceptable zone within 48px.",
                    ),
                    (
                        "SettleSlot completes the illusion.",
                        "Wrap the element your drop just created and mark it active: it holds its space invisibly while the ghost glides onto its exact rect, then swaps in as the ghost unmounts - one object from pickup to landing, never two copies.",
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
    DragOverlay::<Card> {
        settle: true,
        match_source: true,          // ghost wears the grabbed card's rect
        class: "shadow-xl",
        Ghost {}
    }
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
    // Book currently mid-glide (pointer drops only - keyboard drops never
    // settle): its SettleSlot holds the space and swaps in as the ghost
    // lands. No arrival effect beyond the glide itself.
    let mut landing = use_signal(|| None::<u32>);
    let move_card = move |o: DropOutcome<Card>| {
        let id = o.payload.id;
        let mut b = bins.write();
        for cards in b.values_mut() {
            cards.retain(|c| c.id != id);
        }
        b.entry(o.to).or_default().push(o.payload);
        drop(b);
        if o.mode != DragMode::Keyboard {
            landing.set(Some(id));
        }
    };

    rsx! {
        Section {
            title: "Reading list",
            note: "Two shelves: what you're reading, and what you've finished. Pick a book up and it lifts off its shelf; release it over the other shelf and it glides into its new place.",
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
                                p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#7A776C]",
                                    "{name}"
                                }
                                span { class: "text-[10px] text-[#BBB8AE]", "{hint}" }
                            }
                            for card in bins.read().get(&zone).cloned().unwrap_or_default() {
                                // The slot makes the drop read as one object:
                                // the landed card holds its space invisibly
                                // while the ghost glides onto this exact spot,
                                // then swaps in as the ghost unmounts.
                                SettleSlot::<Card> {
                                    key: "{card.id}",
                                    active: landing() == Some(card.id),
                                    Draggable::<Card> {
                                        payload: card.clone(),
                                        zone,
                                        label: card.title.clone(),
                                        // One copy, ever: the ghost wears this card's
                                        // exact rect at pickup, so hiding the original
                                        // outright reads as the card itself lifting off
                                        // (its empty slot stays open behind it).
                                        class: format!("{ITEM} data-dragging:!opacity-0"),
                                        div { class: ROW,
                                            CardFace { card: card.clone() }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // The ghost is the card, not a summary of it: match_source
                // wears the grabbed row's exact rect (so the cursor stays
                // where you pressed) and the content mirrors CardFace. The
                // landing card takes over silently as the glide ends.
                DragOverlay::<Card> {
                    settle: true,
                    match_source: true,
                    duration: 160.0,
                    easing: "cubic-bezier(0.22, 1, 0.36, 1)",
                    on_settled: move |_| landing.set(None),
                    class: GHOST,
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
            CardFace { card: c }
        }
    }
}

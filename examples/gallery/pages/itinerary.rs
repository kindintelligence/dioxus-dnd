//! Itinerary: closest-edge insertion indicators on bare drop zones.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn ItineraryPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Structure",
            title: "Itinerary",
            lead: "Drop above a row to slot in before it, below to follow it. The zone's edge prop reads which edge the pointer is nearest - live, as data-edge - and delivers the answer with the drop, so a plain DropZone becomes a precise insertion target.",
        }
        ItineraryDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "One prop turns on the signal.",
                        "edge: EdgeSet::Vertical tells the zone its items stack vertically, so only the top and bottom edges compete. While an acceptable pointer drag hovers, the zone carries data-edge=\"top\" or \"bottom\", recomputed on every pointer move from the zone's measured rect.",
                    ),
                    (
                        "Indicators are pure CSS.",
                        "Style the two attribute values and you have insertion lines with zero state: a value selector like data-[edge=top]:shadow-[0_-3px_0_0_currentColor] draws a line above the row without shifting layout the way a border would.",
                    ),
                    (
                        "The drop carries the answer.",
                        "The delivered DropOutcome::edge records the edge held at release, so the handler maps Top to \"insert before this row\" and Bottom to \"after it\". No re-deriving geometry in your handler, no stale hover state.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Each row is an ordinary DropZone wrapping an ordinary Draggable, so rows are simultaneously targets and sources - reordering falls out for free. The edge only refines where the payload lands; acceptance, highlighting and keyboard reachability are unchanged."
                }
            }
            DioxusNote {
                p {
                    "data-[edge=top]: is Tailwind's arbitrary-value variant: it matches the attribute value the zone renders. Plain CSS works the same way - a [data-edge=top] selector with a box-shadow."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "The closest-edge kit",
                rows: vec![
                    ("edge (DropZone prop)", "Option<EdgeSet>", "Opt-in. Vertical tracks top/bottom, Horizontal left/right, All every edge. Renders data-edge while an acceptable pointer drag hovers."),
                    ("DropOutcome::edge", "Option<Edge>", "The edge held at release. None for keyboard drops (their release point is the zone center) and on zones that didn't opt in - treat None as your neutral intent."),
                    ("edge_of(point, rect, edges)", "-> Edge", "The pure function behind both, public for custom zones: clamps the point into the rect and returns the nearest allowed edge."),
                    ("Edge / EdgeSet", "enums", "Top, Right, Bottom, Left, with as_str() matching the attribute values; sets are named by stacking direction, like sortable's Axis."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Edges are physical, not logical:",
                        "data-edge=\"left\" always means the screen-left side, even under dir: Rtl, because the styling it drives targets a screen side. Map to logical before/after in your handler if your model needs it.",
                    ),
                    (
                        "Restricting the set beats filtering in CSS.",
                        "With All, a wide row reads left or right in its end strips and your top/bottom styles would silently show nothing there. Vertical keeps the whole row answering the question you're asking.",
                    ),
                    (
                        "Keyboard drops carry None,",
                        "so give it a sensible meaning - this demo inserts after the focused row. Ties at the exact center resolve toward top, then left.",
                    ),
                    (
                        "Want the list to make room as you hover?",
                        "That's the live-preview pattern - reach for SortableList. Closest-edge is the lightweight alternative: indicators only, any zone shape, your model.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"for (ix, stop) in plan.read().iter().enumerate() {
    DropZone::<Card> {
        id: ZoneId(BASE + stop.id as u64),
        label: stop.title.clone(),
        edge: EdgeSet::Vertical,   // a vertical stack: top/bottom compete
        class: "data-[edge=top]:shadow-[0_-3px_0_0_#1C4A38] \
                data-[edge=bottom]:shadow-[0_3px_0_0_#1C4A38]",
        on_drop: move |o: DropOutcome<Card>| {
            let at = match o.edge {
                Some(Edge::Top) => ix, // above this row
                _ => ix + 1,           // below it (keyboard drops too)
            };
            insert_stop(o.payload, at);
        },
        StopRow { stop }
    }
}"#;

// --- 16. itinerary (closest-edge insertion on bare zones) --------------------

/// Row zones get stable ids derived from the stop's id, well clear of the
/// other pages' explicit ids.
const EDGE_BASE: u64 = 9200;
const WRAP_UP: ZoneId = ZoneId(9299);

#[component]
fn ItineraryDemo() -> Element {
    let mut plan = use_signal(|| {
        vec![
            Card::new(1, "Morning market", "coffee first"),
            Card::new(2, "Old town walk", "2 hours"),
            Card::new(3, "Lunch by the river", "book ahead"),
            Card::new(4, "Museum of maps", "closes at five"),
        ]
    });
    let mut ideas = use_signal(|| {
        vec![
            Card::new(5, "Kayak tour", "morning slots"),
            Card::new(6, "Night food stalls", "cash only"),
            Card::new(7, "Hilltop lookout", "sunset"),
        ]
    });

    // One mutation for every landing: pull the payload out of wherever it
    // lives, then insert at the requested slot (index math accounts for the
    // removal shifting the list).
    let mut land = move |payload: Card, mut at: usize| {
        ideas.write().retain(|c| c.id != payload.id);
        let mut p = plan.write();
        if let Some(old) = p.iter().position(|c| c.id == payload.id) {
            p.remove(old);
            if old < at {
                at -= 1;
            }
        }
        let at = at.min(p.len());
        p.insert(at, payload);
    };

    rsx! {
        Section {
            title: "Itinerary",
            note: "Build the day: drag ideas into the plan, and drag stops around inside it. The line above or below a row shows exactly where the drop will land - drawn purely from data-edge.",
            tag: "edge_of",
            DndProvider::<Card> {
                LiveRegion::<Card> {}
                div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                    // --- the plan: every row is zone + source ----------------
                    div {
                        p { class: "mb-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#7A776C]",
                            "The plan"
                        }
                        ol { class: "space-y-2",
                            for (ix , stop) in plan.read().clone().into_iter().enumerate() {
                                li { key: "{stop.id}",
                                    DropZone::<Card> {
                                        id: ZoneId(EDGE_BASE + stop.id as u64),
                                        label: stop.title.clone(),
                                        edge: EdgeSet::Vertical,
                                        accepts: {
                                            let self_id = stop.id;
                                            move |c: Card| c.id != self_id
                                        },
                                        on_drop: move |o: DropOutcome<Card>| {
                                            let at = match o.edge {
                                                Some(Edge::Top) => ix,
                                                _ => ix + 1,
                                            };
                                            land(o.payload, at);
                                        },
                                        class: "block rounded-xl transition-shadow data-[edge=top]:shadow-[0_-3px_0_0_#1C4A38] data-[edge=bottom]:shadow-[0_3px_0_0_#1C4A38]",
                                        Draggable::<Card> {
                                            payload: stop.clone(),
                                            zone: ZoneId(EDGE_BASE + stop.id as u64),
                                            label: stop.title.clone(),
                                            class: ITEM,
                                            div { class: ROW,
                                                span { class: "w-5 text-right font-mono text-[11px] tabular-nums text-[#BBB8AE]",
                                                    {format!("{:02}", ix + 1)}
                                                }
                                                CardFace { card: stop.clone() }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        DropZone::<Card> {
                            id: WRAP_UP,
                            label: "End of the day",
                            on_drop: move |o: DropOutcome<Card>| {
                                let at = plan.read().len();
                                land(o.payload, at);
                            },
                            class: "mt-2 grid min-h-10 place-items-center rounded-xl border border-dashed border-[#D7D4C9] text-[11.5px] font-medium text-[#9B988D] transition data-active:border-[#6C9984] data-active:text-[#45423B] data-over:border-solid data-over:border-[#1C4A38] data-over:bg-[#E4ECDD]",
                            "End of the day"
                        }
                    }
                    // --- the tray: plain sources ------------------------------
                    div { class: "rounded-xl bg-[#EEEADF] p-3 ring-1 ring-[#E8E5D9]",
                        p { class: "mb-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#7A776C]",
                            "Ideas"
                        }
                        div { class: "space-y-2",
                            for idea in ideas.read().clone() {
                                Draggable::<Card> {
                                    key: "{idea.id}",
                                    payload: idea.clone(),
                                    label: idea.title.clone(),
                                    class: ITEM,
                                    div { class: ROW,
                                        CardFace { card: idea.clone() }
                                    }
                                }
                            }
                            if ideas.read().is_empty() {
                                p { class: "py-2 text-center text-[12px] text-[#9B988D]",
                                    "All scheduled."
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

//! Sprint board: live demo plus how the pattern works.

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn SprintBoardPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Structure",
            title: "Sprint board",
            lead: "BoardColumn appends, BoardSlot inserts at an exact index, and one accepts closure on the column becomes a WIP limit that both the column and its slots enforce, on every input path.",
        }
        SprintDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "Cards are BoardItems carrying a BoardPayload (the item, its source column and its index) through context. Dropping on a column produces a MoveEvent whose target index is None, meaning append; dropping on a slot produces Some(index) for a precise insert. apply_move applies either to a HashMap board model, adjusting same-column moves correctly."
                }
                p {
                    "The column's accepts closure inherits to its slots via context. Here it returns false when In progress is full unless the card already lives there, so the column stops lighting up and the drop is refused outright."
                }
                p {
                    "One rule worth internalizing: keep slot geometry constant mid-drag. The pointer path hit-tests rects cached at drag start, so a slot that grows in the layout shifts everything below it. Show the open state with a transform or an indicator line, never with reflow. The invisible hit band here is a taller element pulled back by negative margins with pointer-events: none."
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
                        "Slots are real zones:",
                        "pointer, touch and keyboard drops can all target them; each announces as \"Insert at position N\" unless you pass a label.",
                    ),
                    (
                        "Same-column no-ops are detectable:",
                        "compare the drag payload's from and index against a slot to suppress its indicator.",
                    ),
                    (
                        "Auto ids start at 2^32,",
                        "so explicit column ids in the u32 range can never collide with a slot's auto id.",
                    ),
                    (
                        "Boards nest:",
                        "inner drag scopes stop propagation, so a board inside a board owns its own gestures.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"BoardColumn::<Task> {
    id: DOING,                       // explicit ids: any u32-range value
    accepts: move |p: BoardPayload<Task>| {
        p.from == DOING || count(DOING) < WIP
    },
    on_move: move |mv: MoveEvent<Task>| apply_move(&mut board.write(), mv),

    BoardSlot::<Task> { column: DOING, index: 0, on_move }
    for (ix, task) in tasks.iter().enumerate() {
        BoardItem::<Task> { item: task.clone(), column: DOING, index: ix, TaskCard {} }
        BoardSlot::<Task> { column: DOING, index: ix + 1, on_move }
    }
}"#;

// --- 8. sprint board (kanban: insertion slots + a WIP limit that refuses) ----

// High ids, far above the `use_zone_id` auto counter: `BoardSlot`s draw
// auto ids (11, 12, ...) from the same process-wide sequence, and the zone
// registry replaces by id, so a low explicit column id can collide with a
// slot's auto id and silently knock it out of the registry.
const BACKLOG: ContainerId = ZoneId(9101);
const DOING: ContainerId = ZoneId(9102);
const SHIPPED: ContainerId = ZoneId(9103);

/// In progress holds this many cards, no more.
const WIP: usize = 2;

/// Two-letter initials for the assignee chip.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect()
}

#[component]
fn SprintDemo() -> Element {
    let board = use_signal(|| {
        let mut m: HashMap<ContainerId, Vec<Card>> = HashMap::new();
        m.insert(
            BACKLOG,
            vec![
                Card::new(1, "Dark mode tokens", "Priya Nair"),
                Card::new(2, "Fix drop flicker", "Sam Ortiz"),
                Card::new(3, "Touch handles", "Mara Chen"),
            ],
        );
        m.insert(DOING, vec![Card::new(4, "Keyboard traversal", "Chad N")]);
        m.insert(SHIPPED, vec![Card::new(5, "Pointer capture", "Sam Ortiz")]);
        m
    });
    rsx! {
        Section {
            title: "Sprint board",
            note: "Point between two cards and a clay line marks the exact insert, not just an append. In progress is capped at two: once full it stops lighting up and refuses the drop until something ships.",
            tag: "BoardSlot",
            DndProvider::<BoardPayload<Card>> {
                LiveRegion::<BoardPayload<Card>> {}
                SprintColumns { board }
            }
        }
    }
}

/// The columns live in their own component so `use_dnd` runs *inside* the
/// `DndProvider` above (context is provided to children, not siblings).
#[component]
fn SprintColumns(board: Signal<HashMap<ContainerId, Vec<Card>>>) -> Element {
    let mut board = board;
    let dnd = use_dnd::<BoardPayload<Card>>();
    let count = move |col: ContainerId| board.read().get(&col).map(|v| v.len()).unwrap_or(0);
    let on_move = move |mv: MoveEvent<Card>| apply_move(&mut board.write(), mv);
    // The WIP rule, enforced by the library: when In progress is full, neither
    // the column nor its slots light up, and the drop is refused outright.
    // Moves *within* the column stay allowed - the count doesn't change.
    let wip_gate = move |col: ContainerId, p: BoardPayload<Card>| {
        col != DOING || p.from == DOING || count(DOING) < WIP
    };
    // The two slots hugging the dragged card are no-op drops (the card would
    // land where it already is), so their indicator is suppressed: only slots
    // that actually move something light up.
    let is_noop = move |col: ContainerId, ix: usize| {
        dnd.payload()
            .map(|p| p.from == col && (ix == p.index || ix == p.index + 1))
            .unwrap_or(false)
    };
    // Slot geometry never changes mid-drag: the pointer path hit-tests rects
    // cached at drag start, so a slot that grows in the layout would shift
    // every card below it and strand the highlight on stale geometry. The
    // open state is a clay insertion line scaling in, with zero reflow.
    //
    // The visible gap stays 12px, but the slot's *element* is a 32px band
    // (h-8 pulled back by -my-2.5) overlapping the card edges: the library
    // hit-tests the measured rect, so pointing anywhere near the seam
    // resolves to the slot. pointer-events-none keeps that invisible overlap
    // from stealing pointerdown on the cards themselves.
    const SLOT: &str = "pointer-events-none relative -my-2.5 flex h-8 items-center px-1 [&[data-over]>span]:scale-x-100 [&[data-over]>span]:opacity-100";
    const SLOT_LINE: &str = "h-[3px] w-full origin-center scale-x-50 rounded-full bg-[#D97D55] opacity-0 shadow-[0_0_12px_rgba(217,125,85,0.7)] transition-all duration-150";

    rsx! {
        div { class: "grid grid-cols-1 gap-3 sm:grid-cols-3",
            for (name, col) in [("Backlog", BACKLOG), ("In progress", DOING), ("Shipped", SHIPPED)] {
                BoardColumn::<Card> {
                    id: col,
                    label: name,
                    on_move,
                    accepts: move |p: BoardPayload<Card>| wip_gate(col, p),
                    class: "rounded-xl bg-[#26211a] p-2.5 min-h-36 shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)] transition data-active:ring-1 data-active:ring-[#B8C4A9]/60 data-active:bg-[#B8C4A9]/12",
                    div { class: "mb-1 flex items-center justify-between px-1",
                        p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9c8f77]",
                            "{name}"
                        }
                        if col == DOING {
                            span { class: if count(DOING) >= WIP { "rounded-full bg-[#D97D55] px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-white" } else { "rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-[#b8ab93] ring-1 ring-white/10" },
                                "{count(DOING)}/{WIP}"
                            }
                        } else {
                            span { class: "min-w-5 rounded-full bg-white/10 px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums text-[#b8ab93] ring-1 ring-white/10",
                                "{count(col)}"
                            }
                        }
                    }
                    BoardSlot::<Card> {
                        column: col,
                        index: 0,
                        on_move,
                        class: SLOT,
                        if !is_noop(col, 0) {
                            span { class: SLOT_LINE }
                        }
                    }
                    for (ix, card) in board.read().get(&col).cloned().unwrap_or_default().into_iter().enumerate() {
                        BoardItem::<Card> {
                            item: card.clone(),
                            column: col,
                            index: ix,
                            label: card.title.clone(),
                            class: ITEM,
                            div { class: ROW,
                                span { class: "h-7 w-1 shrink-0 rounded-full {swatch(card.id)}" }
                                div { class: "min-w-0 flex-1",
                                    div { class: "truncate font-medium text-[#f4e9d7]",
                                        "{card.title}"
                                    }
                                    div { class: "truncate text-[11px] text-[#9c8f77]",
                                        "{card.sub}"
                                    }
                                }
                                span { class: "grid h-6 w-6 shrink-0 place-items-center rounded-full bg-white/10 text-[9px] font-bold uppercase text-[#e0a37f] ring-1 ring-white/10",
                                    "{initials(&card.sub)}"
                                }
                            }
                        }
                        BoardSlot::<Card> {
                            column: col,
                            index: ix + 1,
                            on_move,
                            class: SLOT,
                            if !is_noop(col, ix + 1) {
                                span { class: SLOT_LINE }
                            }
                        }
                    }
                }
            }
        }
    }
}

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
            lead: "The kanban pattern: cards travel between columns, thin slots between cards catch precise inserts, and a single accepts closure on a column becomes a WIP limit that every input path respects.",
        }
        SprintDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Cards carry their origin.",
                        "BoardItem is a thin wrapper over Draggable whose payload is a BoardPayload: your item plus the column and index it was picked up from. The provider's type is DndProvider::<BoardPayload<Card>>.",
                    ),
                    (
                        "Columns append, slots insert.",
                        "Dropping on a BoardColumn emits a MoveEvent whose target index is None, meaning append to the end. Dropping on a BoardSlot emits Some(index) for that exact position. apply_move applies either to a HashMap board model, adjusting indices correctly when a card moves within its own column.",
                    ),
                    (
                        "One filter rules them all.",
                        "A column's accepts closure inherits to every slot inside it through context. Here it refuses new cards when In progress is full, so neither the column nor its slots light up, and the drop bounces on pointer, touch and keyboard alike.",
                    ),
                    (
                        "Slots must not reflow.",
                        "The pointer path hit-tests rects cached at drag start, so a slot that grows mid-drag would shift every card below it and strand the highlight. Show the open state with a transform or an indicator line; this demo scales in a clay line and widens the hit area with an invisible band.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "The rhythm is slot, card, slot, card, slot: one insertion point before each card and one at the end. Both zone kinds share the same on_move handler, so the model code neither knows nor cares whether a drop was an append or a precise insert."
                }
            }
            DioxusNote {
                p {
                    "Context flows downward: BoardColumn provides its acceptance filter and the slots inside discover it automatically, the same mechanism DndProvider uses to reach every draggable. That is also why the demo keeps its columns in a child component: use_dnd must be called inside the provider, not beside it."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "BoardColumn props",
                rows: vec![
                    ("id", "ContainerId, required", "The column's identity (a ZoneId). Use explicit u32-range ids so handlers can name columns."),
                    ("on_move", "EventHandler<MoveEvent<T>>, required", "Receives every completed move targeting this column."),
                    ("accepts", "Callback<BoardPayload<T>, bool>", "Refuse payloads (WIP limits, type rules). Inherited by nested slots."),
                    ("label", "Option<String>", "Screen-reader name for keyboard navigation."),
                ],
            }
            PropsTable {
                title: "BoardSlot and BoardItem props",
                rows: vec![
                    ("BoardSlot: column, index", "ContainerId, usize", "Which column this insertion point belongs to and the index a drop should insert at. Carries data-active and data-over for styling."),
                    ("BoardItem: item, column, index", "T, ContainerId, usize", "The card and where it currently lives; packed into the BoardPayload on pickup."),
                ],
            }
            PropsTable {
                title: "The move model",
                rows: vec![
                    ("BoardPayload<T>", "item, from, index", "What travels through context while a card is in flight."),
                    ("MoveEvent<T>", "item, from: (col, ix), to: (col, Option<ix>)", "A completed move. A None target index means append."),
                    ("apply_move(&mut board, mv)", "", "Applies a MoveEvent to a HashMap of ContainerId to Vec, handling same-column index shifts and creating unknown target columns."),
                ],
            }
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
                        "compare the drag payload's from and index against a slot to suppress its indicator, as this demo does for the two slots hugging the dragged card.",
                    ),
                    (
                        "Auto ids start at 2^32,",
                        "so explicit column ids in the u32 range can never collide with a slot's auto-generated id.",
                    ),
                    (
                        "Moves within a column stay allowed",
                        "under a WIP limit when the filter checks the payload's origin: the count does not change, so reordering full columns still works.",
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

// Explicit column ids. `BoardSlot` auto ids start at 2^32, so explicit
// ids anywhere in u32 range can never collide with them.
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
    const SLOT_LINE: &str = "h-[3px] w-full origin-center scale-x-50 rounded-full bg-[#1C4A38] opacity-0 transition-all duration-150";

    rsx! {
        div { class: "grid grid-cols-1 gap-3 sm:grid-cols-3",
            for (name, col) in [("Backlog", BACKLOG), ("In progress", DOING), ("Shipped", SHIPPED)] {
                BoardColumn::<Card> {
                    id: col,
                    label: name,
                    on_move,
                    accepts: move |p: BoardPayload<Card>| wip_gate(col, p),
                    class: "rounded-xl bg-[#EEEADF] p-2.5 min-h-36 shadow-[inset_0_1px_2px_rgba(26,24,21,0.07)] transition data-active:ring-1 data-active:ring-[#6C9984]/60 data-active:bg-[#6C9984]/12",
                    div { class: "mb-1 flex items-center justify-between px-1",
                        p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#7A776C]",
                            "{name}"
                        }
                        if col == DOING {
                            span { class: if count(DOING) >= WIP { "rounded-full bg-[#7A3E25] px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-white" } else { "rounded-full bg-[#E4ECDD] px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-[#1C4A38] ring-1 ring-[#CFDDCF]" },
                                "{count(DOING)}/{WIP}"
                            }
                        } else {
                            span { class: "min-w-5 rounded-full bg-[#E4ECDD] px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums text-[#1C4A38] ring-1 ring-[#CFDDCF]",
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
                            key: "{card.id}",
                            item: card.clone(),
                            column: col,
                            index: ix,
                            label: card.title.clone(),
                            class: ITEM,
                            div { class: ROW,
                                span { class: "h-7 w-1 shrink-0 rounded-full {swatch(card.id)}" }
                                div { class: "min-w-0 flex-1",
                                    div { class: "truncate font-medium text-[#1A1815]",
                                        "{card.title}"
                                    }
                                    div { class: "truncate text-[11px] text-[#7A776C]",
                                        "{card.sub}"
                                    }
                                }
                                span { class: "grid h-6 w-6 shrink-0 place-items-center rounded-full bg-[#7A776C]/10 text-[9px] font-bold uppercase text-[#12362A] ring-1 ring-[#D7D4C9]",
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

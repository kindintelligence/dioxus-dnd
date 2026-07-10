//! Cross-container moves - the kanban pattern. Items travel between columns
//! (and optionally to a position within a column) via the shared
//! [`crate::core::DndContext`].
//!
//! The payload type flowing through the context is [`BoardPayload<T>`], which
//! remembers where the item came from. Wrap your app (or board) in
//! `DndProvider::<BoardPayload<Card>>`.
//!
//! ```text
//! DndProvider::<BoardPayload<Card>> {
//!     for (col_id, cards) in columns {
//!         BoardColumn::<Card> {
//!             id: col_id,
//!             on_move: move |mv: MoveEvent<Card>| {
//!                 apply_move(&mut board.write(), &mv);
//!             },
//!             for (ix, card) in cards.iter().enumerate() {
//!                 BoardItem::<Card> { item: card.clone(), column: col_id, index: ix,
//!                     CardView { card: card.clone() }
//!                 }
//!             }
//!         }
//!     }
//! }
//! ```

use std::collections::HashMap;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::core::{
    use_dnd, use_zone_id, use_zone_registry, Draggable, DropOutcome, DropZone, ParentZone, ZoneId,
    ZoneRecord,
};

/// Columns are just zones.
pub type ContainerId = ZoneId;

/// What travels through the context while a board item is dragged.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardPayload<T> {
    pub item: T,
    /// Column the item was picked up from.
    pub from: ContainerId,
    /// Index within that column.
    pub index: usize,
}

/// Context a [`BoardColumn`] provides so nested [`BoardSlot`]s inherit its
/// acceptance filter (WIP limits) with no extra wiring - a precise-insert slot
/// then honors the same limit as an append to the column.
struct ColumnAccepts<T: Clone + 'static>(Option<Callback<BoardPayload<T>, bool>>);

// Manual impls: `derive` would demand `T: Copy`, but the field is just a
// `Callback` handle (Copy) wrapped in an `Option`.
impl<T: Clone + 'static> Clone for ColumnAccepts<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone + 'static> Copy for ColumnAccepts<T> {}

/// A completed cross-container move.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveEvent<T> {
    pub item: T,
    /// `(column, index)` the item came from.
    pub from: (ContainerId, usize),
    /// Target column, and target index - `None` means "append to the end".
    pub to: (ContainerId, Option<usize>),
}

/// Apply a [`MoveEvent`] to a `HashMap<ContainerId, Vec<T>>` board model.
/// Removes from the source (by index, falling back gracefully if the model
/// drifted) and inserts at the target position.
pub fn apply_move<T>(board: &mut HashMap<ContainerId, Vec<T>>, mv: MoveEvent<T>) {
    let (from_col, from_ix) = mv.from;
    let mut removed = false;
    if let Some(src) = board.get_mut(&from_col) {
        if from_ix < src.len() {
            src.remove(from_ix);
            removed = true;
        }
    }
    let (to_col, to_ix) = mv.to;
    let adjusted_to_ix = match to_ix {
        Some(ix) if removed && from_col == to_col && from_ix < ix => Some(ix - 1),
        other => other,
    };
    let dst = board.entry(to_col).or_default();
    match adjusted_to_ix {
        Some(ix) if ix <= dst.len() => dst.insert(ix, mv.item),
        _ => dst.push(mv.item),
    }
}

/// A draggable card living in a column. Thin wrapper over
/// [`crate::core::Draggable`] that packs origin info into the payload.
#[component]
pub fn BoardItem<T: Clone + PartialEq + 'static>(
    item: T,
    /// Column this item currently lives in.
    column: ContainerId,
    /// Index within the column.
    index: usize,
    /// Label for screen-reader announcements.
    #[props(default)]
    label: Option<String>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        Draggable::<BoardPayload<T>> {
            payload: BoardPayload { item, from: column, index },
            zone: column,
            label,
            attributes,
            {children}
        }
    }
}

/// A column that receives [`BoardItem`]s. Emits [`MoveEvent`] with
/// `to.1 = None` (append). For precise within-column positions, nest
/// [`BoardSlot`]s between items.
#[component]
pub fn BoardColumn<T: Clone + PartialEq + 'static>(
    id: ContainerId,
    /// Human label for screen-reader announcements ("Over {label}").
    #[props(default)]
    label: Option<String>,
    on_move: EventHandler<MoveEvent<T>>,
    /// Reject payloads (e.g. WIP limits). Receives the full payload.
    #[props(default)]
    accepts: Option<Callback<BoardPayload<T>, bool>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    // Share the column's acceptance filter with any nested `BoardSlot`s so
    // precise inserts respect the same WIP limit as an append.
    use_context_provider(|| ColumnAccepts(accepts));
    rsx! {
        DropZone::<BoardPayload<T>> {
            id,
            label,
            accepts,
            on_drop: move |outcome: DropOutcome<BoardPayload<T>>| {
                let p = outcome.payload;
                on_move.call(MoveEvent {
                    item: p.item,
                    from: (p.from, p.index),
                    to: (id, None),
                });
            },
            attributes,
            {children}
        }
    }
}

/// An insertion point between items in a column. Dropping on it produces a
/// `MoveEvent` targeting exactly `(column, Some(index))`.
///
/// Stop-gap-free precise ordering: render one slot before each item and one
/// at the end. While a drag is in flight the slot carries
/// `data-active="true"` (absent otherwise) - style it visible then, e.g.
/// Tailwind `h-0 data-active:h-2`.
#[component]
pub fn BoardSlot<T: Clone + PartialEq + 'static>(
    /// The column this slot belongs to.
    column: ContainerId,
    /// The index an item dropped here should be inserted at.
    index: usize,
    /// Human label for screen-reader announcements.
    #[props(default)]
    label: Option<String>,
    on_move: EventHandler<MoveEvent<T>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let dnd = use_dnd::<BoardPayload<T>>();
    let mut registry = use_zone_registry::<BoardPayload<T>>();
    let zone_id = use_zone_id();
    let parent = try_use_context::<ParentZone>().map(|p| p.0);
    // The enclosing column's acceptance filter (WIP limits), inherited via
    // context so a precise-insert honors the same limit as an append. The
    // `Callback` is a stable handle whose closure reads live state at call
    // time, so capturing it once (below) still sees the current column.
    let column_accepts = try_use_context::<ColumnAccepts<T>>().and_then(|c| c.0);
    let accepts = move |p: BoardPayload<T>| column_accepts.map(|cb| cb.call(p)).unwrap_or(true);

    // `index` is positional - it shifts as items move above this slot - so the
    // registered drop must read the *current* props, not the ones captured when
    // the zone first registered. Mirror them through signals.
    let mut column_now = use_signal(|| column);
    let mut index_now = use_signal(|| index);
    let mut on_move_now = use_signal(|| on_move);
    if *column_now.peek() != column {
        column_now.set(column);
    }
    if *index_now.peek() != index {
        index_now.set(index);
    }
    if *on_move_now.peek() != on_move {
        on_move_now.set(on_move);
    }

    let slot_label = label
        .clone()
        .or_else(|| Some(format!("Insert at position {index}")));

    let registered_accepts = Callback::new(move |p: BoardPayload<T>| accepts(p));
    let registered_drop = Callback::new(move |outcome: DropOutcome<BoardPayload<T>>| {
        let p = outcome.payload;
        if !accepts(p.clone()) {
            return;
        }
        on_move_now.peek().call(MoveEvent {
            item: p.item,
            from: (p.from, p.index),
            to: (*column_now.peek(), Some(*index_now.peek())),
        });
    });
    let registered_label = slot_label.clone();
    let registration = use_hook(move || {
        registry.register(ZoneRecord {
            id: zone_id,
            parent,
            label: registered_label.clone(),
            on_drop: registered_drop,
            accepts: Some(registered_accepts),
            mounted: None,
            rect: None,
        })
    });
    use_drop(move || {
        registry.unregister(zone_id);
    });
    registry.sync_label(zone_id, slot_label);

    // Does the in-flight payload pass the inherited column filter?
    let acceptable = move || dnd.payload().map(accepts).unwrap_or(false);

    rsx! {
        div {
            "data-active": if acceptable() { "true" },
            "data-over": if dnd.over() == Some(zone_id) && acceptable() { "true" },
            onmounted: move |evt: Event<MountedData>| {
                let mut registry = registry;
                registry.set_mounted(registration, evt.data());
            },
            ..attributes,
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_between_columns() {
        let a = crate::core::ZoneId(1);
        let b = crate::core::ZoneId(2);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        board.insert(a, vec!["x", "y"]);
        board.insert(b, vec!["z"]);

        // precise insert at index 0 of column b
        apply_move(
            &mut board,
            MoveEvent {
                item: "y",
                from: (a, 1),
                to: (b, Some(0)),
            },
        );
        assert_eq!(board[&a], vec!["x"]);
        assert_eq!(board[&b], vec!["y", "z"]);

        // append (None index) into a brand-new column
        let c = crate::core::ZoneId(3);
        apply_move(
            &mut board,
            MoveEvent {
                item: "x",
                from: (a, 0),
                to: (c, None),
            },
        );
        assert!(board[&a].is_empty());
        assert_eq!(board[&c], vec!["x"]);
    }

    #[test]
    fn move_within_column_adjusts_forward_insert_after_removal() {
        let a = crate::core::ZoneId(1);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        board.insert(a, vec!["a", "b", "c", "d"]);

        apply_move(
            &mut board,
            MoveEvent {
                item: "a",
                from: (a, 0),
                to: (a, Some(3)),
            },
        );

        assert_eq!(board[&a], vec!["b", "c", "a", "d"]);
    }

    #[test]
    fn move_within_column_keeps_backward_insert_index() {
        let a = crate::core::ZoneId(1);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        board.insert(a, vec!["a", "b", "c", "d"]);

        apply_move(
            &mut board,
            MoveEvent {
                item: "d",
                from: (a, 3),
                to: (a, Some(1)),
            },
        );

        assert_eq!(board[&a], vec!["a", "d", "b", "c"]);
    }

    #[test]
    fn move_within_column_appends_after_removal() {
        let a = crate::core::ZoneId(1);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        board.insert(a, vec!["a", "b", "c"]);

        apply_move(
            &mut board,
            MoveEvent {
                item: "a",
                from: (a, 0),
                to: (a, None),
            },
        );

        assert_eq!(board[&a], vec!["b", "c", "a"]);
    }
}

//! Cross-container moves — the kanban pattern. Items travel between columns
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

use dioxus::prelude::*;

use crate::core::{use_dnd, DropOutcome, DropZone, ZoneId};
use crate::pointer::PointerDraggable;

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

/// A completed cross-container move.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveEvent<T> {
    pub item: T,
    /// `(column, index)` the item came from.
    pub from: (ContainerId, usize),
    /// Target column, and target index — `None` means "append to the end".
    pub to: (ContainerId, Option<usize>),
}

/// Apply a [`MoveEvent`] to a `HashMap<ContainerId, Vec<T>>` board model.
/// Removes from the source (by index, falling back gracefully if the model
/// drifted) and inserts at the target position.
pub fn apply_move<T>(board: &mut HashMap<ContainerId, Vec<T>>, mv: MoveEvent<T>) {
    let (from_col, from_ix) = mv.from;
    if let Some(src) = board.get_mut(&from_col) {
        if from_ix < src.len() {
            src.remove(from_ix);
        }
    }
    let (to_col, to_ix) = mv.to;
    let dst = board.entry(to_col).or_default();
    match to_ix {
        Some(ix) if ix <= dst.len() => dst.insert(ix, mv.item),
        _ => dst.push(mv.item),
    }
}

/// A draggable card living in a column. Thin wrapper over
/// [`crate::pointer::PointerDraggable`] (so cards work with mouse, touch,
/// pen and keyboard) that packs origin info into the payload.
#[component]
pub fn BoardItem<T: Clone + PartialEq + 'static>(
    item: T,
    /// Column this item currently lives in.
    column: ContainerId,
    /// Index within the column.
    index: usize,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        PointerDraggable::<BoardPayload<T>> {
            payload: BoardPayload { item, from: column, index },
            zone: column,
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
/// at the end.
#[component]
pub fn BoardSlot<T: Clone + PartialEq + 'static>(
    /// The column this slot belongs to.
    column: ContainerId,
    /// The index an item dropped here should be inserted at.
    index: usize,
    on_move: EventHandler<MoveEvent<T>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let dnd = use_dnd::<BoardPayload<T>>();

    rsx! {
        div {
            "data-active": dnd.dragging(),
            ondragover: move |evt: DragEvent| {
                if dnd.dragging() {
                    evt.prevent_default();
                }
            },
            ondrop: {
                let mut dnd = dnd;
                move |evt: DragEvent| {
                    evt.prevent_default();
                    evt.stop_propagation();
                    if let Some((p, _)) = dnd.take() {
                        on_move.call(MoveEvent {
                            item: p.item,
                            from: (p.from, p.index),
                            to: (column, Some(index)),
                        });
                    }
                }
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
}

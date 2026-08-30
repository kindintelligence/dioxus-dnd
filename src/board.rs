#![doc = include_str!("../docs/api/boards.md")]

use std::collections::HashMap;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::core::{
    use_dnd, use_joined_window, use_parent_zone, use_zone_id, use_zone_registry, Draggable,
    DropEffect, DropOutcome, DropZone, ZoneId, ZoneRecord,
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
struct ColumnAccepts<T: Clone + 'static>(Callback<BoardPayload<T>, bool>);

// Manual impls: `derive` would demand `T: Copy`, but the field is just a
// `Callback` handle (Copy) wrapped in an `Option`.
impl<T: Clone + 'static> Clone for ColumnAccepts<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone + 'static> Copy for ColumnAccepts<T> {}

/// A completed cross-container move.
///
/// Non-exhaustive so move context can be added without a major release;
/// synthesize your own (tests, undo stacks) via [`MoveEvent::new`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MoveEvent<T> {
    pub item: T,
    /// `(column, index)` the item came from.
    pub from: (ContainerId, usize),
    /// Target column, and target index - `None` means "append to the end".
    pub to: (ContainerId, Option<usize>),
}

impl<T> MoveEvent<T> {
    /// A move of `item` from `(column, index)` to `(column, index)`, where
    /// a `None` target index means "append to the end".
    pub fn new(item: T, from: (ContainerId, usize), to: (ContainerId, Option<usize>)) -> Self {
        Self { item, from, to }
    }
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
    /// Stable identity for this column.
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
    let column_id = id;
    // Share the column's acceptance filter with any nested `BoardSlot`s so
    // precise inserts respect the same WIP limit as an append.
    let inherited_accepts =
        use_callback(move |payload| accepts.map(|cb| cb.call(payload)).unwrap_or(true));
    use_context_provider(|| ColumnAccepts(inherited_accepts));
    rsx! {
        DropZone::<BoardPayload<T>> {
            id: column_id,
            label,
            accepts,
            on_drop: move |outcome: DropOutcome<BoardPayload<T>>| {
                let p = outcome.payload;
                on_move.call(MoveEvent {
                    item: p.item,
                    from: (p.from, p.index),
                    to: (column_id, None),
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
/// `data-active="true"` (absent otherwise) - reveal it without moving
/// layout, e.g. Tailwind `h-2 opacity-0 data-active:opacity-100`. Growing
/// the slot itself (`h-0` to `h-2`) reflows the column mid-drag and strands
/// the cached zone rects.
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
    let joined = use_joined_window::<BoardPayload<T>>();
    let mut registry = use_zone_registry::<BoardPayload<T>>();
    let zone_id = use_zone_id();
    let parent = use_parent_zone();
    // The enclosing column's acceptance filter (WIP limits), inherited via
    // context so a precise-insert honors the same limit as an append. The
    // `Callback` is a stable handle whose closure reads live state at call
    // time, so capturing it once (below) still sees the current column.
    let column_accepts = try_use_context::<ColumnAccepts<T>>();
    let accepts = use_callback(move |payload: BoardPayload<T>| {
        column_accepts
            .map(|accepts| accepts.0.call(payload))
            .unwrap_or(true)
    });

    let slot_label = label
        .clone()
        .or_else(|| Some(format!("Insert at position {index}")));

    let registered_accepts = accepts;
    let registered_drop = use_callback(move |outcome: DropOutcome<BoardPayload<T>>| {
        let p = outcome.payload;
        if !accepts.call(p.clone()) {
            return;
        }
        on_move.call(MoveEvent {
            item: p.item,
            from: (p.from, p.index),
            to: (column, Some(index)),
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
        registry.unregister_registration(registration);
    });
    let label_for_sync = slot_label.clone();
    use_effect(use_reactive!(|(label_for_sync)| {
        registry.sync_label(zone_id, label_for_sync);
    }));
    use_effect(use_reactive!(|(parent)| {
        registry.sync_parent(registration, parent);
    }));

    // Does the in-flight payload pass the inherited column filter?
    let acceptable = move || {
        (dnd.proposed_effect() != DropEffect::None)
            && dnd
                .payload()
                .map(|payload| accepts.call(payload))
                .unwrap_or(false)
    };
    let is_over = move || match joined {
        Some(joined) => joined.is_over(zone_id),
        None => dnd.over() == Some(zone_id),
    };
    let mut attributes = attributes;
    crate::core::components::protect_attributes(
        &mut attributes,
        &["data-active", "data-over", "onmounted"],
    );

    rsx! {
        div {
            "data-active": if acceptable() { "true" },
            "data-over": if is_over() && acceptable() { "true" },
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

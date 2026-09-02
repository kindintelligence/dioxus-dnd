#![doc = include_str!("../docs/api/boards.md")]

use std::collections::HashMap;
use std::fmt;

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
/// Removes from the source **by index** and inserts at the target position.
///
/// The index is trusted: if the source column changed between pickup and
/// drop, whichever item now sits at `from.1` is removed and the moved item
/// is still inserted. An out-of-range index or missing column skips the
/// removal, so the item is never lost - but it may be duplicated. When the
/// model can change under a live drag, use [`try_apply_move`], which finds
/// the item by key and refuses to guess.
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

/// A checked board helper refused to mutate the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApplyMoveError {
    /// No item in the source column matched the moved item's key. The board
    /// was left untouched.
    SourceNotFound { column: ContainerId },
}

impl fmt::Display for ApplyMoveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceNotFound { column } => {
                write!(formatter, "no item matched the move in column {}", column.0)
            }
        }
    }
}

impl std::error::Error for ApplyMoveError {}

/// Checked, identity-based form of [`apply_move`].
///
/// The moved item is located in the source column by `key` and removed from
/// wherever it now sits; `from.1` is not consulted. The same-column forward
/// adjustment uses the index the item was actually found at, so a column
/// that reordered during the drag still lands the item at the intended
/// slot. When no item in the source column matches, the board is left
/// untouched and [`ApplyMoveError::SourceNotFound`] is returned - the drop
/// is stale, and silently inserting would duplicate the item.
///
/// Target handling matches `apply_move`: `None` or an index past the end
/// appends, and a missing target column is created.
pub fn try_apply_move<T, K>(
    board: &mut HashMap<ContainerId, Vec<T>>,
    mv: MoveEvent<T>,
    key: impl Fn(&T) -> K,
) -> Result<(), ApplyMoveError>
where
    K: PartialEq,
{
    let MoveEvent {
        item,
        from: (from_col, _),
        to: (to_col, to_ix),
    } = mv;
    let item_key = key(&item);
    let source = board
        .get_mut(&from_col)
        .ok_or(ApplyMoveError::SourceNotFound { column: from_col })?;
    let Some(removed_ix) = source
        .iter()
        .position(|candidate| key(candidate) == item_key)
    else {
        return Err(ApplyMoveError::SourceNotFound { column: from_col });
    };
    source.remove(removed_ix);
    let adjusted_to_ix = match to_ix {
        Some(ix) if from_col == to_col && removed_ix < ix => Some(ix - 1),
        other => other,
    };
    let destination = board.entry(to_col).or_default();
    match adjusted_to_ix {
        Some(ix) if ix <= destination.len() => destination.insert(ix, item),
        _ => destination.push(item),
    }
    Ok(())
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

    /// The index-trusting helper's documented hazard, pinned so the checked
    /// variant's reason to exist stays visible: a column that reordered
    /// under the drag makes `apply_move` remove the wrong card.
    #[test]
    fn index_move_removes_whatever_sits_at_the_stale_index() {
        let a = crate::core::ZoneId(1);
        let b = crate::core::ZoneId(2);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        // "x" was picked up at index 0, then the column reordered.
        board.insert(a, vec!["y", "x"]);
        board.insert(b, vec![]);

        apply_move(&mut board, MoveEvent::new("x", (a, 0), (b, None)));

        assert_eq!(board[&a], vec!["x"], "the wrong card was removed");
        assert_eq!(board[&b], vec!["x"], "and the moved one now exists twice");
    }

    #[test]
    fn keyed_move_finds_the_item_after_source_drift() {
        let a = crate::core::ZoneId(1);
        let b = crate::core::ZoneId(2);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        board.insert(a, vec!["y", "x"]);
        board.insert(b, vec!["z"]);

        assert_eq!(
            try_apply_move(&mut board, MoveEvent::new("x", (a, 0), (b, Some(0))), |s| {
                *s
            }),
            Ok(())
        );
        assert_eq!(board[&a], vec!["y"]);
        assert_eq!(board[&b], vec!["x", "z"]);
    }

    #[test]
    fn keyed_move_adjusts_forward_inserts_from_the_found_index() {
        let a = crate::core::ZoneId(1);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        // The event says index 0; the item actually sits at index 1 now.
        board.insert(a, vec!["b", "a", "c", "d"]);

        assert_eq!(
            try_apply_move(&mut board, MoveEvent::new("a", (a, 0), (a, Some(3))), |s| {
                *s
            }),
            Ok(())
        );
        assert_eq!(board[&a], vec!["b", "c", "a", "d"]);

        // Backward moves keep the target as-is, like `apply_move`.
        assert_eq!(
            try_apply_move(&mut board, MoveEvent::new("d", (a, 9), (a, Some(0))), |s| {
                *s
            }),
            Ok(())
        );
        assert_eq!(board[&a], vec!["d", "b", "c", "a"]);
    }

    #[test]
    fn keyed_move_refuses_a_stale_drop_without_mutating() {
        let a = crate::core::ZoneId(1);
        let b = crate::core::ZoneId(2);
        let c = crate::core::ZoneId(3);
        let mut board: HashMap<ContainerId, Vec<&str>> = HashMap::new();
        board.insert(a, vec!["x"]);
        let before = board.clone();

        // Item gone from the source column.
        assert_eq!(
            try_apply_move(&mut board, MoveEvent::new("q", (a, 0), (b, None)), |s| *s),
            Err(ApplyMoveError::SourceNotFound { column: a })
        );
        // Source column gone entirely.
        assert_eq!(
            try_apply_move(&mut board, MoveEvent::new("x", (c, 0), (b, None)), |s| *s),
            Err(ApplyMoveError::SourceNotFound { column: c })
        );
        assert_eq!(board, before, "a refused move leaves the board untouched");
        assert!(!board.contains_key(&b), "and creates no target column");

        assert_eq!(
            ApplyMoveError::SourceNotFound { column: a }.to_string(),
            "no item matched the move in column 1"
        );
    }
}

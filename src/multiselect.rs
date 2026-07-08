//! Multi-select drag: select several items, drag them as one payload.
//!
//! The design leans on the core being generic: the payload type flowing
//! through the provider is simply `Vec<K>` - so wrap your app in
//! `DndProvider::<Vec<K>>` and your `DropZone`s receive the whole selection
//! in one `DropOutcome<Vec<K>>`.
//!
//! What this module adds is the interaction layer:
//! - [`use_selection`] - selection state with the usual click semantics
//!   (click selects one, Ctrl/Cmd+click toggles).
//! - [`SelectableDraggable`] - a `Draggable` that resolves its payload from
//!   the selection: dragging a *selected* item carries the whole selection;
//!   dragging an *unselected* item carries just that item.
//! - [`SelectionCount`] - a badge for your `DragOverlay` ghost ("3 items").
//!
//! ```text
//! let selection = use_selection::<FileId>();
//! rsx! {
//!     DndProvider::<Vec<FileId>> {
//!         for file in files {
//!             SelectableDraggable::<FileId> {
//!                 key: "{file.id.0}",
//!                 item: file.id,
//!                 selection,
//!                 FileRow { file }
//!             }
//!         }
//!         DropZone::<Vec<FileId>> {
//!             on_drop: move |o: DropOutcome<Vec<FileId>>| trash(o.payload),
//!             "Trash"
//!         }
//!         DragOverlay::<Vec<FileId>> { SelectionCount::<FileId> {} }
//!     }
//! }
//! ```

use dioxus::prelude::*;

use crate::core::{use_dnd, Draggable, DropEffect, ZoneId};

/// Selection state for keys of type `K`. Cheap to copy.
pub struct Selection<K: Clone + PartialEq + 'static> {
    items: Signal<Vec<K>>,
}

impl<K: Clone + PartialEq + 'static> Copy for Selection<K> {}
impl<K: Clone + PartialEq + 'static> Clone for Selection<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: Clone + PartialEq + 'static> PartialEq for Selection<K> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl<K: Clone + PartialEq + 'static> Selection<K> {
    /// Wrap an existing signal. Prefer [`use_selection`].
    pub fn from_signal(items: Signal<Vec<K>>) -> Self {
        Self { items }
    }

    /// Is `key` currently selected?
    pub fn is_selected(&self, key: &K) -> bool {
        self.items.read().contains(key)
    }

    /// Replace the selection with just `key`.
    pub fn select_only(&mut self, key: K) {
        self.items.set(vec![key]);
    }

    /// Add or remove `key` (Ctrl/Cmd+click semantics).
    pub fn toggle(&mut self, key: K) {
        let mut items = self.items.write();
        if let Some(ix) = items.iter().position(|k| *k == key) {
            items.remove(ix);
        } else {
            items.push(key);
        }
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.items.write().clear();
    }

    /// Snapshot of the selected keys, in selection order.
    pub fn items(&self) -> Vec<K> {
        self.items.read().clone()
    }

    /// Number of selected keys.
    pub fn len(&self) -> usize {
        self.items.read().len()
    }

    /// Is nothing selected?
    pub fn is_empty(&self) -> bool {
        self.items.read().is_empty()
    }

    /// Apply the standard click convention: plain click selects only this
    /// key; a click with Ctrl or Cmd held toggles it.
    pub fn click(&mut self, key: K, modifiers: Modifiers) {
        if modifiers.contains(Modifiers::CONTROL) || modifiers.contains(Modifiers::META) {
            self.toggle(key);
        } else {
            self.select_only(key);
        }
    }
}

/// Create selection state owned by this component.
pub fn use_selection<K: Clone + PartialEq + 'static>() -> Selection<K> {
    Selection {
        items: use_signal(Vec::new),
    }
}

/// A draggable list/grid item participating in a selection.
///
/// - Click / Ctrl+click manage the selection (via [`Selection::click`]).
/// - Dragging a selected item picks up **the whole selection**; dragging an
///   unselected one picks up just that item (and selects it).
/// - Works with mouse, touch, pen and keyboard.
/// - The wrapper exposes `data-selected="true"` for styling (absent when
///   unselected, so presence-based selectors like Tailwind
///   `data-selected:ring-2` work directly).
///
/// Requires a `DndProvider::<Vec<K>>` ancestor.
#[component]
pub fn SelectableDraggable<K: Clone + PartialEq + 'static>(
    /// This item's key.
    item: K,
    /// Shared selection state from [`use_selection`].
    selection: Selection<K>,
    /// The zone this item lives in.
    #[props(default)]
    zone: Option<ZoneId>,
    /// Drop effect. Defaults to `Move`.
    #[props(default)]
    effect: DropEffect,
    /// Label for screen-reader announcements.
    #[props(default)]
    label: Option<String>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let _ = use_dnd::<Vec<K>>(); // fail fast with a clear panic if unprovided
    let selected = selection.is_selected(&item);
    // Payload resolved from *current* selection each render: a selected item
    // drags the group, an unselected one drags itself.
    let payload = if selected {
        selection.items()
    } else {
        vec![item.clone()]
    };
    let click_key = item.clone();
    let mut selection = selection;

    rsx! {
        div {
            "data-selected": if selected { "true" },
            onclick: move |evt: MouseEvent| {
                selection.click(click_key.clone(), evt.modifiers());
            },
            ..attributes,
            Draggable::<Vec<K>> {
                payload,
                zone,
                effect,
                label,
                {children}
            }
        }
    }
}

/// A "N items" badge for the drag ghost. Render inside
/// `DragOverlay::<Vec<K>>`; shows the size of the payload being dragged.
#[component]
pub fn SelectionCount<K: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<K>,
) -> Element {
    let _ = phantom;
    let dnd = use_dnd::<Vec<K>>();
    let n = dnd.payload().map(|p| p.len()).unwrap_or(0);
    rsx! {
        span { "{n} item(s)" }
    }
}

//! Accessibility helpers.
//!
//! The keyboard interaction itself is built into the core
//! [`crate::core::Draggable`] - every draggable is focusable and operable
//! with Space/Enter (pick up / drop), arrow keys (choose a drop target from
//! the registered zones) and Escape (cancel). What this module adds is the
//! **voice**: a visually-hidden `aria-live` region that reads the context's
//! announcement channel to screen readers.
//!
//! Render exactly one `LiveRegion` per provider, anywhere in the subtree:
//!
//! ```text
//! DndProvider::<Card> {
//!     LiveRegion::<Card> {}
//!     // ... draggables and zones ...
//! }
//! ```
//!
//! Give `Draggable` a `label` and `DropZone` a `label` for meaningful
//! announcements ("Picked up Ship it. …", "Over Done."). Custom flows can
//! push their own messages with [`crate::core::DndContext::announce`].

use dioxus::prelude::*;

use crate::core::use_dnd;

/// Visually-hidden `aria-live="polite"` region voicing drag announcements.
#[component]
pub fn LiveRegion<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
) -> Element {
    let _ = phantom;
    let dnd = use_dnd::<T>();
    let text = dnd.announcement();

    rsx! {
        div {
            aria_live: "polite",
            aria_atomic: "true",
            role: "status",
            // Standard visually-hidden recipe: present to the accessibility
            // tree, invisible on screen.
            style: "position: absolute; width: 1px; height: 1px; padding: 0; \
                    margin: -1px; overflow: hidden; clip: rect(0 0 0 0); \
                    white-space: nowrap; border: 0;",
            "{text}"
        }
    }
}

/// Headless move-up / move-down buttons - the most robust accessibility
/// fallback of all: reordering with plain button presses, no drag gesture
/// (pointer *or* keyboard-drag) required.
///
/// Renders two `button`s with ARIA labels, disabled at the list edges, and
/// `data-reorder="up" | "down"` hooks for styling. Emits the same
/// [`crate::sortable::SortEvent`] your drag path already handles, so one
/// `on_sort` serves both inputs.
///
/// ```text
/// SortableList {
///     len: items.read().len(),
///     render: move |ix: usize| rsx! {
///         span { "{items.read()[ix]}" }
///         ReorderButtons { index: ix, total: items.read().len(), on_sort }
///     },
///     on_sort,
/// }
/// ```
#[component]
pub fn ReorderButtons(
    /// This row's index.
    index: usize,
    /// Total number of rows.
    total: usize,
    /// Accessible name of the item, used in the button labels.
    #[props(default)]
    label: Option<String>,
    /// Fired with the same event shape as drag-reordering.
    on_sort: EventHandler<crate::sortable::SortEvent>,
    #[props(extends = span, extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let name = label.unwrap_or_else(|| format!("item {}", index + 1));
    let up_label = format!("Move {name} up");
    let down_label = format!("Move {name} down");

    rsx! {
        span {
            ..attributes,
            button {
                r#type: "button",
                "data-reorder": "up",
                aria_label: "{up_label}",
                disabled: index == 0,
                onclick: move |evt| {
                    evt.stop_propagation();
                    if index > 0 {
                        on_sort.call(crate::sortable::SortEvent { from: index, to: index - 1 });
                    }
                },
                "↑"
            }
            button {
                r#type: "button",
                "data-reorder": "down",
                aria_label: "{down_label}",
                disabled: index + 1 >= total,
                onclick: move |evt| {
                    evt.stop_propagation();
                    if index + 1 < total {
                        on_sort.call(crate::sortable::SortEvent { from: index, to: index + 1 });
                    }
                },
                "↓"
            }
        }
    }
}

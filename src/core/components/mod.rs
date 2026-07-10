//! Ready-made components over the shared drag context.
//!
//! ```text
//! rsx! {
//!     DndProvider::<Card> {
//!         Draggable::<Card> { payload: card.clone(), "Drag me" }
//!         DropZone::<Card> {
//!             on_drop: move |outcome: DropOutcome<Card>| { /* ... */ },
//!             "Drop here"
//!         }
//!     }
//! }
//! ```

use dioxus::prelude::*;

mod delivery;
mod draggable;
mod drop_zone;
mod overlay;
mod pointer;
mod provider;

pub use draggable::Draggable;
pub use drop_zone::{BridgeDropZone, DropZone, ParentZone};
pub use overlay::{DragOverlay, SettleSlot};
pub use provider::DndProvider;

pub(crate) use delivery::{deliver_drop, DropCompletion, RELEASE_RECOVERY_MOVES};
pub(crate) use overlay::overlay_style;
pub(crate) use pointer::{primary_press, touch_style, HoldTimer};

/// Pull a user-provided `style` out of forwarded attributes and append it to
/// a functional inline style. Spread attributes land after static ones and
/// replace them wholesale, so without this a caller passing any `style`
/// would silently delete functional CSS (`touch-action`, overlay
/// positioning). The user's declarations come last, so they still win on a
/// per-property basis.
pub(crate) fn merge_style(attributes: &mut Vec<Attribute>, functional: &str) -> String {
    let user = attributes
        .iter()
        .position(|a| a.name == "style")
        .map(|i| attributes.remove(i));
    match user.map(|a| a.value) {
        Some(dioxus::core::AttributeValue::Text(s)) => format!("{functional} {s}"),
        _ => functional.to_string(),
    }
}

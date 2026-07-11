//! The [`DndProvider`] component: provides a `DndContext<T>` to a subtree.

use dioxus::prelude::*;

use crate::core::hooks::{use_dnd_provider, use_zone_registry};
use crate::core::types::Direction;

/// Provides a `DndContext<T>` to its children.
#[component]
pub fn DndProvider<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
    /// Layout direction: `Direction::Rtl` mirrors keyboard navigation and
    /// spatial zone ordering to follow the visual right-to-left flow.
    #[props(default)]
    dir: Direction,
    children: Element,
) -> Element {
    let _ = phantom;
    use_dnd_provider::<T>();
    // Synced every render (a compare-and-set no-op when unchanged), so a
    // live direction switch propagates.
    use_zone_registry::<T>().set_direction(dir);
    rsx! {
        {children}
    }
}

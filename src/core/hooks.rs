//! Hooks for providing and consuming the drag context.

use dioxus::prelude::*;

use super::registry::ZoneRegistry;
use super::state::{DndContext, DragState};
use super::types::{Point, ZoneId};

/// Provide a `DndContext<T>` (and its zone registry) to this component's
/// subtree. Call once, high up (or use the
/// [`crate::core::components::DndProvider`] component).
pub fn use_dnd_provider<T: Clone + 'static>() -> DndContext<T> {
    let state = use_store(DragState::<T>::default);
    let announcement = use_signal(String::new);
    use_context_provider(|| ZoneRegistry::<T>::from_signal(Signal::new(Vec::new())));
    use_context_provider(move || DndContext::from_parts(state, announcement))
}

/// Grab the nearest `DndContext<T>` from context.
///
/// # Panics
/// Panics if no ancestor provided a context for this payload type.
pub fn use_dnd<T: Clone + 'static>() -> DndContext<T> {
    use_context()
}

/// Grab the zone registry (mounted drop zones, in order). Provided alongside
/// the context by [`use_dnd_provider`].
pub fn use_zone_registry<T: Clone + 'static>() -> ZoneRegistry<T> {
    use_context()
}

/// A stable, auto-generated [`ZoneId`] for this component instance.
pub fn use_zone_id() -> ZoneId {
    use_hook(ZoneId::auto)
}

/// Client (viewport) coordinates of a native drag event as a [`Point`].
/// In-app drags don't produce `DragEvent`s; this serves the boundary
/// modules ([`crate::files`], [`crate::external`]) and custom native zones.
pub fn client_point(evt: &DragEvent) -> Point {
    let c = evt.client_coordinates();
    Point::new(c.x, c.y)
}

/// Element-relative coordinates of a native drag event as a [`Point`].
/// See [`client_point`] for when these apply.
pub fn element_point(evt: &DragEvent) -> Point {
    let c = evt.element_coordinates();
    Point::new(c.x, c.y)
}

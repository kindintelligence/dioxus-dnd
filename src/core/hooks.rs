//! Hooks for providing and consuming the drag context.

use dioxus::prelude::*;

use super::registry::{RectRefresh, ZoneRegistry};
use super::state::{DndContext, DragState};
use super::types::{DragId, Point, ZoneId};

/// Marker flag: a settle-enabled `DragOverlay<T>` is mounted somewhere in
/// this provider's subtree, so `Draggable<T>` should route successful
/// pointer drops through [`DndContext::take_settling`] instead of
/// [`DndContext::take`]. Typed so nested providers of different payloads
/// can't arm each other.
pub(crate) struct SettleFlag<T> {
    pub(crate) armed: Signal<bool>,
    marker: std::marker::PhantomData<T>,
}

impl<T> Copy for SettleFlag<T> {}
impl<T> Clone for SettleFlag<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Provide a `DndContext<T>` (and its zone registry) to this component's
/// subtree. Call once, high up (or use the
/// [`crate::core::components::DndProvider`] component).
pub fn use_dnd_provider<T: Clone + 'static>() -> DndContext<T> {
    let state = use_store(DragState::<T>::default);
    let announcement = use_signal(String::new);
    let registry = use_context_provider(|| ZoneRegistry::<T>::from_signal(Signal::new(Vec::new())));
    let ctx = use_context_provider(move || DndContext::from_parts(state, announcement));
    use_context_provider(|| SettleFlag::<T> {
        armed: Signal::new(false),
        marker: std::marker::PhantomData,
    });

    // One rect-refresh channel per provider *tree*: the outermost provider
    // creates it, nested providers inherit and re-provide the same one. A
    // scroll surface anywhere below then reaches every registry above it
    // through a single type-erased handle.
    use_rect_refresh_provider();
    // Re-measure this registry on ping - but only mid-drag. Rects are
    // measured fresh at every pickup, so an idle provider has nothing to
    // keep current, and the gate makes scroll-event pings free while idle.
    use_rect_refresh_thunk(move |_| {
        if ctx.dragging() {
            registry.refresh_rects();
        }
    });

    ctx
}

/// Create-or-inherit the tree's [`RectRefresh`] channel and provide it to
/// descendants. The outermost participant (a `DndProvider`, an
/// [`crate::autoscroll::AutoScroll`]) owns the signal; everyone below
/// shares it, so self-contained components like `SortableList` can join
/// even with no provider anywhere.
pub(crate) fn use_rect_refresh_provider() -> RectRefresh {
    let bus = use_hook(|| {
        // Plain context lookup (not the memoizing hook - we're inside one).
        try_consume_context::<RectRefresh>()
            .unwrap_or_else(|| RectRefresh::from_signal(Signal::new(Vec::new())))
    });
    use_context_provider(|| bus);
    bus
}

/// Register a re-measure thunk on the tree's channel for this component's
/// lifetime; it leaves the channel on unmount. Quietly does nothing when no
/// channel exists above (nothing could ever ping it). The thunk must gate
/// itself on its own drag state - pings arrive for every scroll.
pub(crate) fn use_rect_refresh_thunk(thunk: impl FnMut(()) + 'static) {
    let joined = use_hook(move || {
        try_consume_context::<RectRefresh>().map(|mut bus| {
            let key = DragId::auto().0;
            bus.register(key, Callback::new(thunk));
            (bus, key)
        })
    });
    use_drop(move || {
        if let Some((mut bus, key)) = joined {
            bus.unregister(key);
        }
    });
}

/// The provider tree's [`RectRefresh`] channel: ping `refresh_all()` after
/// you move layout under a live drag (scrolling a custom container,
/// collapsing a panel) so hit-testing and `data-over` track the new
/// geometry. [`crate::autoscroll::AutoScroll`] pings it for you.
///
/// # Panics
/// Panics if no ancestor provided a drag context.
pub fn use_rect_refresh() -> RectRefresh {
    use_context()
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

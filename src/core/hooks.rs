//! Hooks for providing and consuming the drag context.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use super::registry::{RectRefresh, ZoneRecord, ZoneRegistration, ZoneRegistry};
use super::state::{DndContext, DragState};
use super::types::{DragId, DropOutcome, Point, Rect, ZoneId};
use super::world::{DndWorld, JoinedWindow, WindowGeometry, WorldMembership};

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
///
/// When a [`DndWorld<T>`] is in context (see
/// [`crate::core::world::use_dnd_world`]), the provider **joins** it
/// instead of creating isolated state: it re-provides the world's shared
/// context and registers this window's zones for cross-window drags.
/// Nested providers of the same `T` keep today's shadowing semantics -
/// only the outermost provider in a window joins.
pub fn use_dnd_provider<T: Clone + 'static>() -> DndContext<T> {
    // Fallback state, created unconditionally (hooks must be stable) and
    // simply unused when a world is joined.
    let state = use_store(DragState::<T>::default);
    let announcement = use_signal(String::new);
    let registry = use_context_provider(|| ZoneRegistry::<T>::from_signal(Signal::new(Vec::new())));
    let settle_flag = use_context_provider(|| SettleFlag::<T> {
        armed: Signal::new(false),
        marker: std::marker::PhantomData,
    });
    // World membership is decided once, at mount: a provider that finds a
    // world (and isn't nested under a provider of the same T) joins as one
    // window. `provide_context` inside the hook is deliberate - every
    // provider publishes a membership (even `None`), so nested providers
    // shadow their ancestors' membership exactly like they shadow contexts.
    let membership = use_hook(move || {
        let joined = try_consume_context::<DndWorld<T>>()
            .filter(|_| try_consume_context::<WorldMembership<T>>().is_none())
            .map(|world| {
                let geometry = try_consume_context::<WindowGeometry>().unwrap_or_default();
                let key = world.join(
                    geometry,
                    registry,
                    settle_flag,
                    Callback::new(move |_| registry.refresh_rects()),
                );
                JoinedWindow {
                    world,
                    key,
                    geometry,
                }
            });
        provide_context(WorldMembership::<T>(joined));
        joined
    });
    use_drop(move || {
        if let Some(j) = membership {
            j.world.leave(j.key);
        }
    });
    let ctx = use_context_provider(move || match membership {
        Some(j) => j.world.context(),
        None => DndContext::from_parts(state, announcement),
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

/// A plain, component-owned fan-out for one bridge element's geometry.
///
/// Create one with [`Default`] and pass a clone to every [`use_bridge_world`]
/// call for the element. Its mount and rect methods copy one DOM observation
/// into every joined provider-owned registry without creating Dioxus signals
/// or callbacks in the child scope.
#[derive(Clone, Default)]
pub struct BridgeGeometry {
    writers: Rc<RefCell<Vec<BridgeGeometryWriter>>>,
}

#[derive(Clone)]
struct BridgeGeometryWriter {
    mounted: Rc<dyn Fn(Rc<MountedData>)>,
    rect: Rc<dyn Fn(Rect)>,
}

impl std::fmt::Debug for BridgeGeometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeGeometry")
            .field("worlds", &self.writers.borrow().len())
            .finish()
    }
}

impl PartialEq for BridgeGeometry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.writers, &other.writers)
    }
}

impl BridgeGeometry {
    /// Copy the bridge element's mounted handle into every joined registry.
    pub fn set_mounted(&self, mounted: &Rc<MountedData>) {
        for writer in self.writers.borrow().iter() {
            (writer.mounted)(mounted.clone());
        }
    }

    /// Copy a completed bridge measurement into every registration that is
    /// still current.
    pub fn set_rect_if_present(&self, rect: Rect) {
        for writer in self.writers.borrow().iter() {
            (writer.rect)(rect);
        }
    }

    fn register<T: Clone + 'static>(
        &self,
        registry: ZoneRegistry<T>,
        registration: ZoneRegistration,
    ) {
        self.writers.borrow_mut().push(BridgeGeometryWriter {
            mounted: Rc::new(move |mounted| {
                let mut registry = registry;
                registry.set_mounted(registration, mounted);
            }),
            rect: Rc::new(move |rect| {
                let mut registry = registry;
                registry.set_rect_if_present(registration, rect);
            }),
        });
    }
}

/// Live, type-erased view of one payload world at a bridge zone, as returned
/// by [`use_bridge_world`] - so callers can OR any number of worlds together
/// without naming their `T`s again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeWorld {
    /// An acceptable drag of this world's payload is in flight.
    pub active: bool,
    /// That drag currently hovers this zone.
    pub over: bool,
}

/// Register `zone_id` as a drop target in `T`'s payload world and report
/// that world's live state this render.
///
/// This is the building block behind `BridgeDropZone` and the
/// [`crate::bridge_drop_zone!`] macro: call it once per coexisting provider
/// type with the same id and [`BridgeGeometry`]. Every registry owns its own
/// plain geometry copy, while each drop still arrives through its own typed
/// callback - no downcasts, no shared erased channel.
///
/// # Panics
/// Panics if no ancestor provided a `DndProvider<T>`.
pub fn use_bridge_world<T: Clone + PartialEq + 'static>(
    zone_id: ZoneId,
    parent: Option<ZoneId>,
    label: Option<String>,
    accepts: Option<Callback<T, bool>>,
    on_drop: EventHandler<DropOutcome<T>>,
    geometry: BridgeGeometry,
) -> BridgeWorld {
    let dnd = use_dnd::<T>();
    let mut reg = use_zone_registry::<T>();
    let registration = use_hook(|| {
        reg.register(ZoneRecord {
            id: zone_id,
            parent,
            label: label.clone(),
            on_drop: Callback::new(move |o| on_drop.call(o)),
            accepts,
            mounted: None,
            rect: None,
        })
    });
    use_drop(move || reg.unregister(zone_id));
    reg.sync_label(zone_id, label);
    use_hook(move || {
        geometry.register(reg, registration);
    });

    let acceptable = match dnd.payload() {
        Some(p) => accepts.map(|cb| cb.call(p)).unwrap_or(true),
        None => false,
    };
    BridgeWorld {
        active: dnd.dragging() && acceptable,
        over: dnd.over() == Some(zone_id) && acceptable,
    }
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

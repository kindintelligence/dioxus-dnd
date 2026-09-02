//! Hooks for providing and consuming the drag context.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::html::MountedData;
use dioxus::prelude::*;

use super::components::{drop_query, resolve_drag_hover};
use super::registry::{RectRefresh, ZoneRecord, ZoneRegistration, ZoneRegistry};
use super::state::{DndContext, DragState};
use super::types::{DropEffect, DropOutcome, Point, Rect, ZoneId};
use super::world::{
    use_joined_window, DndWorld, JoinedWindow, WindowGeometry, WorldHit, WorldMembership,
};

// Identity freshness only: Relaxed is sufficient because the counter carries
// no synchronization. Correctness assumes this process-lifetime u64 never
// wraps; do not narrow it.
static NEXT_REFRESH_THUNK: AtomicU64 = AtomicU64::new(1);

/// Marker flag: a settle-enabled `DragOverlay<T>` is mounted somewhere in
/// this provider's subtree, so `Draggable<T>` should route successful
/// pointer drops through [`DndContext::take_settling`] instead of
/// [`DndContext::take`]. Typed so nested providers of different payloads
/// can't arm each other.
pub(crate) struct SettleFlag<T> {
    armed: Signal<Option<u64>>,
    marker: std::marker::PhantomData<T>,
}

impl<T> Copy for SettleFlag<T> {}
impl<T> Clone for SettleFlag<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> SettleFlag<T> {
    pub(crate) fn arm(self, capability: u64) {
        let mut armed = self.armed;
        if let Ok(mut value) = armed.try_write() {
            if *value != Some(capability) {
                *value = Some(capability);
            }
        };
    }

    pub(crate) fn is_armed(self) -> bool {
        matches!(self.armed.try_peek().as_deref(), Ok(Some(_)))
    }

    /// Retire `capability` only if it still owns this provider's settle
    /// capability. The return value snapshots that ownership for teardown.
    pub(crate) fn release(self, capability: u64) -> bool {
        let mut armed = self.armed;
        let Ok(mut value) = armed.try_write() else {
            return false;
        };
        if *value != Some(capability) {
            return false;
        }
        *value = None;
        true
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
        armed: Signal::new(None),
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
        None => DndContext::managed(state, announcement),
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
        if !ctx.dragging() {
            return;
        }
        let drag_id = ctx.drag_id();
        let session = ctx.drag_session_id();
        registry.refresh_rects_then(move || {
            // Receiver callbacks can synchronously finish this drag and
            // start another while measurements are in flight. Never let an
            // old batch alter the successor's hover.
            if !ctx.alive()
                || !ctx.dragging()
                || ctx.drag_id() != drag_id
                || ctx.drag_session_id() != session
            {
                return;
            }

            let proposed = ctx.proposed_effect();
            let point = membership
                .and_then(|joined| joined.local_pointer())
                .unwrap_or_else(|| ctx.pointer());
            match membership {
                Some(joined) => {
                    let query = ctx
                        .payload()
                        .map(|payload| drop_query(&ctx, payload, proposed));
                    match query
                        .as_ref()
                        .map(|query| joined.zone_under_query(point, query))
                        .unwrap_or(WorldHit::Unresolved)
                    {
                        WorldHit::Zone(location) => joined.enter(location),
                        WorldHit::Window => joined.clear_hover(),
                        WorldHit::Unresolved => {
                            match resolve_drag_hover(registry, &ctx, point, proposed) {
                                Some(zone) => joined.enter(joined.location(zone)),
                                None => joined.clear_hover(),
                            }
                        }
                    }
                }
                None => match resolve_drag_hover(registry, &ctx, point, proposed) {
                    Some(zone) => {
                        let mut ctx = ctx;
                        ctx.enter(zone);
                    }
                    None => {
                        if let Some(over) = ctx.over() {
                            let mut ctx = ctx;
                            ctx.leave(over);
                        }
                    }
                },
            }
        });
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
            let key = NEXT_REFRESH_THUNK.fetch_add(1, Ordering::Relaxed);
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
    state: Rc<RefCell<BridgeGeometryState>>,
}

#[derive(Default)]
struct BridgeGeometryState {
    next_writer: u64,
    writers: Vec<BridgeGeometryWriter>,
    mounted: Option<Rc<MountedData>>,
    rect: Option<Rect>,
}

#[derive(Clone)]
struct BridgeGeometryWriter {
    id: u64,
    mounted: Rc<dyn Fn(Rc<MountedData>)>,
    rect: Rc<dyn Fn(Rect)>,
}

struct BridgeGeometryRegistration {
    state: Rc<RefCell<BridgeGeometryState>>,
    id: u64,
}

impl Drop for BridgeGeometryRegistration {
    fn drop(&mut self) {
        self.state
            .borrow_mut()
            .writers
            .retain(|writer| writer.id != self.id);
    }
}

impl std::fmt::Debug for BridgeGeometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeGeometry")
            .field("worlds", &self.state.borrow().writers.len())
            .finish()
    }
}

impl PartialEq for BridgeGeometry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }
}

impl BridgeGeometry {
    /// Copy the bridge element's mounted handle into every joined registry.
    pub fn set_mounted(&self, mounted: &Rc<MountedData>) {
        let writers = {
            let mut state = self.state.borrow_mut();
            state.mounted = Some(mounted.clone());
            state.writers.clone()
        };
        for writer in writers {
            (writer.mounted)(mounted.clone());
        }
    }

    /// Copy a completed bridge measurement into every registration that is
    /// still current.
    pub fn set_rect_if_present(&self, rect: Rect) {
        let writers = {
            let mut state = self.state.borrow_mut();
            state.rect = Some(rect);
            state.writers.clone()
        };
        for writer in writers {
            (writer.rect)(rect);
        }
    }

    fn register<T: Clone + 'static>(
        &self,
        registry: ZoneRegistry<T>,
        registration: ZoneRegistration,
    ) -> BridgeGeometryRegistration {
        let mut state = self.state.borrow_mut();
        let id = state.next_writer;
        state.next_writer = state.next_writer.wrapping_add(1);
        let writer = BridgeGeometryWriter {
            id,
            mounted: Rc::new(move |mounted| {
                let mut registry = registry;
                registry.set_mounted(registration, mounted);
            }),
            rect: Rc::new(move |rect| {
                let mut registry = registry;
                registry.set_rect_if_present(registration, rect);
            }),
        };
        let mounted = state.mounted.clone();
        let rect = state.rect;
        state.writers.push(writer.clone());
        drop(state);
        if let Some(mounted) = mounted {
            (writer.mounted)(mounted);
        }
        if let Some(rect) = rect {
            (writer.rect)(rect);
        }
        BridgeGeometryRegistration {
            state: self.state.clone(),
            id,
        }
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
    let joined = use_joined_window::<T>();
    let mut reg = use_zone_registry::<T>();
    let on_drop = use_callback(move |outcome| on_drop.call(outcome));
    // Register one stable callback whose Dioxus callback slot is refreshed
    // with the current optional policy every render. Delivery and styling
    // therefore consult the same policy without waiting for a post-render
    // registry synchronization pass.
    let registered_accepts = use_callback(move |payload| {
        accepts
            .map(|callback| callback.call(payload))
            .unwrap_or(true)
    });
    let initial_label = label.clone();
    let initial_geometry = geometry.clone();
    let registrations = use_hook(move || {
        let registration = reg.register(ZoneRecord {
            id: zone_id,
            parent,
            label: initial_label,
            on_drop,
            accepts: Some(registered_accepts),
            mounted: None,
            rect: None,
        });
        let writer = initial_geometry.register(reg, registration);
        Rc::new(RefCell::new(Some((zone_id, parent, registration, writer))))
    });
    let effect_registrations = registrations.clone();
    let effect_geometry = geometry.clone();
    use_effect(use_reactive!(|(zone_id, parent, label)| {
        let unchanged = effect_registrations.borrow().as_ref().is_some_and(
            |(current_id, current_parent, ..)| *current_id == zone_id && *current_parent == parent,
        );
        if unchanged {
            reg.sync_label(zone_id, label);
            return;
        }

        if let Some((_, _, old_registration, old_writer)) = effect_registrations.borrow_mut().take()
        {
            reg.unregister_registration(old_registration);
            drop(old_writer);
        }
        let registration = reg.register(ZoneRecord {
            id: zone_id,
            parent,
            label,
            on_drop,
            accepts: Some(registered_accepts),
            mounted: None,
            rect: None,
        });
        let writer = effect_geometry.register(reg, registration);
        *effect_registrations.borrow_mut() = Some((zone_id, parent, registration, writer));
    }));
    use_drop(move || {
        if let Some((_, _, registration, writer)) = registrations.borrow_mut().take() {
            reg.unregister_registration(registration);
            drop(writer);
        }
    });

    let acceptable = dnd.proposed_effect() != DropEffect::None
        && match dnd.payload() {
            Some(p) => accepts.map(|cb| cb.call(p)).unwrap_or(true),
            None => false,
        };
    BridgeWorld {
        active: dnd.dragging() && acceptable,
        over: match joined {
            Some(joined) => joined.is_over(zone_id),
            None => dnd.over() == Some(zone_id),
        } && acceptable,
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

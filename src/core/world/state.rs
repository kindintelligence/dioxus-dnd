//! World construction and the joined-window table: process-lived state,
//! join/leave lifecycle, and window lookup.

use std::cell::RefCell;

use dioxus::prelude::*;
use dioxus::signals::{AnyStorage, Owner, SyncStorage, UnsyncStorage};

use crate::core::hooks::SettleFlag;
use crate::core::registry::ZoneRegistry;
use crate::core::state::{DndContext, DragState};
use crate::core::types::{Point, ZoneId};

use super::drag::ActiveDrag;
use super::geometry::{WindowGeometry, WindowKey};
use super::settle::SettleClaim;

thread_local! {
    /// Owners of every world's state, held for the life of the process
    /// (all of an app's windows share one thread). Worlds are deliberately
    /// immortal: scope-owned state would die with its creating window and
    /// panic every surviving window that still renders from it, and no
    /// close order should be able to do that. Bounded: a few signals per
    /// world, one world per payload type per app, window records pruned on
    /// close. Both storage flavors: signals live in unsync storage, but a
    /// store's subscription tree allocates in SYNC storage.
    static WORLD_OWNERS: RefCell<Vec<(Owner<UnsyncStorage>, Owner<SyncStorage>)>> =
        const { RefCell::new(Vec::new()) };
}

/// The world's initial bridging policy from `DIOXUS_DND_NO_BRIDGE`,
/// read once at creation so end users can disable host-side bridging
/// without a rebuild. Opt-out semantics: only an explicit non-`0`,
/// non-empty value disables - an unset or neutered variable must never
/// strand the flagship feature by accident.
fn default_bridging(no_bridge: Option<&str>) -> bool {
    match no_bridge {
        None | Some("") | Some("0") => true,
        Some(_) => false,
    }
}

/// A drop-zone identity qualified by the joined window that owns it.
///
/// Legacy single-window APIs continue to expose [`ZoneId`]; worlds use this
/// richer identity so separate windows may safely reuse the same explicit id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ZoneLocation {
    pub window: WindowKey,
    pub zone: ZoneId,
}

/// One window joined to a [`DndWorld`]: its geometry, its zone registry,
/// and the per-window handles drop delivery needs.
pub struct WindowRecord<T: Clone + 'static> {
    pub key: WindowKey,
    pub geometry: WindowGeometry,
    pub registry: ZoneRegistry<T>,
    /// The window's own settle flag: a drop landing here settles iff *this*
    /// window has a settle-enabled overlay mounted.
    pub(crate) settle: SettleFlag<T>,
    /// Re-measures the window's zones. Created by the window's provider, so
    /// calling it runs `refresh_rects` inside that window's own runtime
    /// (`Callback::call` re-enters its origin runtime) - the spawned
    /// measurements land on the right scheduler.
    pub(crate) refresh: Callback<()>,
}

impl<T: Clone + 'static> Copy for WindowRecord<T> {}
impl<T: Clone + 'static> Clone for WindowRecord<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A drag world shared by several windows: one [`DndContext`] every joined
/// provider re-provides, plus the window table cross-window hit-testing
/// walks. Cheap to copy; pass it to a sibling window via
/// `VirtualDom::with_root_context`.
pub struct DndWorld<T: Clone + 'static> {
    pub(super) ctx: DndContext<T>,
    windows: Signal<Vec<WindowRecord<T>>>,
    /// The window the in-flight drag started in - the coordinate anchor:
    /// `ctx.pointer()` is always in *this* window's client px.
    pub(super) active: Signal<Option<ActiveDrag>>,
    /// Exact host- or world-resolved pointer position in global physical px.
    /// Kept separate from `active` so pointer ticks do not invalidate
    /// session-metadata subscribers.
    pub(super) global_pointer: Signal<Option<Point>>,
    /// Window-qualified hover identity. The legacy id remains in
    /// `DragState` for single-window and custom-source compatibility.
    pub(super) over_location: Signal<Option<ZoneLocation>>,
    /// The elected settle presenter and its freshness generation. Kept as
    /// one value so owner and generation can never disagree.
    pub(super) settle_claim: Signal<Option<SettleClaim>>,
    /// Host-side bridging kill switch (see [`DndWorld::set_bridging`]).
    /// Owned by the world, not the desktop adapter, so a custom host
    /// cannot keep driving a world whose app disabled bridging.
    bridging: Signal<bool>,
}

impl<T: Clone + 'static> Copy for DndWorld<T> {}
impl<T: Clone + 'static> Clone for DndWorld<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone + 'static> PartialEq for DndWorld<T> {
    fn eq(&self, other: &Self) -> bool {
        self.windows == other.windows
    }
}

impl<T: Clone + 'static> DndWorld<T> {
    /// Create a world. Its state is **process-lived** (see the module docs
    /// on lifetimes), so windows may close in any order afterwards. Must
    /// run inside a Dioxus app; prefer [`use_dnd_world`](super::use_dnd_world), which also
    /// provides the world in context.
    pub fn new() -> Self {
        let owner = UnsyncStorage::owner();
        let sync_owner = SyncStorage::owner();
        let world = dioxus::core::with_owner(owner.clone(), || {
            dioxus::core::with_owner(sync_owner.clone(), || Self {
                ctx: DndContext::from_parts(
                    Store::new(DragState::default()),
                    Signal::new(String::new()),
                ),
                windows: Signal::new(Vec::new()),
                active: Signal::new(None),
                global_pointer: Signal::new(None),
                over_location: Signal::new(None),
                settle_claim: Signal::new(None),
                bridging: Signal::new(default_bridging(
                    std::env::var("DIOXUS_DND_NO_BRIDGE").ok().as_deref(),
                )),
            })
        });
        WORLD_OWNERS.with_borrow_mut(|owners| owners.push((owner, sync_owner)));
        world
    }

    /// The shared drag context every joined provider re-provides.
    pub fn context(&self) -> DndContext<T> {
        self.ctx
    }

    /// Enable or disable host-side bridging at runtime - the lever for the
    /// day a webview or OS update ships a cross-window regression that a
    /// rebuild cannot wait for. While disabled, every host-drive entry
    /// point ([`Self::track_global`], [`Self::drop_at_global`]) is inert
    /// and the `desktop` feature's bridge legs stand down, so drags
    /// degrade to per-window - exactly the already-modeled Wayland
    /// behavior. Local drags, geometry, settle and delivery are untouched.
    /// [`Self::cancel_drag`] deliberately stays live: it is an escape
    /// hatch, not a bridge leg.
    ///
    /// End users can flip the same switch without a rebuild by setting
    /// `DIOXUS_DND_NO_BRIDGE=1` before launch (read once at world
    /// creation; `0` or an empty value leaves bridging on).
    pub fn set_bridging(&self, enabled: bool) {
        let mut bridging = self.bridging;
        if *bridging.peek() != enabled {
            bridging.set(enabled);
        }
    }

    /// Is host-side bridging currently enabled? (See [`Self::set_bridging`].)
    pub fn bridging_enabled(&self) -> bool {
        // try_peek: callable from destructors and foreign runtimes, like
        // every other world read on the leg paths.
        self.bridging.try_peek().map(|b| *b).unwrap_or(true)
    }

    /// Join a window. Called by `use_dnd_provider` when it finds a world in
    /// context; call directly only from custom provider integrations.
    pub(crate) fn join(
        &self,
        geometry: WindowGeometry,
        registry: ZoneRegistry<T>,
        settle: SettleFlag<T>,
        refresh: Callback<()>,
    ) -> WindowKey {
        let key = WindowKey::auto();
        let mut windows = self.windows;
        windows.write().push(WindowRecord {
            key,
            geometry,
            registry,
            settle,
            refresh,
        });
        key
    }

    /// Remove a window (its provider unmounted, usually because the window
    /// closed). An active drag that originated there aborts because its
    /// coordinate anchor is gone; a receiver-owned settle survives from its
    /// snapshotted release point. A drag merely hovering one of the leaving
    /// window's zones just loses the hover. Pruning keeps the world from ever
    /// calling into a closed window's runtime.
    pub(crate) fn leave(&self, key: WindowKey) {
        // Worlds are process-lived, so this should be unreachable - but
        // leave runs inside a destructor, where a panic aborts the whole
        // process, so degrade to a no-op rather than trusting that.
        if self.windows.try_peek().is_err() {
            return;
        }
        let mut ctx = self.ctx;
        {
            let mut windows = self.windows;
            windows.write().retain(|w| w.key != key);
        }
        // Qualified hover tells us exactly which window disappeared, even
        // when another survivor reuses the same explicit ZoneId. Retain the
        // legacy reachability fallback for keyboard/custom paths that have
        // not supplied world-qualified metadata.
        let qualified_over = *self.over_location.peek();
        if qualified_over.is_some_and(|over| over.window == key) {
            if let Some(over) = ctx.over() {
                ctx.leave(over);
            }
            let mut over_location = self.over_location;
            over_location.set(None);
        } else if qualified_over.is_none() {
            // Checked against the REMAINING windows, not the leaving one:
            // scopes drop children first, so the leaving window's zones have
            // usually unregistered before its provider leaves.
            if let Some(over) = ctx.over() {
                let reachable = self
                    .windows
                    .peek()
                    .iter()
                    .any(|w| w.registry.contains(over));
                if !reachable {
                    ctx.leave(over);
                }
            }
        }
        let active_drag = *self.active.peek();
        if active_drag.is_some_and(|active| active.origin == key) {
            if ctx.dragging() {
                match active_drag.and_then(|active| active.session) {
                    Some(session) if ctx.is_session(session) => {
                        // A built-in source normally finishes from its own
                        // cleanup before its provider leaves. Never call an
                        // unknown custom source after child teardown.
                        ctx.abandon_session(session);
                    }
                    // An untracked replacement can coexist briefly with the
                    // old source's committed completion. Cancel the closing
                    // window's drag without consuming that unrelated slot.
                    _ => ctx.cancel(),
                }
                self.clear_world_state();
                return;
            }
            if ctx.settling().is_some() {
                // A settle elected in another window no longer needs the
                // origin runtime: the release point and origin scale were
                // snapshotted into world state. Keep them until it lands.
                match self.settle_presenter() {
                    Some(presenter) if presenter != key => {}
                    Some(_) => {
                        self.finish_settle_from(key);
                    }
                    None => {
                        // Compatibility for a custom source that entered the
                        // context's public settle state without a world claim:
                        // the origin is its only possible presenter.
                        ctx.finish_settle();
                        self.clear_world_state();
                    }
                }
                return;
            }
            self.clear_world_state();
            return;
        }
        if self.settle_presenter_is(key) {
            // The elected overlay is gone, so no transition listener remains
            // to finish the glide. Non-presenter closure is intentionally
            // inert because this equality fails for every other window.
            self.finish_settle_from(key);
        }
    }

    /// Look up a joined window. `None` for unknown keys.
    pub fn record(&self, key: WindowKey) -> Option<WindowRecord<T>> {
        self.windows
            .try_peek()
            .ok()?
            .iter()
            .find(|w| w.key == key)
            .copied()
    }

    /// Every joined window, in join order. Subscribing read, like
    /// [`ZoneRegistry::records`] - its consumers are renderers and tests.
    pub fn windows(&self) -> Vec<WindowRecord<T>> {
        self.windows
            .try_read()
            .map(|w| w.to_vec())
            .unwrap_or_default()
    }

    /// The window containing `global` (physical px), most recently focused
    /// first when several overlap. `None` while no live geometry contains
    /// the point.
    pub fn window_under(&self, global: Point) -> Option<WindowRecord<T>> {
        self.windows
            .try_peek()
            .ok()?
            .iter()
            .filter(|w| w.geometry.contains_global(global))
            .max_by_key(|w| w.geometry.focus_stamp())
            .copied()
    }

    /// Resolve a global point to (window, client-local point). `None` when
    /// no live window contains it.
    pub fn resolve_global(&self, global: Point) -> Option<(WindowRecord<T>, Point)> {
        let rec = self.window_under(global)?;
        let local = rec.geometry.to_client(global)?;
        Some((rec, local))
    }

    /// Ask every joined window to re-measure its zones, each inside its own
    /// runtime through the window's internal refresh callback.
    pub fn refresh_all_rects(&self) {
        let Ok(windows) = self.windows.try_peek() else {
            return;
        };
        for rec in windows.iter() {
            rec.refresh.call(());
        }
    }
}

impl<T: Clone + 'static> Default for DndWorld<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridging_defaults_on_unless_no_bridge_is_meaningfully_set() {
        assert!(default_bridging(None));
        assert!(default_bridging(Some("")));
        assert!(default_bridging(Some("0")));
        assert!(!default_bridging(Some("1")));
        assert!(!default_bridging(Some("true")));
    }
}

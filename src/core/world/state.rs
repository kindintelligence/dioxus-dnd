//! World construction and the joined-window table: process-lived state,
//! join/leave lifecycle, and window lookup.

use std::cell::RefCell;

use dioxus::prelude::*;
use dioxus::signals::{AnyStorage, Owner, SyncStorage, UnsyncStorage};

use crate::core::hooks::SettleFlag;
use crate::core::registry::ZoneRegistry;
use crate::core::state::{DndContext, DragState};
use crate::core::types::Point;

use super::drag::ActiveDrag;
use super::geometry::{WindowGeometry, WindowKey};

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
    /// The window whose overlay presents the current settle glide (set on
    /// cross-window drops; `None` means the origin window presents).
    pub(super) settling_in: Signal<Option<WindowKey>>,
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
                settling_in: Signal::new(None),
            })
        });
        WORLD_OWNERS.with_borrow_mut(|owners| owners.push((owner, sync_owner)));
        world
    }

    /// The shared drag context every joined provider re-provides.
    pub fn context(&self) -> DndContext<T> {
        self.ctx
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
    /// closed). A drag that *originated* there aborts - its coordinate
    /// anchor is gone; a drag merely *hovering* one of its zones just loses
    /// the hover. Pruning matters: it keeps the world from ever calling
    /// into a closed window's runtime.
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
        // Clear a hover that now points into nowhere. Checked against the
        // REMAINING windows, not the leaving one: scopes drop children
        // first, so the leaving window's zones have usually unregistered
        // themselves from its registry before the provider's leave runs.
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
            }
            let mut active = self.active;
            active.set(None);
        }
        if *self.settling_in.peek() == Some(key) {
            ctx.finish_settle();
            let mut settling_in = self.settling_in;
            settling_in.set(None);
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
    /// runtime (see [`WindowRecord::refresh`]).
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

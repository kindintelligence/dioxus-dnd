//! Multi-window drag worlds: one shared drag state spanning several
//! windows of a desktop app, each window an independent `VirtualDom`.
//!
//! Dioxus desktop polls every window's `VirtualDom` on the main thread, and
//! signal storage is thread-local rather than runtime-local, so a `Signal`
//! (and therefore a [`DndContext`]) created in one window's runtime can be
//! read, written and subscribed from another's - a write in window A
//! re-renders window B through B's own scheduler. `DndWorld` builds on
//! exactly that: the payload crosses windows as a live Rust value, with no
//! serialization and none of the platform roulette of native HTML5
//! drag-and-drop. (`DataTransfer` interop for drags that leave the app
//! entirely stays in [`crate::external`].)
//!
//! # Coordinate spaces
//!
//! Everything zone-shaped stays in **client CSS pixels of its own window**,
//! exactly as in single-window use. The world adds one more space: **global
//! desktop physical pixels**, in which windows are located and hit-tested.
//! Each window's [`WindowGeometry`] carries the conversion: the client
//! area's top-left in physical px (`inner_position()` on desktop), the
//! window scale factor, and the client-area size in physical px. Conversion
//! happens only at the world boundary.
//!
//! # Wiring
//!
//! ```text
//! // main window: create the world (root scope - it must outlive every
//! // joining window), spawn siblings with it in their root context
//! fn main_window() -> Element {
//!     let world = use_dnd_world::<Card>();
//!     // dioxus_desktop::window().new_window(
//!     //     VirtualDom::new(popup).with_root_context(world), Default::default());
//!     rsx! { DndProvider::<Card> { /* ... */ } }   // joins via context
//! }
//!
//! fn popup() -> Element {
//!     rsx! { DndProvider::<Card> { /* ... */ } }   // joins via root context
//! }
//! ```
//!
//! A `DndProvider<T>` that finds a `DndWorld<T>` in context joins it
//! instead of creating isolated state (nested providers keep today's
//! shadowing semantics: only a window's outermost provider of `T` joins).
//! Feed each window's [`WindowGeometry`] from your windowing layer - on
//! desktop, sample `inner_position()` / `inner_size()` / `scale_factor()`
//! on move/resize events and call [`WindowGeometry::set`]; call
//! [`WindowGeometry::mark_focused`] on focus so overlapping windows resolve
//! to the frontmost. **Without geometry the world degrades gracefully**:
//! drags behave exactly as single-window drags (this is also the honest
//! Wayland story, where a client can learn neither the cursor's global
//! position nor its own windows' positions).
//!
//! # Lifetimes: close windows in any order
//!
//! A world's own state (the shared context and the window table) is
//! **process-lived**: it is created under an owner this module holds for
//! the life of the app, not under any window's scope. Whichever window
//! created the world can close first and every other window keeps
//! dragging - cross-window between the survivors, single-window when only
//! one remains. Closing a joined window prunes it from the table and
//! aborts an in-flight drag that originated there (its coordinate anchor
//! is gone). The cost is a deliberate, bounded leak: a handful of signals
//! per world, once per app.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;
use dioxus::signals::{AnyStorage, Owner, SyncStorage, UnsyncStorage};

use super::hooks::SettleFlag;
use super::registry::ZoneRegistry;
use super::state::{DndContext, DragState};
use super::types::{effective_effect, DragSessionId, Point, PointerKind, ZoneId};

static NEXT_WINDOW_KEY: AtomicU64 = AtomicU64::new(1);
/// Focus stamps start at 1 so a never-focused window's 0 always loses.
static NEXT_FOCUS_STAMP: AtomicU64 = AtomicU64::new(1);
static NEXT_SETTLE_GENERATION: AtomicU64 = AtomicU64::new(1);

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

/// Identifies one joined window within a [`DndWorld`]. Process-unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowKey(pub u64);

impl WindowKey {
    /// Generate a process-unique window key.
    pub fn auto() -> Self {
        Self(NEXT_WINDOW_KEY.fetch_add(1, Ordering::Relaxed))
    }
}

/// A drop-zone identity qualified by the joined window that owns it.
///
/// Legacy single-window APIs continue to expose [`ZoneId`]; worlds use this
/// richer identity internally so two windows may safely reuse the same
/// explicit id without both rendering as hovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ZoneLocation {
    pub window: WindowKey,
    pub zone: ZoneId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ActiveDrag {
    origin: WindowKey,
    session: Option<DragSessionId>,
    origin_scale: f64,
}

// --- pure conversion math (unit-tested; signals stay out of it) --------

/// Client CSS px of a window -> global desktop physical px.
pub(crate) fn client_to_global(client: Point, origin: Point, scale: f64) -> Point {
    Point::new(origin.x + client.x * scale, origin.y + client.y * scale)
}

/// Global desktop physical px -> client CSS px of a window.
pub(crate) fn global_to_client(global: Point, origin: Point, scale: f64) -> Point {
    let s = if scale > 0.0 { scale } else { 1.0 };
    Point::new((global.x - origin.x) / s, (global.y - origin.y) / s)
}

/// Is `global` inside a window whose client area starts at `origin` with
/// `size`, both in physical px? Inclusive of edges, like [`super::types::Rect`].
pub(crate) fn window_contains(global: Point, origin: Point, size: (f64, f64)) -> bool {
    global.x >= origin.x
        && global.x <= origin.x + size.0
        && global.y >= origin.y
        && global.y <= origin.y + size.1
}

/// One window's placement on the desktop, as reactive signals the host
/// feeds. Copy handle; create one per window (the provider creates an inert
/// one when none is in context) and keep it updated from your windowing
/// layer. Inert (no origin/size) means "geometry unknown": the window still
/// drags internally, it just can't take part in cross-window hit-testing.
pub struct WindowGeometry {
    /// Client-area top-left in global physical px (`inner_position()`).
    origin: Signal<Option<Point>>,
    /// Client-area size in physical px.
    size: Signal<Option<(f64, f64)>>,
    /// Window scale factor (physical px per CSS px).
    scale: Signal<f64>,
    /// Monotonic focus stamp; higher = more recently focused. Breaks ties
    /// when overlapping windows both contain a point (no z-order queries
    /// exist on desktop, so focus recency approximates it).
    focused: Signal<u64>,
    /// Whether the host currently considers the window eligible for global
    /// hit-testing (visible, restored, and otherwise interactive).
    eligible: Signal<bool>,
}

impl Copy for WindowGeometry {}
impl Clone for WindowGeometry {
    fn clone(&self) -> Self {
        *self
    }
}
impl PartialEq for WindowGeometry {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin
    }
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowGeometry {
    /// A fresh, inert geometry owned by the current scope.
    pub fn new() -> Self {
        Self {
            origin: Signal::new(None),
            size: Signal::new(None),
            scale: Signal::new(1.0),
            focused: Signal::new(0),
            eligible: Signal::new(true),
        }
    }

    /// Update the window's placement. `origin` and `size` describe the
    /// client area in global physical px; `scale` is the window's scale
    /// factor. No-op writes are skipped, so this is safe to call from
    /// high-frequency window events.
    pub fn set(&self, origin: Point, size: (f64, f64), scale: f64) {
        let (mut o, mut sz, mut sc) = (self.origin, self.size, self.scale);
        if *o.peek() != Some(origin) {
            o.set(Some(origin));
        }
        if *sz.peek() != Some(size) {
            sz.set(Some(size));
        }
        if *sc.peek() != scale {
            sc.set(scale);
        }
    }

    /// Forget the placement (geometry became unavailable); the window keeps
    /// working as a single-window drag surface.
    pub fn clear(&self) {
        let (mut o, mut sz) = (self.origin, self.size);
        if o.peek().is_some() {
            o.set(None);
        }
        if sz.peek().is_some() {
            sz.set(None);
        }
    }

    /// Include or exclude this window from global hit-testing without
    /// discarding its last known placement.
    pub fn set_eligible(&self, eligible: bool) {
        let mut value = self.eligible;
        if *value.peek() != eligible {
            value.set(eligible);
        }
    }

    /// Whether the host currently allows this window to receive a global
    /// drag. This is a subscribing read.
    pub fn eligible(&self) -> bool {
        *self.eligible.read()
    }

    /// Record that this window was just focused (see `focused`).
    pub fn mark_focused(&self) {
        let mut f = self.focused;
        f.set(NEXT_FOCUS_STAMP.fetch_add(1, Ordering::Relaxed));
    }

    /// Is the placement known and currently eligible for global hit-testing?
    pub fn live(&self) -> bool {
        self.origin.read().is_some() && self.size.read().is_some() && *self.eligible.read()
    }

    /// This window's client CSS px -> global physical px. `None` until the
    /// placement is known.
    pub fn to_global(&self, client: Point) -> Option<Point> {
        let origin = (*self.origin.peek())?;
        Some(client_to_global(client, origin, *self.scale.peek()))
    }

    /// Global physical px -> this window's client CSS px. `None` until the
    /// placement is known.
    pub fn to_client(&self, global: Point) -> Option<Point> {
        let origin = (*self.origin.peek())?;
        Some(global_to_client(global, origin, *self.scale.peek()))
    }

    /// Does this window's client area contain `global` (physical px)?
    /// Always false while the placement is unknown.
    pub fn contains_global(&self, global: Point) -> bool {
        if !*self.eligible.peek() {
            return false;
        }
        match (*self.origin.peek(), *self.size.peek()) {
            (Some(origin), Some(size)) => window_contains(global, origin, size),
            _ => false,
        }
    }

    /// The window's scale factor.
    pub fn scale(&self) -> f64 {
        *self.scale.peek()
    }

    /// The current focus stamp (0 = never focused).
    pub fn focus_stamp(&self) -> u64 {
        *self.focused.peek()
    }
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
    ctx: DndContext<T>,
    windows: Signal<Vec<WindowRecord<T>>>,
    /// The window the in-flight drag started in - the coordinate anchor:
    /// `ctx.pointer()` is always in *this* window's client px.
    active: Signal<Option<ActiveDrag>>,
    /// The one window whose overlay owns the current settle glide.
    settling_in: Signal<Option<WindowKey>>,
    settle_generation: Signal<Option<u64>>,
    /// Exact host-reported pointer position, in global physical pixels.
    global_pointer: Signal<Option<Point>>,
    /// Window-qualified source and hover identity. The legacy ids remain in
    /// `DragState` for API compatibility.
    source_location: Signal<Option<ZoneLocation>>,
    over_location: Signal<Option<ZoneLocation>>,
    pointer_kind: Signal<PointerKind>,
    modifiers: Signal<Modifiers>,
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
    /// run inside a Dioxus app; prefer [`use_dnd_world`], which also
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
                settle_generation: Signal::new(None),
                global_pointer: Signal::new(None),
                source_location: Signal::new(None),
                over_location: Signal::new(None),
                pointer_kind: Signal::new(PointerKind::Unknown),
                modifiers: Signal::new(Modifiers::empty()),
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
        if self
            .over_location
            .peek()
            .is_some_and(|over| over.window == key)
        {
            if let Some(over) = ctx.over() {
                ctx.leave(over);
            }
            let mut over_location = self.over_location;
            over_location.set(None);
        }
        if self
            .active
            .peek()
            .is_some_and(|active| active.origin == key)
        {
            if ctx.dragging() {
                if let Some(session) = ctx.active_session() {
                    // A Draggable normally completes from its own cleanup
                    // before the provider leaves. Never call an unknown
                    // custom source after child scopes have been torn down.
                    ctx.abandon_session(session);
                } else {
                    ctx.cancel();
                }
                self.clear_world_state();
                return;
            }
            if ctx.settling().is_some() {
                // A receiver-owned settle no longer needs the origin
                // runtime: global release coordinates and origin scale were
                // snapshotted into the world. Let the elected receiver land.
                if *self.settling_in.peek() == Some(key) {
                    ctx.finish_settle();
                    self.clear_world_state();
                }
                return;
            }
            self.clear_world_state();
            return;
        }
        if *self.settling_in.peek() == Some(key) {
            ctx.finish_settle();
            self.clear_world_state();
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

    /// Mark a drag as begun from `key` and reset stale presentation state.
    /// `Draggable` calls this at pickup; call it from custom drag sources
    /// so the world knows which window's client px `ctx.pointer()` is in.
    /// This compatibility entry point assumes a mouse, matching the
    /// pre-pointer-kind host behavior; custom touch/pen sources should call
    /// [`Self::begin_pointer_from`] with their actual kind.
    pub fn begin_from(&self, key: WindowKey) {
        self.begin_pointer_from(key, PointerKind::Mouse, Modifiers::empty());
    }

    /// Mark a built-in pointer drag as begun and record the native input
    /// metadata desktop host completion needs.
    pub fn begin_pointer_from(&self, key: WindowKey, kind: PointerKind, modifiers: Modifiers) {
        let active_drag = ActiveDrag {
            origin: key,
            // A receiver callback may synchronously start an untracked drag
            // while the previous source result is committed but not yet
            // publicly finalized. That old generation must not be attached
            // to the new world drag.
            session: self
                .ctx
                .active_session()
                .filter(|session| self.ctx.session_result(*session).is_none()),
            origin_scale: self
                .record(key)
                .map_or(1.0, |record| record.geometry.scale()),
        };
        let mut active = self.active;
        if *active.peek() != Some(active_drag) {
            active.set(Some(active_drag));
        }
        let mut settling_in = self.settling_in;
        if settling_in.peek().is_some() {
            settling_in.set(None);
        }
        let mut settle_generation = self.settle_generation;
        if settle_generation.peek().is_some() {
            settle_generation.set(None);
        }
        let mut source_location = self.source_location;
        source_location.set(
            self.ctx
                .source()
                .map(|zone| ZoneLocation { window: key, zone }),
        );
        let mut over_location = self.over_location;
        over_location.set(None);
        let mut pointer_kind = self.pointer_kind;
        pointer_kind.set(kind);
        let mut current_modifiers = self.modifiers;
        current_modifiers.set(modifiers);
        let mut global_pointer = self.global_pointer;
        global_pointer.set(
            self.record(key)
                .and_then(|r| r.geometry.to_global(self.ctx.pointer())),
        );
    }

    /// The record of the window the in-flight drag started in.
    pub fn active_record(&self) -> Option<WindowRecord<T>> {
        self.record(self.active.peek().as_ref()?.origin)
    }

    fn active_drag(&self) -> Option<ActiveDrag> {
        *self.active.peek()
    }

    /// The in-flight pointer in global physical px. `None` when no drag is
    /// active or the origin window's geometry is unknown.
    pub fn global_pointer(&self) -> Option<Point> {
        *self.global_pointer.read()
    }

    /// Claim settle presentation for the receiving window before the
    /// shared context enters its settling state.
    /// Elect `key` as presenter before a custom world delivery calls
    /// [`DndContext::take_settling`]. Built-in drop paths do this
    /// automatically.
    pub fn claim_settle(&self, key: WindowKey) {
        let mut settling_in = self.settling_in;
        settling_in.set(Some(key));
        let mut generation = self.settle_generation;
        generation.set(Some(NEXT_SETTLE_GENERATION.fetch_add(1, Ordering::Relaxed)));
    }

    pub(crate) fn settle_token(&self, key: WindowKey) -> Option<u64> {
        if *self.settling_in.read() == Some(key) {
            *self.settle_generation.read()
        } else {
            None
        }
    }

    /// Complete a settle only from its elected overlay. Returns whether
    /// this caller owned and finished it.
    /// Finish a custom or built-in settle from its elected window. Custom
    /// world overlays should use this instead of calling
    /// [`DndContext::finish_settle`] directly so world metadata is cleared.
    pub fn finish_settle_from(&self, key: WindowKey) -> bool {
        let Some(generation) = self.settle_token(key) else {
            return false;
        };
        self.finish_settle_generation(key, generation)
    }

    pub(crate) fn finish_settle_generation(&self, key: WindowKey, generation: u64) -> bool {
        if *self.settling_in.peek() != Some(key)
            || *self.settle_generation.peek() != Some(generation)
            || self.ctx.settling().is_none()
        {
            return false;
        }
        let mut ctx = self.ctx;
        ctx.finish_settle();
        self.clear_world_state();
        true
    }

    /// The window elected to present the current settle glide.
    pub fn settling_in(&self) -> Option<WindowKey> {
        self.ctx.settling().and_then(|_| *self.settling_in.read())
    }

    /// Window-qualified source and hover locations for the active world
    /// drag. These are additive metadata; existing `DndContext` id accessors
    /// remain unchanged.
    pub fn source_location(&self) -> Option<ZoneLocation> {
        *self.source_location.read()
    }

    pub fn over_location(&self) -> Option<ZoneLocation> {
        *self.over_location.read()
    }

    /// Pointer kind and modifiers currently associated with host delivery.
    pub fn pointer_kind(&self) -> PointerKind {
        *self.pointer_kind.read()
    }

    pub fn modifiers(&self) -> Modifiers {
        *self.modifiers.read()
    }

    pub fn update_modifiers(&self, modifiers: Modifiers) {
        let mut value = self.modifiers;
        if *value.peek() != modifiers {
            value.set(modifiers);
        }
    }

    /// Current pointer-drag generation, used by host pollers to reject a
    /// stale task after a rapid restart.
    pub fn drag_session(&self) -> Option<DragSessionId> {
        self.active.peek().as_ref()?.session
    }

    pub fn is_drag_session(&self, session: DragSessionId) -> bool {
        self.drag_session() == Some(session) && self.ctx.is_session(session)
    }

    /// The key of the window the in-flight drag started in, if any.
    pub fn origin_window(&self) -> Option<WindowKey> {
        (self.ctx.dragging() || self.ctx.settling().is_some())
            .then(|| self.active.peek().as_ref().map(|active| active.origin))
            .flatten()
    }

    fn clear_world_state(&self) {
        let mut active = self.active;
        active.set(None);
        let mut settling_in = self.settling_in;
        settling_in.set(None);
        let mut settle_generation = self.settle_generation;
        settle_generation.set(None);
        let mut global_pointer = self.global_pointer;
        global_pointer.set(None);
        let mut source_location = self.source_location;
        source_location.set(None);
        let mut over_location = self.over_location;
        over_location.set(None);
        let mut pointer_kind = self.pointer_kind;
        pointer_kind.set(PointerKind::Unknown);
        let mut modifiers = self.modifiers;
        modifiers.set(Modifiers::empty());
    }

    fn enter_location(&self, location: ZoneLocation) {
        let mut over_location = self.over_location;
        if *over_location.peek() != Some(location) {
            over_location.set(Some(location));
        }
        let mut ctx = self.ctx;
        ctx.enter(location.zone);
    }

    fn clear_hover(&self) {
        let mut ctx = self.ctx;
        if let Some(over) = ctx.over() {
            ctx.leave(over);
        }
        let mut over_location = self.over_location;
        if over_location.peek().is_some() {
            over_location.set(None);
        }
    }

    pub(crate) fn commit_session(&self, session: DragSessionId, dropped: bool) -> bool {
        if !self.is_drag_session(session) {
            return false;
        }
        let mut ctx = self.ctx;
        ctx.commit_source(session, dropped)
    }

    pub(crate) fn finalize_session(&self, session: DragSessionId) -> bool {
        let Some(result) = self.ctx.session_result(session) else {
            return false;
        };
        self.finish_session(session, result)
    }

    pub(crate) fn finish_session(&self, session: DragSessionId, dropped: bool) -> bool {
        let mut ctx = self.ctx;
        if !ctx.is_session(session) {
            return false;
        }
        let owns_metadata = self.drag_session() == Some(session);
        let result = ctx.session_result(session).unwrap_or(dropped);
        let finished = if ctx.session_result(session).is_some() {
            ctx.finalize_source(session)
        } else if dropped {
            ctx.finish_source(session, true)
        } else {
            ctx.cancel_session(session)
        };
        if !finished {
            return false;
        }
        if !owns_metadata || self.drag_session() != Some(session) {
            return true;
        }
        // `on_drag_end` is user code and may synchronously start another
        // drag. Its new `begin_*` call owns the world metadata now.
        if ctx.dragging() {
            return true;
        }
        if result && ctx.settling().is_some() {
            let mut active = self.active;
            let current = *active.peek();
            if let Some(mut current) = current {
                current.session = None;
                active.set(Some(current));
            }
            self.clear_hover();
        } else {
            self.clear_world_state();
        }
        true
    }

    pub(crate) fn finish_untracked(&self, dropped: bool) {
        let mut ctx = self.ctx;
        if !dropped && ctx.dragging() {
            ctx.cancel();
        }
        if ctx.dragging() {
            return;
        }
        if dropped && ctx.settling().is_some() {
            self.clear_hover();
        } else {
            self.clear_world_state();
        }
    }

    /// Ask every joined window to re-measure its zones, each inside its own
    /// runtime through the window record's refresh callback.
    pub fn refresh_all_rects(&self) {
        let Ok(windows) = self.windows.try_peek() else {
            return;
        };
        for rec in windows.iter() {
            rec.refresh.call(());
        }
    }
}

/// Host-side drive: entry points for desktop glue that sees the pointer
/// where webviews cannot. Webview pointer events stop at the viewport
/// edge (and under a pointer grab, every non-origin window is fully
/// event-blind on all platforms), so cross-window pointer data must come
/// from the windowing layer: poll the global cursor while a drag is in
/// flight and feed it here.
impl<T: Clone + PartialEq + 'static> DndWorld<T> {
    /// Track an in-flight pointer drag from a host-reported cursor
    /// position (global physical px): updates the shared pointer (in the
    /// origin window's client px, the coordinate anchor everything else
    /// expects) and enters/leaves zones across every joined window. No-op
    /// when nothing is dragging or the origin window is unknown.
    pub fn track_global(&self, global: Point) {
        let mut ctx = self.ctx;
        if !ctx.dragging() {
            return;
        }
        let Some(origin) = self.active_record() else {
            return;
        };
        let mut global_pointer = self.global_pointer;
        if *global_pointer.peek() != Some(global) {
            global_pointer.set(Some(global));
        }
        if let Some(local) = origin.geometry.to_client(global) {
            ctx.update_pointer(local);
        }
        let location = self.resolve_global(global).and_then(|(rec, local)| {
            rec.registry.hit_test(local).map(|zone| ZoneLocation {
                window: rec.key,
                zone,
            })
        });
        match location {
            Some(location) => self.enter_location(location),
            None => self.clear_hover(),
        }
    }

    /// Complete an in-flight pointer drag at a host-reported cursor
    /// position (global physical px): exact zone hit in whichever window
    /// contains the point, else that window's 48px snap (in its own CSS
    /// px), else cancel. Returns the receiving zone. Used by glue that
    /// detects a release the webviews never saw - e.g. a non-origin
    /// window receiving its first pointer event mid-"drag", which proves
    /// the button is up. A no-op returning `None` when nothing is
    /// dragging, so double delivery (webview pointerup plus host echo)
    /// is harmless.
    pub fn drop_at_global(&self, global: Point) -> Option<ZoneId> {
        let mut ctx = self.ctx;
        if !ctx.dragging() {
            return None;
        }
        // Make release coordinates authoritative even when the pointer was
        // stationary and no final host tracking tick preceded the release.
        self.track_global(global);
        let session = self.drag_session();
        let Some((rec, local)) = self.resolve_global(global) else {
            match session {
                Some(session) => {
                    self.finish_session(session, false);
                }
                None => self.finish_untracked(false),
            }
            return None;
        };
        let target = rec.registry.hit_test(local).or_else(|| {
            ctx.payload()
                .and_then(|p| rec.registry.hit_test_closest(local, &p, 48.0))
        });
        let effect = effective_effect(ctx.effect(), *self.modifiers.peek());
        let delivered = target.filter(|t| {
            super::components::deliver_drop(
                rec.registry,
                &mut ctx,
                super::components::SettleRoute {
                    flag: Some(rec.settle),
                    owner: Some((self, rec.key)),
                },
                super::components::DropCompletion::World {
                    world: self,
                    session,
                },
                *t,
                local,
                effect,
            )
        });
        match delivered {
            Some(zone) => Some(zone),
            None => {
                match session {
                    Some(session) => {
                        self.finish_session(session, false);
                    }
                    None => self.finish_untracked(false),
                }
                None
            }
        }
    }

    /// Abort an in-flight drag from the host side (a window manager
    /// signal, an escape hatch). No-op when nothing is dragging.
    pub fn cancel_drag(&self) {
        if !self.ctx.dragging() {
            return;
        }
        match self.drag_session() {
            Some(session) => {
                self.finish_session(session, false);
            }
            None => self.finish_untracked(false),
        }
    }
}

impl<T: Clone + 'static> Default for DndWorld<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a `DndWorld<T>` (process-lived - see the module docs on
/// lifetimes) and provide it in context, so providers in this window join
/// it. Pass the returned handle to sibling windows via
/// `VirtualDom::with_root_context`. Call it once, in any window.
pub fn use_dnd_world<T: Clone + 'static>() -> DndWorld<T> {
    use_hook(|| provide_context(DndWorld::<T>::new()))
}

/// The enclosing provider's world membership, if it joined a world - the
/// handle desktop glue needs to bridge host-side input (see
/// [`DndWorld::track_global`] / [`DndWorld::drop_at_global`]). Call it
/// anywhere below the `DndProvider`.
pub fn use_joined_window<T: Clone + 'static>() -> Option<JoinedWindow<T>> {
    try_use_context::<WorldMembership<T>>().and_then(|m| m.0)
}

/// This provider tree's world membership: which world it joined and as
/// which window. Every provider provides one (with `None` inside when it
/// created isolated state), so nested providers shadow their ancestors'
/// membership exactly like they shadow drag contexts.
pub(crate) struct WorldMembership<T: Clone + 'static>(pub(crate) Option<JoinedWindow<T>>);

impl<T: Clone + 'static> Copy for WorldMembership<T> {}
impl<T: Clone + 'static> Clone for WorldMembership<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A provider's handle to the world it joined: the world, this window's
/// key, and this window's geometry - everything the pointer path needs to
/// think cross-window.
pub struct JoinedWindow<T: Clone + 'static> {
    pub world: DndWorld<T>,
    pub key: WindowKey,
    pub geometry: WindowGeometry,
}

impl<T: Clone + 'static> Copy for JoinedWindow<T> {}
impl<T: Clone + 'static> Clone for JoinedWindow<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone + 'static> PartialEq for JoinedWindow<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.world == other.world
    }
}

/// What the world made of a pointer position (client px of the joined
/// window asking).
pub(crate) enum WorldHit {
    /// Some window's zone is under the pointer.
    Zone(ZoneLocation),
    /// A window is under the pointer, but no zone in it.
    Window,
    /// The world can't resolve the point (no geometry, or outside every
    /// window) - fall back to window-local behavior.
    Unresolved,
}

impl<T: Clone + 'static> JoinedWindow<T> {
    /// Resolve a point in **this window's client px** to whichever window's
    /// zone lies under it.
    pub(crate) fn zone_under(&self, client: Point) -> WorldHit {
        let Some(global) = self.geometry.to_global(client) else {
            return WorldHit::Unresolved;
        };
        let mut global_pointer = self.world.global_pointer;
        if *global_pointer.peek() != Some(global) {
            global_pointer.set(Some(global));
        }
        let Some((rec, local)) = self.world.resolve_global(global) else {
            return WorldHit::Unresolved;
        };
        match rec.registry.hit_test(local) {
            Some(zone) => WorldHit::Zone(ZoneLocation {
                window: rec.key,
                zone,
            }),
            None => WorldHit::Window,
        }
    }

    /// Qualify one of this window's local zone ids for world state.
    pub fn location(&self, zone: ZoneId) -> ZoneLocation {
        ZoneLocation {
            window: self.key,
            zone,
        }
    }

    /// Mark a window-qualified zone as hovered. Custom world-aware sources
    /// should use this instead of `DndContext::enter`, which cannot identify
    /// which window owns a reused `ZoneId`.
    pub fn enter(&self, location: ZoneLocation) {
        self.world.enter_location(location);
    }

    /// Clear the world's qualified hover.
    pub fn clear_hover(&self) {
        self.world.clear_hover();
    }

    /// Whether this exact window/zone pair owns the world hover.
    pub fn is_over(&self, zone: ZoneId) -> bool {
        *self.world.over_location.read() == Some(self.location(zone))
    }

    /// Latest global pointer converted into this window's client CSS
    /// coordinates. The origin-local context pointer is used only when no
    /// host/global conversion is available for the origin itself.
    pub fn local_pointer(&self) -> Option<Point> {
        if let Some(global) = *self.world.global_pointer.read() {
            return self.geometry.to_client(global);
        }
        (self.world.origin_window() == Some(self.key)).then(|| self.world.ctx.pointer())
    }

    /// Resolve a point in this window's client px to a **foreign** window
    /// (and the point in its client px). `None` for the own window, an
    /// unresolvable point, or no window - callers then run the classic
    /// local path, preserving single-window semantics exactly.
    pub(crate) fn foreign_window_under(&self, client: Point) -> Option<(WindowRecord<T>, Point)> {
        let global = self.geometry.to_global(client)?;
        let (rec, local) = self.world.resolve_global(global)?;
        (rec.key != self.key).then_some((rec, local))
    }

    /// Where this window's overlay should draw the ghost, if this window is
    /// the presenting one: `Some((top-left in this window's client px,
    /// origin-to-here scale ratio for size matching))`. `None` means
    /// another window presents the ghost this frame.
    ///
    /// Presentation follows the pointer: whichever window contains the
    /// global pointer presents; when none does (or no geometry exists), the
    /// origin window keeps the ghost, anchored to its raw client coords.
    /// During a settle, the window the drop landed in presents.
    pub(crate) fn present_overlay(
        &self,
        pointer: Point,
        grab: Point,
        settling: bool,
    ) -> Option<(Point, f64)> {
        let raw = pointer - grab;
        let Some(active) = self.world.active_drag() else {
            // The drag didn't register an origin window (custom source):
            // fall back to raw anchoring everywhere, as before worlds.
            return Some((raw, 1.0));
        };
        let origin = self.world.record(active.origin);
        let origin_scale = origin
            .map(|record| record.geometry.scale())
            .unwrap_or(active.origin_scale);
        let global_anchor = origin
            .and_then(|record| record.geometry.to_global(raw))
            .or_else(|| {
                self.world.global_pointer().map(|global| {
                    Point::new(
                        global.x - grab.x * origin_scale,
                        global.y - grab.y * origin_scale,
                    )
                })
            });
        let Some(global_anchor) = global_anchor else {
            // Origin geometry unknown: only the origin window can place it.
            return (self.key == active.origin).then_some((raw, 1.0));
        };
        let presenting = if settling {
            self.world.settling_in()?
        } else {
            let pointer_global = self
                .world
                .global_pointer()
                .or_else(|| origin.and_then(|record| record.geometry.to_global(pointer)))
                .unwrap_or(global_anchor);
            self.world
                .window_under(pointer_global)
                .map(|r| r.key)
                .unwrap_or(active.origin)
        };
        if presenting != self.key {
            return None;
        }
        match self.geometry.to_client(global_anchor) {
            Some(local) => {
                let own_scale = self.geometry.scale();
                let ratio = if own_scale > 0.0 {
                    origin_scale / own_scale
                } else {
                    1.0
                };
                Some((local, ratio))
            }
            // Presenting window without geometry can only be the origin.
            None => (self.key == active.origin).then_some((raw, 1.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_round_trip_under_mixed_scales() {
        for scale in [1.0, 1.5, 2.0] {
            let origin = Point::new(1200.0, 300.0);
            let client = Point::new(80.0, 40.5);
            let global = client_to_global(client, origin, scale);
            assert_eq!(
                global,
                Point::new(1200.0 + 80.0 * scale, 300.0 + 40.5 * scale)
            );
            let back = global_to_client(global, origin, scale);
            assert!((back.x - client.x).abs() < 1e-9);
            assert!((back.y - client.y).abs() < 1e-9);
        }
    }

    #[test]
    fn degenerate_scale_does_not_divide_by_zero() {
        let p = global_to_client(Point::new(10.0, 10.0), Point::new(0.0, 0.0), 0.0);
        assert_eq!(p, Point::new(10.0, 10.0));
    }

    #[test]
    fn window_containment_is_edge_inclusive() {
        let origin = Point::new(100.0, 100.0);
        let size = (800.0, 600.0);
        assert!(window_contains(Point::new(100.0, 100.0), origin, size));
        assert!(window_contains(Point::new(900.0, 700.0), origin, size));
        assert!(!window_contains(Point::new(99.9, 100.0), origin, size));
        assert!(!window_contains(Point::new(901.0, 300.0), origin, size));
    }

    #[test]
    fn window_keys_are_unique() {
        let a = WindowKey::auto();
        let b = WindowKey::auto();
        assert_ne!(a, b);
    }
}

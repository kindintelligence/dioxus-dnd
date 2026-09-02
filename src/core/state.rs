//! The shared drag state. One `DndContext<T>` lives in Dioxus context and is
//! read/written by `Draggable` and `DropZone` components (and by you, if you
//! wire events manually).
//!
//! Payloads travel through this Rust-side store - not through the browser's
//! `DataTransfer` - so they can be any `Clone` type with zero serialization.
//! (`DataTransfer` interop for external drags lives in [`crate::external`].)
//!
//! State is held in a [`struct@Store`], Dioxus 0.7's fine-grained reactivity
//! primitive: each field gets its own lazy subscription. A component that
//! reads `dnd.over()` in its render only reruns when the hovered zone
//! changes - not on every pointer move.

use dioxus::prelude::*;

use super::monitor::{CancelReason, DndEvent, DndMonitor, DragSnapshot, DropReceipt};
use super::session::SourceCompletion;
use super::types::{DragId, DragMode, DragSessionId, DropEffect, Point, PointerKind, Rect, ZoneId};

/// A snapshot of an in-flight drag.
///
/// Deriving [`macro@Store`] generates per-field lenses, which
/// [`DndContext`]'s accessors use for granular subscriptions.
#[derive(Store, Debug, Clone, PartialEq)]
pub struct DragState<T: 'static> {
    /// The payload currently being dragged, if any.
    pub payload: Option<T>,
    /// Zone the drag started from.
    pub source: Option<ZoneId>,
    /// Zone the pointer is currently over.
    pub over: Option<ZoneId>,
    /// Last known pointer position (client coordinates).
    pub pointer: Point,
    /// Where inside the dragged element the user grabbed it.
    pub grab: Point,
    /// Effect requested by the draggable.
    pub effect: DropEffect,
    /// How this drag is being driven (pointer vs keyboard).
    pub mode: DragMode,
    /// Which pointer device drives a pointer drag (mouse/touch/pen).
    /// Meaningful only while `mode` is [`DragMode::Pointer`]; host-side
    /// glue reads it to bridge exactly the input layers the device
    /// needs (see [`PointerKind`]). `Draggable` records it at pickup;
    /// custom sources that never do get the safe `Mouse` default.
    pub pointer_kind: PointerKind,
    /// Client rect of the dragged element, measured at pickup. Feeds
    /// size-matched ghosts (`DragOverlay { match_source: true }`); `None`
    /// until the async measurement lands or when a custom source never set
    /// it.
    pub source_rect: Option<Rect>,
    /// Payload of a just-completed keyboard drop, awaiting focus
    /// restoration: the drop re-mounts the moved item at its landing place
    /// and the browser dumps focus on `<body>` when the source element
    /// unmounts, so the matching `Draggable` claims this on mount and
    /// focuses itself - keyboard users keep their place. Cleared by the
    /// claim or by the next drag starting.
    pub refocus: Option<T>,
    /// Destination rect of a just-completed drop whose overlay is still
    /// gliding home (the drop-settle animation). While set, `dragging()` is
    /// false but `payload` stays readable so the ghost keeps its content.
    pub settle: Option<Rect>,
}

/// Complete input for starting a drag with an explicit stable identity.
///
/// Construct this with [`DragStart::new`] and use the builder methods for
/// optional metadata. The struct is non-exhaustive so future drag metadata
/// can be added without breaking downstream callers.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DragStart<T> {
    pub payload: T,
    pub source: Option<ZoneId>,
    pub pointer: Point,
    pub grab: Point,
    pub effect: DropEffect,
    pub mode: DragMode,
    pub pointer_kind: PointerKind,
    pub source_rect: Option<Rect>,
}

impl<T> DragStart<T> {
    /// Create a pointer drag at `pointer` with default grab and move effect.
    pub fn new(payload: T, pointer: Point) -> Self {
        Self {
            payload,
            source: None,
            pointer,
            grab: Point::default(),
            effect: DropEffect::default(),
            mode: DragMode::default(),
            pointer_kind: PointerKind::default(),
            source_rect: None,
        }
    }

    pub fn with_source(mut self, source: Option<ZoneId>) -> Self {
        self.source = source;
        self
    }

    pub fn with_grab(mut self, grab: Point) -> Self {
        self.grab = grab;
        self
    }

    pub fn with_effect(mut self, effect: DropEffect) -> Self {
        self.effect = effect;
        self
    }

    pub fn with_mode(mut self, mode: DragMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_pointer_kind(mut self, pointer_kind: PointerKind) -> Self {
        self.pointer_kind = pointer_kind;
        self
    }

    pub fn with_source_rect(mut self, source_rect: Option<Rect>) -> Self {
        self.source_rect = source_rect;
        self
    }
}

pub(super) enum DragIdentity {
    Generated,
    Explicit(DragId),
}

impl<T> Default for DragState<T> {
    fn default() -> Self {
        Self {
            payload: None,
            source: None,
            over: None,
            pointer: Point::default(),
            grab: Point::default(),
            effect: DropEffect::default(),
            mode: DragMode::default(),
            pointer_kind: PointerKind::default(),
            source_rect: None,
            refocus: None,
            settle: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DragPhase {
    #[default]
    Idle,
    Dragging,
    Settling,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DragRuntimeState {
    id: Option<DragId>,
    identity_explicit: bool,
    session: Option<DragSessionId>,
    proposed_effect: DropEffect,
    phase: DragPhase,
}

impl Default for DragRuntimeState {
    fn default() -> Self {
        Self {
            id: None,
            identity_explicit: false,
            session: None,
            proposed_effect: DropEffect::default(),
            phase: DragPhase::Idle,
        }
    }
}

pub(super) struct DragRuntime<T: 'static> {
    state: Signal<DragRuntimeState>,
    pub(super) completion: Signal<Option<SourceCompletion>>,
    monitor: DndMonitor<T>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PhaseAuthority {
    /// Preserve the published `from_parts` contract: the caller owns the
    /// store and may update it through another handle.
    State,
    /// Provider/world contexts own every transition, so the private phase is
    /// authoritative and can enforce terminal-event exclusivity.
    Runtime,
}

impl<T> Copy for DragRuntime<T> {}
impl<T> Clone for DragRuntime<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for DragRuntime<T> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
            && self.completion == other.completion
            && self.monitor == other.monitor
    }
}

impl<T: Clone + 'static> DragRuntime<T> {
    fn new() -> Self {
        Self {
            state: Signal::new(DragRuntimeState::default()),
            completion: Signal::new(None),
            monitor: DndMonitor::new(),
        }
    }
}

/// Handle to the shared drag state. Cheap to copy: it contains only Dioxus
/// store and signal keys.
pub struct DndContext<T: Clone + 'static> {
    state: Store<DragState<T>>,
    /// Screen-reader announcement channel, rendered by
    /// [`crate::a11y::LiveRegion`].
    announcement: Signal<String>,
    pub(super) runtime: DragRuntime<T>,
    phase_authority: PhaseAuthority,
}

// Manual impls: `derive` would add unnecessary `T: Copy` / `T: PartialEq`
// bounds, but the handle contains only store and signal keys.
impl<T: Clone + 'static> Copy for DndContext<T> {}
impl<T: Clone + 'static> Clone for DndContext<T> {
    fn clone(&self) -> Self {
        *self
    }
}
/// Handle identity, not state identity. Two handles are equal when they
/// share the same announcement signal, which every construction path pairs
/// one-to-one with the drag store. Memoizing on a `DndContext` therefore
/// tracks *which* provider it came from, never what that provider's state
/// currently holds; read the accessors for that.
impl<T: Clone + 'static> PartialEq for DndContext<T> {
    fn eq(&self, other: &Self) -> bool {
        // Preserve the 3.x handle identity contract. In particular, two
        // `from_parts` wrappers around the same announcement remain equal
        // even though new-feature sidecars are private to each construction.
        self.announcement == other.announcement
    }
}

impl<T: Clone + 'static> DndContext<T> {
    /// Wrap existing state. Prefer [`crate::core::hooks::use_dnd_provider`].
    pub fn from_parts(state: Store<DragState<T>>, announcement: Signal<String>) -> Self {
        Self::from_parts_with_authority(state, announcement, PhaseAuthority::State)
    }

    /// Construct a provider/world-owned context whose private phase is the
    /// terminal-state authority. Public `from_parts` cannot use this mode:
    /// its caller retains the store and is allowed to mutate it independently.
    pub(crate) fn managed(state: Store<DragState<T>>, announcement: Signal<String>) -> Self {
        Self::from_parts_with_authority(state, announcement, PhaseAuthority::Runtime)
    }

    fn from_parts_with_authority(
        state: Store<DragState<T>>,
        announcement: Signal<String>,
        phase_authority: PhaseAuthority,
    ) -> Self {
        let phase = if state.settle().peek().is_some() {
            DragPhase::Settling
        } else if state.payload().peek().is_some() {
            DragPhase::Dragging
        } else {
            DragPhase::Idle
        };
        let mut runtime = DragRuntime::new();
        {
            let mut runtime_state = runtime.state.write();
            runtime_state.phase = phase;
            runtime_state.proposed_effect = *state.effect().peek();
        }
        Self {
            state,
            announcement,
            runtime,
            phase_authority,
        }
    }

    /// Begin a drag. Notifies all fields (state transition).
    pub fn start(
        &mut self,
        payload: T,
        source: Option<ZoneId>,
        pointer: Point,
        grab: Point,
        effect: DropEffect,
        mode: DragMode,
    ) {
        if !self.prepare_start() {
            return;
        }
        self.start_with_metadata(
            DragIdentity::Generated,
            None,
            DragStart::new(payload, pointer)
                .with_source(source)
                .with_grab(grab)
                .with_effect(effect)
                .with_mode(mode),
        );
    }

    /// Begin a drag with a stable source identity.
    pub fn start_with_id(&mut self, id: DragId, start: DragStart<T>) {
        if !self.prepare_start() {
            return;
        }
        self.start_with_metadata(DragIdentity::Explicit(id), None, start);
    }

    pub(super) fn start_with_metadata(
        &mut self,
        identity: DragIdentity,
        session: Option<DragSessionId>,
        start: DragStart<T>,
    ) {
        let (id, identity_explicit) = match identity {
            DragIdentity::Generated => (DragId::auto(), false),
            DragIdentity::Explicit(id) => (id, true),
        };
        self.runtime.state.set(DragRuntimeState {
            id: Some(id),
            identity_explicit,
            session,
            proposed_effect: start.effect,
            phase: DragPhase::Dragging,
        });
        self.state.set(DragState {
            payload: Some(start.payload),
            source: start.source,
            over: None,
            pointer: start.pointer,
            grab: start.grab,
            effect: start.effect,
            mode: start.mode,
            pointer_kind: start.pointer_kind,
            source_rect: start.source_rect,
            // A new drag supersedes any unclaimed focus restoration.
            refocus: None,
            // Starting a new drag interrupts any settle still gliding.
            settle: None,
        });
        self.runtime
            .monitor
            .emit_lazy(|| self.snapshot().map(DndEvent::Started));
    }

    /// Record which pointer device drives the current drag (see
    /// [`DragState::pointer_kind`]). `Draggable` sets this right after
    /// pickup from the initiating event's `pointerType`; call it from
    /// custom pointer sources so host-side glue (cursor pollers, raw
    /// input bridges) can tell captured pointers from blind ones. Built-in
    /// sources install this in their initial snapshot; custom sources that
    /// use `start` or `start_with_id` can refine the default afterwards.
    pub fn set_pointer_kind(&mut self, kind: PointerKind) {
        self.state.pointer_kind().set(kind);
    }

    /// Record that `payload` just landed via a keyboard drop and its new
    /// element should take focus when it mounts (see
    /// [`DragState::refocus`]). `Draggable` calls this on its own keyboard
    /// drops; call it from custom keyboard sources to get the same focus
    /// continuity.
    pub fn request_refocus(&mut self, payload: T) {
        self.state.refocus().set(Some(payload));
    }

    /// Claim a pending focus restoration if it matches `payload`; returns
    /// whether the caller should focus itself. First matching claimant
    /// wins - the request is consumed.
    pub fn claim_refocus(&mut self, payload: &T) -> bool
    where
        T: PartialEq,
    {
        let mut refocus = self.state.refocus();
        let hit = refocus.peek().as_ref() == Some(payload);
        if hit {
            refocus.set(None);
        }
        hit
    }

    /// Record the dragged element's client rect (see
    /// [`DragState::source_rect`]). `Draggable` measures and sets this right
    /// after pickup; call it from custom drag sources so size-matched ghosts
    /// (`DragOverlay { match_source: true }`) can dress themselves.
    pub fn set_source_rect(&mut self, rect: Option<Rect>) {
        self.state.source_rect().set(rect);
    }

    /// Update the tracked pointer position (drives `DragOverlay`). Granular:
    /// only `pointer` subscribers rerun.
    ///
    /// An exact `(0, 0)` sample is ignored. Some webviews report it for
    /// synthetic events, and nothing else in the sample can tell that apart
    /// from a real pointer at the viewport corner. A drag that does reach
    /// the exact corner keeps its previous sample, which arrived at most a
    /// few CSS px away, so hit-testing and delivery are unaffected in
    /// practice.
    pub fn update_pointer(&mut self, pointer: Point) {
        // See the doc comment: value is the only signal available, and a
        // rejected real sample costs a few px at the corner while an
        // accepted synthetic one would jump the overlay across the viewport.
        if pointer.x == 0.0 && pointer.y == 0.0 {
            return;
        }
        self.state.pointer().set(pointer);
        self.runtime
            .monitor
            .emit_lazy(|| self.snapshot().map(DndEvent::Moved));
    }

    /// Record the modifier-adjusted effect used by the current pointer
    /// sample. Geometry refresh completion reuses it to keep rich target
    /// acceptance consistent with the pointer path.
    pub(crate) fn set_proposed_effect(&mut self, effect: DropEffect) {
        let mut runtime = self.runtime.state;
        if runtime.peek().proposed_effect != effect {
            runtime.write().proposed_effect = effect;
        }
    }

    /// Mark `zone` as hovered. Granular: only `over` subscribers rerun.
    pub fn enter(&mut self, zone: ZoneId) {
        let previous = self.over();
        if previous == Some(zone) {
            return;
        }
        self.state.over().set(Some(zone));
        self.runtime.monitor.emit_lazy(|| {
            self.snapshot().map(|drag| DndEvent::TargetChanged {
                drag,
                previous,
                current: Some(zone),
            })
        });
    }

    /// Clear hover, but only if `zone` is still the hovered one (avoids
    /// enter/leave races between adjacent zones).
    pub fn leave(&mut self, zone: ZoneId) {
        let previous = self.over();
        if previous == Some(zone) {
            self.state.over().set(None);
            self.runtime.monitor.emit_lazy(|| {
                self.snapshot().map(|drag| DndEvent::TargetChanged {
                    drag,
                    previous,
                    current: None,
                })
            });
        }
    }

    /// Consume the payload on a successful drop. Returns `(payload, source)`.
    /// After this, `dragging()` is false.
    pub fn take(&mut self) -> Option<(T, Option<ZoneId>)> {
        let (payload, source) = {
            let mut s = self.state.write();
            (s.payload.take(), s.source)
        };
        let payload = payload?;
        self.state.set(DragState::default());
        self.runtime.state.set(DragRuntimeState::default());
        Some((payload, source))
    }

    /// Consume the payload on a successful drop, like [`Self::take`], but
    /// enter the *settling* phase instead of resetting: the returned clone
    /// goes to the drop handler while the stored payload stays readable and
    /// `settle` records the destination rect, so a settle-enabled
    /// [`crate::core::components::DragOverlay`] can glide the ghost home.
    /// After this, `dragging()` is false and `over()` is cleared; call
    /// [`Self::finish_settle`] (the overlay does) to reset fully.
    ///
    /// Custom sources in a joined [`crate::core::world::DndWorld`] must call
    /// [`crate::core::world::DndWorld::claim_settle`] first: world overlays
    /// only present and finish a settle for the elected window.
    pub fn take_settling(&mut self, to: Rect) -> Option<(T, Option<ZoneId>)> {
        let mut s = self.state.write();
        let payload = s.payload.clone()?;
        let source = s.source;
        s.over = None;
        s.settle = Some(to);
        drop(s);
        self.runtime.state.write().phase = DragPhase::Settling;
        Some((payload, source))
    }

    /// Re-aim an in-flight settle at a better rect - typically the landed
    /// element's own, measured after the drop re-rendered the model
    /// (`SettleSlot` does this for you). The overlay's glide retargets
    /// smoothly, mid-flight included. A no-op unless currently settling.
    pub fn retarget_settle(&mut self, to: Rect) {
        let mut settle = self.state.settle();
        // The equality guard is load-bearing: a `SettleSlot` retargets from
        // an effect that (via its render) subscribes to `settle`, and
        // signal writes notify even when the value is unchanged - writing
        // the same rect back would loop effect -> write -> effect forever.
        if settle.peek().is_some() && *settle.peek() != Some(to) {
            settle.set(Some(to));
        }
    }

    /// End the settling phase and reset all state. A no-op unless currently
    /// settling, so a late `transitionend` can never clobber a new drag.
    pub fn finish_settle(&mut self) {
        if self.phase_peek() == DragPhase::Settling {
            self.state.set(DragState::default());
            self.runtime.state.set(DragRuntimeState::default());
        }
    }

    pub(crate) fn phase_peek(&self) -> DragPhase {
        match self.phase_authority {
            PhaseAuthority::State => {
                if self.state.settle().peek().is_some() {
                    DragPhase::Settling
                } else if self.state.payload().peek().is_some() {
                    DragPhase::Dragging
                } else {
                    DragPhase::Idle
                }
            }
            PhaseAuthority::Runtime => self
                .runtime
                .state
                .try_peek()
                .map(|runtime| runtime.phase)
                .unwrap_or(DragPhase::Idle),
        }
    }

    /// Is the underlying state still alive? Destructors check this before
    /// touching the context, because store lens access on a dead store
    /// panics (even `try_` reads - the selector internals do) and a panic
    /// in a destructor aborts the process. A world context is process-
    /// lived so this holds by construction there; the gate keeps every
    /// other wiring (custom `from_parts` contexts, unforeseen drop orders)
    /// degrading gracefully instead. Probed through the announcement
    /// signal, a plain `Signal` created alongside the store, whose
    /// `try_peek` IS dead-safe.
    pub(crate) fn alive(&self) -> bool {
        self.announcement.try_peek().is_ok()
    }

    /// Abort the drag and reset all state.
    pub fn cancel(&mut self) {
        self.cancel_with_reason(CancelReason::User);
    }

    /// Abort the drag with an explicit reason for monitor consumers.
    pub fn cancel_with_reason(&mut self, reason: CancelReason) {
        if self.phase_peek() == DragPhase::Settling {
            self.finish_settle();
            return;
        }
        if let Some(session) = self.drag_session_id() {
            if self.cancel_session(session, reason) {
                return;
            }
        }
        self.cancel_state(reason);
    }

    pub(super) fn cancel_state(&mut self, reason: CancelReason) {
        let snapshot = (self.phase_peek() == DragPhase::Dragging
            && self.runtime.monitor.has_listeners())
        .then(|| self.snapshot())
        .flatten();
        self.state.set(DragState::default());
        self.runtime.state.set(DragRuntimeState::default());
        if let Some(drag) = snapshot {
            self.runtime
                .monitor
                .emit(DndEvent::Cancelled { drag, reason });
        }
    }

    // --- read accessors -----------------------------------------------
    // Each reads through a field lens, so render-time reads subscribe only
    // to that field.

    /// Is a drag currently in flight? False while a completed drop is still
    /// settling, even though [`Self::payload`] remains readable.
    pub fn dragging(&self) -> bool {
        match self.phase_authority {
            PhaseAuthority::State => {
                self.state.payload().is_some() && self.state.settle().is_none()
            }
            PhaseAuthority::Runtime => self.runtime.state.read().phase == DragPhase::Dragging,
        }
    }

    /// Destination rect of a drop currently settling (see
    /// [`Self::take_settling`]), if any.
    pub fn settling(&self) -> Option<Rect> {
        match self.phase_authority {
            PhaseAuthority::State => self.state.settle().cloned(),
            PhaseAuthority::Runtime => (self.runtime.state.read().phase == DragPhase::Settling)
                .then(|| self.state.settle().cloned())
                .flatten(),
        }
    }

    /// Non-subscribing version of [`Self::settling`] for imperative world
    /// bookkeeping (destructors, event handlers) that must not subscribe.
    pub(crate) fn settling_peek(&self) -> bool {
        self.phase_peek() == DragPhase::Settling
    }

    /// Clone of the current payload, if dragging.
    pub fn payload(&self) -> Option<T> {
        self.state.payload().cloned()
    }

    /// Stable identity of the active draggable.
    pub fn drag_id(&self) -> Option<DragId> {
        self.runtime.state.read().id
    }

    /// Whether the active source deliberately supplied its stable id.
    pub fn has_explicit_drag_id(&self) -> bool {
        self.runtime.state.read().identity_explicit
    }

    /// Fresh identity of the active tracked pointer gesture.
    pub fn drag_session_id(&self) -> Option<DragSessionId> {
        self.runtime.state.read().session
    }

    /// Zone currently hovered.
    pub fn over(&self) -> Option<ZoneId> {
        self.state.over().cloned()
    }

    /// Zone the drag started from.
    pub fn source(&self) -> Option<ZoneId> {
        self.state.source().cloned()
    }

    /// Last known pointer position.
    pub fn pointer(&self) -> Point {
        self.state.pointer().cloned()
    }

    /// Grab offset inside the dragged element.
    pub fn grab(&self) -> Point {
        self.state.grab().cloned()
    }

    /// Client rect of the dragged element measured at pickup, if available.
    pub fn source_rect(&self) -> Option<Rect> {
        self.state.source_rect().cloned()
    }

    /// Effect the drag was started with.
    pub fn effect(&self) -> DropEffect {
        self.state.effect().cloned()
    }

    pub(crate) fn proposed_effect(&self) -> DropEffect {
        self.runtime.state.read().proposed_effect
    }

    /// How the current drag is being driven.
    pub fn mode(&self) -> DragMode {
        self.state.mode().cloned()
    }

    /// Which pointer device drives the current drag (meaningful for
    /// [`DragMode::Pointer`] drags; `Mouse` otherwise and by default).
    pub fn pointer_kind(&self) -> PointerKind {
        self.state.pointer_kind().cloned()
    }

    /// Complete monitor snapshot of the active drag.
    pub fn snapshot(&self) -> Option<DragSnapshot<T>> {
        Some(DragSnapshot {
            id: self.drag_id()?,
            session: self.drag_session_id(),
            payload: self.payload()?,
            source: self.source(),
            over: self.over(),
            pointer: self.pointer(),
            grab: self.grab(),
            effect: self.effect(),
            mode: self.mode(),
            pointer_kind: self.pointer_kind(),
            source_rect: self.source_rect(),
        })
    }

    pub(crate) fn monitor_mut(&mut self) -> DndMonitor<T> {
        self.runtime.monitor
    }

    pub(crate) fn emit_dropped(&self, receipt: DropReceipt<T>) {
        self.runtime.monitor.emit(DndEvent::Dropped(receipt));
    }

    pub(crate) fn monitor_has_listeners(&self) -> bool {
        self.runtime.monitor.has_listeners()
    }

    /// Push a screen-reader announcement (rendered by
    /// [`crate::a11y::LiveRegion`]). Called automatically by the built-in
    /// keyboard interaction; call it yourself for custom flows.
    pub fn announce(&mut self, msg: impl Into<String>) {
        self.announcement.set(msg.into());
    }

    /// The current announcement text.
    pub fn announcement(&self) -> String {
        self.announcement.read().clone()
    }
}

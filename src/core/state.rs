//! The shared drag state. One `DndContext<T>` lives in Dioxus context and is
//! read/written by `Draggable` and `DropZone` components (and by you, if you
//! wire events manually).
//!
//! Payloads travel through this Rust-side store - not through the browser's
//! `DataTransfer` - so they can be any `Clone` type with zero serialization.
//! (`DataTransfer` interop for external drags lives in [`crate::external`].)
//!
//! State is held in a [`struct@Store`], Dioxus 0.8's fine-grained reactivity
//! primitive: each field gets its own lazy subscription. A component that
//! reads `dnd.over()` in its render only reruns when the hovered zone
//! changes - not on every pointer move.

use dioxus::prelude::*;

use super::types::{DragMode, DropEffect, Point, PointerKind, Rect, ZoneId};

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

/// Handle to the shared drag state. Cheap to copy - it's just a store key.
pub struct DndContext<T: Clone + 'static> {
    state: Store<DragState<T>>,
    /// Screen-reader announcement channel, rendered by
    /// [`crate::a11y::LiveRegion`].
    announcement: Signal<String>,
}

// Manual impls: `derive` would add unnecessary `T: Copy` / `T: PartialEq`
// bounds, but the handle is just a store key plus a signal key.
impl<T: Clone + 'static> Copy for DndContext<T> {}
impl<T: Clone + 'static> Clone for DndContext<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone + 'static> PartialEq for DndContext<T> {
    fn eq(&self, other: &Self) -> bool {
        self.announcement == other.announcement
    }
}

impl<T: Clone + 'static> DndContext<T> {
    /// Wrap existing state. Prefer [`crate::core::hooks::use_dnd_provider`].
    pub fn from_parts(state: Store<DragState<T>>, announcement: Signal<String>) -> Self {
        Self {
            state,
            announcement,
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
        self.state.set(DragState {
            payload: Some(payload),
            source,
            over: None,
            pointer,
            grab,
            effect,
            mode,
            // The safe default; the drag source refines it right after
            // this call via `set_pointer_kind` (as `Draggable` does).
            pointer_kind: PointerKind::default(),
            // Measured (async) by the drag source right after this call.
            source_rect: None,
            // A new drag supersedes any unclaimed focus restoration.
            refocus: None,
            // Starting a new drag interrupts any settle still gliding.
            settle: None,
        });
    }

    /// Record which pointer device drives the current drag (see
    /// [`DragState::pointer_kind`]). `Draggable` sets this right after
    /// pickup from the initiating event's `pointerType`; call it from
    /// custom pointer sources so host-side glue (cursor pollers, raw
    /// input bridges) can tell captured pointers from blind ones. Left
    /// alone, every drag reads as `Mouse`.
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
    pub fn update_pointer(&mut self, pointer: Point) {
        // An exact (0,0) is overwhelmingly a bogus platform report (some
        // webviews emit it for synthetic events), not a real drag at the
        // viewport corner; ignore it so the overlay doesn't jump there.
        if pointer.x == 0.0 && pointer.y == 0.0 {
            return;
        }
        self.state.pointer().set(pointer);
    }

    /// Mark `zone` as hovered. Granular: only `over` subscribers rerun.
    pub fn enter(&mut self, zone: ZoneId) {
        self.state.over().set(Some(zone));
    }

    /// Clear hover, but only if `zone` is still the hovered one (avoids
    /// enter/leave races between adjacent zones).
    pub fn leave(&mut self, zone: ZoneId) {
        let mut over = self.state.over();
        if *over.peek() == Some(zone) {
            over.set(None);
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
        Some((payload, source))
    }

    /// Consume the payload on a successful drop, like [`Self::take`], but
    /// enter the *settling* phase instead of resetting: the returned clone
    /// goes to the drop handler while the stored payload stays readable and
    /// `settle` records the destination rect, so a settle-enabled
    /// [`crate::core::components::DragOverlay`] can glide the ghost home.
    /// After this, `dragging()` is false and `over()` is cleared; call
    /// [`Self::finish_settle`] (the overlay does) to reset fully.
    pub fn take_settling(&mut self, to: Rect) -> Option<(T, Option<ZoneId>)> {
        let mut s = self.state.write();
        let payload = s.payload.clone()?;
        let source = s.source;
        s.over = None;
        s.settle = Some(to);
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
        if self.state.settle().peek().is_some() {
            self.state.set(DragState::default());
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
        self.state.set(DragState::default());
    }

    // --- read accessors -----------------------------------------------
    // Each reads through a field lens, so render-time reads subscribe only
    // to that field.

    /// Is a drag currently in flight? False while a completed drop is still
    /// settling, even though [`Self::payload`] remains readable.
    pub fn dragging(&self) -> bool {
        self.state.payload().is_some() && self.state.settle().is_none()
    }

    /// Destination rect of a drop currently settling (see
    /// [`Self::take_settling`]), if any.
    pub fn settling(&self) -> Option<Rect> {
        self.state.settle().cloned()
    }

    /// Clone of the current payload, if dragging.
    pub fn payload(&self) -> Option<T> {
        self.state.payload().cloned()
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

    /// How the current drag is being driven.
    pub fn mode(&self) -> DragMode {
        self.state.mode().cloned()
    }

    /// Which pointer device drives the current drag (meaningful for
    /// [`DragMode::Pointer`] drags; `Mouse` otherwise and by default).
    pub fn pointer_kind(&self) -> PointerKind {
        self.state.pointer_kind().cloned()
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

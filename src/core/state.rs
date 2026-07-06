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

use super::types::{DragMode, DropEffect, Point, ZoneId};

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
        });
    }

    /// Update the tracked pointer position (drives `DragOverlay`). Granular:
    /// only `pointer` subscribers rerun.
    pub fn update_pointer(&mut self, pointer: Point) {
        // Some webviews fire `drag` with (0,0); ignore those so the overlay
        // doesn't jump to the corner.
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
    /// After this, `dragging()` is false - which is how `Draggable` tells a
    /// completed drop apart from a cancelled one in `ondragend`.
    pub fn take(&mut self) -> Option<(T, Option<ZoneId>)> {
        let (payload, source) = {
            let mut s = self.state.write();
            (s.payload.take(), s.source)
        };
        let payload = payload?;
        self.state.set(DragState::default());
        Some((payload, source))
    }

    /// Abort the drag and reset all state.
    pub fn cancel(&mut self) {
        self.state.set(DragState::default());
    }

    // --- read accessors -----------------------------------------------
    // Each reads through a field lens, so render-time reads subscribe only
    // to that field.

    /// Is a drag currently in flight?
    pub fn dragging(&self) -> bool {
        self.state.payload().is_some()
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

    /// Effect the drag was started with.
    pub fn effect(&self) -> DropEffect {
        self.state.effect().cloned()
    }

    /// How the current drag is being driven.
    pub fn mode(&self) -> DragMode {
        self.state.mode().cloned()
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

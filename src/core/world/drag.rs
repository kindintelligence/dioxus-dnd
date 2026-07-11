//! Drag-session anchoring: which window the in-flight drag started in,
//! and the origin-window conversion behind the global pointer.

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

use crate::core::types::{DragSessionId, Point};

use super::geometry::WindowKey;
use super::state::{DndWorld, WindowRecord, ZoneLocation};

// Identity freshness only: Relaxed is sufficient because the counter carries
// no synchronization. Correctness assumes this process-lifetime u64 never
// wraps; do not narrow it.
static NEXT_WORLD_DRAG_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ActiveDrag {
    pub(super) origin: WindowKey,
    /// Fresh for every `begin_from`, including custom/untracked sources. Host
    /// adapters bind observations to this rather than treating `session: None`
    /// as an authority token that could attach to a successor drag.
    pub(super) generation: u64,
    pub(super) session: Option<DragSessionId>,
    pub(super) origin_scale: f64,
    pub(super) source_location: Option<ZoneLocation>,
    pub(super) modifiers: Modifiers,
}

impl<T: Clone + 'static> DndWorld<T> {
    /// Mark a drag as begun from `key` and reset stale presentation state.
    /// `Draggable` calls this at pickup; call it from custom drag sources
    /// so the world knows which window's client px `ctx.pointer()` is in.
    pub fn begin_from(&self, key: WindowKey) {
        let origin = self.record(key);
        let active_drag = ActiveDrag {
            origin: key,
            generation: NEXT_WORLD_DRAG_GENERATION.fetch_add(1, Ordering::Relaxed),
            // Receiver code may synchronously start an untracked drag while
            // the old source result is committed but not yet finalized. Do
            // not attach that old generation to the replacement.
            session: self
                .ctx
                .active_session()
                .filter(|session| self.ctx.session_result(*session).is_none()),
            origin_scale: origin.map_or(1.0, |record| record.geometry.scale()),
            source_location: self
                .ctx
                .source()
                .map(|zone| ZoneLocation { window: key, zone }),
            modifiers: Modifiers::empty(),
        };
        let mut active = self.active;
        if *active.peek() != Some(active_drag) {
            active.set(Some(active_drag));
        }
        let mut settle_claim = self.settle_claim;
        if settle_claim.peek().is_some() {
            settle_claim.set(None);
        }
        let mut global_pointer = self.global_pointer;
        let initial_global =
            origin.and_then(|record| record.geometry.to_global(self.ctx.pointer()));
        if *global_pointer.peek() != initial_global {
            global_pointer.set(initial_global);
        }
        let mut over_location = self.over_location;
        if over_location.peek().is_some() {
            over_location.set(None);
        }
    }

    /// The record of the window the in-flight drag started in.
    pub fn active_record(&self) -> Option<WindowRecord<T>> {
        let origin = self.active.peek().as_ref()?.origin;
        self.record(origin)
    }

    pub(super) fn active_drag(&self) -> Option<ActiveDrag> {
        *self.active.peek()
    }

    /// The in-flight pointer in global physical px. `None` until a world
    /// pointer can be resolved or after the world drag finishes.
    pub fn global_pointer(&self) -> Option<Point> {
        *self.global_pointer.read()
    }

    /// Window-qualified source and hover locations for the active world
    /// drag. The legacy `DndContext` id accessors remain unchanged.
    pub fn source_location(&self) -> Option<ZoneLocation> {
        self.active
            .read()
            .as_ref()
            .and_then(|active| active.source_location)
    }

    pub fn over_location(&self) -> Option<ZoneLocation> {
        *self.over_location.read()
    }

    /// Current tracked pointer-drag generation, if this world owns one.
    pub fn drag_session(&self) -> Option<DragSessionId> {
        self.active.peek().as_ref()?.session
    }

    /// Private host-adapter token for the current world drag. The generation
    /// is mandatory; the optional source session adds exactly-once completion
    /// ownership for built-in tracked sources.
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub(crate) fn drag_generation(&self) -> Option<(u64, Option<DragSessionId>)> {
        let active = self.active.read();
        let active = active.as_ref()?;
        Some((active.generation, active.session))
    }

    /// Non-subscribing generation read for imperative host event handlers.
    /// Async resources use [`Self::drag_generation`] so `begin_from` wakes a
    /// new run even when all other drag gates retain the same values.
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub(crate) fn drag_generation_peek(&self) -> Option<(u64, Option<DragSessionId>)> {
        let active = self.active_drag()?;
        Some((active.generation, active.session))
    }

    /// Whether both halves of a captured host token still name the active
    /// drag. For untracked custom sources, `None` is valid only alongside the
    /// matching mandatory world generation.
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub(crate) fn is_drag_generation(
        &self,
        generation: u64,
        session: Option<DragSessionId>,
    ) -> bool {
        let Some(active) = self.active_drag() else {
            return false;
        };
        if !self.ctx.dragging() || active.generation != generation || active.session != session {
            return false;
        }
        session.is_none_or(|session| self.ctx.is_session(session))
    }

    pub(crate) fn is_drag_session(&self, session: DragSessionId) -> bool {
        self.drag_session() == Some(session) && self.ctx.is_session(session)
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
        // Source notification is user code and may synchronously begin a
        // replacement. Its new begin_from call owns the metadata now.
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

    pub(super) fn clear_world_state(&self) {
        let mut active = self.active;
        active.set(None);
        let mut global_pointer = self.global_pointer;
        global_pointer.set(None);
        let mut over_location = self.over_location;
        over_location.set(None);
        let mut settle_claim = self.settle_claim;
        settle_claim.set(None);
    }

    pub(super) fn enter_location(&self, location: ZoneLocation) {
        let mut over_location = self.over_location;
        if *over_location.peek() != Some(location) {
            over_location.set(Some(location));
        }
        let mut ctx = self.ctx;
        ctx.enter(location.zone);
    }

    pub(super) fn clear_hover(&self) {
        let mut ctx = self.ctx;
        if let Some(over) = ctx.over() {
            ctx.leave(over);
        }
        let mut over_location = self.over_location;
        if over_location.peek().is_some() {
            over_location.set(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::core::types::{DragMode, DropEffect};

    thread_local! {
        static WORLD: RefCell<Option<DndWorld<String>>> = const { RefCell::new(None) };
        static COMPLETION: RefCell<Option<Callback<bool>>> = const { RefCell::new(None) };
        static REPLACEMENT_ORIGIN: RefCell<Option<WindowKey>> = const { RefCell::new(None) };
    }

    fn test_app() -> Element {
        let world = use_hook(DndWorld::<String>::new);
        let completion = use_callback(move |dropped: bool| {
            assert!(dropped);
            let replacement =
                REPLACEMENT_ORIGIN.with_borrow(|key| key.expect("replacement origin"));
            let mut ctx = world.context();
            ctx.start(
                "replacement".to_string(),
                None,
                Point::new(20.0, 30.0),
                Point::default(),
                DropEffect::Move,
                DragMode::Pointer,
            );
            world.begin_from(replacement);
        });
        WORLD.with_borrow_mut(|slot| *slot = Some(world));
        COMPLETION.with_borrow_mut(|slot| *slot = Some(completion));
        rsx! {}
    }

    #[test]
    fn source_completion_started_drag_owns_replacement_metadata() {
        let mut dom = VirtualDom::new(test_app);
        dom.rebuild_in_place();
        let world = WORLD.with_borrow(|slot| slot.expect("test world"));
        let completion = COMPLETION.with_borrow(|slot| slot.expect("completion callback"));
        dom.in_runtime(|| {
            let original = WindowKey::auto();
            let replacement = WindowKey::auto();
            REPLACEMENT_ORIGIN.with_borrow_mut(|key| *key = Some(replacement));
            let mut ctx = world.context();
            let session = ctx.start_tracked(
                "original".to_string(),
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                completion,
            );
            world.begin_from(original);
            assert_eq!(world.drag_session(), Some(session));

            assert!(ctx.take().is_some());
            assert!(world.finish_session(session, true));

            assert!(ctx.dragging());
            assert_eq!(ctx.payload().as_deref(), Some("replacement"));
            assert_eq!(
                world.active_drag().map(|drag| drag.origin),
                Some(replacement)
            );
            assert_eq!(world.drag_session(), None);
            world.finish_untracked(false);
        });
    }
}

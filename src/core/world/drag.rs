//! Drag-session anchoring: which window the in-flight drag started in,
//! and the origin-window conversion behind the global pointer.

use dioxus::prelude::*;

use crate::core::types::{DragSessionId, Point};

use super::geometry::WindowKey;
use super::state::{DndWorld, WindowRecord};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ActiveDrag {
    pub(super) origin: WindowKey,
    pub(super) session: Option<DragSessionId>,
}

impl<T: Clone + 'static> DndWorld<T> {
    /// Mark a drag as begun from `key` and reset stale presentation state.
    /// `Draggable` calls this at pickup; call it from custom drag sources
    /// so the world knows which window's client px `ctx.pointer()` is in.
    pub fn begin_from(&self, key: WindowKey) {
        let active_drag = ActiveDrag {
            origin: key,
            // Receiver code may synchronously start an untracked drag while
            // the old source result is committed but not yet finalized. Do
            // not attach that old generation to the replacement.
            session: self
                .ctx
                .active_session()
                .filter(|session| self.ctx.session_result(*session).is_none()),
        };
        let mut active = self.active;
        if *active.peek() != Some(active_drag) {
            active.set(Some(active_drag));
        }
        let mut settling_in = self.settling_in;
        if settling_in.peek().is_some() {
            settling_in.set(None);
        }
    }

    /// The record of the window the in-flight drag started in.
    pub fn active_record(&self) -> Option<WindowRecord<T>> {
        let origin = self.active.peek().as_ref()?.origin;
        self.record(origin)
    }

    /// The in-flight pointer in global physical px: the origin window's
    /// conversion of `ctx.pointer()`. `None` when no drag is active or the
    /// origin window's geometry is unknown.
    pub fn global_pointer(&self) -> Option<Point> {
        let origin = self.active_record()?;
        origin.geometry.to_global(self.ctx.pointer())
    }

    /// Current tracked pointer-drag generation, if this world owns one.
    pub fn drag_session(&self) -> Option<DragSessionId> {
        self.active.peek().as_ref()?.session
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
        let mut settling_in = self.settling_in;
        settling_in.set(None);
    }

    fn clear_hover(&self) {
        let mut ctx = self.ctx;
        if let Some(over) = ctx.over() {
            ctx.leave(over);
        }
    }
}

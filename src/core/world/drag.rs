//! Drag-session anchoring: which window the in-flight drag started in,
//! and the origin-window conversion behind the global pointer.

use dioxus::prelude::*;

use crate::core::types::Point;

use super::geometry::WindowKey;
use super::state::{DndWorld, WindowRecord};

impl<T: Clone + 'static> DndWorld<T> {
    /// Mark a drag as begun from `key` and reset stale presentation state.
    /// `Draggable` calls this at pickup; call it from custom drag sources
    /// so the world knows which window's client px `ctx.pointer()` is in.
    pub fn begin_from(&self, key: WindowKey) {
        let mut active = self.active;
        if *active.peek() != Some(key) {
            active.set(Some(key));
        }
        let mut settling_in = self.settling_in;
        if settling_in.peek().is_some() {
            settling_in.set(None);
        }
    }

    /// The record of the window the in-flight drag started in.
    pub fn active_record(&self) -> Option<WindowRecord<T>> {
        let key = (*self.active.peek())?;
        self.record(key)
    }

    /// The in-flight pointer in global physical px: the origin window's
    /// conversion of `ctx.pointer()`. `None` when no drag is active or the
    /// origin window's geometry is unknown.
    pub fn global_pointer(&self) -> Option<Point> {
        let origin = self.active_record()?;
        origin.geometry.to_global(self.ctx.pointer())
    }
}

//! Settle-glide presentation routing: which window's overlay presents a
//! cross-window drop's settle.

use dioxus::prelude::*;

use super::geometry::WindowKey;
use super::state::DndWorld;

impl<T: Clone + 'static> DndWorld<T> {
    /// Have a cross-window drop's settle glide presented by `key`'s
    /// overlay. No-op unless a settle is actually in flight.
    pub(crate) fn present_settle_in(&self, key: WindowKey) {
        if self.ctx.settling().is_some() {
            let mut settling_in = self.settling_in;
            settling_in.set(Some(key));
        }
    }

    /// The window presenting the current settle glide, if a cross-window
    /// drop routed it somewhere other than the origin.
    pub(crate) fn settling_in(&self) -> Option<WindowKey> {
        *self.settling_in.read()
    }
}

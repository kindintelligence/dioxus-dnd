//! Host-neutral drive operations: entry points for glue that sees the
//! pointer where webviews cannot. No windowing-toolkit types, no OS
//! branches - custom (non-Tao) hosts call these too.

use dioxus::prelude::*;

use crate::core::components::DropCompletion;
use crate::core::types::{Point, ZoneId};

use super::geometry::WindowKey;
use super::state::DndWorld;

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
        if let Some(local) = origin.geometry.to_client(global) {
            ctx.update_pointer(local);
        }
        let zone = self
            .resolve_global(global)
            .and_then(|(rec, local)| rec.registry.hit_test(local));
        match zone {
            Some(z) => ctx.enter(z),
            None => {
                if let Some(over) = ctx.over() {
                    ctx.leave(over);
                }
            }
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
        let effect = ctx.effect();
        let delivered = target.filter(|t| {
            crate::core::components::deliver_drop(
                rec.registry,
                &mut ctx,
                Some(rec.settle),
                DropCompletion::World {
                    world: self,
                    session,
                },
                *t,
                local,
                effect,
            )
        });
        match delivered {
            Some(zone) => {
                self.present_settle_in(rec.key);
                Some(zone)
            }
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
        if let Some(session) = self.drag_session() {
            self.finish_session(session, false);
        } else if self.ctx.dragging() {
            self.finish_untracked(false);
        }
    }

    /// The key of the window the in-flight drag started in, if any - glue
    /// uses it to tell "origin window, webview owns the events" from
    /// "foreign window, I am the drag's eyes".
    pub fn origin_window(&self) -> Option<WindowKey> {
        (self.ctx.dragging() || self.ctx.settling().is_some())
            .then(|| self.active.peek().as_ref().map(|active| active.origin))
            .flatten()
    }
}

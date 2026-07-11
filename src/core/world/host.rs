//! Host-neutral drive operations: entry points for glue that sees the
//! pointer where webviews cannot. No windowing-toolkit types, no OS
//! branches - custom (non-Tao) hosts call these too.

use dioxus::prelude::*;

use crate::core::components::{resolve_release_target, DropCompletion, SettleRoute};
use crate::core::types::{effective_effect, DragMode, Point, ZoneId};

use super::geometry::WindowKey;
use super::state::{DndWorld, ZoneLocation};

/// Host-side drive: entry points for desktop glue that sees the pointer
/// where webviews cannot. Webview pointer events stop at the viewport
/// edge (and under a pointer grab, every non-origin window is fully
/// event-blind on all platforms), so cross-window pointer data must come
/// from the windowing layer: poll the global cursor while a drag is in
/// flight and feed it here.
impl<T: Clone + 'static> DndWorld<T> {
    /// Modifiers currently associated with host delivery. Returns an empty
    /// set outside an active world drag.
    pub fn modifiers(&self) -> Modifiers {
        self.active
            .read()
            .as_ref()
            .map_or_else(Modifiers::empty, |active| active.modifiers)
    }

    /// Update the live modifiers for the active world drag. Late host events
    /// after completion are ignored once the context stops dragging.
    pub fn update_modifiers(&self, modifiers: Modifiers) {
        if !self.ctx.dragging() {
            return;
        }
        let mut active = self.active;
        let Some(mut current) = *active.peek() else {
            return;
        };
        if current.modifiers != modifiers {
            current.modifiers = modifiers;
            active.set(Some(current));
        }
    }

    /// Track an in-flight pointer drag from a host-reported cursor
    /// position (global physical px): updates the shared pointer (in the
    /// origin window's client px, the coordinate anchor everything else
    /// expects) and enters/leaves zones across every joined window. No-op
    /// when nothing is dragging or the origin window is unknown.
    ///
    /// Every host leg converges here, so overlapping legs are safe by
    /// construction rather than by leg exclusivity:
    /// - Two legs reporting the same tick are idempotent: every write below
    ///   is guarded by an equality check, and re-entering the current zone
    ///   is a no-op.
    /// - Legs run on one event-loop thread, so ticks serialize; a staler
    ///   position arriving after a fresher one moves the hover briefly and
    ///   the next tick corrects it - visual, transient, never structural.
    /// - A tick landing after a drop cannot resurrect the drag: the
    ///   `dragging()` gate below is dead after completion, and each leg
    ///   additionally re-validates its captured `BridgeGeneration`
    ///   immediately before calling in, so drag N's sleeper cannot feed
    ///   replacement drag N+1 even during the same event burst.
    pub fn track_global(&self, global: Point) {
        // The kill switch gates the world entry point, not just the tao
        // legs, so a custom host cannot keep cross-window drive alive on a
        // world whose app disabled bridging (see `set_bridging`).
        if !self.bridging_enabled() {
            return;
        }
        let mut ctx = self.ctx;
        if !ctx.dragging() || ctx.mode() != DragMode::Pointer {
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
    /// position (global physical px): last acceptable exact hit in registry
    /// order within whichever window contains the point, else that window's
    /// 48px snap (in its own CSS px), else cancel. Rejecting overlaps are
    /// skipped. Returns the receiving zone. Used by glue that
    /// detects a release the webviews never saw - e.g. a non-origin
    /// window receiving its first pointer event mid-"drag", which proves
    /// the button is up. A no-op returning `None` when nothing is
    /// dragging, so double delivery (webview pointerup plus host echo)
    /// is harmless.
    pub fn drop_at_global(&self, global: Point) -> Option<ZoneId>
    where
        T: PartialEq,
    {
        // Same kill-switch gate as `track_global`. An in-flight drag is not
        // stranded: the origin webview still completes in-viewport releases
        // itself, and out-of-viewport ones reconcile through the same
        // held-button paths a Wayland session uses.
        if !self.bridging_enabled() {
            return None;
        }
        let mut ctx = self.ctx;
        if !ctx.dragging() || ctx.mode() != DragMode::Pointer {
            return None;
        }
        // The release is authoritative even when no final tracking tick ran.
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
        // Release selection is acceptance-aware even for an exact overlap:
        // a rejecting later registry record must not mask an accepting one.
        let target = ctx
            .payload()
            .and_then(|p| resolve_release_target(rec.registry, &p, local, 48.0));
        // Imperative host delivery peeks the active snapshot rather than
        // subscribing the bridge runtime to modifier updates.
        let modifiers = self
            .active
            .peek()
            .as_ref()
            .map_or_else(Modifiers::empty, |active| active.modifiers);
        let effect = effective_effect(ctx.effect(), modifiers);
        let delivered = target.filter(|t| {
            crate::core::components::deliver_drop(
                rec.registry,
                &mut ctx,
                SettleRoute {
                    flag: Some(rec.settle),
                    owner: Some((self, rec.key)),
                },
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

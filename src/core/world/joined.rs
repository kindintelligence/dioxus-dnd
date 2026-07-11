//! A provider's world membership and the joined-window handle: qualified
//! zone resolution, foreign-window lookup, and overlay presentation.

use dioxus::prelude::{ReadableExt, WritableExt};

use crate::core::types::{Point, ZoneId};

use super::geometry::{WindowGeometry, WindowKey};
use super::state::{DndWorld, WindowRecord, ZoneLocation};

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
    /// should use this rather than the legacy id-only context method.
    pub fn enter(&self, location: ZoneLocation) {
        self.world.enter_location(location);
    }

    /// Clear both qualified world hover and the legacy context hover.
    pub fn clear_hover(&self) {
        self.world.clear_hover();
    }

    /// Whether this exact window/zone pair owns the world hover.
    pub fn is_over(&self, zone: ZoneId) -> bool {
        self.world.over_location() == Some(self.location(zone))
    }

    /// Latest global pointer converted into this window's client CSS
    /// coordinates. If geometry disappeared mid-gesture, the origin window
    /// retains its established context-local fallback.
    pub fn local_pointer(&self) -> Option<Point> {
        if let Some(local) = self
            .world
            .global_pointer()
            .and_then(|global| self.geometry.to_client(global))
        {
            return Some(local);
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

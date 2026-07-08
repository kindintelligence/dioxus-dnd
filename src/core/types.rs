//! Shared identity, geometry and result types used by every drop module.

use std::sync::atomic::{AtomicU64, Ordering};

/// Auto-generated ids start far above any id a human writes by hand. The
/// zone registry replaces records by id, so if the auto sequence began at 1
/// it would eventually collide with explicit low ids (`ZoneId(11)`) in the
/// same provider and *silently knock that zone out of the registry* - e.g. a
/// `BoardSlot`'s auto id landing on a neighboring column's hand-picked id.
/// Reserving everything below 2^32 for explicit ids makes the collision
/// impossible: any id that fits in a `u32` can never clash with an auto id.
const AUTO_ID_BASE: u64 = 1 << 32;

static NEXT_ID: AtomicU64 = AtomicU64::new(AUTO_ID_BASE);

/// Identifies a drop zone (a list, a column, a canvas, a tree node…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ZoneId(pub u64);

impl ZoneId {
    /// Generate a process-unique zone id. Handy when you don't care about
    /// stable ids across renders - call it inside `use_hook` so it sticks.
    ///
    /// Auto ids live at `2^32` and above; explicit ids below that (anything
    /// that fits in a `u32`) can never collide with them.
    pub fn auto() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl From<u64> for ZoneId {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

/// Identifies a draggable item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DragId(pub u64);

impl DragId {
    /// Generate a process-unique drag id.
    pub fn auto() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl From<u64> for DragId {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

/// A 2D point in CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

/// An axis-aligned rectangle in client (viewport) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Is the point inside (inclusive of edges)?
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.x + self.width && p.y >= self.y && p.y <= self.y + self.height
    }

    /// Center point.
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Top-left corner.
    pub fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// Which browser/input path a drag source should use.
///
/// Native HTML5 drag gives you `DataTransfer` and browser-level behavior
/// such as dragging into another tab or application. Pointer-driven drag is
/// synthetic: it uses pointer events and the crate's own state/registry, so
/// the browser does not create or style a native drag image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragInputMode {
    /// Use pointer events for mouse, touch and pen.
    #[default]
    Pointer,
    /// Use native HTML5 drag only.
    Native,
    /// Current compatibility behavior: native mouse, pointer touch/pen.
    Hybrid,
}

impl DragInputMode {
    /// Should this pointer event drive the synthetic pointer path?
    pub fn uses_pointer(self, pointer_type: &str) -> bool {
        match self {
            DragInputMode::Pointer => true,
            DragInputMode::Native => false,
            DragInputMode::Hybrid => pointer_type != "mouse",
        }
    }

    /// Should the rendered element opt into native HTML5 drag events?
    pub fn uses_native(self) -> bool {
        matches!(self, DragInputMode::Native | DragInputMode::Hybrid)
    }
}

/// How the current drag is being driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragMode {
    /// Pointer-event driven drag.
    #[default]
    Pointer,
    /// Native HTML5 drag.
    Native,
    /// Keyboard-driven drag (Space/Enter to pick up, arrows to navigate).
    Keyboard,
}

/// The visual/semantic effect of a drop, mirroring the HTML5
/// `dropEffect`/`effectAllowed` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropEffect {
    #[default]
    Move,
    Copy,
    Link,
    None,
}

impl DropEffect {
    /// The string the native `DataTransfer` API expects.
    pub fn as_str(&self) -> &'static str {
        match self {
            DropEffect::Move => "move",
            DropEffect::Copy => "copy",
            DropEffect::Link => "link",
            DropEffect::None => "none",
        }
    }
}

/// Resolve the drop effect a drag should use given the currently held
/// modifier keys - the file-manager convention: **Ctrl/Cmd forces Copy**,
/// **Alt forces Link**, otherwise the drag's base effect applies. A base of
/// `None` (drops disabled) is never overridden.
pub fn effective_effect(base: DropEffect, modifiers: dioxus::prelude::Modifiers) -> DropEffect {
    use dioxus::prelude::Modifiers;
    if base == DropEffect::None {
        return base;
    }
    if modifiers.contains(Modifiers::CONTROL) || modifiers.contains(Modifiers::META) {
        DropEffect::Copy
    } else if modifiers.contains(Modifiers::ALT) {
        DropEffect::Link
    } else {
        base
    }
}

/// Everything a consumer needs to know about a completed drop.
#[derive(Debug, Clone, PartialEq)]
pub struct DropOutcome<T> {
    /// The payload that was being dragged.
    pub payload: T,
    /// The zone the drag originated from, if the `Draggable` declared one.
    pub from: Option<ZoneId>,
    /// The zone that received the drop.
    pub to: ZoneId,
    /// The effect the drag was started with.
    pub effect: DropEffect,
    /// Which input path produced this completed drop.
    pub mode: DragMode,
    /// Pointer position in client (viewport) coordinates at drop time.
    pub client: Point,
    /// Pointer position relative to the drop zone's element.
    pub element: Point,
    /// Where inside the dragged element the pointer grabbed it, at pickup.
    /// `element - grab` is where the element's top-left should land - what
    /// [`crate::canvas::CanvasDropZone`] uses for exact free-position drops.
    /// Zero for keyboard drops (no pointer offset).
    pub grab: Point,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::prelude::Modifiers;

    #[test]
    fn modifier_effects_follow_convention() {
        assert_eq!(
            effective_effect(DropEffect::Move, Modifiers::empty()),
            DropEffect::Move
        );
        assert_eq!(
            effective_effect(DropEffect::Move, Modifiers::CONTROL),
            DropEffect::Copy
        );
        assert_eq!(
            effective_effect(DropEffect::Move, Modifiers::META),
            DropEffect::Copy
        );
        assert_eq!(
            effective_effect(DropEffect::Move, Modifiers::ALT),
            DropEffect::Link
        );
        // Ctrl wins over Alt when both held
        assert_eq!(
            effective_effect(DropEffect::Move, Modifiers::CONTROL | Modifiers::ALT),
            DropEffect::Copy
        );
        // disabled zones stay disabled
        assert_eq!(
            effective_effect(DropEffect::None, Modifiers::CONTROL),
            DropEffect::None
        );
    }

    #[test]
    fn input_modes_select_pointer_and_native_paths() {
        assert!(DragInputMode::Pointer.uses_pointer("mouse"));
        assert!(DragInputMode::Pointer.uses_pointer("touch"));
        assert!(!DragInputMode::Pointer.uses_native());

        assert!(!DragInputMode::Native.uses_pointer("mouse"));
        assert!(!DragInputMode::Native.uses_pointer("touch"));
        assert!(DragInputMode::Native.uses_native());

        assert!(!DragInputMode::Hybrid.uses_pointer("mouse"));
        assert!(DragInputMode::Hybrid.uses_pointer("touch"));
        assert!(DragInputMode::Hybrid.uses_native());
    }

    #[test]
    fn rect_contains_and_center() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(Point::new(10.0, 10.0)));
        assert!(r.contains(Point::new(110.0, 60.0)));
        assert!(!r.contains(Point::new(111.0, 30.0)));
        assert_eq!(r.center(), Point::new(60.0, 35.0));
    }

    /// Auto ids must never collide with hand-written explicit ids: the zone
    /// registry replaces records by id, so a `BoardSlot`'s auto id landing on
    /// a column's explicit `ZoneId(11)` silently unregistered the slot. All
    /// auto ids live at `2^32` and above; explicit `u32`-range ids are safe.
    #[test]
    fn auto_ids_stay_above_the_explicit_range() {
        for _ in 0..64 {
            assert!(ZoneId::auto().0 >= super::AUTO_ID_BASE);
            assert!(DragId::auto().0 >= super::AUTO_ID_BASE);
        }
    }
}

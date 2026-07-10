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
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

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

/// Identifies one pointer-drag gesture from pickup through its exactly-once
/// completion. Unlike [`DragId`], which applications may use as item
/// identity, this id is generated afresh for every gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DragSessionId(pub u64);

impl DragSessionId {
    pub(crate) fn auto() -> Self {
        Self(NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed))
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

/// One side of a drop zone's rectangle - which edge the pointer is nearest.
/// The vocabulary for insertion indicators on a bare drop zone: "drop
/// *above* this row" is `Top`, "append after it" is `Bottom`. Edges are
/// physical (styling targets a screen side), not logical, so they don't
/// mirror under RTL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

impl Edge {
    /// The attribute value [`crate::core::components::DropZone`] renders in
    /// `data-edge`.
    pub fn as_str(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Right => "right",
            Edge::Bottom => "bottom",
            Edge::Left => "left",
        }
    }
}

/// Which edges compete in [`edge_of`]. Named by stacking direction, like
/// [`crate::sortable::Axis`]: a `Vertical` list stacks items top to bottom,
/// so its insertion candidates are the top and bottom edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeSet {
    /// All four edges.
    #[default]
    All,
    /// Top and bottom only - items stacked vertically.
    Vertical,
    /// Left and right only - items flowing horizontally.
    Horizontal,
}

impl EdgeSet {
    fn allows(self, edge: Edge) -> bool {
        match self {
            EdgeSet::All => true,
            EdgeSet::Vertical => matches!(edge, Edge::Top | Edge::Bottom),
            EdgeSet::Horizontal => matches!(edge, Edge::Left | Edge::Right),
        }
    }
}

/// The zone edge nearest to `point` - the generic closest-edge primitive
/// for insertion indicators (drop above/below, insert left/right).
///
/// The point is clamped into the rect first, so out-of-range coordinates
/// (a touch drop snapped from outside, a degenerate rect) still resolve.
/// Ties prefer `Top`, then `Bottom`, then `Left`, then `Right`, so a dead
/// center point over `EdgeSet::All` reads `Top`.
///
/// [`crate::core::components::DropZone`]'s opt-in `edge` prop renders this
/// as a live `data-edge` attribute and delivers it in [`DropOutcome::edge`];
/// call it directly for custom zones (e.g. against
/// [`DropOutcome::element`] with the zone rect at the origin).
pub fn edge_of(point: Point, rect: Rect, edges: EdgeSet) -> Edge {
    let w = rect.width.max(0.0);
    let h = rect.height.max(0.0);
    let x = (point.x - rect.x).clamp(0.0, w);
    let y = (point.y - rect.y).clamp(0.0, h);
    let candidates = [
        (Edge::Top, y),
        (Edge::Bottom, h - y),
        (Edge::Left, x),
        (Edge::Right, w - x),
    ];
    let mut best: Option<(Edge, f64)> = None;
    for (edge, distance) in candidates {
        if !edges.allows(edge) {
            continue;
        }
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((edge, distance));
        }
    }
    // Every EdgeSet variant allows at least two edges.
    best.expect("EdgeSet allows at least one edge").0
}

/// Horizontal layout direction. Keyboard navigation mirrors under RTL:
/// ArrowRight ascends instead of descending (the WAI-ARIA tree convention)
/// and spatial zone ordering runs right-to-left within a row. Set it on
/// [`crate::core::DndProvider`] (or via
/// [`crate::core::ZoneRegistry::set_direction`] when using the hooks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left-to-right (the default).
    #[default]
    Ltr,
    /// Right-to-left (Arabic, Hebrew, ...).
    Rtl,
}

/// How the current drag is being driven.
///
/// Non-exhaustive: input paths accrete (gamepad and switch-access drags
/// are plausible futures), so compare against the variants you handle
/// rather than matching exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum DragMode {
    /// Pointer-event driven drag.
    #[default]
    Pointer,
    /// Keyboard-driven drag (Space/Enter to pick up, arrows to navigate).
    Keyboard,
}

/// Which kind of pointer device is driving a pointer drag. Recorded at
/// pickup (see [`crate::core::DndContext::set_pointer_kind`]) so
/// host-side glue can decide which input layers need bridging: a touch
/// contact is implicitly captured by the browser - the source element
/// keeps receiving the whole gesture, out-of-viewport moves and the
/// release included - while mouse and pen go blind at the viewport edge
/// whenever native capture is unavailable. Feeding a captured pointer's
/// drag from a second host-side source (cursor pollers, raw input)
/// double-drives it: on Windows the touch-synthesized mouse cursor
/// trails the finger, and its synthesized button transitions can end
/// the drag early.
///
/// Non-exhaustive: pointer taxonomies grow with input hardware. Glue
/// deciding whether to bridge must use [`PointerKind::implicitly_captured`],
/// which encodes the safe default for kinds it has never heard of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PointerKind {
    /// A mouse - or anything unrecognized, because the safe default for
    /// glue is to bridge (an unbridged blind pointer loses drops; a
    /// double-driven captured one merely jitters).
    #[default]
    Mouse,
    /// A touch contact; implicitly captured by the browser.
    Touch,
    /// A pen/stylus. The engines this crate targets route pen like
    /// mouse for capture purposes, so glue should bridge it too.
    Pen,
}

impl PointerKind {
    /// Map a DOM `pointerType` string (`"mouse"` / `"touch"` / `"pen"`,
    /// or empty when the browser cannot tell) to a kind.
    pub fn from_pointer_type(pointer_type: &str) -> Self {
        match pointer_type {
            "touch" => Self::Touch,
            "pen" => Self::Pen,
            _ => Self::Mouse,
        }
    }

    /// Does the browser implicitly capture this pointer to the source
    /// element - i.e. does the webview itself keep streaming the whole
    /// gesture, making host-side bridging unnecessary (and harmful)?
    pub fn implicitly_captured(&self) -> bool {
        matches!(self, Self::Touch)
    }
}

/// How a `Draggable` (or a whole-row sortable) shares touch input with the
/// page's native gestures.
///
/// Mouse and pen are unaffected: they always promote on plain travel past
/// the threshold. This only decides what a *finger* means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TouchSense {
    /// The safe default for anything that can sit in a scrollable view:
    /// the element carries `touch-action: pan-y`, so a vertical swipe keeps
    /// scrolling the page, while a short hold (250ms with the finger still)
    /// or a sideways-dominant pull picks the item up. Once a drag begins,
    /// further touch moves are consumed so the page stays put.
    #[default]
    Auto,
    /// The element owns every touch from the first pixel
    /// (`touch-action: none`): any travel past the threshold drags, and
    /// finger-scrolling across the element is disabled - the behavior of
    /// releases before 2.5. Reach for it on surfaces that never scroll
    /// (a full-screen canvas, a game board).
    Immediate,
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
    /// The zone edge nearest the release point (see [`edge_of`]) - `Some`
    /// only when the receiving `DropZone` opted in via its `edge` prop and
    /// the drop was pointer-driven. Keyboard drops carry `None` (their
    /// "release point" is the zone center, which names no edge); treat it
    /// as your neutral intent, e.g. append.
    pub edge: Option<Edge>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::prelude::Modifiers;

    #[test]
    fn drag_session_ids_are_fresh_for_each_gesture() {
        let first = DragSessionId::auto();
        let second = DragSessionId::auto();

        assert_ne!(first, second);
        assert!(second.0 > first.0);
    }

    #[test]
    fn pointer_kind_maps_dom_pointer_types() {
        assert_eq!(PointerKind::from_pointer_type("mouse"), PointerKind::Mouse);
        assert_eq!(PointerKind::from_pointer_type("touch"), PointerKind::Touch);
        assert_eq!(PointerKind::from_pointer_type("pen"), PointerKind::Pen);
        // Unrecognized (including the empty string some browsers report)
        // must fall back to Mouse: glue then bridges, the safe default.
        assert_eq!(PointerKind::from_pointer_type(""), PointerKind::Mouse);
        assert_eq!(
            PointerKind::from_pointer_type("gamepad"),
            PointerKind::Mouse
        );
        // Only touch is implicitly captured; mouse AND pen need bridging.
        assert!(PointerKind::Touch.implicitly_captured());
        assert!(!PointerKind::Mouse.implicitly_captured());
        assert!(!PointerKind::Pen.implicitly_captured());
        assert_eq!(PointerKind::default(), PointerKind::Mouse);
    }

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
    fn edge_of_picks_the_nearest_allowed_edge() {
        let r = Rect::new(100.0, 50.0, 200.0, 40.0);
        assert_eq!(edge_of(Point::new(200.0, 55.0), r, EdgeSet::All), Edge::Top);
        assert_eq!(
            edge_of(Point::new(200.0, 85.0), r, EdgeSet::All),
            Edge::Bottom
        );
        assert_eq!(
            edge_of(Point::new(103.0, 70.0), r, EdgeSet::All),
            Edge::Left
        );
        assert_eq!(
            edge_of(Point::new(297.0, 70.0), r, EdgeSet::All),
            Edge::Right
        );
    }

    /// A wide list row would read Left/Right in its end strips under `All`;
    /// restricting to the stacking axis keeps insertion indicators sane.
    #[test]
    fn edge_of_respects_the_edge_set() {
        let r = Rect::new(0.0, 0.0, 300.0, 40.0);
        assert_eq!(edge_of(Point::new(4.0, 15.0), r, EdgeSet::All), Edge::Left);
        assert_eq!(
            edge_of(Point::new(4.0, 15.0), r, EdgeSet::Vertical),
            Edge::Top
        );
        assert_eq!(
            edge_of(Point::new(4.0, 25.0), r, EdgeSet::Vertical),
            Edge::Bottom
        );
        assert_eq!(
            edge_of(Point::new(100.0, 39.0), r, EdgeSet::Horizontal),
            Edge::Left
        );
    }

    #[test]
    fn edge_of_clamps_and_breaks_ties_toward_top_then_left() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        // Dead center: everything ties; documented order wins.
        assert_eq!(edge_of(Point::new(50.0, 50.0), r, EdgeSet::All), Edge::Top);
        assert_eq!(
            edge_of(Point::new(50.0, 50.0), r, EdgeSet::Horizontal),
            Edge::Left
        );
        // Out-of-rect points clamp in before comparing.
        assert_eq!(
            edge_of(Point::new(-30.0, 50.0), r, EdgeSet::All),
            Edge::Left
        );
        assert_eq!(
            edge_of(Point::new(50.0, 500.0), r, EdgeSet::Vertical),
            Edge::Bottom
        );
        // A degenerate rect resolves instead of panicking.
        assert_eq!(
            edge_of(
                Point::new(0.0, 0.0),
                Rect::new(0.0, 0.0, -5.0, 0.0),
                EdgeSet::All
            ),
            Edge::Top
        );
    }

    #[test]
    fn edge_strings_match_the_attribute_contract() {
        assert_eq!(Edge::Top.as_str(), "top");
        assert_eq!(Edge::Right.as_str(), "right");
        assert_eq!(Edge::Bottom.as_str(), "bottom");
        assert_eq!(Edge::Left.as_str(), "left");
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

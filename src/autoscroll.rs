//! Auto-scroll: when a drag hovers near the edge of a scrollable container,
//! scroll it - the missing piece for long lists and tall boards.
//!
//! Scrolling and measuring go through Dioxus's `MountedData`: `dragover`
//! (native boundary drags) and active `pointermove` events (in-app pointer
//! drags via [`crate::core::Draggable`]) feed pointer positions; when the
//! pointer sits within `threshold` px of an edge, the container is scrolled
//! by up to `speed` px per event, scaled by proximity.
//!
//! Scroll *observation* (the rect-refresh ping and the `on_scroll` prop)
//! rides the events that cause or accompany scrolling - wheel, pointer
//! contact moves, and the auto-scrolls this component performs - each of
//! which samples the offset through `MountedData` and reports when it
//! changed. It has to work this way: dioxus-web 0.7 never delivers
//! element-level `scroll` events to `onscroll` handlers, and its eval
//! channel drops messages that resolve after the receiver parked, so
//! neither a Rust `onscroll` nor a JS listener bridge can carry the
//! signal. The known blind spot is a scroll no event accompanies (a
//! programmatic `scroll-to-index` with the pointer at rest) - the code
//! that initiates one should update its own state, and the next pointer
//! or wheel activity trues everything up.
//!
//! ```text
//! AutoScroll {
//!     style: "height: 300px; overflow-y: auto;",
//!     for item in long_list { Row { item } }
//! }
//! ```

use std::rc::Rc;

use dioxus::html::geometry::PixelsVector2D;
use dioxus::html::{MountedData, ScrollBehavior};
use dioxus::prelude::*;

use crate::core::hooks::use_rect_refresh_provider;
use crate::core::{Point, Rect};

/// Which axes to auto-scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollAxis {
    /// Vertical only (the common case for lists).
    #[default]
    Y,
    /// Horizontal only.
    X,
    /// Both.
    Both,
}

/// Per-axis scroll delta for a pointer at `pos` inside `rect`.
/// Returns `(dx, dy)`, each in `-speed..=speed`, scaled by how deep into the
/// edge band the pointer is. Pure, for testability.
pub fn edge_delta(
    pos: Point,
    rect: Rect,
    threshold: f64,
    speed: f64,
    axis: ScrollAxis,
) -> (f64, f64) {
    // Only scroll while the pointer is within the container. Under pointer
    // capture the container keeps receiving (bubbled) pointermove events even
    // when the cursor is far outside it; without this gate the delta pins to
    // full `speed` and the container scrolls forever. A pointer right at the
    // edge still scrolls - `contains` is edge-inclusive.
    if !rect.contains(pos) {
        return (0.0, 0.0);
    }
    let ramp = |dist_into_band: f64| (dist_into_band / threshold.max(1.0)).clamp(0.0, 1.0) * speed;
    // Scroll toward whichever edge is nearer on this axis. Choosing the nearer
    // edge (rather than a plain `if left else if right`) means a container
    // narrower than `2 * threshold` - where the pointer is within the band of
    // both edges at once - still scrolls both ways instead of the near edge
    // always winning.
    let edge = |lo: f64, hi: f64| -> f64 {
        if lo <= hi {
            if lo < threshold {
                -ramp(threshold - lo)
            } else {
                0.0
            }
        } else if hi < threshold {
            ramp(threshold - hi)
        } else {
            0.0
        }
    };
    let mut dx = 0.0;
    let mut dy = 0.0;
    if matches!(axis, ScrollAxis::X | ScrollAxis::Both) {
        dx = edge(pos.x - rect.x, rect.x + rect.width - pos.x);
    }
    if matches!(axis, ScrollAxis::Y | ScrollAxis::Both) {
        dy = edge(pos.y - rect.y, rect.y + rect.height - pos.y);
    }
    (dx, dy)
}

/// Whether a pointer move should drive auto-scroll.
///
/// Mouse pointer drags report contact through held buttons. Touch and pen
/// paths commonly report pressure during contact, and some platforms also
/// expose held buttons for them.
fn pointer_move_should_scroll(
    pointer_type: &str,
    pressure: f32,
    has_held_button: bool,
    active: Option<bool>,
) -> bool {
    match active {
        Some(active) => active,
        None => has_held_button || (pointer_type != "mouse" && pressure > 0.0),
    }
}

/// A scrollable container that scrolls itself while a drag hovers near its
/// edges. Give it the `overflow` CSS yourself (via `style`/`class`).
#[component]
pub fn AutoScroll(
    /// Edge band size in px.
    #[props(default = 48.0)]
    threshold: f64,
    /// Max scroll px per event.
    #[props(default = 24.0)]
    speed: f64,
    /// Axes to scroll.
    #[props(default)]
    axis: ScrollAxis,
    /// Optional external drag-state gate. `Some(true)` scrolls on pointer
    /// movement, `Some(false)` suppresses it, and `None` uses the built-in
    /// pointer contact heuristic.
    #[props(default)]
    active: Option<bool>,
    /// Fired with the container's scroll offset when a sample sees it
    /// changed - after the auto-scroll's own scrolling, a wheel/trackpad
    /// scroll, or pointer movement over the container - following the
    /// rect-refresh ping. Drive a windowed (virtualized) list from
    /// `offset.y`. See the module docs for how observation works and its
    /// one blind spot.
    #[props(default)]
    on_scroll: Option<EventHandler<Point>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    // In-flight guard so a burst of dragover events doesn't queue a pile of
    // overlapping async scrolls.
    let busy = use_signal(|| false);
    // Scrolling this container moves everything inside it, so cached
    // hit-test rects go stale the moment we scroll. Create-or-inherit the
    // tree's rect-refresh channel: with a DndProvider above we join its
    // channel; without one (self-contained sortables, native pages) we
    // anchor a channel ourselves so the components inside can register.
    let refresh = use_rect_refresh_provider();
    // Last offset `sample` saw, deduplicating pings and on_scroll reports.
    let last_offset = use_signal(Point::default);

    // The observer: read the offset, and when it moved, ping the
    // rect-refresh channel and report to on_scroll. Called from every
    // event that can cause or accompany scrolling; the dedup makes the
    // common nothing-changed case one cheap async read.
    let sample = move || {
        let Some(m) = mounted.peek().clone() else {
            return;
        };
        let mut last_offset = last_offset;
        spawn(async move {
            if let Ok(o) = m.get_scroll_offset().await {
                let now = Point::new(o.x, o.y);
                if *last_offset.peek() != now {
                    last_offset.set(now);
                    // The zones inside just moved: re-measure (free while
                    // no drag is in flight), then let the app re-slice its
                    // window.
                    refresh.refresh_all();
                    if let Some(h) = &on_scroll {
                        h.call(now);
                    }
                }
            }
        });
    };

    let scroll_for = move |point: Point| {
        let Some(m) = mounted.peek().clone() else {
            return;
        };
        if *busy.peek() {
            return;
        }
        let mut busy = busy;
        busy.set(true);
        spawn(async move {
            if let Ok(r) = m.get_client_rect().await {
                let rect = Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height);
                let (dx, dy) = edge_delta(point, rect, threshold, speed, axis);
                if dx != 0.0 || dy != 0.0 {
                    if let Ok(offset) = m.get_scroll_offset().await {
                        let _ = m
                            .scroll(
                                PixelsVector2D::new(offset.x + dx, offset.y + dy),
                                ScrollBehavior::Instant,
                            )
                            .await;
                        // Everything just moved under the drag: re-measure
                        // so hover and the eventual drop hit what the user
                        // sees, not where things sat at pickup - and report
                        // the new offset so a windowed list re-slices.
                        refresh.refresh_all();
                        sample();
                    }
                }
            }
            busy.set(false);
        });
    };

    rsx! {
        div {
            onmounted: move |evt: Event<MountedData>| {
                mounted.set(Some(evt.data()));
                // Report the initial offset (restored scroll positions
                // exist) so windowing starts aligned.
                sample();
            },
            // Wheel and trackpad scrolling, idle or mid-drag. Wheel events
            // go to the element under the cursor regardless of pointer
            // capture, and the sample's async offset read resolves after
            // the browser applied the scroll this event causes.
            onwheel: move |_| sample(),
            // Native boundary drags: dragover fires continuously while
            // hovering. Note: no prevent_default here - drop permission stays
            // the business of the zones inside.
            ondragover: move |evt: DragEvent| {
                let c = evt.client_coordinates();
                scroll_for(Point::new(c.x, c.y));
            },
            // Pointer-driven drags: mouse uses held buttons, while touch and
            // pen commonly report pressure during contact.
            onpointermove: move |evt: PointerEvent| {
                if pointer_move_should_scroll(
                    &evt.pointer_type(),
                    evt.pressure(),
                    !evt.held_buttons().is_empty(),
                    active,
                ) {
                    let c = evt.client_coordinates();
                    scroll_for(Point::new(c.x, c.y));
                }
                // Sample on every move, contact or hover: it trues up the
                // window after scrollbar drags and programmatic scrolls
                // the moment the pointer stirs.
                sample();
            },
            ..attributes,
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_ramp_toward_edges() {
        let rect = Rect::new(0.0, 0.0, 200.0, 400.0);
        // dead center: no scroll
        assert_eq!(
            edge_delta(Point::new(100.0, 200.0), rect, 48.0, 24.0, ScrollAxis::Both),
            (0.0, 0.0)
        );
        // near top: negative dy, magnitude below max
        let (_, dy) = edge_delta(Point::new(100.0, 10.0), rect, 48.0, 24.0, ScrollAxis::Y);
        assert!((-24.0..0.0).contains(&dy));
        // at the very bottom edge: full speed down
        let (_, dy) = edge_delta(Point::new(100.0, 400.0), rect, 48.0, 24.0, ScrollAxis::Y);
        assert_eq!(dy, 24.0);
        // axis filtering: Y-only ignores horizontal proximity
        let (dx, _) = edge_delta(Point::new(1.0, 200.0), rect, 48.0, 24.0, ScrollAxis::Y);
        assert_eq!(dx, 0.0);
    }

    #[test]
    fn no_scroll_when_pointer_leaves_the_container() {
        // Under pointer capture a bubbled move can report a cursor far outside
        // the container; that must not scroll (previously it pinned to full
        // speed forever).
        let rect = Rect::new(0.0, 0.0, 200.0, 400.0);
        assert_eq!(
            edge_delta(Point::new(100.0, 900.0), rect, 48.0, 24.0, ScrollAxis::Both),
            (0.0, 0.0)
        );
        assert_eq!(
            edge_delta(Point::new(-50.0, 200.0), rect, 48.0, 24.0, ScrollAxis::Both),
            (0.0, 0.0)
        );
    }

    #[test]
    fn narrow_container_scrolls_toward_the_nearer_edge() {
        // 40px wide, band 48: the pointer is within both edges' bands, so the
        // nearer edge must win rather than the left always winning.
        let rect = Rect::new(0.0, 0.0, 40.0, 400.0);
        let (dx, _) = edge_delta(Point::new(35.0, 200.0), rect, 48.0, 24.0, ScrollAxis::X);
        assert!(
            dx > 0.0,
            "near the right edge should scroll right, got {dx}"
        );
        let (dx, _) = edge_delta(Point::new(5.0, 200.0), rect, 48.0, 24.0, ScrollAxis::X);
        assert!(dx < 0.0, "near the left edge should scroll left, got {dx}");
    }

    #[test]
    fn pointer_scroll_predicate_matches_active_pointer_drags() {
        assert!(
            pointer_move_should_scroll("mouse", 0.0, true, None),
            "default mouse pointer drags keep a held button during movement"
        );
        assert!(
            !pointer_move_should_scroll("mouse", 0.0, false, None),
            "passive mouse hover must not scroll"
        );
        assert!(
            pointer_move_should_scroll("touch", 0.5, false, None),
            "touch contact can report pressure instead of held buttons"
        );
        assert!(
            pointer_move_should_scroll("pen", 0.0, true, None),
            "pen contact can also surface as held buttons"
        );
        assert!(
            !pointer_move_should_scroll("touch", 0.5, false, Some(false)),
            "callers that track drag state can explicitly gate scrolling off"
        );
        assert!(
            pointer_move_should_scroll("mouse", 0.0, false, Some(true)),
            "callers that track drag state can explicitly gate scrolling on"
        );
    }
}

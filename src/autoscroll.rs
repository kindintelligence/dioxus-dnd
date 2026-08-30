#![doc = include_str!("../docs/api/autoscroll.md")]

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::html::geometry::PixelsVector2D;
use dioxus::html::{MountedData, ScrollBehavior};
use dioxus::prelude::*;

use crate::core::hooks::use_rect_refresh_provider;
use crate::core::{Point, Rect};

static NEXT_SCROLL_CONTAINER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ScrollContainerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScrollEntry {
    id: ScrollContainerId,
    rect: Rect,
    available: bool,
    blocked: bool,
}

/// Coordinates nested auto-scroll surfaces. The smallest available
/// containing rect owns movement; when it reaches a boundary it marks itself
/// blocked and the next containing surface takes over.
pub(crate) struct ScrollCoordinator {
    entries: Signal<Vec<ScrollEntry>>,
}

impl Copy for ScrollCoordinator {}
impl Clone for ScrollCoordinator {
    fn clone(&self) -> Self {
        *self
    }
}
impl PartialEq for ScrollCoordinator {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl ScrollCoordinator {
    fn new() -> Self {
        Self {
            entries: Signal::new(Vec::new()),
        }
    }

    fn update(&mut self, id: ScrollContainerId, rect: Rect, available: bool) {
        let mut entries = self.entries.write();
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
            entry.rect = rect;
            entry.available = available;
        } else {
            entries.push(ScrollEntry {
                id,
                rect,
                available,
                blocked: false,
            });
        }
    }

    fn set_blocked(&mut self, id: ScrollContainerId, blocked: bool) {
        if let Some(entry) = self.entries.write().iter_mut().find(|entry| entry.id == id) {
            entry.blocked = blocked;
        }
    }

    fn owner(&self, point: Point) -> Option<ScrollContainerId> {
        self.entries
            .peek()
            .iter()
            .filter(|entry| entry.available && !entry.blocked && entry.rect.contains(point))
            .min_by(|a, b| {
                let aa = a.rect.width.max(0.0) * a.rect.height.max(0.0);
                let ba = b.rect.width.max(0.0) * b.rect.height.max(0.0);
                aa.total_cmp(&ba)
            })
            .map(|entry| entry.id)
    }

    fn unregister(&mut self, id: ScrollContainerId) {
        if let Ok(mut entries) = self.entries.try_write() {
            entries.retain(|entry| entry.id != id);
        }
    }
}

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
    let threshold = threshold.max(1.0);
    let speed = speed.max(0.0);
    let ramp = |dist_into_band: f64| (dist_into_band / threshold).clamp(0.0, 1.0) * speed;
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

/// Convert a pixels-per-second velocity into one frame's movement.
pub fn frame_delta(velocity: (f64, f64), elapsed_seconds: f64) -> (f64, f64) {
    let elapsed = if elapsed_seconds.is_finite() {
        elapsed_seconds.clamp(0.0, 0.1)
    } else {
        0.0
    };
    (velocity.0 * elapsed, velocity.1 * elapsed)
}

fn animation_elapsed_seconds(elapsed_seconds: f32) -> f64 {
    let elapsed_seconds = f64::from(elapsed_seconds);
    if elapsed_seconds.is_finite() && elapsed_seconds > 0.0 {
        elapsed_seconds
    } else {
        // The clock animation is 16 ms. A renderer that omits its elapsed
        // duration still advances at the declared cadence.
        0.016
    }
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

/// Select a host-driven pointer sample only when the caller explicitly
/// confirms that its drag is active. An externally retained coordinate must
/// never keep scrolling idle or settling content.
fn external_pointer_sample(active: Option<bool>, drag_pointer: Option<Point>) -> Option<Point> {
    (active == Some(true)).then_some(drag_pointer).flatten()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClockOwner {
    Pointer,
    NativeDrag,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClockToken {
    owner: ClockOwner,
    epoch: u64,
}

fn resolved_velocity(speed: f64, speed_px_per_second: Option<f64>) -> f64 {
    speed_px_per_second.unwrap_or(speed * 60.0)
}

/// A scrollable container that scrolls itself while a drag hovers near its
/// edges. Give it the `overflow` CSS yourself (via `style`/`class`) - and
/// consider `overscroll-behavior: contain` alongside it, so a wheel or
/// touch scroll that hits the container's end mid-drag doesn't chain into
/// scrolling the page. (The edge-scrolling itself is programmatic, clamps
/// at the container's bounds, and never chains.)
#[component]
pub fn AutoScroll(
    /// Edge band size in px.
    #[props(default = 48.0)]
    threshold: f64,
    /// Legacy maximum movement per nominal 60 Hz frame. Kept for 3.x source
    /// and behavior compatibility; new code should prefer
    /// `speed_px_per_second`.
    #[props(default = 24.0)]
    speed: f64,
    /// Exact maximum scroll velocity in CSS pixels per second. When set, this
    /// takes precedence over the legacy `speed` prop.
    #[props(default)]
    speed_px_per_second: Option<f64>,
    /// Axes to scroll.
    #[props(default)]
    axis: ScrollAxis,
    /// Optional external drag-state gate. `Some(true)` scrolls on pointer
    /// movement, `Some(false)` suppresses it, and `None` uses the built-in
    /// pointer contact heuristic.
    #[props(default)]
    active: Option<bool>,
    /// Optional pointer supplied by a host that tracks movement outside this
    /// element's DOM event stream, expressed in this window's client
    /// coordinates. The sample is used only with `active: Some(true)`; pass
    /// the matching drag's live active state so a retained coordinate cannot
    /// scroll idle or settling content.
    #[props(default)]
    drag_pointer: Option<Point>,
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
    let max_velocity = resolved_velocity(speed, speed_px_per_second);
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    // In-flight guard so a burst of dragover events doesn't queue a pile of
    // overlapping async scrolls.
    let busy = use_signal(|| false);
    let mut latest_pointer = use_signal(|| None::<Point>);
    let mut clock_running = use_signal(|| false);
    let mut clock_generation = use_signal(|| 0u64);
    let mut clock_owner = use_signal(|| None::<ClockOwner>);
    let mut clock_epoch = use_signal(|| 0u64);
    let mut native_drag_depth = use_signal(|| 0u32);
    let scroll_id =
        use_hook(|| ScrollContainerId(NEXT_SCROLL_CONTAINER.fetch_add(1, Ordering::Relaxed)));
    let coordinator = use_hook(|| {
        try_consume_context::<ScrollCoordinator>().unwrap_or_else(ScrollCoordinator::new)
    });
    use_context_provider(|| coordinator);
    use_drop(move || {
        let mut coordinator = coordinator;
        coordinator.unregister(scroll_id);
    });
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

    let scroll_for = move |point: Point, elapsed_seconds: f64, token: ClockToken| {
        let Some(m) = mounted.peek().clone() else {
            return;
        };
        let still_owned = move || {
            *clock_running.peek()
                && *clock_owner.peek() == Some(token.owner)
                && *clock_epoch.peek() == token.epoch
        };
        if !still_owned() {
            return;
        }
        if *busy.peek() {
            return;
        }
        let mut busy = busy;
        let mut clock_running = clock_running;
        let mut coordinator = coordinator;
        busy.set(true);
        spawn(async move {
            if let Ok(r) = m.get_client_rect().await {
                if !still_owned() {
                    busy.set(false);
                    return;
                }
                let rect = Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height);
                let velocity = edge_delta(point, rect, threshold, max_velocity, axis);
                let (dx, dy) = frame_delta(velocity, elapsed_seconds);
                let available = dx != 0.0 || dy != 0.0;
                coordinator.update(scroll_id, rect, available);
                if !available {
                    if still_owned() {
                        clock_running.set(false);
                        clock_owner.set(None);
                        clock_epoch += 1;
                    }
                    busy.set(false);
                    return;
                }
                if coordinator.owner(point) != Some(scroll_id) {
                    busy.set(false);
                    return;
                }
                if dx != 0.0 || dy != 0.0 {
                    if let Ok(offset) = m.get_scroll_offset().await {
                        if !still_owned() {
                            busy.set(false);
                            return;
                        }
                        let _ = m
                            .scroll(
                                PixelsVector2D::new(offset.x + dx, offset.y + dy),
                                ScrollBehavior::Instant,
                            )
                            .await;
                        if !still_owned() {
                            busy.set(false);
                            return;
                        }
                        let moved = m
                            .get_scroll_offset()
                            .await
                            .map(|after| after.x != offset.x || after.y != offset.y)
                            .unwrap_or(true);
                        coordinator.set_blocked(scroll_id, !moved);
                        if !moved {
                            clock_running.set(false);
                            clock_owner.set(None);
                            clock_epoch += 1;
                        }
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

    let mut start_clock = move |point: Point, owner: ClockOwner| {
        latest_pointer.set(Some(point));
        // A fresh pointer sample is a new opportunity for a container that
        // previously hit a boundary. Geometry updates during an existing
        // clock must not clear this bit, or the blocked inner container would
        // repeatedly reclaim ownership before the outer surface can move.
        let mut coordinator = coordinator;
        coordinator.set_blocked(scroll_id, false);
        if !*clock_running.peek() || *clock_owner.peek() != Some(owner) {
            clock_owner.set(Some(owner));
            clock_epoch += 1;
            clock_running.set(true);
            clock_generation += 1;
        }
    };
    let mut stop_clock = move |owner: ClockOwner| {
        if *clock_owner.peek() == Some(owner) {
            clock_running.set(false);
            clock_owner.set(None);
            clock_epoch += 1;
        }
    };

    // A host-driven receiver may be event-blind while another surface owns
    // the pointer. React to its client-space feed through the same scroll
    // path as DOM pointer movement, with the explicit active gate above.
    use_effect(use_reactive!(|(active, drag_pointer)| {
        if let Some(point) = external_pointer_sample(active, drag_pointer) {
            start_clock(point, ClockOwner::External);
        } else {
            stop_clock(ClockOwner::External);
        }
    }));
    let mut attributes = attributes;
    crate::core::components::protect_attributes(
        &mut attributes,
        &[
            "onmounted",
            "onwheel",
            "ondragenter",
            "ondragover",
            "ondragleave",
            "ondrop",
            "onpointermove",
            "onpointerup",
            "onpointercancel",
        ],
    );

    rsx! {
        style { style: "display: none;",
            "@keyframes dnd-scroll-clock {{ from {{ opacity: 0.99; }} to {{ opacity: 1; }} }}"
        }
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
            // hovering. The enter/leave depth prevents movement between
            // descendants from looking like departure from this container.
            // Note: no prevent_default here - drop permission stays the
            // business of the zones inside.
            ondragenter: move |_| native_drag_depth += 1,
            ondragover: move |evt: DragEvent| {
                if *native_drag_depth.peek() == 0 {
                    native_drag_depth.set(1);
                }
                let c = evt.client_coordinates();
                start_clock(Point::new(c.x, c.y), ClockOwner::NativeDrag);
            },
            ondragleave: move |_| {
                let next = native_drag_depth.peek().saturating_sub(1);
                native_drag_depth.set(next);
                if next == 0 {
                    stop_clock(ClockOwner::NativeDrag);
                }
            },
            ondrop: move |_| {
                native_drag_depth.set(0);
                stop_clock(ClockOwner::NativeDrag);
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
                    start_clock(Point::new(c.x, c.y), ClockOwner::Pointer);
                }
                // Sample on every move, contact or hover: it trues up the
                // window after scrollbar drags and programmatic scrolls
                // the moment the pointer stirs.
                sample();
            },
            onpointerup: move |_| stop_clock(ClockOwner::Pointer),
            onpointercancel: move |_| stop_clock(ClockOwner::Pointer),
            ..attributes,
            if let Some(owner) = clock_running().then_some(clock_owner()).flatten() {
                div {
                    key: "{clock_generation}",
                    style: "position: absolute; width: 0; height: 0; overflow: hidden; \
                            animation: dnd-scroll-clock 16ms linear forwards;",
                    aria_hidden: true,
                    onanimationend: move |event: AnimationEvent| {
                        let token = ClockToken {
                            owner,
                            epoch: *clock_epoch.peek(),
                        };
                        let elapsed = animation_elapsed_seconds(event.data().elapsed_time());
                        if let Some(point) = *latest_pointer.peek() {
                            scroll_for(point, elapsed, token);
                        }
                        if *clock_running.peek()
                            && *clock_owner.peek() == Some(token.owner)
                            && *clock_epoch.peek() == token.epoch
                        {
                            clock_generation += 1;
                        }
                    },
                }
            }
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn external_pointer_app() -> Element {
        rsx! {
            AutoScroll {
                active: true,
                drag_pointer: Point::new(5.0, 5.0),
                "receiver"
            }
        }
    }

    #[test]
    fn external_pointer_feed_is_available_without_dom_pointer_events() {
        let mut dom = VirtualDom::new(external_pointer_app);
        dom.rebuild_in_place();
        assert!(dioxus_ssr::render(&dom).contains("receiver"));
    }

    #[derive(Clone, Props)]
    struct DynamicPointerProps {
        state: Rc<Cell<(Option<bool>, Option<Point>)>>,
    }

    impl PartialEq for DynamicPointerProps {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.state, &other.state)
        }
    }

    fn dynamic_pointer_app(props: DynamicPointerProps) -> Element {
        let (active, drag_pointer) = props.state.get();
        rsx! {
            AutoScroll { active, drag_pointer, "receiver" }
        }
    }

    fn flush_effects(dom: &mut VirtualDom) {
        for _ in 0..3 {
            dom.process_events();
            dom.render_immediate(&mut dioxus::core::NoOpMutations);
        }
    }

    #[test]
    fn external_pointer_prop_changes_start_and_stop_the_clock() {
        let state = Rc::new(Cell::new((Some(false), None)));
        let mut dom = VirtualDom::new_with_props(
            dynamic_pointer_app,
            DynamicPointerProps {
                state: state.clone(),
            },
        );
        dom.rebuild_in_place();
        flush_effects(&mut dom);
        assert!(!dioxus_ssr::render(&dom).contains("position: absolute; width: 0"));

        state.set((Some(true), Some(Point::new(5.0, 5.0))));
        dom.mark_dirty(ScopeId::APP);
        flush_effects(&mut dom);
        assert!(dioxus_ssr::render(&dom).contains("position: absolute; width: 0"));

        state.set((None, Some(Point::new(5.0, 5.0))));
        dom.mark_dirty(ScopeId::APP);
        flush_effects(&mut dom);
        assert!(!dioxus_ssr::render(&dom).contains("position: absolute; width: 0"));

        state.set((Some(true), Some(Point::new(5.0, 5.0))));
        dom.mark_dirty(ScopeId::APP);
        flush_effects(&mut dom);
        assert!(dioxus_ssr::render(&dom).contains("position: absolute; width: 0"));

        state.set((Some(true), None));
        dom.mark_dirty(ScopeId::APP);
        flush_effects(&mut dom);
        assert!(!dioxus_ssr::render(&dom).contains("position: absolute; width: 0"));
    }

    #[test]
    fn external_pointer_requires_an_explicit_active_gate() {
        let point = Point::new(5.0, 5.0);
        assert_eq!(
            external_pointer_sample(Some(true), Some(point)),
            Some(point)
        );
        assert_eq!(external_pointer_sample(Some(false), Some(point)), None);
        assert_eq!(external_pointer_sample(None, Some(point)), None);
        assert_eq!(external_pointer_sample(Some(true), None), None);
    }

    #[test]
    fn legacy_speed_keeps_its_nominal_sixty_hertz_behavior() {
        assert_eq!(resolved_velocity(24.0, None), 1440.0);
        assert_eq!(resolved_velocity(24.0, Some(720.0)), 720.0);
    }

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
    fn velocity_is_scaled_by_elapsed_time() {
        assert_eq!(frame_delta((600.0, -300.0), 0.02), (12.0, -6.0));
        // A suspended tab cannot produce a giant catch-up jump.
        assert_eq!(frame_delta((100.0, 100.0), 5.0), (10.0, 10.0));
        assert_eq!(frame_delta((100.0, 100.0), f64::NAN), (0.0, 0.0));
        assert_eq!(animation_elapsed_seconds(0.0), 0.016);
        assert_eq!(animation_elapsed_seconds(f32::NAN), 0.016);
    }

    fn coordinator_probe() -> Element {
        let mut coordinator = ScrollCoordinator::new();
        let outer = ScrollContainerId(1);
        let inner = ScrollContainerId(2);
        let point = Point::new(50.0, 50.0);
        coordinator.update(outer, Rect::new(0.0, 0.0, 200.0, 200.0), true);
        coordinator.update(inner, Rect::new(25.0, 25.0, 50.0, 50.0), true);
        assert_eq!(coordinator.owner(point), Some(inner));
        coordinator.set_blocked(inner, true);
        assert_eq!(coordinator.owner(point), Some(outer));
        rsx! {}
    }

    #[test]
    fn nested_coordinator_hands_boundary_to_outer_container() {
        let mut dom = VirtualDom::new(coordinator_probe);
        dom.rebuild_in_place();
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

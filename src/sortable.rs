//! Reorderable lists - drag a row and the list tells you where it landed.
//!
//! Self-contained: `SortableList` manages its own drag state (indices only),
//! so it needs no `DndProvider`. You keep ownership of the data - the
//! component just tells you *"move index 3 to index 0"* and you apply it
//! (usually with [`apply_sort`]).
//!
//! Mouse, touch and pen all drive the same pointer-event gesture machine, so
//! the browser never creates a native drag image. By default the whole row
//! is the touch target, which sets `touch-action: none` on it - fine for
//! short lists, but it stops finger-scrolling through the rows. Inside a
//! scrollable list, set `touch_handle: true` to confine pointer drags to a
//! leading grip (style it via `[data-sort-handle]`) so the rows themselves
//! still scroll.
//!
//! Headless: the component ships behavior plus a couple of `data-*` styling
//! hooks; you compose the looks. Rows slide to preview the drop by default -
//! opt into a floating, caller-composed ghost with `overlay`.
//!
//! ```text
//! let mut items = use_signal(|| vec!["a".to_string(), "b".into(), "c".into()]);
//! rsx! {
//!     SortableList {
//!         len: items.read().len(),
//!         on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
//!         render: move |ix: usize| rsx! { li { "{items.read()[ix]}" } },
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::a11y::use_reduced_motion_css;
use crate::core::components::overlay_style;
use crate::core::hooks::use_rect_refresh_thunk;
use crate::core::{platform, transition, GestureEffect, GestureEvent, GesturePhase, Point, Rect};

fn pointer_client(evt: &PointerEvent) -> Point {
    let c = evt.client_coordinates();
    Point::new(c.x, c.y)
}

/// "Move the item at `from` so it ends up at index `to`."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortEvent {
    pub from: usize,
    pub to: usize,
}

/// What a completed reorder gesture means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReorderMode {
    /// Remove the item and insert it at the target index (list reorder).
    #[default]
    Insert,
    /// Exchange the two items' positions (grid/tile swap).
    Swap,
}

/// Apply a [`SortEvent`] as a swap: the two items exchange positions.
pub fn apply_swap<T>(list: &mut [T], ev: SortEvent) {
    if ev.from != ev.to && ev.from < list.len() && ev.to < list.len() {
        list.swap(ev.from, ev.to);
    }
}

/// The live-preview offset (CSS px along the list axis) for the row at `ix`
/// while row `from` is dragged over row `over` - the mid-drag preview
/// dnd-kit and react-beautiful-dnd made the baseline expectation.
///
/// Two moves happen at once: rows between the two indices shift by `step`
/// (the dragged row's size) to close the source slot, and the **source row
/// itself translates to the target slot** - without that second part the
/// shifted neighbors would overlap the source, which still occupies its
/// slot during a drag. Assumes uniform row sizes for the source's
/// travel distance. Pure, for testability.
pub fn displacement(ix: usize, from: usize, over: usize, step: f64) -> f64 {
    if ix == from {
        (over as f64 - from as f64) * step
    } else if from < over && ix > from && ix <= over {
        -step
    } else if over < from && ix >= over && ix < from {
        step
    } else {
        0.0
    }
}

/// Distance between consecutive row origins, including CSS margin/gap.
fn slot_pitch(rects: &HashMap<usize, Rect>, ix: usize, axis: Axis) -> Option<f64> {
    let pos = |r: &Rect| match axis {
        Axis::Vertical => r.y,
        Axis::Horizontal => r.x,
    };
    let cur = rects.get(&ix)?;
    if let Some(next) = rects.get(&(ix + 1)) {
        return Some(pos(next) - pos(cur));
    }
    if let Some(prev) = ix.checked_sub(1).and_then(|p| rects.get(&p)) {
        return Some(pos(cur) - pos(prev));
    }
    Some(match axis {
        Axis::Vertical => cur.height,
        Axis::Horizontal => cur.width,
    })
}

pub(crate) fn refresh_rects(
    mounteds: Signal<HashMap<usize, Rc<MountedData>>>,
    rects: Signal<HashMap<usize, Rect>>,
) {
    for (i, m) in mounteds.peek().clone() {
        let mut rects = rects;
        spawn(async move {
            if let Ok(r) = m.get_client_rect().await {
                rects.write().insert(
                    i,
                    Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height),
                );
            }
        });
    }
}

/// Shift every cached rect by `(dx, dy)`. Pure, for testability.
fn shift_rects(rects: &mut HashMap<usize, Rect>, dx: f64, dy: f64) {
    for rect in rects.values_mut() {
        rect.x += dx;
        rect.y += dy;
    }
}

/// Track scrolling mid-drag by re-anchoring, not re-measuring. Rows carry
/// live-preview transforms (often mid-transition), so `get_client_rect` on
/// a row reads a displaced, interpolated box that no subtraction can
/// reliably invert. The list *wrapper* never transforms, and rows never
/// move within it during a drag - so one measurement of the wrapper gives
/// the exact distance everything shifted, and the cached base slots move
/// with it. A ping from an unrelated scroll surface measures zero movement
/// and is a no-op.
///
/// `busy`/`pending` coalesce overlapping pings: two concurrent shifts
/// racing the same anchor would double-count, and simply dropping a ping
/// could leave the *final* scroll position unapplied.
fn reanchor_rects(
    container: Signal<Option<Rc<MountedData>>>,
    anchor: Signal<Option<Point>>,
    rects: Signal<HashMap<usize, Rect>>,
    busy: Signal<bool>,
    pending: Signal<bool>,
) {
    let Some(m) = container.peek().clone() else {
        return;
    };
    if *busy.peek() {
        let mut pending = pending;
        pending.set(true);
        return;
    }
    let mut busy = busy;
    busy.set(true);
    spawn(async move {
        let mut anchor = anchor;
        let mut rects = rects;
        let mut pending = pending;
        loop {
            if let Ok(r) = m.get_client_rect().await {
                let new = Point::new(r.origin.x, r.origin.y);
                // Read the anchor *after* the await: another task may have
                // applied a shift while we were measuring.
                if let Some(old) = *anchor.peek() {
                    let (dx, dy) = (new.x - old.x, new.y - old.y);
                    if dx != 0.0 || dy != 0.0 {
                        shift_rects(&mut rects.write(), dx, dy);
                    }
                }
                anchor.set(Some(new));
            }
            if *pending.peek() {
                pending.set(false);
            } else {
                break;
            }
        }
        busy.set(false);
    });
}

/// Capture the wrapper's current origin as the shift baseline. Runs
/// alongside every full row measurement (drag start), so subsequent
/// [`reanchor_rects`] pings shift from a matching snapshot.
fn capture_anchor(container: Signal<Option<Rc<MountedData>>>, anchor: Signal<Option<Point>>) {
    let Some(m) = container.peek().clone() else {
        return;
    };
    let mut anchor = anchor;
    spawn(async move {
        if let Ok(r) = m.get_client_rect().await {
            anchor.set(Some(Point::new(r.origin.x, r.origin.y)));
        }
    });
}

/// Which row should be the drop target while a pointer drag from row `from`
/// hovers at `at`, given per-row rects measured at drag start (so the test
/// runs against the stable, pre-displacement layout). A row is adopted only once the pointer
/// crosses its center in the travel direction, and while the pointer is
/// over the source row or outside every rect, the previous target is kept.
/// Pure, for testability.
pub fn pointer_target(
    rects: &HashMap<usize, Rect>,
    from: usize,
    current: Option<usize>,
    at: Point,
    axis: Axis,
) -> Option<usize> {
    let Some((&ix, rect)) = rects.iter().find(|(_, r)| r.contains(at)) else {
        return current;
    };
    if ix == from || Some(ix) == current {
        return current;
    }
    let (pos, size) = match axis {
        Axis::Vertical => (at.y - rect.y, rect.height),
        Axis::Horizontal => (at.x - rect.x, rect.width),
    };
    let crossed = if from < ix {
        pos > size * 0.5
    } else {
        pos < size * 0.5
    };
    if crossed {
        Some(ix)
    } else {
        current
    }
}

/// The bounding box of all measured rows - the list's occupied area. Used to
/// decide whether a pointer release landed on the list at all: a drop outside
/// this box commits no reorder. `None` when no rows are measured yet.
pub(crate) fn list_bounds(rects: &HashMap<usize, Rect>) -> Option<Rect> {
    let mut it = rects.values();
    let first = it.next()?;
    let (mut min_x, mut min_y) = (first.x, first.y);
    let (mut max_x, mut max_y) = (first.x + first.width, first.y + first.height);
    for r in it {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.width);
        max_y = max_y.max(r.y + r.height);
    }
    Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

/// Layout direction of the list - decides whether the midpoint test uses
/// the Y axis (vertical lists) or the X axis (horizontal ones).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Axis {
    #[default]
    Vertical,
    Horizontal,
}

/// Apply a [`SortEvent`] to a `Vec` in place.
pub fn apply_sort<T>(list: &mut Vec<T>, ev: SortEvent) {
    if ev.from == ev.to || ev.from >= list.len() || ev.to >= list.len() {
        return;
    }
    let item = list.remove(ev.from);
    list.insert(ev.to, item);
}

/// A list whose items can be dragged to reorder.
///
/// Data-agnostic: give it a `len` and a `render` callback keyed by index. It
/// renders one wrapper per item and emits a [`SortEvent`] on drop. The hovered
/// drop target gets `data-drop-target` on its wrapper and the dragged item gets
/// `data-dragging`; both are absent while inactive.
///
/// Headless by default - the component ships behavior, you compose the looks.
/// With `overlay` set, the picked-up row is hidden in place and *your* overlay
/// element floats at the pointer (the dnd-kit feel); keep it lightweight, it is
/// your content, not a clone of the row. Without it, the row stays visible and
/// slides.
#[component]
pub fn SortableList(
    /// Number of items.
    len: usize,
    /// Renders the item at the given index.
    render: Callback<usize, Element>,
    /// Fired when the user drops an item at a new position.
    on_sort: EventHandler<SortEvent>,
    /// List direction: which axis rows are laid out (and shifted) along.
    #[props(default)]
    axis: Axis,
    /// Open a live gap where the drop would land, by translating the rows
    /// in between. Set `false` for the plain highlight-only behavior.
    #[props(default = true)]
    live_preview: bool,
    /// Duration (ms) of the row-slide transition during live preview.
    #[props(default = 160)]
    transition_ms: u32,
    /// Opt-in floating ghost: renders `overlay(index)` pinned to the pointer
    /// while dragging, and hides the picked-up row in place so its slot reads
    /// as the drop gap. Absent → the row itself slides. Keep the ghost
    /// lightweight; it is your content, not a clone of the row.
    #[props(default)]
    overlay: Option<Callback<usize, Element>>,
    /// Confine touch/pen drags to a leading grip element instead of the
    /// whole row. The grip carries `touch-action: none` so the rest of the
    /// row keeps scrolling by finger - use this inside scrollable lists.
    /// Style it via `[data-sort-handle]`.
    #[props(default = false)]
    touch_handle: bool,
    /// Content for the `touch_handle` grip, keyed by index. Defaults to a
    /// braille-dots glyph when unset.
    #[props(default)]
    handle: Option<Callback<usize, Element>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut drag_from = use_signal(|| None::<usize>);
    let mut over = use_signal(|| None::<usize>);
    let mut press_from = use_signal(|| None::<usize>);
    let mut press_at = use_signal(|| None::<Point>);
    let mut pointer_at = use_signal(|| None::<Point>);
    // Per-row client rects (measured on mount, re-measured at pointer-drag
    // start) drive both the displacement step and hit-testing.
    let rects = use_signal(HashMap::<usize, Rect>::new);
    let mounteds = use_signal(HashMap::<usize, Rc<MountedData>>::new);
    let mut rects_for_len = rects;
    let mut mounteds_for_len = mounteds;
    use_effect(use_reactive!(|len| {
        rects_for_len.write().retain(|ix, _| *ix < len);
        mounteds_for_len.write().retain(|ix, _| *ix < len);
    }));
    let size_of = move |ix: usize| {
        rects
            .peek()
            .get(&ix)
            .map(|r| match axis {
                Axis::Vertical => r.height,
                Axis::Horizontal => r.width,
            })
            .unwrap_or(40.0)
    };

    // Mid-drag scrolls (an AutoScroll above, or anything pinging the tree's
    // rect-refresh channel) move the rows under the pointer. Re-anchor the
    // cached slots against the wrapper's movement - see `reanchor_rects`
    // for why this beats re-measuring the (transformed) rows.
    let container = use_signal(|| None::<Rc<MountedData>>);
    let anchor = use_signal(|| None::<Point>);
    let reanchor_busy = use_signal(|| false);
    let reanchor_pending = use_signal(|| false);
    use_rect_refresh_thunk(move |_| {
        if drag_from.peek().is_some() {
            reanchor_rects(container, anchor, rects, reanchor_busy, reanchor_pending);
        }
    });

    // Drags run the same formal gesture machine as `Draggable`. `over` (the
    // drop target) is resolved synchronously on every tracked move - no
    // derived effect - so the gap never lags the pointer.
    let mut gesture = use_signal(|| GesturePhase::Idle);
    let mut step = move |event: GestureEvent| -> GestureEffect {
        let (next, fx) = transition(*gesture.peek(), event, 8.0);
        gesture.set(next);
        fx
    };
    // Feed one pointer event and act on the machine's effect. The source row is
    // latched on pointerdown because mouse move/up events bubble through the
    // list container rather than staying on the pressed row.
    let mut feed = move |event: GestureEvent| {
        match step(event) {
            GestureEffect::Begin { at, .. } => {
                let Some(ix) = *press_from.peek() else {
                    return;
                };
                drag_from.set(Some(ix));
                pointer_at.set(Some(at));
                over.set(pointer_target(&rects.peek(), ix, None, at, axis));
                // Client rects go stale when the list scrolls or layout shifts;
                // re-measure every row at drag start so hit-testing runs against
                // the current (pre-displacement) slots, and re-baseline the
                // wrapper anchor the scroll-tracking shifts run from.
                refresh_rects(mounteds, rects);
                capture_anchor(container, anchor);
            }
            GestureEffect::Track { at } => {
                let Some(from) = *drag_from.peek() else {
                    return;
                };
                pointer_at.set(Some(at));
                let next = pointer_target(&rects.peek(), from, *over.peek(), at, axis);
                if next != *over.peek() {
                    over.set(next);
                }
            }
            GestureEffect::Drop { at } => {
                let from_opt = *drag_from.peek();
                // A release outside the list's bounds cancels rather than
                // committing a reorder - dropping a row "nowhere" shouldn't
                // move it. Inside the bounds, snap to the hovered target.
                let to = {
                    let rects_ref = rects.peek();
                    if list_bounds(&rects_ref)
                        .map(|b| b.contains(at))
                        .unwrap_or(false)
                    {
                        from_opt.and_then(|from| {
                            pointer_target(&rects_ref, from, *over.peek(), at, axis)
                        })
                    } else {
                        None
                    }
                };
                // Clear ALL drag state BEFORE notifying: `on_sort` mutates the
                // caller's list, which re-renders this component; observing a
                // still-active drag would re-apply the preview to the already-
                // reordered rows.
                press_from.set(None);
                press_at.set(None);
                drag_from.set(None);
                over.set(None);
                pointer_at.set(None);
                if let (Some(from), Some(to)) = (from_opt, to) {
                    if from != to {
                        on_sort.call(SortEvent { from, to });
                    }
                }
            }
            GestureEffect::Abort => {
                press_from.set(None);
                press_at.set(None);
                drag_from.set(None);
                over.set(None);
                pointer_at.set(None);
            }
            GestureEffect::Tap => {
                press_from.set(None);
                press_at.set(None);
                pointer_at.set(None);
            }
            GestureEffect::None => {}
        }
    };

    // Rows glide via inline transitions; honor prefers-reduced-motion.
    let reduced_motion_css = use_reduced_motion_css();

    let primary_pointer = move |evt: &PointerEvent| evt.is_primary();
    let mut cancel_drag = move || {
        feed(GestureEvent::Cancel);
        press_from.set(None);
        press_at.set(None);
        drag_from.set(None);
        over.set(None);
        pointer_at.set(None);
    };

    // Opt-in floating ghost (caller-composed). Only computed when we have a
    // measured source rect and both pointer positions, so the in-flow original
    // is hidden *only* when a replacement is guaranteed to render - no rect, no
    // ghost, no disappearing row (it just slides). Carries the callback so the
    // render below needs nothing else.
    let overlay_ghost: Option<(Callback<usize, Element>, usize, Point, Rect)> =
        overlay.zip(drag_from()).and_then(|(cb, from)| {
            let r = rects.peek().get(&from).copied()?;
            let p0 = press_at()?;
            let p1 = pointer_at()?;
            Some((
                cb,
                from,
                Point::new(r.x + (p1.x - p0.x), r.y + (p1.y - p0.y)),
                r,
            ))
        });
    let ghost_from = overlay_ghost.map(|(_, f, _, _)| f);

    rsx! {
        div {
            onmounted: move |evt: Event<MountedData>| {
                let mut container = container;
                container.set(Some(evt.data()));
            },
            onpointermove: move |evt: PointerEvent| {
                let at = pointer_client(&evt);
                // Recovery for a mouse released while the cursor sat outside the
                // list. With the `web` feature, `platform::capture_pointer`
                // routes the release back here; without it (feature off, or a
                // non-web renderer) no `pointerup` arrives, so when the pointer
                // returns over the list with no button held we finish the drop
                // rather than track a phantom drag. Touch/pen hold a button
                // throughout contact, so this only trips for a released mouse.
                if drag_from.peek().is_some() && evt.held_buttons().is_empty() {
                    if let Some(from) = *drag_from.peek() {
                        if let Some(n) = mounteds.peek().get(&from).cloned() {
                            platform::release_pointer(&n, evt.pointer_id());
                        }
                    }
                    feed(GestureEvent::Up { at, pointer_id: evt.pointer_id() });
                    return;
                }
                feed(GestureEvent::Move { at, pointer_id: evt.pointer_id() });
            },
            onpointerup: move |evt: PointerEvent| {
                if let Some(from) = *drag_from.peek() {
                    if let Some(n) = mounteds.peek().get(&from).cloned() {
                        platform::release_pointer(&n, evt.pointer_id());
                    }
                }
                feed(GestureEvent::Up { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
            },
            // Genuine interruptions (touch cancelled, browser stole capture)
            // abort the drag. Merely leaving the list does NOT: without pointer
            // capture, cancelling on `pointerleave` would kill every drag that
            // strays a pixel past an edge.
            onpointercancel: move |evt: PointerEvent| {
                if let Some(from) = *drag_from.peek() {
                    if let Some(n) = mounteds.peek().get(&from).cloned() {
                        platform::release_pointer(&n, evt.pointer_id());
                    }
                }
                cancel_drag();
            },
            onlostpointercapture: move |_| cancel_drag(),
            ..attributes,
            {reduced_motion_css}
            for ix in 0..len {
                div {
                    key: "{ix}",
                    "data-dnd-motion": true,
                    "data-dragging": if drag_from() == Some(ix) { "true" },
                    "data-drop-target": if over() == Some(ix) && drag_from() != Some(ix) { "true" },
                    style: {
                        // Live preview: rows slide so every slot stays filled by
                        // exactly one box. When `overlay` is set, the picked-up
                        // row is drawn by the floating ghost, so its in-flow
                        // original is hidden (opacity 0) while still translating
                        // to the target slot - that invisible slot is the gap the
                        // neighbours part around. Without `overlay` the row stays
                        // visible and slides.
                        let base = match (live_preview, drag_from()) {
                            (true, Some(from)) => {
                                let step = slot_pitch(&rects.peek(), from, axis)
                                    .unwrap_or_else(|| size_of(from));
                                let o = over().unwrap_or(from);
                                let d = displacement(ix, from, o, step);
                                let (x, y) = match axis {
                                    Axis::Vertical => (0.0, d),
                                    Axis::Horizontal => (d, 0.0),
                                };
                                let hidden = if ghost_from == Some(ix) {
                                    " opacity: 0;"
                                } else {
                                    ""
                                };
                                format!("transform: translate({x}px, {y}px); transition: transform {transition_ms}ms ease;{hidden}")
                            }
                            _ => format!(
                                "transform: translate(0px, 0px); transition: transform {transition_ms}ms; opacity: 1;"
                            ),
                        };
                        if touch_handle {
                            format!("display: flex; align-items: stretch; width: 100%; {base}")
                        } else {
                            format!("touch-action: none; {base}")
                        }
                    },
                    // Pointer path (whole-row mode). With `touch_handle` these
                    // are inert and the grip below owns the gesture.
                    onpointerdown: move |evt: PointerEvent| {
                        if touch_handle || !primary_pointer(&evt) {
                            return;
                        }
                        evt.prevent_default();
                        evt.stop_propagation();
                        refresh_rects(mounteds, rects);
                        capture_anchor(container, anchor);
                        press_from.set(Some(ix));
                        press_at.set(Some(pointer_client(&evt)));
                        // Capture on the stable row wrapper so a mouse drag
                        // survives the cursor leaving the list (real capture with
                        // the `web` feature; a no-op otherwise, backed by the
                        // button-release recovery above). Move/up still bubble to
                        // the container handlers.
                        if let Some(n) = mounteds.peek().get(&ix).cloned() {
                            platform::capture_pointer(&n, evt.pointer_id());
                        }
                        feed(GestureEvent::Down { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                    },
                    onmounted: move |evt: Event<MountedData>| {
                        let m: Rc<MountedData> = evt.data();
                        let mut mounteds = mounteds;
                        let mut rects = rects;
                        mounteds.write().insert(ix, m.clone());
                        spawn(async move {
                            if let Ok(r) = m.get_client_rect().await {
                                rects.write().insert(
                                    ix,
                                    Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height),
                                );
                            }
                        });
                    },
                    if touch_handle {
                        span {
                            "data-sort-handle": true,
                            aria_hidden: true,
                            style: "touch-action: none; user-select: none; -webkit-user-select: none; display: grid; place-items: center;",
                            onpointerdown: move |evt: PointerEvent| {
                                if !primary_pointer(&evt) {
                                    return;
                                }
                                evt.prevent_default();
                                evt.stop_propagation();
                                refresh_rects(mounteds, rects);
                                capture_anchor(container, anchor);
                                press_from.set(Some(ix));
                                press_at.set(Some(pointer_client(&evt)));
                                // Capture on the row wrapper (not the grip): it is
                                // stable across live-preview re-renders, and
                                // captured events still bubble to the container.
                                if let Some(n) = mounteds.peek().get(&ix).cloned() {
                                    platform::capture_pointer(&n, evt.pointer_id());
                                }
                                feed(GestureEvent::Down { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                            },
                            if let Some(h) = handle {
                                {h.call(ix)}
                            } else {
                                "⠿"
                            }
                        }
                        div {
                            "data-sort-content": true,
                            style: "flex: 1 1 auto; min-width: 0;",
                            {render.call(ix)}
                        }
                    } else {
                        {render.call(ix)}
                    }
                }
            }
            if let Some((cb, from, pos, rect)) = overlay_ghost {
                div {
                    style: format!(
                        "{} width: {}px; height: {}px;",
                        overlay_style(pos),
                        rect.width,
                        rect.height
                    ),
                    {cb.call(from)}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_moves_forward_and_back() {
        let mut v = vec!["a", "b", "c", "d"];
        apply_sort(&mut v, SortEvent { from: 0, to: 2 });
        assert_eq!(v, vec!["b", "c", "a", "d"]);
        apply_sort(&mut v, SortEvent { from: 3, to: 0 });
        assert_eq!(v, vec!["d", "b", "c", "a"]);
    }

    #[test]
    fn sort_ignores_out_of_bounds_and_noops() {
        let mut v = vec![1, 2, 3];
        apply_sort(&mut v, SortEvent { from: 1, to: 1 });
        apply_sort(&mut v, SortEvent { from: 9, to: 0 });
        apply_sort(&mut v, SortEvent { from: 0, to: 9 });
        assert_eq!(v, vec![1, 2, 3]);
    }
}

#[cfg(test)]
mod pointer_target_tests {
    use super::*;

    /// Three 40px rows stacked at y = 0, 40, 80.
    fn rows() -> HashMap<usize, Rect> {
        (0..3)
            .map(|i| (i, Rect::new(0.0, i as f64 * 40.0, 200.0, 40.0)))
            .collect()
    }

    #[test]
    fn adopts_a_row_only_past_its_midpoint() {
        let r = rows();
        // dragging row 0 downward into row 1's top half: not yet crossed
        let t = pointer_target(&r, 0, None, Point::new(50.0, 45.0), Axis::Vertical);
        assert_eq!(t, None);
        // past row 1's midpoint (y = 60): adopted
        let t = pointer_target(&r, 0, None, Point::new(50.0, 65.0), Axis::Vertical);
        assert_eq!(t, Some(1));
        // dragging row 2 upward into row 1's bottom half: not yet crossed
        let t = pointer_target(&r, 2, None, Point::new(50.0, 75.0), Axis::Vertical);
        assert_eq!(t, None);
        let t = pointer_target(&r, 2, None, Point::new(50.0, 55.0), Axis::Vertical);
        assert_eq!(t, Some(1));
    }

    #[test]
    fn keeps_current_over_source_row_and_outside_all_rects() {
        let r = rows();
        // hovering the source row keeps the previous target
        let t = pointer_target(&r, 0, Some(2), Point::new(50.0, 10.0), Axis::Vertical);
        assert_eq!(t, Some(2));
        // finger wandered off the list entirely: previous target survives
        let t = pointer_target(&r, 0, Some(2), Point::new(500.0, 500.0), Axis::Vertical);
        assert_eq!(t, Some(2));
    }

    #[test]
    fn horizontal_axis_uses_x() {
        let r: HashMap<usize, Rect> = (0..3)
            .map(|i| (i, Rect::new(i as f64 * 60.0, 0.0, 60.0, 40.0)))
            .collect();
        let t = pointer_target(&r, 0, None, Point::new(100.0, 20.0), Axis::Horizontal);
        assert_eq!(t, Some(1)); // past x = 90 midpoint of tile 1
    }

    #[test]
    fn list_bounds_covers_all_rows_and_excludes_outside() {
        let r = rows(); // three 40px rows spanning y 0..120, x 0..200
        let b = list_bounds(&r).unwrap();
        assert_eq!(b, Rect::new(0.0, 0.0, 200.0, 120.0));
        // a release inside the span (even in a gap) is on the list
        assert!(b.contains(Point::new(50.0, 60.0)));
        // a release well outside is not - the Drop arm cancels there
        assert!(!b.contains(Point::new(500.0, 500.0)));
        assert!(!b.contains(Point::new(50.0, 130.0)));
        // no measured rows: no bounds
        assert_eq!(list_bounds(&HashMap::new()), None);
    }
}

#[cfg(test)]
mod swap_tests {
    use super::*;

    #[test]
    fn swap_exchanges_and_guards_bounds() {
        let mut v = vec![1, 2, 3, 4];
        apply_swap(&mut v, SortEvent { from: 0, to: 3 });
        assert_eq!(v, vec![4, 2, 3, 1]);
        apply_swap(&mut v, SortEvent { from: 9, to: 0 });
        assert_eq!(v, vec![4, 2, 3, 1]);
    }
}

#[cfg(test)]
mod shift_rects_tests {
    use super::*;

    /// Scroll tracking moves every cached base slot by exactly the
    /// wrapper's movement, preserving sizes and relative spacing - which is
    /// all `pointer_target`'s model needs to stay correct mid-scroll.
    #[test]
    fn shift_moves_all_slots_uniformly() {
        let mut rects: HashMap<usize, Rect> = (0..3)
            .map(|i| (i, Rect::new(10.0, i as f64 * 40.0, 200.0, 40.0)))
            .collect();
        // Container scrolled down 130px: content moved up by 130.
        shift_rects(&mut rects, 0.0, -130.0);
        for i in 0..3 {
            assert_eq!(
                rects[&i],
                Rect::new(10.0, i as f64 * 40.0 - 130.0, 200.0, 40.0)
            );
        }
        // Pitch is preserved exactly.
        assert_eq!(slot_pitch(&rects, 1, Axis::Vertical), Some(40.0));
    }
}

#[cfg(test)]
mod slot_pitch_tests {
    use super::*;

    #[test]
    fn pitch_includes_spacing_between_rows() {
        let rows: HashMap<usize, Rect> = (0..3)
            .map(|i| (i, Rect::new(0.0, i as f64 * 46.0, 200.0, 42.0)))
            .collect();

        assert_eq!(slot_pitch(&rows, 0, Axis::Vertical), Some(46.0));
        assert_eq!(slot_pitch(&rows, 1, Axis::Vertical), Some(46.0));
        assert_eq!(slot_pitch(&rows, 2, Axis::Vertical), Some(46.0));
    }

    #[test]
    fn pitch_falls_back_to_size_for_single_row() {
        let rows: HashMap<usize, Rect> = [(0, Rect::new(0.0, 0.0, 200.0, 42.0))]
            .into_iter()
            .collect();

        assert_eq!(slot_pitch(&rows, 0, Axis::Vertical), Some(42.0));
        assert_eq!(slot_pitch(&rows, 9, Axis::Vertical), None);
    }
}

#[cfg(test)]
mod displacement_tests {
    use super::*;

    #[test]
    fn displacement_moves_source_to_target_and_neighbors_aside() {
        // dragging row 1 down over row 3, rows are 40px:
        // source travels +2 slots; rows 2..=3 shift up into the freed space
        let d: Vec<f64> = (0..5).map(|ix| displacement(ix, 1, 3, 40.0)).collect();
        assert_eq!(d, vec![0.0, 80.0, -40.0, -40.0, 0.0]);
        // dragging row 3 up over row 1: source travels -2 slots
        let d: Vec<f64> = (0..5).map(|ix| displacement(ix, 3, 1, 40.0)).collect();
        assert_eq!(d, vec![0.0, 40.0, 40.0, -80.0, 0.0]);
        // hovering the source itself: nothing moves
        assert!((0..5).all(|ix| displacement(ix, 2, 2, 40.0) == 0.0));
        // slot occupancy is conserved: offsets sum to zero
        let sum: f64 = (0..5).map(|ix| displacement(ix, 1, 3, 40.0)).sum();
        assert_eq!(sum, 0.0);
    }
}

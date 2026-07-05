//! Reordering items within a single list.
//!
//! Self-contained: `SortableList` manages its own drag state (indices only),
//! so it needs no `DndProvider`. You keep ownership of the data — the
//! component just tells you *"move index 3 to index 0"* and you apply it
//! (usually with [`apply_sort`]).
//!
//! Touch and pen work instantly in every browser: alongside the native
//! HTML5 drag path (mouse), each row runs the same pointer-event gesture
//! machine as [`crate::pointer::PointerDraggable`]. By default the whole
//! row is the touch target, which sets `touch-action: none` on it — fine
//! for short lists, but it stops finger-scrolling through the rows. Inside
//! a scrollable list, set `touch_handle: true` to confine touch drags to a
//! leading grip (style it via `[data-sort-handle]`) so the rows themselves
//! still scroll.
//!
//! ```rust,ignore
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

use crate::core::{transition, GestureEffect, GestureEvent, GesturePhase, Point, Rect};
use crate::pointer::pointer_client;

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
/// while row `from` is dragged over row `over` — the mid-drag preview
/// dnd-kit and react-beautiful-dnd made the baseline expectation.
///
/// Two moves happen at once: rows between the two indices shift by `step`
/// (the dragged row's size) to close the source slot, and the **source row
/// itself translates to the target slot** — without that second part the
/// shifted neighbors would overlap the source, which still occupies its
/// slot during a native drag. Assumes uniform row sizes for the source's
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

/// Which row should be the drop target while a pointer drag from row `from`
/// hovers at `at`, given per-row rects measured at drag start (so the test
/// runs against the stable, pre-displacement layout). Mirrors the native
/// path's midpoint hysteresis: a row is adopted only once the pointer
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

/// Layout direction of the list — decides whether the midpoint test uses
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
/// The component is data-agnostic: give it a `len` and a `render` callback
/// keyed by index. It renders one wrapper `div[draggable]` per item and emits
/// a [`SortEvent`] when the user drops. The item currently hovered as a drop
/// target gets `data-drop-target="true"` on its wrapper for styling, and the
/// dragged item gets `data-dragging="true"`.
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
    /// Confine touch/pen drags to a leading grip element instead of the
    /// whole row. The grip carries `touch-action: none` so the rest of the
    /// row keeps scrolling by finger — use this inside scrollable lists.
    /// Style it via `[data-sort-handle]`.
    #[props(default = false)]
    touch_handle: bool,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut drag_from = use_signal(|| None::<usize>);
    let mut over = use_signal(|| None::<usize>);
    // Per-row client rects (measured on mount, re-measured at pointer-drag
    // start) drive both the displacement step and touch hit-testing.
    let rects = use_signal(HashMap::<usize, Rect>::new);
    let mounteds = use_signal(HashMap::<usize, Rc<MountedData>>::new);
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

    // Touch/pen drags run the same formal gesture machine as
    // `PointerDraggable`; mouse input keeps the native HTML5 path below.
    let mut gesture = use_signal(|| GesturePhase::Idle);
    let mut step = move |event: GestureEvent| -> GestureEffect {
        let (next, fx) = transition(*gesture.peek(), event, 8.0);
        gesture.set(next);
        fx
    };
    // Feed one pointer event for row `ix` and act on the machine's effect.
    let mut feed = move |ix: usize, event: GestureEvent| {
        match step(event) {
            GestureEffect::Begin { .. } => {
                drag_from.set(Some(ix));
                over.set(None);
                // Client rects go stale when the list scrolls or layout
                // shifts; re-measure every row at drag start so hit-testing
                // runs against the current (pre-displacement) slots.
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
            GestureEffect::Track { at } => {
                if let Some(from) = *drag_from.peek() {
                    let next = pointer_target(&rects.peek(), from, *over.peek(), at, axis);
                    if next != *over.peek() {
                        over.set(next);
                    }
                }
            }
            GestureEffect::Drop { .. } => {
                if let (Some(from), Some(to)) = (*drag_from.peek(), *over.peek()) {
                    if from != to {
                        on_sort.call(SortEvent { from, to });
                    }
                }
                drag_from.set(None);
                over.set(None);
            }
            GestureEffect::Abort => {
                drag_from.set(None);
                over.set(None);
            }
            GestureEffect::Tap | GestureEffect::None => {}
        }
    };
    let touch_pointer = |evt: &PointerEvent| evt.pointer_type() != "mouse" && evt.is_primary();

    rsx! {
        div {
            ..attributes,
            for ix in 0..len {
                div {
                    key: "{ix}",
                    draggable: true,
                    "data-dragging": drag_from() == Some(ix),
                    "data-drop-target": over() == Some(ix) && drag_from() != Some(ix),
                    style: {
                        let base = match (live_preview, drag_from(), over()) {
                            (true, Some(from), Some(o)) => {
                                let d = displacement(ix, from, o, size_of(from));
                                let (x, y) = match axis {
                                    Axis::Vertical => (0.0, d),
                                    Axis::Horizontal => (d, 0.0),
                                };
                                format!("transform: translate({x}px, {y}px); transition: transform 160ms ease;")
                            }
                            (true, Some(_), None) => {
                                "transform: none; transition: transform 160ms ease;".to_string()
                            }
                            _ => String::new(),
                        };
                        if touch_handle {
                            format!("display: flex; align-items: stretch; width: 100%; {base}")
                        } else {
                            format!("touch-action: none; {base}")
                        }
                    },
                    // Touch/pen path (whole-row mode). With `touch_handle`
                    // these are inert and the grip below owns the gesture.
                    onpointerdown: move |evt: PointerEvent| {
                        if touch_handle || !touch_pointer(&evt) { return; }
                        feed(ix, GestureEvent::Down { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                    },
                    onpointermove: move |evt: PointerEvent| {
                        if touch_handle { return; }
                        feed(ix, GestureEvent::Move { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                    },
                    onpointerup: move |evt: PointerEvent| {
                        if touch_handle { return; }
                        feed(ix, GestureEvent::Up { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                    },
                    onpointercancel: move |_| {
                        if touch_handle { return; }
                        feed(ix, GestureEvent::Cancel);
                    },
                    onlostpointercapture: move |_| {
                        // Fires benignly after every pointerup (the machine is
                        // Idle then and ignores it) and protectively when the
                        // browser rips capture away mid-drag.
                        if touch_handle { return; }
                        feed(ix, GestureEvent::Cancel);
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
                    ondragstart: move |evt: DragEvent| {
                        // Nested sortables: the innermost list owns the drag.
                        // The outer list's `drag_from` stays `None`, so its
                        // dragover/drop guards no-op for this gesture.
                        evt.stop_propagation();
                        let _ = evt.data_transfer().set_data("text/plain", "dioxus-dnd-sort");
                        drag_from.set(Some(ix));
                    },
                    ondragover: move |evt: DragEvent| {
                        let Some(from) = drag_from() else { return };
                        evt.prevent_default();
                        if from == ix || over() == Some(ix) {
                            return;
                        }
                        // Midpoint hysteresis: only adopt this row as the
                        // target once the pointer crosses its center in the
                        // travel direction — prevents the gap from
                        // oscillating as displaced rows slide under the
                        // cursor.
                        let pos = match axis {
                            Axis::Vertical => evt.element_coordinates().y,
                            Axis::Horizontal => evt.element_coordinates().x,
                        };
                        let mid = size_of(ix) * 0.5;
                        let crossed = if from < ix { pos > mid } else { pos < mid };
                        if crossed {
                            over.set(Some(ix));
                        }
                    },
                    ondrop: move |evt: DragEvent| {
                        evt.prevent_default();
                        evt.stop_propagation();
                        if let Some(from) = drag_from() {
                            if from != ix {
                                on_sort.call(SortEvent { from, to: ix });
                            }
                        }
                        drag_from.set(None);
                        over.set(None);
                    },
                    ondragend: move |_| {
                        drag_from.set(None);
                        over.set(None);
                    },
                    if touch_handle {
                        span {
                            "data-sort-handle": true,
                            aria_hidden: true,
                            style: "touch-action: none; cursor: grab; user-select: none; -webkit-user-select: none; flex: 0 0 1.35rem; display: grid; place-items: center;",
                            onpointerdown: move |evt: PointerEvent| {
                                if !touch_pointer(&evt) { return; }
                                feed(ix, GestureEvent::Down { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                            },
                            onpointermove: move |evt: PointerEvent| {
                                feed(ix, GestureEvent::Move { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                            },
                            onpointerup: move |evt: PointerEvent| {
                                feed(ix, GestureEvent::Up { at: pointer_client(&evt), pointer_id: evt.pointer_id() });
                            },
                            onpointercancel: move |_| feed(ix, GestureEvent::Cancel),
                            onlostpointercapture: move |_| feed(ix, GestureEvent::Cancel),
                            "⠿"
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

//! 2D grids — dashboards, tile galleries, icon views. A grid is a flat
//! `Vec` displayed in `cols` columns; dragging a tile onto another either
//! **inserts** (everything reflows, like a photo gallery) or **swaps**
//! (tiles trade places, like a dashboard) depending on [`ReorderMode`].
//!
//! Reuses the sortable vocabulary: drops emit [`SortEvent`]s you apply with
//! [`crate::sortable::apply_sort`] or [`crate::sortable::apply_swap`].
//!
//! ```rust,ignore
//! let mut tiles = use_signal(|| (0..12).collect::<Vec<u32>>());
//! rsx! {
//!     SortableGrid {
//!         len: tiles.read().len(),
//!         cols: 4,
//!         mode: ReorderMode::Swap,
//!         render: move |ix: usize| rsx! { Tile { n: tiles.read()[ix] } },
//!         on_sort: move |ev: SortEvent| apply_swap(&mut tiles.write(), ev),
//!     }
//! }
//! ```
//!
//! Grid coordinate helpers ([`cell_of`], [`index_of`]) are provided for
//! custom layouts and keyboard grid navigation.
//!
//! Touch and pen drags work instantly in every browser via the same
//! pointer-event gesture machine as [`crate::pointer::PointerDraggable`];
//! tiles carry `touch-action: none` (grids rarely need to scroll by
//! dragging across their own tiles). The hovered tile is simply the one
//! under the finger — no hysteresis needed, since tiles don't shift while
//! you hover in swap/insert grids.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::core::{transition, GestureEffect, GestureEvent, GesturePhase, Rect};
use crate::pointer::pointer_client;
use crate::sortable::{ReorderMode, SortEvent};

/// `(row, col)` of a flat index in a grid with `cols` columns.
pub fn cell_of(index: usize, cols: usize) -> (usize, usize) {
    let cols = cols.max(1);
    (index / cols, index % cols)
}

/// Flat index of `(row, col)` in a grid with `cols` columns, or `None` if
/// outside `len`.
pub fn index_of(row: usize, col: usize, cols: usize, len: usize) -> Option<usize> {
    let cols = cols.max(1);
    if col >= cols {
        return None;
    }
    let ix = row * cols + col;
    (ix < len).then_some(ix)
}

/// A grid of tiles reordered (or swapped) by dragging.
///
/// Renders a `display: grid` wrapper with `cols` equal columns — pass your
/// own `class`/`style` for gaps and sizing (your `grid-template-columns`
/// wins if you set one, since spread attributes land after the default).
/// The hovered tile gets `data-drop-target="true"`, the dragged one
/// `data-dragging="true"`.
#[component]
pub fn SortableGrid(
    /// Number of tiles.
    len: usize,
    /// Number of columns.
    cols: usize,
    /// Renders the tile at the given index.
    render: Callback<usize, Element>,
    /// Fired when the user drops a tile on another.
    on_sort: EventHandler<SortEvent>,
    /// Insert-and-reflow (gallery) or swap (dashboard). Default: insert.
    #[props(default)]
    mode: ReorderMode,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    // `mode` only affects what the caller does with the SortEvent, but we
    // surface it as a data attribute so styling can differ (e.g. swap
    // targets often highlight the whole tile, insert targets show an edge).
    let mode_str = match mode {
        ReorderMode::Insert => "insert",
        ReorderMode::Swap => "swap",
    };
    let mut drag_from = use_signal(|| None::<usize>);
    let mut over = use_signal(|| None::<usize>);

    // Touch/pen path: per-tile rects measured at drag start, hovered tile =
    // the one containing the pointer.
    let rects = use_signal(HashMap::<usize, Rect>::new);
    let mounteds = use_signal(HashMap::<usize, Rc<MountedData>>::new);
    let mut gesture = use_signal(|| GesturePhase::Idle);
    let mut step = move |event: GestureEvent| -> GestureEffect {
        let (next, fx) = transition(*gesture.peek(), event, 8.0);
        gesture.set(next);
        fx
    };
    let mut feed = move |ix: usize, event: GestureEvent| match step(event) {
        GestureEffect::Begin { .. } => {
            drag_from.set(Some(ix));
            over.set(None);
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
            let next = rects
                .peek()
                .iter()
                .find(|(_, r)| r.contains(at))
                .map(|(&i, _)| i)
                .filter(|&i| Some(i) != *drag_from.peek())
                .or(*over.peek());
            if next != *over.peek() {
                over.set(next);
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
    };

    rsx! {
        div {
            style: "display: grid; grid-template-columns: repeat({cols}, 1fr);",
            "data-mode": mode_str,
            ..attributes,
            for ix in 0..len {
                div {
                    key: "{ix}",
                    draggable: true,
                    style: "touch-action: none;",
                    "data-dragging": drag_from() == Some(ix),
                    "data-drop-target": over() == Some(ix) && drag_from() != Some(ix),
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
                    onpointerdown: move |evt: PointerEvent| {
                        if evt.pointer_type() == "mouse" || !evt.is_primary() { return; }
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
                    ondragstart: move |evt: DragEvent| {
                        evt.stop_propagation();
                        let _ = evt.data_transfer().set_data("text/plain", "dioxus-dnd-grid");
                        drag_from.set(Some(ix));
                    },
                    ondragover: move |evt: DragEvent| {
                        if drag_from().is_some() {
                            evt.prevent_default();
                            if over() != Some(ix) {
                                over.set(Some(ix));
                            }
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
                    {render.call(ix)}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_coordinates_round_trip() {
        assert_eq!(cell_of(0, 4), (0, 0));
        assert_eq!(cell_of(5, 4), (1, 1));
        assert_eq!(index_of(1, 1, 4, 12), Some(5));
        assert_eq!(index_of(0, 4, 4, 12), None); // col out of range
        assert_eq!(index_of(3, 0, 4, 12), None); // beyond len
        assert_eq!(cell_of(7, 0), (7, 0)); // degenerate cols clamps to 1
    }
}

//! 2D grids - dashboards, tile galleries, icon views. A grid is a flat
//! `Vec` displayed in `cols` columns; dragging a tile onto another either
//! **inserts** (everything reflows, like a photo gallery) or **swaps**
//! (tiles trade places, like a dashboard) depending on [`ReorderMode`].
//!
//! Reuses the sortable vocabulary: drops emit [`SortEvent`]s you apply with
//! [`crate::sortable::apply_sort`] or [`crate::sortable::apply_swap`].
//!
//! ```text
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
//! Mouse, touch and pen use pointer events by default via the same
//! gesture machine as [`crate::pointer::PointerDraggable`], so the browser
//! does not create a native drag image. Set `input: DragInputMode::Native`
//! or `Hybrid` if you explicitly want HTML5 drag behavior. Tiles carry
//! `touch-action: none` (grids rarely need to scroll by dragging across
//! their own tiles). The hovered tile is simply the one under the pointer -
//! no hysteresis needed, since tiles don't shift while you hover in
//! swap/insert grids.

use std::collections::HashMap;
use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::core::components::merge_style;
use crate::core::{
    platform, transition, DragInputMode, GestureEffect, GestureEvent, GesturePhase, Rect,
};
use crate::pointer::pointer_client;
use crate::sortable::{list_bounds, ReorderMode, SortEvent};

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
/// Renders a `display: grid` wrapper with `cols` equal columns - pass your
/// own `class`/`style` for gaps and sizing. A forwarded `style` is merged
/// *after* the default, so per-property overrides win (e.g.
/// `style: "grid-template-columns: 2fr 1fr 1fr;"` for custom tracks) while
/// `display: grid` stays; spacing needs no override at all (`class:
/// "gap-2"`).
/// The hovered tile gets `data-drop-target="true"`, the dragged one
/// `data-dragging="true"` - both attributes are *absent* otherwise, so
/// presence-based selectors (CSS `[data-dragging]`, Tailwind
/// `data-dragging:opacity-50`) work directly. Use `item_class` to put
/// classes on the tile wrappers.
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
    /// Input/browser drag path. Defaults to pointer events for all pointer
    /// types, which avoids the native browser drag image. Use `Native` or
    /// `Hybrid` to opt back into HTML5 drag.
    #[props(default)]
    input: DragInputMode,
    /// Classes for each tile's wrapper div - the element that carries
    /// `data-dragging` / `data-drop-target`.
    #[props(default)]
    item_class: Option<String>,
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
    let mut press_from = use_signal(|| None::<usize>);
    let mut attributes = attributes;
    let style = merge_style(
        &mut attributes,
        &format!("display: grid; grid-template-columns: repeat({cols}, 1fr);"),
    );

    // Pointer path: per-tile rects measured at drag start, hovered tile =
    // the one containing the pointer.
    let rects = use_signal(HashMap::<usize, Rect>::new);
    let mounteds = use_signal(HashMap::<usize, Rc<MountedData>>::new);
    let mut gesture = use_signal(|| GesturePhase::Idle);
    let mut step = move |event: GestureEvent| -> GestureEffect {
        let (next, fx) = transition(*gesture.peek(), event, 8.0);
        gesture.set(next);
        fx
    };
    let mut feed = move |event: GestureEvent, fallback_ix: Option<usize>| match step(event) {
        GestureEffect::Begin { at, .. } => {
            let Some(ix) = *press_from.peek() else {
                return;
            };
            drag_from.set(Some(ix));
            let next = rects
                .peek()
                .iter()
                .find(|(_, r)| r.contains(at))
                .map(|(&i, _)| i)
                .or(fallback_ix)
                .filter(|&i| i != ix);
            over.set(next);
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
                .or(fallback_ix)
                .filter(|&i| Some(i) != *drag_from.peek())
                .or(*over.peek());
            if next != *over.peek() {
                over.set(next);
            }
        }
        GestureEffect::Drop { at } => {
            // A release outside the grid's tile bounds commits no reorder,
            // matching the native path (a drop off the tiles cancels). Inside,
            // use the hovered tile.
            let inside = list_bounds(&rects.peek())
                .map(|b| b.contains(at))
                .unwrap_or(false);
            let pair = (*drag_from.peek(), *over.peek());
            // Clear all drag state BEFORE notifying: `on_sort` mutates the
            // caller's list and re-renders this component, and observing a
            // still-active drag mid-apply is the hazard SortableList documents.
            press_from.set(None);
            drag_from.set(None);
            over.set(None);
            if inside {
                if let (Some(from), Some(to)) = pair {
                    if from != to {
                        on_sort.call(SortEvent { from, to });
                    }
                }
            }
        }
        GestureEffect::Abort => {
            press_from.set(None);
            drag_from.set(None);
            over.set(None);
        }
        GestureEffect::Tap => {
            press_from.set(None);
        }
        GestureEffect::None => {}
    };
    let primary_pointer =
        move |evt: &PointerEvent| evt.is_primary() && input.uses_pointer(&evt.pointer_type());

    rsx! {
        div {
            style: style,
            "data-mode": mode_str,
            onpointermove: move |evt: PointerEvent| {
                if !input.uses_pointer(&evt.pointer_type()) {
                    return;
                }
                let at = pointer_client(&evt);
                // Capture-free recovery (mirrors SortableList): a mouse that
                // returns over the grid with no button held was released off
                // it, so no `pointerup` reached us - finalize the drop instead
                // of tracking a phantom drag that can never end. No-op with the
                // `web` feature (capture delivers the real pointerup).
                if drag_from.peek().is_some() && evt.held_buttons().is_empty() {
                    if let Some(from) = *drag_from.peek() {
                        if let Some(n) = mounteds.peek().get(&from).cloned() {
                            platform::release_pointer(&n, evt.pointer_id());
                        }
                    }
                    feed(GestureEvent::Up { at, pointer_id: evt.pointer_id() }, None);
                    return;
                }
                feed(GestureEvent::Move { at, pointer_id: evt.pointer_id() }, None);
            },
            onpointerup: move |evt: PointerEvent| {
                if !input.uses_pointer(&evt.pointer_type()) {
                    return;
                }
                if let Some(from) = *drag_from.peek() {
                    if let Some(n) = mounteds.peek().get(&from).cloned() {
                        platform::release_pointer(&n, evt.pointer_id());
                    }
                }
                feed(
                    GestureEvent::Up { at: pointer_client(&evt), pointer_id: evt.pointer_id() },
                    None,
                );
            },
            onpointercancel: move |evt: PointerEvent| {
                if let Some(from) = *drag_from.peek() {
                    if let Some(n) = mounteds.peek().get(&from).cloned() {
                        platform::release_pointer(&n, evt.pointer_id());
                    }
                }
                feed(GestureEvent::Cancel, None);
            },
            onlostpointercapture: move |_| feed(GestureEvent::Cancel, None),
            ..attributes,
            for ix in 0..len {
                div {
                    key: "{ix}",
                    draggable: input.uses_native(),
                    class: item_class.clone(),
                    style: "touch-action: none;",
                    "data-dragging": if drag_from() == Some(ix) { "true" },
                    "data-drop-target": if over() == Some(ix) && drag_from() != Some(ix) { "true" },
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
                        if !primary_pointer(&evt) { return; }
                        evt.stop_propagation();
                        press_from.set(Some(ix));
                        // Capture on the stable tile so a mouse drag survives
                        // the cursor leaving it (no-op without the `web`
                        // feature).
                        if let Some(n) = mounteds.peek().get(&ix).cloned() {
                            platform::capture_pointer(&n, evt.pointer_id());
                        }
                        feed(
                            GestureEvent::Down { at: pointer_client(&evt), pointer_id: evt.pointer_id() },
                            None,
                        );
                    },
                    onpointermove: move |evt: PointerEvent| {
                        // Gate on the input mode like every other handler, so
                        // Native/Hybrid-mouse drags don't run the synthetic
                        // machine on each move (the container listener already
                        // tracks the drag; this per-tile one is a fallback).
                        if !input.uses_pointer(&evt.pointer_type()) {
                            return;
                        }
                        feed(
                            GestureEvent::Move { at: pointer_client(&evt), pointer_id: evt.pointer_id() },
                            Some(ix),
                        );
                    },
                    ondragstart: move |evt: DragEvent| {
                        if !input.uses_native() {
                            return;
                        }
                        evt.stop_propagation();
                        let _ = evt.data_transfer().set_data("text/plain", "dioxus-dnd-grid");
                        press_from.set(None);
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
                        press_from.set(None);
                        drag_from.set(None);
                        over.set(None);
                    },
                    ondragend: move |_| {
                        press_from.set(None);
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

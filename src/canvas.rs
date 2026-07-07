//! Free-position drops - node editors, whiteboards, floor planners. The drop
//! answers not just "what landed" but "*where* exactly", corrected for grab
//! offset, optionally snapped to a grid and clamped to bounds.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::core::{
    element_point, use_dnd, use_zone_id, use_zone_registry, DragMode, DropOutcome, ParentZone,
    Point, Rect, ZoneId, ZoneRecord,
};

/// A payload dropped at a position on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasDrop<T> {
    pub payload: T,
    /// Top-left position for the dropped element, relative to the canvas -
    /// already corrected for grab offset, snapping, and bounds.
    pub position: Point,
    /// The raw pointer position relative to the canvas, untouched.
    pub pointer: Point,
}

/// Snap positions to a square grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapGrid(pub f64);

impl SnapGrid {
    pub fn snap(&self, p: Point) -> Point {
        if self.0 <= 0.0 {
            return p;
        }
        Point::new(
            (p.x / self.0).round() * self.0,
            (p.y / self.0).round() * self.0,
        )
    }
}

/// Where a keyboard-driven canvas drop should place its pointer.
///
/// Pointer and native drops always use their event geometry. This policy is
/// only applied when the completed drop came from keyboard interaction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CanvasKeyboardPlacement {
    /// Use the selected zone geometry supplied by core keyboard navigation.
    #[default]
    Center,
    /// Place at the canvas origin.
    Origin,
    /// Place at a fixed canvas-local point.
    Fixed(Point),
}

/// Clamp reported top-left positions into `0..=width` × `0..=height`.
///
/// Bounds constrain the drop position returned in [`CanvasDrop::position`].
/// They do not account for the dropped element's own width or height; subtract
/// that yourself when you need the whole element to stay inside the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub fn clamp(&self, p: Point) -> Point {
        Point::new(p.x.clamp(0.0, self.width), p.y.clamp(0.0, self.height))
    }

    /// Clamp a top-left position so an item of `width` × `height` stays fully
    /// inside these bounds. If the item is larger than the bounds on an axis,
    /// that axis pins to zero.
    pub fn clamp_item(&self, p: Point, width: f64, height: f64) -> Point {
        Point::new(
            clamp_axis(p.x, 0.0, self.width - width),
            clamp_axis(p.y, 0.0, self.height - height),
        )
    }

    /// Clamp a rectangle by moving its top-left corner so the whole rectangle
    /// stays inside these bounds. The returned point is the corrected
    /// top-left.
    pub fn clamp_rect(&self, rect: Rect) -> Point {
        self.clamp_item(Point::new(rect.x, rect.y), rect.width, rect.height)
    }
}

/// Convert a viewport/client point to canvas-local coordinates.
pub fn client_to_canvas(client: Point, canvas_rect: Rect) -> Point {
    client - canvas_rect.origin()
}

/// Convert a canvas-local point to viewport/client coordinates.
pub fn canvas_to_client(point: Point, canvas_rect: Rect) -> Point {
    point + canvas_rect.origin()
}

/// Compute the corrected top-left canvas placement from a raw canvas-relative
/// pointer position and grab offset, then apply optional snap and bounds.
pub fn canvas_position(
    pointer: Point,
    grab: Point,
    snap: Option<SnapGrid>,
    bounds: Option<Bounds>,
) -> Point {
    let mut position = pointer - grab;
    if let Some(g) = snap {
        position = g.snap(position);
    }
    if let Some(b) = bounds {
        position = b.clamp(position);
    }
    position
}

/// Resolve the canvas-local pointer for a keyboard drop.
pub fn canvas_keyboard_pointer(policy: CanvasKeyboardPlacement, element: Point) -> Point {
    match policy {
        CanvasKeyboardPlacement::Center => element,
        CanvasKeyboardPlacement::Origin => Point::default(),
        CanvasKeyboardPlacement::Fixed(point) => point,
    }
}

fn clamp_axis(v: f64, min: f64, max: f64) -> f64 {
    if min > max {
        min
    } else {
        v.clamp(min, max)
    }
}

/// A canvas that reports drop positions.
///
/// Uses the shared `DndContext<T>`; start drags with the core `Draggable`
/// (its recorded grab offset is what makes the drop position feel exact -
/// the element lands where its ghost was, not where the pointer tip was).
///
/// While a drag is in flight the div carries `data-active="true"` (absent
/// otherwise) - style the canvas as a target then, e.g. Tailwind
/// `data-active:outline-dashed`.
#[component]
pub fn CanvasDropZone<T: Clone + PartialEq + 'static>(
    /// Stable identity; auto-generated if omitted.
    #[props(default)]
    id: Option<ZoneId>,
    /// Snap the corrected position to a grid.
    #[props(default)]
    snap: Option<SnapGrid>,
    /// Clamp the corrected top-left position into these bounds.
    #[props(default)]
    bounds: Option<Bounds>,
    /// Placement policy for keyboard-driven canvas drops.
    #[props(default)]
    keyboard: CanvasKeyboardPlacement,
    /// Announced to screen readers when a keyboard drag targets the canvas.
    #[props(default)]
    label: Option<String>,
    on_drop: EventHandler<CanvasDrop<T>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut dnd = use_dnd::<T>();
    let mut registry = use_zone_registry::<T>();
    let auto_id = use_zone_id();
    let zone_id = id.unwrap_or(auto_id);

    // Mirror `snap`/`bounds` into signals so the registry callback - which is
    // registered once (first render) - reads the *current* values, not the
    // ones captured at mount. Keep them current during render so child probes
    // and same-frame drops observe the latest geometry.
    let mut snap_now = use_signal(|| snap);
    let mut bounds_now = use_signal(|| bounds);
    let mut keyboard_now = use_signal(|| keyboard);
    if *snap_now.peek() != snap {
        snap_now.set(snap);
    }
    if *bounds_now.peek() != bounds {
        bounds_now.set(bounds);
    }
    if *keyboard_now.peek() != keyboard {
        keyboard_now.set(keyboard);
    }

    // Turn a corrected drop at `pointer` (canvas-relative) into a CanvasDrop.
    let place = move |payload: T, pointer: Point, grab: Point| {
        let position = canvas_position(pointer, grab, *snap_now.peek(), *bounds_now.peek());
        on_drop.call(CanvasDrop {
            payload,
            position,
            pointer,
        });
    };

    // --- pointer/keyboard path: register as a zone so `PointerDraggable`
    // (touch, pen, and mouse under the `web` feature) can drop here. The
    // registry delivers a `DropOutcome`; `element` is the pointer relative to
    // the canvas and `grab` is the pickup offset - exactly what native `ondrop`
    // computes below, so both paths place the element identically.
    let parent = try_use_context::<ParentZone>().map(|p| p.0);
    let mounted = use_signal(|| None::<Rc<MountedData>>);
    let rect = use_signal(|| None::<Rect>);
    let registered_drop = Callback::new(move |o: DropOutcome<T>| {
        let pointer = if o.mode == DragMode::Keyboard {
            canvas_keyboard_pointer(*keyboard_now.peek(), o.element)
        } else {
            o.element
        };
        place(o.payload, pointer, o.grab);
    });
    use_hook(|| {
        registry.register(ZoneRecord {
            id: zone_id,
            parent,
            label: label.clone(),
            on_drop: registered_drop,
            accepts: None,
            mounted,
            rect,
        });
    });
    use_drop(move || {
        registry.unregister(zone_id);
    });
    // Keep the registered label in sync if the prop changes across renders.
    // Registry readers only `peek`, so this render-time write can't loop.
    registry.sync_label(zone_id, label.clone());

    rsx! {
        div {
            "data-active": if dnd.dragging() { "true" },
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                let mut mounted = mounted;
                let mut rect = rect;
                mounted.set(Some(m.clone()));
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        rect.set(Some(Rect::new(
                            r.origin.x,
                            r.origin.y,
                            r.size.width,
                            r.size.height,
                        )));
                    }
                });
            },
            // --- native path: core `Draggable` sources drop through HTML5 drag.
            ondragover: move |evt: DragEvent| {
                if dnd.dragging() {
                    evt.prevent_default();
                }
            },
            ondrop: move |evt: DragEvent| {
                evt.prevent_default();
                let pointer = element_point(&evt);
                let grab = dnd.grab();
                if let Some((payload, _)) = dnd.take() {
                    place(payload, pointer, grab);
                }
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
    fn snap_and_clamp() {
        let g = SnapGrid(10.0);
        let p = g.snap(Point::new(14.9, 15.1));
        assert_eq!((p.x, p.y), (10.0, 20.0));
        assert_eq!(
            SnapGrid(0.0).snap(Point::new(3.3, 4.4)),
            Point::new(3.3, 4.4)
        );

        let b = Bounds {
            width: 100.0,
            height: 50.0,
        };
        let p = b.clamp(Point::new(-5.0, 999.0));
        assert_eq!((p.x, p.y), (0.0, 50.0));

        let corrected = Point::new(107.0, 46.0) - Point::new(9.0, 8.0);
        let positioned = b.clamp(g.snap(corrected));
        assert_eq!(positioned, Point::new(100.0, 40.0));
    }

    #[test]
    fn bounds_can_clamp_whole_items() {
        let b = Bounds {
            width: 100.0,
            height: 50.0,
        };

        assert_eq!(
            b.clamp_item(Point::new(90.0, 45.0), 20.0, 12.0),
            Point::new(80.0, 38.0)
        );
        assert_eq!(
            b.clamp_rect(Rect::new(-5.0, 60.0, 20.0, 10.0)),
            Point::new(0.0, 40.0)
        );
        assert_eq!(
            b.clamp_item(Point::new(20.0, 20.0), 150.0, 80.0),
            Point::new(0.0, 0.0)
        );
    }

    #[test]
    fn coordinate_helpers_convert_between_client_and_canvas() {
        let rect = Rect::new(40.0, 80.0, 320.0, 200.0);
        let client = Point::new(64.0, 128.0);
        let canvas = client_to_canvas(client, rect);

        assert_eq!(canvas, Point::new(24.0, 48.0));
        assert_eq!(canvas_to_client(canvas, rect), client);
    }

    #[test]
    fn canvas_position_applies_grab_snap_then_bounds() {
        let p = canvas_position(
            Point::new(107.0, 46.0),
            Point::new(9.0, 8.0),
            Some(SnapGrid(10.0)),
            Some(Bounds {
                width: 100.0,
                height: 50.0,
            }),
        );

        assert_eq!(p, Point::new(100.0, 40.0));
    }

    #[test]
    fn canvas_keyboard_pointer_uses_center_element_by_default() {
        assert_eq!(
            canvas_keyboard_pointer(CanvasKeyboardPlacement::default(), Point::new(40.0, 20.0)),
            Point::new(40.0, 20.0)
        );
    }

    #[test]
    fn canvas_keyboard_pointer_can_use_origin() {
        assert_eq!(
            canvas_keyboard_pointer(CanvasKeyboardPlacement::Origin, Point::new(40.0, 20.0)),
            Point::default()
        );
    }

    #[test]
    fn canvas_keyboard_pointer_can_use_fixed_point() {
        assert_eq!(
            canvas_keyboard_pointer(
                CanvasKeyboardPlacement::Fixed(Point::new(12.0, 18.0)),
                Point::new(40.0, 20.0),
            ),
            Point::new(12.0, 18.0)
        );
    }

    #[test]
    fn canvas_position_is_path_independent_for_same_geometry() {
        let pointer = Point::new(81.0, 97.0);
        let grab = Point::new(13.0, 19.0);
        let snap = Some(SnapGrid(8.0));
        let bounds = Some(Bounds {
            width: 120.0,
            height: 80.0,
        });

        let pointer_path = canvas_position(pointer, grab, snap, bounds);
        let native_path = canvas_position(pointer, grab, snap, bounds);

        assert_eq!(pointer_path, native_path);
        assert_eq!(pointer_path, Point::new(72.0, 80.0));
    }

    #[test]
    fn item_clamp_composes_after_canvas_position() {
        let top_left = canvas_position(Point::new(156.0, 86.0), Point::new(4.0, 5.0), None, None);
        let constrained = Bounds {
            width: 160.0,
            height: 90.0,
        }
        .clamp_item(top_left, 48.0, 32.0);

        assert_eq!(top_left, Point::new(152.0, 81.0));
        assert_eq!(constrained, Point::new(112.0, 58.0));
    }
}

//! Free-position drops — node editors, whiteboards, floor planners. The drop
//! answers not just "what landed" but "*where* exactly", corrected for grab
//! offset, optionally snapped to a grid and clamped to bounds.

use dioxus::prelude::*;

use crate::core::{element_point, use_dnd, use_zone_id, Point, ZoneId};

/// A payload dropped at a position on the canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasDrop<T> {
    pub payload: T,
    /// Top-left position for the dropped element, relative to the canvas —
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

/// Clamp positions into `0..=width` × `0..=height`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub fn clamp(&self, p: Point) -> Point {
        Point::new(p.x.clamp(0.0, self.width), p.y.clamp(0.0, self.height))
    }
}

/// A canvas that reports drop positions.
///
/// Uses the shared `DndContext<T>`; start drags with the core `Draggable`
/// (its recorded grab offset is what makes the drop position feel exact —
/// the element lands where its ghost was, not where the pointer tip was).
#[component]
pub fn CanvasDropZone<T: Clone + PartialEq + 'static>(
    /// Stable identity; auto-generated if omitted.
    #[props(default)]
    id: Option<ZoneId>,
    /// Snap the corrected position to a grid.
    #[props(default)]
    snap: Option<SnapGrid>,
    /// Clamp the corrected position into these bounds.
    #[props(default)]
    bounds: Option<Bounds>,
    on_drop: EventHandler<CanvasDrop<T>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut dnd = use_dnd::<T>();
    let auto_id = use_zone_id();
    let _zone_id = id.unwrap_or(auto_id);

    rsx! {
        div {
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
                    let mut position = pointer - grab;
                    if let Some(g) = snap {
                        position = g.snap(position);
                    }
                    if let Some(b) = bounds {
                        position = b.clamp(position);
                    }
                    on_drop.call(CanvasDrop { payload, position, pointer });
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
    }
}

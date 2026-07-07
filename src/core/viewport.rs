//! Pure pan/zoom geometry for canvas-like coordinate planes.
//!
//! This module intentionally has no event handling or component state. Apps
//! decide how zoom and pan are controlled; the helpers only convert points and
//! deltas between screen/local space and world space.

use super::types::Point;

/// Pan/zoom transform for a coordinate plane.
///
/// `pan` is the screen-space translation, and `zoom` is the scale from world
/// coordinates to screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasViewport {
    pub pan: Point,
    pub zoom: f64,
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            pan: Point::default(),
            zoom: 1.0,
        }
    }
}

impl CanvasViewport {
    pub fn new(pan: Point, zoom: f64) -> Self {
        Self { pan, zoom }
    }

    pub fn clamped_zoom(self, min: f64, max: f64) -> Self {
        let zoom = effective_zoom(self.zoom);
        let min = if min.is_finite() && min > 0.0 {
            min
        } else {
            0.0
        };
        let max = if max.is_finite() && max >= min {
            max
        } else {
            zoom.max(min)
        };

        Self {
            zoom: clamp_axis(zoom, min, max),
            ..self
        }
    }
}

/// Convert a screen/canvas-local point into world coordinates.
pub fn screen_to_world(point: Point, viewport: CanvasViewport) -> Point {
    let zoom = effective_zoom(viewport.zoom);
    Point::new(
        (point.x - viewport.pan.x) / zoom,
        (point.y - viewport.pan.y) / zoom,
    )
}

/// Convert a world point into screen/canvas-local coordinates.
pub fn world_to_screen(point: Point, viewport: CanvasViewport) -> Point {
    let zoom = effective_zoom(viewport.zoom);
    Point::new(
        point.x * zoom + viewport.pan.x,
        point.y * zoom + viewport.pan.y,
    )
}

/// Convert a screen-space movement/offset into world-space units.
pub fn screen_delta_to_world(delta: Point, viewport: CanvasViewport) -> Point {
    let zoom = effective_zoom(viewport.zoom);
    Point::new(delta.x / zoom, delta.y / zoom)
}

/// Convert a world-space movement/offset into screen-space units.
pub fn world_delta_to_screen(delta: Point, viewport: CanvasViewport) -> Point {
    let zoom = effective_zoom(viewport.zoom);
    Point::new(delta.x * zoom, delta.y * zoom)
}

fn clamp_axis(v: f64, min: f64, max: f64) -> f64 {
    if min > max {
        min
    } else {
        v.clamp(min, max)
    }
}

fn effective_zoom(zoom: f64) -> f64 {
    if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_points_round_trip_between_screen_and_world() {
        let viewport = CanvasViewport::new(Point::new(20.0, -10.0), 2.0);
        let world = Point::new(30.0, 25.0);
        let screen = world_to_screen(world, viewport);

        assert_eq!(screen, Point::new(80.0, 40.0));
        assert_eq!(screen_to_world(screen, viewport), world);
    }

    #[test]
    fn viewport_deltas_scale_without_pan() {
        let viewport = CanvasViewport::new(Point::new(20.0, -10.0), 2.0);

        assert_eq!(
            screen_delta_to_world(Point::new(10.0, -6.0), viewport),
            Point::new(5.0, -3.0)
        );
        assert_eq!(
            world_delta_to_screen(Point::new(5.0, -3.0), viewport),
            Point::new(10.0, -6.0)
        );
    }

    #[test]
    fn viewport_invalid_zoom_falls_back_to_identity_scale() {
        let viewport = CanvasViewport::new(Point::new(10.0, 20.0), 0.0);

        assert_eq!(
            screen_to_world(Point::new(15.0, 28.0), viewport),
            Point::new(5.0, 8.0)
        );
        assert_eq!(
            world_to_screen(Point::new(5.0, 8.0), viewport),
            Point::new(15.0, 28.0)
        );
    }

    #[test]
    fn viewport_zoom_can_be_clamped() {
        assert_eq!(
            CanvasViewport::new(Point::default(), 0.1)
                .clamped_zoom(0.5, 2.0)
                .zoom,
            0.5
        );
        assert_eq!(
            CanvasViewport::new(Point::default(), 3.0)
                .clamped_zoom(0.5, 2.0)
                .zoom,
            2.0
        );
    }

    #[test]
    fn viewport_zoom_clamp_ignores_invalid_bounds() {
        assert_eq!(
            CanvasViewport::new(Point::default(), 1.2)
                .clamped_zoom(f64::NAN, f64::NAN)
                .zoom,
            1.2
        );
        assert_eq!(
            CanvasViewport::new(Point::default(), 0.0)
                .clamped_zoom(0.5, f64::NAN)
                .zoom,
            1.0
        );
    }
}

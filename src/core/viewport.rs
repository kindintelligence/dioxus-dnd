//! Pure pan/zoom geometry for canvas-like coordinate planes.
//!
//! This module intentionally has no event handling or component state. Apps
//! decide how zoom and pan are controlled; the helpers only convert points and
//! deltas between screen/local space and world space.

use super::types::{clamp_axis, Point};

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

    /// This viewport zoomed to `zoom` about `at`, a screen/canvas-local point
    /// (the cursor, a pinch's centre): the world point under `at` stays under
    /// it. The zoom is held within `min..=max` first, as [`Self::clamped_zoom`]
    /// holds it, so a zoom past a limit doesn't shift the pan.
    pub fn zoom_at(self, at: Point, zoom: f64, min: f64, max: f64) -> Self {
        let zoom = Self { zoom, ..self }.clamped_zoom(min, max).zoom;
        let anchor = screen_to_world(at, self);
        Self {
            pan: Point::new(at.x - anchor.x * zoom, at.y - anchor.y * zoom),
            zoom,
        }
    }

    /// The zoom held within `min..=max`, with the pan unchanged: it zooms
    /// about the screen point where the world origin is drawn. To keep the
    /// point under the cursor still, use [`Self::zoom_at`].
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

    /// Zooming about a point keeps the world point under it still, and a zoom
    /// past the limit is clamped before the pan is worked out (it was clamped
    /// after, which moved the point: (200, 150) drifted to (0, -50)).
    #[test]
    fn zoom_at_keeps_the_point_under_the_cursor_still_within_the_limits() {
        let viewport = CanvasViewport::new(Point::new(100.0, 50.0), 1.0);
        let cursor = Point::new(200.0, 150.0);
        let under = screen_to_world(cursor, viewport);

        let zoomed = viewport.zoom_at(cursor, 4.0, 0.5, 2.0);

        assert_eq!(zoomed.zoom, 2.0);
        assert_eq!(world_to_screen(under, zoomed), cursor);
        assert_eq!(zoomed.pan, Point::new(0.0, -50.0));
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

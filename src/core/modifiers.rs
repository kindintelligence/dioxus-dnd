//! Composable drag constraints, applied as a chain to a proposed position.
//!
//! Where [`crate::canvas`] bakes snap-and-clamp into one component, this
//! module generalizes the idea (in the spirit of dnd-kit's modifiers): each
//! [`DragModifier`] is a pure `Point → Point` transform, and a chain feeds
//! each output into the next.
//!
//! ```rust
//! use dioxus_dnd::core::{apply_modifiers, DragModifier, ModifierCtx, Point, Rect};
//!
//! let ctx = ModifierCtx {
//!     container: Some(Rect::new(0.0, 0.0, 400.0, 300.0)),
//!     element: Some(Rect::new(0.0, 0.0, 40.0, 40.0)),
//! };
//! let chain = [
//!     DragModifier::LockAxis { horizontal: false, vertical: true },
//!     DragModifier::Snap { x: 8.0, y: 8.0 },
//!     DragModifier::KeepInside,
//! ];
//! let p = apply_modifiers(&chain, Point::new(123.0, 999.0), &ctx);
//! assert_eq!((p.x, p.y), (0.0, 260.0));
//! ```

use super::types::{Point, Rect};

/// Geometry a modifier may need. Fields it doesn't need can stay `None`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModifierCtx {
    /// The container the element should stay inside (for [`DragModifier::KeepInside`]).
    pub container: Option<Rect>,
    /// The dragged element's size, positioned at the proposed point.
    pub element: Option<Rect>,
}

/// A single constraint on a proposed drag position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragModifier {
    /// Zero out movement on locked axes. `horizontal: false` freezes X.
    LockAxis {
        /// Allow horizontal movement.
        horizontal: bool,
        /// Allow vertical movement.
        vertical: bool,
    },
    /// Snap each axis to a grid step; a step `<= 0` leaves that axis alone.
    Snap { x: f64, y: f64 },
    /// Clamp so the element (its `ModifierCtx::element` size) stays inside
    /// `ModifierCtx::container`. No-op when either rect is missing. If the
    /// element is larger than the container on an axis, it pins to the
    /// container's origin on that axis.
    KeepInside,
}

impl DragModifier {
    /// Apply this modifier to a proposed top-left position.
    pub fn apply(self, p: Point, ctx: &ModifierCtx) -> Point {
        match self {
            DragModifier::LockAxis {
                horizontal,
                vertical,
            } => Point::new(
                if horizontal { p.x } else { 0.0 },
                if vertical { p.y } else { 0.0 },
            ),
            DragModifier::Snap { x, y } => Point::new(snap(p.x, x), snap(p.y, y)),
            DragModifier::KeepInside => {
                let (Some(c), Some(e)) = (ctx.container, ctx.element) else {
                    return p;
                };
                Point::new(
                    clamp_axis(p.x, c.x, c.x + c.width - e.width),
                    clamp_axis(p.y, c.y, c.y + c.height - e.height),
                )
            }
        }
    }
}

/// Run a chain of modifiers in order over a proposed position.
pub fn apply_modifiers(chain: &[DragModifier], mut p: Point, ctx: &ModifierCtx) -> Point {
    for m in chain {
        p = m.apply(p, ctx);
    }
    p
}

fn snap(v: f64, step: f64) -> f64 {
    if step > 0.0 {
        (v / step).round() * step
    } else {
        v
    }
}

fn clamp_axis(v: f64, min: f64, max: f64) -> f64 {
    if min > max {
        min // element larger than container: pin to origin
    } else {
        v.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_compose_in_order() {
        let ctx = ModifierCtx {
            container: Some(Rect::new(0.0, 0.0, 100.0, 100.0)),
            element: Some(Rect::new(0.0, 0.0, 30.0, 30.0)),
        };
        // lock X, snap Y to 20, keep inside
        let chain = [
            DragModifier::LockAxis {
                horizontal: false,
                vertical: true,
            },
            DragModifier::Snap { x: 0.0, y: 20.0 },
            DragModifier::KeepInside,
        ];
        let p = apply_modifiers(&chain, Point::new(55.0, 91.0), &ctx);
        assert_eq!((p.x, p.y), (0.0, 70.0)); // x frozen, y 91→100(snap)→70(clamp)
    }

    #[test]
    fn keep_inside_pins_oversized_elements() {
        let ctx = ModifierCtx {
            container: Some(Rect::new(10.0, 10.0, 50.0, 50.0)),
            element: Some(Rect::new(0.0, 0.0, 200.0, 20.0)), // wider than container
        };
        let p = DragModifier::KeepInside.apply(Point::new(-40.0, 100.0), &ctx);
        assert_eq!((p.x, p.y), (10.0, 40.0));
    }

    #[test]
    fn keep_inside_without_rects_is_noop() {
        let p = DragModifier::KeepInside.apply(Point::new(7.0, 8.0), &ModifierCtx::default());
        assert_eq!((p.x, p.y), (7.0, 8.0));
    }
}

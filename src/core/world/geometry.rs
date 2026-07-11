//! Window identity and placement: [`WindowKey`], the pure client-px <->
//! global-physical-px conversion math, and the host-fed [`WindowGeometry`].

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

use crate::core::types::Point;

static NEXT_WINDOW_KEY: AtomicU64 = AtomicU64::new(1);
/// Focus stamps start at 1 so a never-focused window's 0 always loses.
static NEXT_FOCUS_STAMP: AtomicU64 = AtomicU64::new(1);

/// Identifies one joined window within a [`DndWorld`](super::DndWorld). Process-unique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowKey(pub u64);

impl WindowKey {
    /// Generate a process-unique window key.
    pub fn auto() -> Self {
        Self(NEXT_WINDOW_KEY.fetch_add(1, Ordering::Relaxed))
    }
}

// --- pure conversion math (unit-tested; signals stay out of it) --------

/// Client CSS px of a window -> global desktop physical px.
pub(crate) fn client_to_global(client: Point, origin: Point, scale: f64) -> Point {
    Point::new(origin.x + client.x * scale, origin.y + client.y * scale)
}

/// Global desktop physical px -> client CSS px of a window.
pub(crate) fn global_to_client(global: Point, origin: Point, scale: f64) -> Point {
    let s = if scale > 0.0 { scale } else { 1.0 };
    Point::new((global.x - origin.x) / s, (global.y - origin.y) / s)
}

/// Is `global` inside a window whose client area starts at `origin` with
/// `size`, both in physical px? Inclusive of edges, like [`crate::core::types::Rect`].
pub(crate) fn window_contains(global: Point, origin: Point, size: (f64, f64)) -> bool {
    global.x >= origin.x
        && global.x <= origin.x + size.0
        && global.y >= origin.y
        && global.y <= origin.y + size.1
}

/// One window's placement on the desktop, as reactive signals the host
/// feeds. Copy handle; create one per window (the provider creates an inert
/// one when none is in context) and keep it updated from your windowing
/// layer. Missing placement or host ineligibility makes it inert: the window
/// still drags internally, but cannot take part in cross-window hit-testing.
pub struct WindowGeometry {
    /// Client-area top-left in global physical px (`inner_position()`).
    origin: Signal<Option<Point>>,
    /// Client-area size in physical px.
    size: Signal<Option<(f64, f64)>>,
    /// Window scale factor (physical px per CSS px).
    scale: Signal<f64>,
    /// Monotonic focus stamp; higher = more recently focused. Breaks ties
    /// when overlapping windows both contain a point (no z-order queries
    /// exist on desktop, so focus recency approximates it).
    focused: Signal<u64>,
    /// Whether the host currently considers this window eligible for global
    /// hit-testing (visible, restored, and otherwise interactive).
    eligible: Signal<bool>,
}

impl Copy for WindowGeometry {}
impl Clone for WindowGeometry {
    fn clone(&self) -> Self {
        *self
    }
}
impl PartialEq for WindowGeometry {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin
    }
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowGeometry {
    /// A fresh, inert geometry owned by the current scope.
    pub fn new() -> Self {
        Self {
            origin: Signal::new(None),
            size: Signal::new(None),
            scale: Signal::new(1.0),
            focused: Signal::new(0),
            // Existing hosts only feed placement, so eligibility defaults on
            // and remains an additive capability gate.
            eligible: Signal::new(true),
        }
    }

    /// Update the window's placement. `origin` and `size` describe the
    /// client area in global physical px; `scale` is the window's scale
    /// factor. No-op writes are skipped, so this is safe to call from
    /// high-frequency window events.
    pub fn set(&self, origin: Point, size: (f64, f64), scale: f64) {
        // try_write throughout (here and below): host feeds run from
        // windowing-layer callbacks that can fire one event after the
        // owning window's signals died - see the read-side note below.
        let (mut o, mut sz, mut sc) = (self.origin, self.size, self.scale);
        if matches!(o.try_peek().as_deref(), Ok(v) if *v != Some(origin)) {
            if let Ok(mut w) = o.try_write() {
                *w = Some(origin);
            }
        }
        if matches!(sz.try_peek().as_deref(), Ok(v) if *v != Some(size)) {
            if let Ok(mut w) = sz.try_write() {
                *w = Some(size);
            }
        }
        if matches!(sc.try_peek().as_deref(), Ok(v) if *v != scale) {
            if let Ok(mut w) = sc.try_write() {
                *w = scale;
            }
        }
    }

    /// Forget the placement (geometry became unavailable); the window keeps
    /// working as a single-window drag surface.
    pub fn clear(&self) {
        let (mut o, mut sz) = (self.origin, self.size);
        if matches!(o.try_peek().as_deref(), Ok(Some(_))) {
            if let Ok(mut w) = o.try_write() {
                *w = None;
            }
        }
        if matches!(sz.try_peek().as_deref(), Ok(Some(_))) {
            if let Ok(mut w) = sz.try_write() {
                *w = None;
            }
        }
    }

    /// Include or exclude this window from global hit-testing without
    /// discarding its last known placement.
    pub fn set_eligible(&self, eligible: bool) {
        let mut value = self.eligible;
        if matches!(value.try_peek().as_deref(), Ok(current) if *current != eligible) {
            if let Ok(mut writer) = value.try_write() {
                *writer = eligible;
            }
        }
    }

    /// Whether the host currently allows this window to receive a global
    /// drag. This is a subscribing, dead-safe read.
    pub fn eligible(&self) -> bool {
        self.eligible
            .try_read()
            .map(|value| *value)
            .unwrap_or(false)
    }

    /// Record that this window was just focused (see `focused`).
    pub fn mark_focused(&self) {
        let mut f = self.focused;
        if let Ok(mut w) = f.try_write() {
            *w = NEXT_FOCUS_STAMP.fetch_add(1, Ordering::Relaxed);
        };
    }

    // Reads below use try_peek and degrade to "geometry unknown" when the
    // signals are gone. A geometry's signals are host-owned and usually
    // window-scoped, so they die with their window's VirtualDom - but a
    // copy inside a WindowRecord (or a handler closure) can race the
    // pruning and be read one event late. On Windows that read happens
    // inside a Win32 callback, where the resulting panic cannot unwind
    // and kills the process with 0xc000041d (observed; the
    // DioxusLabs/dioxus#4466 failure class). Stale geometry is already a
    // modeled state (Wayland), so degrading is honest, not a mask.

    /// Is the placement known and currently eligible for global hit-testing?
    /// This is a subscribing, dead-safe read.
    pub fn live(&self) -> bool {
        // Read every input independently so a currently inert geometry still
        // subscribes to each capability that can make it live later.
        let has_origin = matches!(self.origin.try_read().as_deref(), Ok(Some(_)));
        let has_size = matches!(self.size.try_read().as_deref(), Ok(Some(_)));
        let eligible = self
            .eligible
            .try_read()
            .map(|value| *value)
            .unwrap_or(false);
        has_origin && has_size && eligible
    }

    fn origin_scale(&self) -> Option<(Point, f64)> {
        let origin = (*self.origin.try_peek().ok()?)?;
        let scale = self.scale.try_peek().map(|s| *s).unwrap_or(1.0);
        Some((origin, scale))
    }

    /// This window's client CSS px -> global physical px. `None` until the
    /// placement is known.
    pub fn to_global(&self, client: Point) -> Option<Point> {
        let (origin, scale) = self.origin_scale()?;
        Some(client_to_global(client, origin, scale))
    }

    /// Global physical px -> this window's client CSS px. `None` until the
    /// placement is known.
    pub fn to_client(&self, global: Point) -> Option<Point> {
        let (origin, scale) = self.origin_scale()?;
        Some(global_to_client(global, origin, scale))
    }

    /// Does this eligible window's client area contain `global` (physical
    /// px)? Always false while placement is unknown or eligibility is off.
    pub fn contains_global(&self, global: Point) -> bool {
        // Imperative hit-testing must not subscribe its caller. Eligibility
        // therefore peeks here even though the public status reads subscribe.
        if !self
            .eligible
            .try_peek()
            .map(|eligible| *eligible)
            .unwrap_or(false)
        {
            return false;
        }
        let origin = self.origin.try_peek().ok().and_then(|o| *o);
        let size = self.size.try_peek().ok().and_then(|s| *s);
        match (origin, size) {
            (Some(origin), Some(size)) => window_contains(global, origin, size),
            _ => false,
        }
    }

    /// The window's scale factor.
    pub fn scale(&self) -> f64 {
        self.scale.try_peek().map(|s| *s).unwrap_or(1.0)
    }

    /// The current focus stamp (0 = never focused).
    pub fn focus_stamp(&self) -> u64 {
        self.focused.try_peek().map(|f| *f).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_round_trip_under_mixed_scales() {
        for scale in [1.0, 1.5, 2.0] {
            let origin = Point::new(1200.0, 300.0);
            let client = Point::new(80.0, 40.5);
            let global = client_to_global(client, origin, scale);
            assert_eq!(
                global,
                Point::new(1200.0 + 80.0 * scale, 300.0 + 40.5 * scale)
            );
            let back = global_to_client(global, origin, scale);
            assert!((back.x - client.x).abs() < 1e-9);
            assert!((back.y - client.y).abs() < 1e-9);
        }
    }

    #[test]
    fn degenerate_scale_does_not_divide_by_zero() {
        let p = global_to_client(Point::new(10.0, 10.0), Point::new(0.0, 0.0), 0.0);
        assert_eq!(p, Point::new(10.0, 10.0));
    }

    #[test]
    fn window_containment_is_edge_inclusive() {
        let origin = Point::new(100.0, 100.0);
        let size = (800.0, 600.0);
        assert!(window_contains(Point::new(100.0, 100.0), origin, size));
        assert!(window_contains(Point::new(900.0, 700.0), origin, size));
        assert!(!window_contains(Point::new(99.9, 100.0), origin, size));
        assert!(!window_contains(Point::new(901.0, 300.0), origin, size));
    }

    #[test]
    fn window_keys_are_unique() {
        let a = WindowKey::auto();
        let b = WindowKey::auto();
        assert_ne!(a, b);
    }
}

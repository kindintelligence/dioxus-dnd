//! A formal state machine for pointer-driven drag gestures.
//!
//! The lifecycle every synthesized drag goes through — press, threshold
//! promotion, tracking, release or abort — is modeled here as a pure
//! transition function over explicit states and events, so every edge
//! (stray pointer ids, release before the threshold, cancellation mid-drag)
//! is an exhaustive match arm with a test, not an ad-hoc `if`.
//!
//! [`crate::pointer::PointerDraggable`] drives this machine; you can drive
//! it yourself to build custom pointer interactions with the same rigor:
//!
//! ```rust
//! use dioxus_dnd::core::{transition, GestureEffect, GesturePhase, GestureEvent, Point};
//!
//! let mut phase = GesturePhase::Idle;
//! let (next, fx) = transition(phase, GestureEvent::Down { at: Point::new(10.0, 10.0), pointer_id: 1 }, 8.0);
//! phase = next;
//! assert_eq!(fx, GestureEffect::None); // pressed, not yet a drag
//!
//! let (next, fx) = transition(phase, GestureEvent::Move { at: Point::new(30.0, 10.0), pointer_id: 1 }, 8.0);
//! assert!(matches!(fx, GestureEffect::Begin { .. })); // crossed the threshold
//! # let _ = next;
//! ```

use super::types::Point;

/// Where a pointer gesture currently stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GesturePhase {
    /// No interaction in progress.
    Idle,
    /// Pointer is down on a draggable but hasn't traveled past the
    /// threshold — could still resolve as a tap.
    Pressed {
        /// Where the press started (client coordinates).
        origin: Point,
        /// The pointer that owns this gesture.
        pointer_id: i32,
    },
    /// An active drag.
    Dragging {
        /// Where the press started.
        origin: Point,
        /// The pointer that owns this gesture.
        pointer_id: i32,
    },
}

/// An input to the machine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureEvent {
    /// Pointer pressed.
    Down { at: Point, pointer_id: i32 },
    /// Pointer moved.
    Move { at: Point, pointer_id: i32 },
    /// Pointer released.
    Up { at: Point, pointer_id: i32 },
    /// The platform cancelled the gesture (`pointercancel`).
    Cancel,
}

/// What the caller should do after a transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureEffect {
    /// Nothing — including events from foreign pointer ids, which the
    /// machine deliberately ignores.
    None,
    /// The threshold was crossed: begin the drag. `origin` is where the
    /// press started (use it for the grab offset), `at` is the current
    /// pointer position.
    Begin { origin: Point, at: Point },
    /// An active drag moved: track the pointer and update hover.
    Track { at: Point },
    /// An active drag released: attempt the drop at `at`.
    Drop { at: Point },
    /// The press resolved as a tap (released before the threshold).
    Tap,
    /// An active drag was aborted: clean up drag state.
    Abort,
}

/// Advance the machine. Pure: same inputs, same outputs, no side effects.
///
/// `threshold` is the travel distance (CSS px) that promotes a press to a
/// drag; releases inside it resolve as [`GestureEffect::Tap`].
pub fn transition(
    phase: GesturePhase,
    event: GestureEvent,
    threshold: f64,
) -> (GesturePhase, GestureEffect) {
    use GestureEffect as Fx;
    use GesturePhase as P;

    match (phase, event) {
        // --- starting -----------------------------------------------------
        (P::Idle, GestureEvent::Down { at, pointer_id }) => (
            P::Pressed {
                origin: at,
                pointer_id,
            },
            Fx::None,
        ),
        // A second pointer pressing mid-gesture doesn't steal it.
        (p @ (P::Pressed { .. } | P::Dragging { .. }), GestureEvent::Down { .. }) => (p, Fx::None),

        // --- pressed: promote, tap, or wait --------------------------------
        (
            P::Pressed { origin, pointer_id },
            GestureEvent::Move {
                at,
                pointer_id: pid,
            },
        ) if pid == pointer_id => {
            let d = at - origin;
            if (d.x * d.x + d.y * d.y).sqrt() >= threshold {
                (P::Dragging { origin, pointer_id }, Fx::Begin { origin, at })
            } else {
                (P::Pressed { origin, pointer_id }, Fx::None)
            }
        }
        (
            P::Pressed { pointer_id, .. },
            GestureEvent::Up {
                pointer_id: pid, ..
            },
        ) if pid == pointer_id => (P::Idle, Fx::Tap),

        // --- dragging: track, drop ----------------------------------------
        (
            P::Dragging { origin, pointer_id },
            GestureEvent::Move {
                at,
                pointer_id: pid,
            },
        ) if pid == pointer_id => (P::Dragging { origin, pointer_id }, Fx::Track { at }),
        (
            P::Dragging { pointer_id, .. },
            GestureEvent::Up {
                at,
                pointer_id: pid,
            },
        ) if pid == pointer_id => (P::Idle, Fx::Drop { at }),

        // --- cancellation ---------------------------------------------------
        (P::Dragging { .. }, GestureEvent::Cancel) => (P::Idle, Fx::Abort),
        (P::Pressed { .. }, GestureEvent::Cancel) => (P::Idle, Fx::None),
        (P::Idle, GestureEvent::Cancel) => (P::Idle, Fx::None),

        // --- everything else is deliberately inert -------------------------
        // Foreign pointer ids, moves/ups while idle: ignored, not errors —
        // browsers deliver stray events and a UI library shrugs them off.
        (p, _) => (p, Fx::None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: f64 = 8.0;

    fn down(x: f64, y: f64, pid: i32) -> GestureEvent {
        GestureEvent::Down {
            at: Point::new(x, y),
            pointer_id: pid,
        }
    }
    fn mv(x: f64, y: f64, pid: i32) -> GestureEvent {
        GestureEvent::Move {
            at: Point::new(x, y),
            pointer_id: pid,
        }
    }
    fn up(x: f64, y: f64, pid: i32) -> GestureEvent {
        GestureEvent::Up {
            at: Point::new(x, y),
            pointer_id: pid,
        }
    }

    #[test]
    fn full_drag_lifecycle() {
        let (p, fx) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        assert_eq!(fx, GestureEffect::None);

        // sub-threshold wiggle stays pressed
        let (p, fx) = transition(p, mv(3.0, 3.0, 1), T);
        assert!(matches!(p, GesturePhase::Pressed { .. }));
        assert_eq!(fx, GestureEffect::None);

        // crossing the threshold begins the drag with the press origin
        let (p, fx) = transition(p, mv(10.0, 0.0, 1), T);
        assert_eq!(
            fx,
            GestureEffect::Begin {
                origin: Point::new(0.0, 0.0),
                at: Point::new(10.0, 0.0)
            }
        );

        let (p, fx) = transition(p, mv(20.0, 5.0, 1), T);
        assert_eq!(
            fx,
            GestureEffect::Track {
                at: Point::new(20.0, 5.0)
            }
        );

        let (p, fx) = transition(p, up(20.0, 5.0, 1), T);
        assert_eq!(p, GesturePhase::Idle);
        assert_eq!(
            fx,
            GestureEffect::Drop {
                at: Point::new(20.0, 5.0)
            }
        );
    }

    #[test]
    fn release_before_threshold_is_a_tap() {
        let (p, _) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        let (p, fx) = transition(p, up(2.0, 2.0, 1), T);
        assert_eq!(p, GesturePhase::Idle);
        assert_eq!(fx, GestureEffect::Tap);
    }

    #[test]
    fn foreign_pointer_ids_are_ignored() {
        let (p, _) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        // a second finger moves and lifts: gesture unaffected
        let (p, fx) = transition(p, mv(100.0, 100.0, 2), T);
        assert!(matches!(p, GesturePhase::Pressed { .. }));
        assert_eq!(fx, GestureEffect::None);
        let (p, fx) = transition(p, up(100.0, 100.0, 2), T);
        assert!(matches!(p, GesturePhase::Pressed { .. }));
        assert_eq!(fx, GestureEffect::None);
        // a second finger pressing doesn't steal ownership
        let (p, fx) = transition(p, down(50.0, 50.0, 2), T);
        assert!(matches!(p, GesturePhase::Pressed { pointer_id: 1, .. }));
        assert_eq!(fx, GestureEffect::None);
    }

    #[test]
    fn cancel_paths() {
        // cancel mid-drag aborts
        let (p, _) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        let (p, _) = transition(p, mv(20.0, 0.0, 1), T);
        let (p, fx) = transition(p, GestureEvent::Cancel, T);
        assert_eq!((p, fx), (GesturePhase::Idle, GestureEffect::Abort));
        // cancel while merely pressed is silent
        let (p, _) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        let (p, fx) = transition(p, GestureEvent::Cancel, T);
        assert_eq!((p, fx), (GesturePhase::Idle, GestureEffect::None));
    }

    #[test]
    fn stray_events_while_idle_are_inert() {
        for ev in [mv(9.0, 9.0, 1), up(9.0, 9.0, 1)] {
            let (p, fx) = transition(GesturePhase::Idle, ev, T);
            assert_eq!((p, fx), (GesturePhase::Idle, GestureEffect::None));
        }
    }

    #[test]
    fn exact_threshold_promotes() {
        let (p, _) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        let (_, fx) = transition(p, mv(8.0, 0.0, 1), T);
        assert!(matches!(fx, GestureEffect::Begin { .. }));
    }
}

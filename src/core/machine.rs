//! A formal state machine for pointer-driven drag gestures.
//!
//! The lifecycle every synthesized drag goes through - press, threshold
//! promotion, tracking, release or abort - is modeled here as a pure
//! transition function over explicit states and events, so every edge
//! (stray pointer ids, release before the threshold, cancellation mid-drag)
//! is an exhaustive match arm with a test, not an ad-hoc `if`.
//!
//! [`crate::core::Draggable`] drives this machine; you can drive it yourself
//! to build custom pointer interactions with the same rigor:
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
    /// threshold - could still resolve as a tap.
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
    /// The press's hold timer elapsed while the pointer stayed put: a
    /// long-press. Promotes a matching [`GesturePhase::Pressed`] straight to
    /// a drag; inert in every other phase, so a stale timer firing after the
    /// gesture already resolved is harmless.
    Hold { pointer_id: i32 },
    /// The platform cancelled the gesture (`pointercancel`).
    Cancel,
}

/// How a press gets promoted to a drag - the policy half of the touch
/// auto-sensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Promotion {
    /// Travel in any direction past the threshold begins the drag. The right
    /// policy for mouse and pen, and for touch surfaces that own every
    /// gesture (`touch-action: none`).
    #[default]
    Distance,
    /// Touch sharing the viewport with native vertical scrolling
    /// (`touch-action: pan-y`): a [`GestureEvent::Hold`] or a
    /// sideways-dominant pull (|dx| > |dy|) past the threshold begins the
    /// drag, while a vertical-dominant pull resolves the press as scroll
    /// intent - the machine returns to `Idle` and the browser's pan takes
    /// the gesture (an exact diagonal counts as scroll).
    HoldOrSideways,
}

/// What the caller should do after a transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureEffect {
    /// Nothing - including events from foreign pointer ids, which the
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

/// Advance the machine with the default [`Promotion::Distance`] policy.
/// Pure: same inputs, same outputs, no side effects.
///
/// `threshold` is the travel distance (CSS px) that promotes a press to a
/// drag; releases inside it resolve as [`GestureEffect::Tap`].
pub fn transition(
    phase: GesturePhase,
    event: GestureEvent,
    threshold: f64,
) -> (GesturePhase, GestureEffect) {
    transition_with(phase, event, threshold, Promotion::Distance)
}

/// Advance the machine under an explicit [`Promotion`] policy. Pure: same
/// inputs, same outputs, no side effects.
///
/// The policy only shapes how a [`GesturePhase::Pressed`] press becomes a
/// drag; everything after `Begin` is policy-independent.
pub fn transition_with(
    phase: GesturePhase,
    event: GestureEvent,
    threshold: f64,
    promotion: Promotion,
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
                match promotion {
                    Promotion::Distance => {
                        (P::Dragging { origin, pointer_id }, Fx::Begin { origin, at })
                    }
                    Promotion::HoldOrSideways if d.x.abs() > d.y.abs() => {
                        (P::Dragging { origin, pointer_id }, Fx::Begin { origin, at })
                    }
                    // Vertical-dominant travel is scroll intent: yield the
                    // gesture. The browser pan (when the surface can scroll)
                    // arrives as `pointercancel`, which finds Idle and stays
                    // silent.
                    Promotion::HoldOrSideways => (P::Idle, Fx::None),
                }
            } else {
                (P::Pressed { origin, pointer_id }, Fx::None)
            }
        }
        // A long-press is a promotion regardless of policy: drivers that
        // never arm a timer simply never feed `Hold`. The drag begins at the
        // press origin, so the grab offset is exactly where the finger sat.
        (P::Pressed { origin, pointer_id }, GestureEvent::Hold { pointer_id: pid })
            if pid == pointer_id =>
        {
            (
                P::Dragging { origin, pointer_id },
                Fx::Begin { origin, at: origin },
            )
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
        // Foreign pointer ids, moves/ups while idle: ignored, not errors -
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

    fn hold(pid: i32) -> GestureEvent {
        GestureEvent::Hold { pointer_id: pid }
    }

    fn step_auto(p: GesturePhase, ev: GestureEvent) -> (GesturePhase, GestureEffect) {
        transition_with(p, ev, T, Promotion::HoldOrSideways)
    }

    #[test]
    fn hold_promotes_at_the_press_origin() {
        let (p, _) = step_auto(GesturePhase::Idle, down(5.0, 5.0, 1));
        let (p, fx) = step_auto(p, hold(1));
        assert!(matches!(p, GesturePhase::Dragging { .. }));
        assert_eq!(
            fx,
            GestureEffect::Begin {
                origin: Point::new(5.0, 5.0),
                at: Point::new(5.0, 5.0)
            }
        );
        // and the drag then tracks normally
        let (_, fx) = step_auto(p, mv(5.0, 40.0, 1));
        assert_eq!(
            fx,
            GestureEffect::Track {
                at: Point::new(5.0, 40.0)
            }
        );
    }

    #[test]
    fn stale_or_foreign_hold_is_inert() {
        // foreign pointer id while pressed
        let (p, _) = step_auto(GesturePhase::Idle, down(0.0, 0.0, 1));
        let (p2, fx) = step_auto(p, hold(2));
        assert_eq!((p2, fx), (p, GestureEffect::None));
        // firing while idle (gesture already resolved)
        let (p, fx) = step_auto(GesturePhase::Idle, hold(1));
        assert_eq!((p, fx), (GesturePhase::Idle, GestureEffect::None));
        // firing mid-drag changes nothing
        let (p, _) = step_auto(GesturePhase::Idle, down(0.0, 0.0, 1));
        let (p, _) = step_auto(p, hold(1));
        let (p2, fx) = step_auto(p, hold(1));
        assert_eq!((p2, fx), (p, GestureEffect::None));
        // hold under the Distance policy still promotes - the policy shapes
        // Move promotion only, and Distance drivers never feed Hold anyway
        let (p, _) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        let (_, fx) = transition(p, hold(1), T);
        assert!(matches!(fx, GestureEffect::Begin { .. }));
    }

    #[test]
    fn sideways_pull_promotes_under_auto() {
        let (p, _) = step_auto(GesturePhase::Idle, down(0.0, 0.0, 1));
        let (_, fx) = step_auto(p, mv(9.0, 4.0, 1));
        assert_eq!(
            fx,
            GestureEffect::Begin {
                origin: Point::new(0.0, 0.0),
                at: Point::new(9.0, 4.0)
            }
        );
    }

    #[test]
    fn vertical_pull_yields_to_scroll_under_auto() {
        let (p, _) = step_auto(GesturePhase::Idle, down(0.0, 0.0, 1));
        // sub-threshold drift keeps waiting
        let (p, fx) = step_auto(p, mv(1.0, 5.0, 1));
        assert!(matches!(p, GesturePhase::Pressed { .. }));
        assert_eq!(fx, GestureEffect::None);
        // vertical-dominant travel past the threshold resolves as scroll
        let (p, fx) = step_auto(p, mv(2.0, 12.0, 1));
        assert_eq!((p, fx), (GesturePhase::Idle, GestureEffect::None));
        // an exact diagonal counts as scroll, not drag
        let (p, _) = step_auto(GesturePhase::Idle, down(0.0, 0.0, 1));
        let (p, fx) = step_auto(p, mv(10.0, 10.0, 1));
        assert_eq!((p, fx), (GesturePhase::Idle, GestureEffect::None));
    }

    #[test]
    fn distance_policy_promotes_vertical_pulls_unchanged() {
        let (p, _) = transition(GesturePhase::Idle, down(0.0, 0.0, 1), T);
        let (_, fx) = transition(p, mv(0.0, 12.0, 1), T);
        assert!(matches!(fx, GestureEffect::Begin { .. }));
    }
}

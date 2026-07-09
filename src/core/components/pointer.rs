//! DOM pointer normalization shared by drag surfaces: which presses begin
//! a drag, how touch shares an element with native scrolling, and the
//! long-press clock behind [`TouchSense::Auto`].

use dioxus::prelude::*;

use crate::core::types::{Point, TouchSense};

/// How long a touch must stay put before [`TouchSense::Auto`] promotes the
/// press to a drag - the familiar mobile long-press beat (dnd-kit and iOS
/// both sit around this value).
pub(crate) const HOLD_DELAY_MS: f64 = 250.0;

/// The functional inline style for a drag surface under each touch policy.
/// `Auto` also pins down selection: a long-press that starts selecting text
/// (or popping the iOS callout) would eat the hold.
pub(crate) fn touch_style(touch: TouchSense) -> &'static str {
    match touch {
        // `pinch-zoom` stays allowed: two fingers were never a drag, and
        // zooming is an accessibility floor.
        TouchSense::Auto => {
            "touch-action: pan-y pinch-zoom; user-select: none; \
             -webkit-user-select: none; -webkit-touch-callout: none;"
        }
        TouchSense::Immediate => {
            "touch-action: none; user-select: none; -webkit-user-select: none; \
             -webkit-touch-callout: none;"
        }
    }
}

/// The long-press clock for [`TouchSense::Auto`], with no timer dependency:
/// a zero-size element runs a no-op CSS animation for the hold duration and
/// `animationend` is the alarm. Mounting arms it, unmounting (the gesture
/// resolved some other way) cancels it - the element's lifecycle IS the
/// timer's, so a stale callback can't outlive its press. Works on any
/// renderer with CSS animations; where there are none, `Auto` quietly loses
/// only its long-press path (sideways pulls still drag).
#[component]
pub(crate) fn HoldTimer(pointer_id: i32, on_hold: EventHandler<i32>) -> Element {
    rsx! {
        // The inline `display: none` matters: dioxus-web renders a bare
        // `style {}` element visibly, so without it the keyframes rule
        // flashes as page text on every press (same guard as
        // `a11y::use_reduced_motion_css`).
        style { style: "display: none;",
            "@keyframes dnd-hold-timer {{ from {{ opacity: 0.99; }} to {{ opacity: 1; }} }}"
        }
        div {
            style: "position: absolute; width: 0; height: 0; overflow: hidden; \
                    animation: dnd-hold-timer {HOLD_DELAY_MS}ms linear forwards;",
            aria_hidden: true,
            onanimationend: move |_| on_hold.call(pointer_id),
        }
    }
}

pub(super) fn pointer_client(evt: &PointerEvent) -> Point {
    let c = evt.client_coordinates();
    Point::new(c.x, c.y)
}

/// Should this press begin a drag? The `button` field on down events is
/// reliable everywhere (GTK reads it from the event itself, not the
/// modifier state mask), while `is_primary` is only meaningful for
/// touch/pen: WebKit hardcodes it `true` for mouse-derived pointers, and
/// synthesized mouse streams (RDP under WSLg, some test harnesses) can
/// flake it `false` - silently swallowing real presses. So mice gate on
/// the trigger button; fingers and pens gate on primacy.
pub(crate) fn primary_press(evt: &PointerEvent) -> bool {
    if evt.pointer_type() == "mouse" {
        evt.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary)
    } else {
        evt.is_primary()
    }
}

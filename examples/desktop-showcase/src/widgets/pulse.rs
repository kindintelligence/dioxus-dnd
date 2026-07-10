//! Crew pulse: an ECG-style trace scrolled by the tick count plus a live
//! BPM readout.

use dioxus::prelude::*;

use crate::model::WidgetState;

/// One heartbeat complex, repeated and scrolled; x values in 0..40.
const BEAT: [(f64, f64); 8] = [
    (0.0, 20.0),
    (10.0, 20.0),
    (14.0, 18.5),
    (17.0, 30.0),
    (20.0, 4.0),
    (23.0, 24.0),
    (28.0, 20.0),
    (40.0, 20.0),
];

#[component]
pub fn PulseBody(state: Signal<WidgetState>) -> Element {
    let s = state();
    // Scroll speed follows bpm loosely; phase in 0..40 svg units.
    let phase = (s.ticks as f64 * (s.bpm / 60.0) * 1.6) % 40.0;
    let mut points = Vec::with_capacity(BEAT.len() * 4);
    for cycle in 0..4 {
        for (x, y) in BEAT {
            points.push(format!("{:.1},{:.1}", x + cycle as f64 * 40.0 - phase, y));
        }
    }
    let points = points.join(" ");
    rsx! {
        svg {
            class: "ecg",
            view_box: "0 0 120 40",
            preserve_aspect_ratio: "none",
            polyline {
                points: "{points}",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.6",
                stroke_linejoin: "round",
            }
        }
        span { class: "readout", {format!("{:>3.0} bpm", s.bpm)} }
    }
}

//! Deploy progress ring: SVG stroke-dashoffset driven by the live level.

use dioxus::prelude::*;

use crate::model::WidgetState;

/// r = 16 -> circumference 2*pi*16.
const CIRCUMFERENCE: f64 = 100.53;

#[component]
pub fn RingBody(state: Signal<WidgetState>) -> Element {
    let level = state().level;
    let offset = CIRCUMFERENCE * (1.0 - level);
    let phase = match (level * 3.0) as u32 {
        0 => "build",
        1 => "test",
        _ => "ship",
    };
    rsx! {
        svg { class: "ring", view_box: "0 0 40 40",
            circle {
                class: "ring-track",
                cx: "20", cy: "20", r: "16",
                fill: "none", stroke_width: "3",
            }
            circle {
                class: "ring-fill",
                cx: "20", cy: "20", r: "16",
                fill: "none", stroke: "currentColor", stroke_width: "3",
                stroke_linecap: "round",
                stroke_dasharray: "{CIRCUMFERENCE}",
                stroke_dashoffset: "{offset}",
                transform: "rotate(-90 20 20)",
            }
        }
        span { class: "readout", {format!("{phase} {:>3.0}%", level * 100.0)} }
    }
}

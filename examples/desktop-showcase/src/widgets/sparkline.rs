//! Streaming telemetry chart: an SVG polyline over the live sample window.

use dioxus::prelude::*;

use crate::model::WidgetState;

#[component]
pub fn SparklineBody(state: Signal<WidgetState>) -> Element {
    let s = state();
    let step = 120.0 / 59.0;
    let points = s
        .samples
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{:.1},{:.1}", i as f64 * step, 38.0 - v * 34.0))
        .collect::<Vec<_>>()
        .join(" ");
    let latest = s.samples.last().copied().unwrap_or(0.5);
    rsx! {
        svg {
            class: "spark",
            view_box: "0 0 120 40",
            preserve_aspect_ratio: "none",
            polyline {
                points: "{points}",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.6",
                stroke_linejoin: "round",
                stroke_linecap: "round",
            }
        }
        span { class: "readout", {format!("{:>3.0} mV", latest * 100.0)} }
    }
}

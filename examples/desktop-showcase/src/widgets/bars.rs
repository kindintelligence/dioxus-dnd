//! Recent channel load rendered as a compact live bar chart.

use dioxus::prelude::*;

use crate::model::WidgetState;

#[component]
pub fn BarsBody(state: Signal<WidgetState>) -> Element {
    let state = state();
    let samples = state.samples.iter().rev().take(8).rev().copied();
    let bars = samples
        .enumerate()
        .map(|(index, value)| {
            let height = (value * 32.0).max(1.0);
            (index, 4 + index * 15, 37.0 - height, height)
        })
        .collect::<Vec<_>>();
    let average = bars
        .iter()
        .map(|(_, _, _, height)| height / 32.0)
        .sum::<f64>()
        / bars.len().max(1) as f64;

    rsx! {
        svg {
            class: "bars",
            view_box: "0 0 120 40",
            preserve_aspect_ratio: "none",
            line { class: "chart-grid", x1: "0", y1: "37", x2: "120", y2: "37" }
            for (index, x, y, height) in bars {
                rect {
                    key: "{index}",
                    class: "chart-bar",
                    x: "{x}", y: "{y:.1}",
                    width: "10", height: "{height:.1}", rx: "1.5",
                }
            }
        }
        span { class: "readout", {format!("8 channels  {:>3.0}% avg", average * 100.0)} }
    }
}

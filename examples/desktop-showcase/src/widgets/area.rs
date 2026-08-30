//! A filled area chart over the most recent traffic samples.

use dioxus::prelude::*;

use crate::model::WidgetState;

#[component]
pub fn AreaBody(state: Signal<WidgetState>) -> Element {
    let state = state();
    let start = state.samples.len().saturating_sub(24);
    let samples = &state.samples[start..];
    let step = 120.0 / samples.len().saturating_sub(1).max(1) as f64;
    let line = samples
        .iter()
        .enumerate()
        .map(|(index, value)| format!("{:.1},{:.1}", index as f64 * step, 37.0 - value * 32.0))
        .collect::<Vec<_>>()
        .join(" ");
    let fill = format!("0,38 {line} 120,38");
    let (minimum, maximum) = samples
        .iter()
        .copied()
        .fold((1.0_f64, 0.0_f64), |(minimum, maximum), value| {
            (minimum.min(value), maximum.max(value))
        });

    rsx! {
        svg {
            class: "area",
            view_box: "0 0 120 40",
            preserve_aspect_ratio: "none",
            line { class: "chart-grid", x1: "0", y1: "12", x2: "120", y2: "12" }
            line { class: "chart-grid", x1: "0", y1: "25", x2: "120", y2: "25" }
            polygon { class: "area-fill", points: "{fill}" }
            polyline {
                class: "area-line",
                points: "{line}",
                stroke_linejoin: "round",
                stroke_linecap: "round",
            }
        }
        span {
            class: "readout",
            {format!("range {:>3.0}–{:>3.0}%", minimum * 100.0, maximum * 100.0)}
        }
    }
}

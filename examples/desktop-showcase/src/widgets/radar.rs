//! Six-axis systems-health radar derived from the live telemetry window.

use dioxus::prelude::*;

use crate::model::WidgetState;

const AXES: usize = 6;
const CENTER_X: f64 = 60.0;
const CENTER_Y: f64 = 20.0;
const RADIUS_X: f64 = 22.0;
const RADIUS_Y: f64 = 17.0;

fn point(index: usize, value: f64) -> (f64, f64) {
    let angle = -std::f64::consts::FRAC_PI_2 + index as f64 * std::f64::consts::TAU / AXES as f64;
    (
        CENTER_X + angle.cos() * RADIUS_X * value,
        CENTER_Y + angle.sin() * RADIUS_Y * value,
    )
}

fn polygon(values: &[f64]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let (x, y) = point(index, *value);
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[component]
pub fn RadarBody(state: Signal<WidgetState>) -> Element {
    let state = state();
    let values = state
        .samples
        .chunks(state.samples.len().div_ceil(AXES).max(1))
        .take(AXES)
        .map(|chunk| chunk.iter().sum::<f64>() / chunk.len() as f64)
        .collect::<Vec<_>>();
    let outer = polygon(&[1.0; AXES]);
    let inner = polygon(&[0.5; AXES]);
    let reading = polygon(&values);
    let spokes = (0..AXES).map(|index| point(index, 1.0));
    let average = values.iter().sum::<f64>() / values.len().max(1) as f64;

    rsx! {
        svg { class: "radar", view_box: "0 0 120 40",
            polygon { class: "chart-grid", points: "{outer}", fill: "none" }
            polygon { class: "chart-grid", points: "{inner}", fill: "none" }
            for (index, (x, y)) in spokes.enumerate() {
                line {
                    key: "{index}", class: "radar-spoke",
                    x1: "{CENTER_X}", y1: "{CENTER_Y}", x2: "{x:.1}", y2: "{y:.1}",
                }
            }
            polygon { class: "radar-fill", points: "{reading}" }
            polygon { class: "radar-line", points: "{reading}" }
        }
        span { class: "readout", {format!("coverage  {:>3.0}%", average * 100.0)} }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scale_polygon_has_one_point_per_axis() {
        assert_eq!(polygon(&[1.0; AXES]).split_whitespace().count(), AXES);
    }
}

//! Mission clock: mm:ss.t derived from the widget's 50ms tick count.

use dioxus::prelude::*;

use crate::model::WidgetState;

#[component]
pub fn StopwatchBody(state: Signal<WidgetState>) -> Element {
    let ms = state().ticks * 50;
    let minutes = ms / 60_000;
    let seconds = (ms / 1_000) % 60;
    let tenths = (ms % 1_000) / 100;
    rsx! {
        span { class: "clock", {format!("{minutes:02}:{seconds:02}.{tenths}")} }
        span { class: "readout", "T+ elapsed" }
    }
}

//! The widget cards: shared chrome + one body module per kind.

mod pulse;
mod ring;
mod sparkline;
mod stopwatch;

use dioxus::prelude::*;

use crate::model::{Widget, WidgetKind};

/// One live widget card. The SAME component renders in the dock, in a
/// satellite, and inside the drag ghost - it reads the widget's live signal,
/// so it keeps animating wherever it is presented, mid-drag included.
#[component]
pub fn WidgetCard(widget: Widget) -> Element {
    rsx! {
        div { class: "widget", "data-kind": widget.kind.name(),
            header { class: "widget-head",
                span { class: "widget-dot" }
                span { class: "widget-title", {widget.kind.title()} }
            }
            div { class: "widget-body",
                match widget.kind {
                    WidgetKind::Sparkline => rsx! { sparkline::SparklineBody { state: widget.state } },
                    WidgetKind::Stopwatch => rsx! { stopwatch::StopwatchBody { state: widget.state } },
                    WidgetKind::Ring => rsx! { ring::RingBody { state: widget.state } },
                    WidgetKind::Pulse => rsx! { pulse::PulseBody { state: widget.state } },
                }
            }
        }
    }
}

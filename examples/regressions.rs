//! Headless regression fixtures for the browser suite. This is not a showcase:
//! each block reproduces one fixed bug in the smallest form a real browser can
//! exercise, with stable DOM hooks (`id`s / `data-*`) the Playwright specs
//! assert against. See `tests/browser/web-pointer-regressions.spec.js`.
//!
//! ```sh
//! dx serve --example regressions --platform web --features web
//! ```

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        h1 { "Regressions" }
        OverlapReject {}
    }
}

// --- #5: a pointer drop over a rejecting zone falls through to an accepting
// zone stacked underneath, instead of cancelling. -----------------------------

#[component]
fn OverlapReject() -> Element {
    // "none" until a drop lands; "accept" if it reached the underlying zone,
    // "reject" if it (wrongly) hit the top one.
    let mut landed = use_signal(|| "none".to_string());
    rsx! {
        h2 { "Overlap reject" }
        DndProvider::<u32> {
            Draggable::<u32> {
                payload: 1u32,
                label: "card",
                style: "display:block; width:140px; padding:10px; border:1px solid #333; \
                        background:#fff; cursor:grab; user-select:none;",
                "drag me"
            }
            // Two zones at the same rect. The accepting zone registers first
            // (so it sits *under* in hit-test order); the rejecting zone is
            // registered second and is therefore the geometric topmost.
            div {
                id: "overlap-stack",
                style: "position:relative; width:220px; height:120px; margin-top:20px;",
                DropZone::<u32> {
                    id: ZoneId(1001),
                    accepts: move |_p: u32| true,
                    on_drop: move |_o: DropOutcome<u32>| landed.set("accept".to_string()),
                    style: "position:absolute; inset:0; background:#dff0d8; \
                            display:flex; align-items:center; justify-content:center;",
                    "accept (under)"
                }
                DropZone::<u32> {
                    id: ZoneId(1002),
                    accepts: move |_p: u32| false,
                    on_drop: move |_o: DropOutcome<u32>| landed.set("reject".to_string()),
                    style: "position:absolute; inset:0; background:rgba(240,90,90,0.35); \
                            display:flex; align-items:center; justify-content:center;",
                    "reject (over)"
                }
            }
            div { id: "overlap-status", "data-landed": "{landed}", "landed: {landed}" }
        }
    }
}

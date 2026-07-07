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
        CanvasNativeChild {}
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
            PointerDraggable::<u32> {
                payload: 1u32,
                input: DragInputMode::Pointer,
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

// --- #2: a native drop landing on a child node inside a canvas reports
// canvas-relative coordinates, not coordinates relative to the child. ----------

#[derive(Clone, PartialEq)]
struct CanvasNode {
    id: u32,
    label: String,
}

#[component]
fn CanvasNativeChild() -> Element {
    let mut pointer = use_signal(|| None::<(f64, f64)>);
    rsx! {
        h2 { "Canvas native child" }
        DndProvider::<CanvasNode> {
            // Native HTML5 drag source (core `Draggable` defaults to native).
            Draggable::<CanvasNode> {
                payload: CanvasNode { id: 1, label: "new".into() },
                style: "display:block; width:120px; padding:10px; border:1px solid #333; \
                        background:#fff; user-select:none;",
                "native source"
            }
            CanvasDropZone::<CanvasNode> {
                id: ZoneId(2001),
                bounds: Bounds { width: 640.0, height: 220.0 },
                on_drop: move |d: CanvasDrop<CanvasNode>| pointer.set(Some((d.pointer.x, d.pointer.y))),
                style: "position:relative; width:640px; height:220px; margin-top:20px; \
                        border:1px solid #333;",
                // An existing child positioned well away from the canvas origin.
                // A drop lands *on* this child, so element-relative coordinates
                // would report the child offset, not the canvas position.
                div {
                    id: "canvas-child",
                    style: "position:absolute; left:200px; top:120px; width:80px; height:30px; \
                            background:#cdd9ff; display:flex; align-items:center; justify-content:center;",
                    "child"
                }
            }
            div {
                id: "canvas-drop-pointer",
                "data-set": if pointer().is_some() { "true" } else { "false" },
                "data-x": pointer().map(|p| format!("{:.2}", p.0)).unwrap_or_default(),
                "data-y": pointer().map(|p| format!("{:.2}", p.1)).unwrap_or_default(),
            }
        }
    }
}

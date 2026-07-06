//! Focused headless canvas example.
//!
//! Shows the intended split:
//! - `CanvasDropZone` reports where the payload landed.
//! - `PointerDraggable` moves existing app nodes and palette items.
//! - `core::modifiers` applies app-owned placement rules such as snap and
//!   item-aware bounds.
//!
//! Run:
//! ```sh
//! dx serve --example canvas --platform web --features web
//! ```

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

const CANVAS: ZoneId = ZoneId(42);
const CANVAS_W: f64 = 720.0;
const CANVAS_H: f64 = 420.0;
const GRID: f64 = 24.0;

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, Copy, PartialEq)]
enum NodeKind {
    Source,
    Transform,
    Output,
}

impl NodeKind {
    fn label(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Transform => "Transform",
            Self::Output => "Output",
        }
    }

    fn tone(self) -> &'static str {
        match self {
            Self::Source => "border-emerald-300 bg-emerald-50 text-emerald-950",
            Self::Transform => "border-blue-300 bg-blue-50 text-blue-950",
            Self::Output => "border-rose-300 bg-rose-50 text-rose-950",
        }
    }
}

#[derive(Clone, PartialEq)]
struct Node {
    id: u32,
    kind: NodeKind,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, PartialEq)]
enum NodeDrag {
    Existing(u32),
    New(NodeKind),
}

#[component]
fn App() -> Element {
    let mut next_id = use_signal(|| 4_u32);
    let mut nodes = use_signal(|| {
        vec![
            Node {
                id: 1,
                kind: NodeKind::Source,
                x: 48.0,
                y: 48.0,
                width: 144.0,
                height: 72.0,
            },
            Node {
                id: 2,
                kind: NodeKind::Transform,
                x: 288.0,
                y: 168.0,
                width: 168.0,
                height: 78.0,
            },
            Node {
                id: 3,
                kind: NodeKind::Output,
                x: 528.0,
                y: 312.0,
                width: 132.0,
                height: 66.0,
            },
        ]
    });
    let mut last_drop = use_signal(|| "Drag a node or palette item onto the canvas.".to_string());

    let mut place = move |drop: CanvasDrop<NodeDrag>| {
        let mut all = nodes.write();
        match drop.payload {
            NodeDrag::Existing(id) => {
                if let Some(node) = all.iter_mut().find(|node| node.id == id) {
                    let p = constrained(drop.position, node.width, node.height);
                    node.x = p.x;
                    node.y = p.y;
                    last_drop.set(format!(
                        "Moved {} to ({:.0}, {:.0})",
                        node.kind.label(),
                        p.x,
                        p.y
                    ));
                }
            }
            NodeDrag::New(kind) => {
                let (width, height) = default_size(kind);
                let p = constrained(drop.position, width, height);
                let id = next_id();
                next_id.set(id + 1);
                all.push(Node {
                    id,
                    kind,
                    x: p.x,
                    y: p.y,
                    width,
                    height,
                });
                last_drop.set(format!(
                    "Created {} at ({:.0}, {:.0})",
                    kind.label(),
                    p.x,
                    p.y
                ));
            }
        }
    };

    rsx! {
        document::Script { src: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" }
        div { class: "min-h-screen bg-slate-100 text-slate-950 antialiased",
            div { class: "mx-auto max-w-6xl px-6 py-10",
                header { class: "mb-6 flex items-end justify-between gap-6",
                    div {
                        p { class: "text-sm font-medium text-slate-500", "dioxus-dnd" }
                        h1 { class: "text-3xl font-semibold tracking-tight", "Canvas editor" }
                    }
                    p { class: "max-w-md text-right text-sm text-slate-600",
                        "Headless canvas drops with pointer drags, exact grab offsets, snap grid and item-aware bounds."
                    }
                }
                DndProvider::<NodeDrag> {
                    LiveRegion::<NodeDrag> {}
                    div { class: "grid gap-4 lg:grid-cols-[220px_1fr]",
                        aside { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                            h2 { class: "mb-3 text-sm font-semibold text-slate-700", "Palette" }
                            div { class: "space-y-2",
                                for kind in [NodeKind::Source, NodeKind::Transform, NodeKind::Output] {
                                    PointerDraggable::<NodeDrag> {
                                        payload: NodeDrag::New(kind),
                                        label: format!("New {}", kind.label()),
                                        class: format!(
                                            "cursor-grab select-none rounded-md border px-3 py-2 text-sm shadow-sm transition data-dragging:opacity-50 {}",
                                            kind.tone()
                                        ),
                                        "{kind.label()}"
                                    }
                                }
                            }
                            p { class: "mt-4 text-xs leading-5 text-slate-500",
                                "New and existing nodes use the same `CanvasDropZone` drop path."
                            }
                        }

                        section { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                            div { class: "mb-3 flex items-center justify-between gap-3",
                                div {
                                    h2 { class: "text-sm font-semibold text-slate-700", "Workbench" }
                                    p { class: "text-xs text-slate-500", "{last_drop}" }
                                }
                                p { class: "text-xs tabular-nums text-slate-500", "{GRID:.0}px grid" }
                            }
                            CanvasDropZone::<NodeDrag> {
                                id: CANVAS,
                                label: "Workbench",
                                on_drop: move |drop| place(drop),
                                class: "relative overflow-hidden rounded-md border border-slate-300 bg-[radial-gradient(#cbd5e1_1px,transparent_1px)] [background-size:24px_24px] data-active:border-slate-900",
                                style: format!("width: {CANVAS_W}px; height: {CANVAS_H}px; max-width: 100%;"),
                                for node in nodes.read().clone() {
                                    CanvasNode { node }
                                }
                            }
                        }
                    }
                    DragOverlay::<NodeDrag> {
                        class: "pointer-events-none",
                        NodeGhost {}
                    }
                }
            }
        }
    }
}

#[component]
fn CanvasNode(node: Node) -> Element {
    rsx! {
        PointerDraggable::<NodeDrag> {
            payload: NodeDrag::Existing(node.id),
            zone: CANVAS,
            label: node.kind.label(),
            style: format!(
                "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px;",
                node.x,
                node.y,
                node.width,
                node.height
            ),
            class: format!(
                "cursor-grab select-none rounded-md border px-3 py-2 text-sm shadow-sm transition data-dragging:opacity-40 {}",
                node.kind.tone()
            ),
            div { class: "font-medium", "{node.kind.label()}" }
            div { class: "mt-1 text-xs tabular-nums opacity-70", "x {node.x:.0}, y {node.y:.0}" }
        }
    }
}

#[component]
fn NodeGhost() -> Element {
    let dnd = use_dnd::<NodeDrag>();
    let label = match dnd.payload() {
        Some(NodeDrag::Existing(_)) => "Move node",
        Some(NodeDrag::New(kind)) => kind.label(),
        None => "",
    };
    rsx! {
        div { class: "rounded-md border border-slate-400 bg-white px-3 py-2 text-sm shadow-xl", "{label}" }
    }
}

fn default_size(kind: NodeKind) -> (f64, f64) {
    match kind {
        NodeKind::Source => (144.0, 72.0),
        NodeKind::Transform => (168.0, 78.0),
        NodeKind::Output => (132.0, 66.0),
    }
}

fn constrained(position: Point, width: f64, height: f64) -> Point {
    let chain = [
        DragModifier::Snap { x: GRID, y: GRID },
        DragModifier::KeepInside,
    ];
    let ctx = ModifierCtx {
        container: Some(Rect::new(0.0, 0.0, CANVAS_W, CANVAS_H)),
        element: Some(Rect::new(position.x, position.y, width, height)),
    };
    apply_modifiers(&chain, position, &ctx)
}

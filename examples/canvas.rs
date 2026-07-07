//! Focused headless canvas example.
//!
//! Shows the intended split:
//! - `CanvasDropZone` reports where the payload landed.
//! - `PointerDraggable` moves existing app nodes and palette items.
//! - `Bounds::clamp_item` applies app-owned item-aware bounds.
//! - Connection handles and edges are app state layered over the headless
//!   canvas primitive.
//!
//! Run:
//! ```sh
//! dx serve --example canvas --platform web --features web
//! ```

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

const CANVAS: ZoneId = ZoneId(42);
const CANVAS_W: f64 = 960.0;
const CANVAS_H: f64 = 560.0;
const GRID: f64 = 24.0;
const MIN_ZOOM: f64 = 0.6;
const MAX_ZOOM: f64 = 1.8;
const ZOOM_STEP: f64 = 0.2;
const PAN_STEP: f64 = 48.0;
const KEYBOARD_FIXED: Point = Point { x: 744.0, y: 408.0 };

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
            Self::Source => "Bad",
            Self::Transform => "Find Comparable Products",
            Self::Output => "Publish Results",
        }
    }

    fn category(self) -> &'static str {
        match self {
            Self::Source => "Condition",
            Self::Transform => "Database",
            Self::Output => "Action",
        }
    }

    fn palette_tone(self) -> &'static str {
        match self {
            Self::Source => "border-pink-400/40 bg-pink-400/10 text-pink-100",
            Self::Transform => "border-sky-400/40 bg-sky-400/10 text-sky-100",
            Self::Output => "border-emerald-400/40 bg-emerald-400/10 text-emerald-100",
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
struct Edge {
    id: u32,
    from: u32,
    to: u32,
}

#[derive(Clone, PartialEq)]
struct BoundaryItem {
    id: u32,
    label: String,
    x: f64,
    y: f64,
}

#[derive(Clone, PartialEq)]
enum NodeDrag {
    Existing(u32),
    New(NodeKind),
}

#[component]
fn App() -> Element {
    let mut next_id = use_signal(|| 4_u32);
    let mut next_edge_id = use_signal(|| 3_u32);
    let mut next_boundary_id = use_signal(|| 1_u32);
    let mut connecting_from = use_signal(|| None::<u32>);
    let mut boundary_items = use_signal(Vec::<BoundaryItem>::new);
    let mut nodes = use_signal(|| {
        vec![
            Node {
                id: 1,
                kind: NodeKind::Source,
                x: 64.0,
                y: 176.0,
                width: 160.0,
                height: 128.0,
            },
            Node {
                id: 2,
                kind: NodeKind::Transform,
                x: 384.0,
                y: 176.0,
                width: 190.0,
                height: 128.0,
            },
            Node {
                id: 3,
                kind: NodeKind::Output,
                x: 720.0,
                y: 176.0,
                width: 150.0,
                height: 128.0,
            },
        ]
    });
    let mut edges = use_signal(|| {
        vec![
            Edge {
                id: 1,
                from: 1,
                to: 2,
            },
            Edge {
                id: 2,
                from: 2,
                to: 3,
            },
        ]
    });
    let mut last_drop = use_signal(|| "Drag a node or palette item onto the canvas.".to_string());
    let mut viewport = use_signal(CanvasViewport::default);
    let mut keyboard_policy = use_signal(CanvasKeyboardPlacement::default);

    let mut place = move |drop: CanvasDrop<NodeDrag>| {
        let view = viewport();
        let world_position = SnapGrid(GRID).snap(screen_to_world(drop.position, view));
        let mut all = nodes.write();
        match drop.payload {
            NodeDrag::Existing(id) => {
                if let Some(ix) = all.iter().position(|node| node.id == id) {
                    let kind = all[ix].kind;
                    let p = constrained(world_position, all[ix].width, all[ix].height);
                    all[ix].x = p.x;
                    all[ix].y = p.y;
                    let moved = all.remove(ix);
                    all.push(moved);
                    last_drop.set(format!(
                        "Moved {} to ({:.0}, {:.0})",
                        kind.label(),
                        p.x,
                        p.y
                    ));
                }
            }
            NodeDrag::New(kind) => {
                let (width, height) = default_size(kind);
                let p = constrained(world_position, width, height);
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
    let mut start_connection = move |from: u32| {
        connecting_from.set(Some(from));
        if let Some(node) = nodes.peek().iter().find(|node| node.id == from) {
            last_drop.set(format!("Connect {} to another node.", node.kind.label()));
        }
    };
    let mut finish_connection = move |to: u32| {
        let Some(from) = connecting_from() else {
            last_drop.set("Pick an output handle first.".to_string());
            return;
        };
        connecting_from.set(None);
        if from == to {
            last_drop.set("A node cannot connect to itself.".to_string());
            return;
        }
        if edges
            .peek()
            .iter()
            .any(|edge| edge.from == from && edge.to == to)
        {
            last_drop.set("That connection already exists.".to_string());
            return;
        }
        let id = next_edge_id();
        next_edge_id.set(id + 1);
        edges.write().push(Edge { id, from, to });
        last_drop.set(format!("Connected node {from} to node {to}."));
    };
    let zoom = viewport().zoom;
    let pan = viewport().pan;
    let keyboard = keyboard_policy();
    let keyboard_preview = keyboard_preview_point(keyboard);
    let mut set_zoom = move |next: f64| {
        let view = viewport();
        viewport.set(CanvasViewport::new(view.pan, next).clamped_zoom(MIN_ZOOM, MAX_ZOOM));
    };
    let mut pan_by = move |delta: Point| {
        let view = viewport();
        viewport.set(CanvasViewport::new(view.pan + delta, view.zoom));
    };
    let reset_view = move |_| viewport.set(CanvasViewport::default());
    let mut receive_boundary = move |drop: ExternalDrop| {
        let label = boundary_label(&drop);
        let point = drop.element;
        let id = next_boundary_id();
        next_boundary_id.set(id + 1);
        boundary_items.write().push(BoundaryItem {
            id,
            label: label.clone(),
            x: point.x,
            y: point.y,
        });
        last_drop.set(format!(
            "Native drop: {label} at ({:.0}, {:.0})",
            point.x, point.y
        ));
    };

    rsx! {
        document::Script { src: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" }
        div { class: "min-h-screen bg-neutral-950 text-neutral-50 antialiased",
            div { class: "mx-auto max-w-7xl px-5 py-6",
                header { class: "mb-5 flex items-center justify-between gap-6 border-b border-white/10 pb-5",
                    div {
                        p { class: "text-xs font-medium uppercase text-neutral-500", "dioxus-dnd" }
                        h1 { class: "text-2xl font-semibold tracking-tight", "Workflow canvas" }
                    }
                    div { class: "flex items-center gap-2 text-xs text-neutral-400",
                        span { class: "rounded-full border border-emerald-400/40 bg-emerald-400/10 px-2.5 py-1 text-emerald-100", "Live" }
                        span { class: "rounded-full border border-white/10 px-2.5 py-1", "{GRID:.0}px grid" }
                    }
                }
                DndProvider::<NodeDrag> {
                    LiveRegion::<NodeDrag> {}
                    div { class: "grid min-h-[680px] gap-4 xl:grid-cols-[280px_minmax(0,1fr)]",
                        aside { class: "space-y-4 xl:sticky xl:top-4 xl:self-start",
                            section { class: "rounded-lg border border-white/10 bg-neutral-900 p-4 shadow-2xl shadow-black/30",
                                h2 { class: "mb-3 text-sm font-semibold text-neutral-200", "Blocks" }
                                div { class: "space-y-2",
                                    for kind in [NodeKind::Source, NodeKind::Transform, NodeKind::Output] {
                                        PointerDraggable::<NodeDrag> {
                                            payload: NodeDrag::New(kind),
                                            label: format!("New {}", kind.label()),
                                            class: format!(
                                                "cursor-grab select-none rounded-md border px-3 py-3 text-sm transition hover:border-white/30 data-dragging:opacity-50 {}",
                                                kind.palette_tone()
                                            ),
                                            "{kind.label()}"
                                        }
                                    }
                                }
                            }

                            section { class: "rounded-lg border border-white/10 bg-neutral-900 p-4 shadow-2xl shadow-black/30",
                                h2 { class: "mb-3 text-sm font-semibold text-neutral-200", "View" }
                                div { class: "grid grid-cols-4 gap-1 text-xs",
                                    button {
                                        class: "rounded border border-white/10 px-2 py-1.5 text-neutral-300 hover:bg-white/10",
                                        aria_label: "Pan canvas left",
                                        onclick: move |_| pan_by(Point::new(PAN_STEP, 0.0)),
                                        "←"
                                    }
                                    button {
                                        class: "rounded border border-white/10 px-2 py-1.5 text-neutral-300 hover:bg-white/10",
                                        aria_label: "Pan canvas up",
                                        onclick: move |_| pan_by(Point::new(0.0, PAN_STEP)),
                                        "↑"
                                    }
                                    button {
                                        class: "rounded border border-white/10 px-2 py-1.5 text-neutral-300 hover:bg-white/10",
                                        aria_label: "Pan canvas down",
                                        onclick: move |_| pan_by(Point::new(0.0, -PAN_STEP)),
                                        "↓"
                                    }
                                    button {
                                        class: "rounded border border-white/10 px-2 py-1.5 text-neutral-300 hover:bg-white/10",
                                        aria_label: "Pan canvas right",
                                        onclick: move |_| pan_by(Point::new(-PAN_STEP, 0.0)),
                                        "→"
                                    }
                                    button {
                                        class: "rounded border border-white/10 px-2 py-1.5 text-neutral-300 hover:bg-white/10",
                                        aria_label: "Zoom canvas out",
                                        onclick: move |_| set_zoom(zoom - ZOOM_STEP),
                                        "−"
                                    }
                                    span { class: "col-span-2 rounded border border-white/10 px-2 py-1.5 text-center text-neutral-400", "{zoom * 100.0:.0}%" }
                                    button {
                                        class: "rounded border border-white/10 px-2 py-1.5 text-neutral-300 hover:bg-white/10",
                                        aria_label: "Zoom canvas in",
                                        onclick: move |_| set_zoom(zoom + ZOOM_STEP),
                                        "+"
                                    }
                                    button {
                                        class: "col-span-4 rounded bg-white px-2 py-1.5 font-medium text-neutral-950",
                                        aria_label: "Reset canvas view",
                                        onclick: reset_view,
                                        "Reset"
                                    }
                                }
                            }

                            section { class: "rounded-lg border border-white/10 bg-neutral-900 p-4 shadow-2xl shadow-black/30",
                                h2 { class: "mb-3 text-sm font-semibold text-neutral-200", "Keyboard" }
                                div {
                                    class: "grid grid-cols-3 gap-1 text-xs",
                                    role: "group",
                                    aria_label: "Keyboard placement",
                                    for (policy, label) in [
                                        (CanvasKeyboardPlacement::Center, "Center"),
                                        (CanvasKeyboardPlacement::Origin, "Origin"),
                                        (CanvasKeyboardPlacement::Fixed(KEYBOARD_FIXED), "Fixed"),
                                    ] {
                                        button {
                                            class: keyboard_policy_class(keyboard == policy),
                                            aria_label: format!("Keyboard placement {label}"),
                                            aria_pressed: if keyboard == policy { "true" } else { "false" },
                                            onclick: move |_| keyboard_policy.set(policy),
                                            "{label}"
                                        }
                                    }
                                }
                            }

                            section { class: "rounded-lg border border-white/10 bg-neutral-900 p-4 shadow-2xl shadow-black/30",
                                h2 { class: "mb-3 text-sm font-semibold text-neutral-200", "Inspector" }
                                dl { class: "space-y-3 text-xs",
                                    div {
                                        dt { class: "text-neutral-500", "Nodes" }
                                        dd { class: "mt-1 text-lg font-semibold text-neutral-100", "{nodes.read().len()}" }
                                    }
                                    div {
                                        dt { class: "text-neutral-500", "Connections" }
                                        dd { class: "mt-1 text-lg font-semibold text-neutral-100", "{edges.read().len()}" }
                                    }
                                    div {
                                        dt { class: "text-neutral-500", "View" }
                                        dd { class: "mt-1 leading-5 text-neutral-300",
                                            "{zoom * 100.0:.0}% at ({pan.x:.0}, {pan.y:.0})"
                                        }
                                    }
                                }
                            }
                        }

                        main { class: "min-w-0 space-y-4",
                            section { class: "overflow-x-auto rounded-lg border border-white/10 bg-neutral-900 p-4 shadow-2xl shadow-black/30",
                                div { class: "mb-3 flex items-center justify-between gap-3 px-1",
                                div {
                                    h2 { class: "text-sm font-semibold text-neutral-200", "Builder" }
                                    p { class: "text-xs text-neutral-500", "{last_drop}" }
                                }
                            }
                            CanvasDropZone::<NodeDrag> {
                                id: CANVAS,
                                label: "Workbench",
                                keyboard,
                                on_drop: move |drop| place(drop),
                                class: "relative overflow-hidden rounded-md border border-white/10 bg-[#080808] bg-[radial-gradient(rgba(255,255,255,0.15)_1px,transparent_1px)] shadow-inner shadow-black data-active:border-white/40",
                                style: format!(
                                    "width: {CANVAS_W}px; height: {CANVAS_H}px; background-size: {}px {}px; background-position: {}px {}px;",
                                    GRID * zoom,
                                    GRID * zoom,
                                    pan.x,
                                    pan.y
                                ),
                                svg {
                                    class: "pointer-events-none absolute inset-0 z-0 h-full w-full",
                                    view_box: format!("0 0 {CANVAS_W} {CANVAS_H}"),
                                    g {
                                        transform: format!("translate({} {}) scale({})", pan.x, pan.y, zoom),
                                        for edge in edges.read().clone() {
                                            EdgePath { edge, nodes: nodes.read().clone() }
                                        }
                                    }
                                }
                                div {
                                    class: "pointer-events-none absolute z-30",
                                    "data-keyboard-placement-preview": "true",
                                    style: format!("left: {}px; top: {}px;", keyboard_preview.x, keyboard_preview.y),
                                    div { class: "h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-amber-200 bg-amber-300/25 shadow-[0_0_0_4px_rgba(251,191,36,0.12)]" }
                                    div { class: "absolute left-3 top-3 rounded bg-amber-200 px-1.5 py-0.5 text-[10px] font-semibold text-neutral-950 shadow-lg shadow-black/40",
                                        "{keyboard_policy_label(keyboard)}"
                                    }
                                }
                                for node in nodes.read().clone() {
                                    CanvasNode {
                                        node,
                                        viewport: viewport(),
                                        connecting: connecting_from(),
                                        on_start_connection: move |id| start_connection(id),
                                        on_finish_connection: move |id| finish_connection(id),
                                    }
                                }
                            }
                        }
                        section { class: "rounded-lg border border-white/10 bg-neutral-900 p-4 shadow-2xl shadow-black/30",
                            div { class: "mb-3 flex items-center justify-between gap-3",
                                div {
                                    h2 { class: "text-sm font-semibold text-neutral-200", "Native boundary" }
                                    p { class: "text-xs text-neutral-500",
                                        "This separate surface uses browser DataTransfer for files, links and text from outside the app."
                                    }
                                }
                                span { class: "rounded-full border border-white/10 px-2.5 py-1 text-xs text-neutral-400",
                                    "{boundary_items.read().len()} received"
                                }
                            }
                            ExternalDropZone {
                                on_drop: move |drop| receive_boundary(drop),
                                class: "relative min-h-40 overflow-hidden rounded-md border border-dashed border-white/15 bg-[#080808] bg-[radial-gradient(rgba(255,255,255,0.12)_1px,transparent_1px)] [background-size:24px_24px] p-4 data-over:border-emerald-300/70",
                                if boundary_items.read().is_empty() {
                                    div { class: "flex h-32 items-center justify-center text-center text-sm text-neutral-500",
                                        "Drop a file, link or text selection here"
                                    }
                                }
                                for item in boundary_items.read().clone() {
                                    div {
                                        key: "{item.id}",
                                        class: "absolute max-w-56 -translate-x-1/2 -translate-y-1/2 rounded-md border border-emerald-300/30 bg-emerald-300/10 px-3 py-2 text-xs text-emerald-50 shadow-lg shadow-black/30",
                                        style: format!(
                                            "left: {}px; top: {}px;",
                                            item.x.clamp(24.0, CANVAS_W - 24.0),
                                            item.y.clamp(24.0, 136.0)
                                        ),
                                        "{item.label}"
                                    }
                                }
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
fn CanvasNode(
    node: Node,
    viewport: CanvasViewport,
    connecting: Option<u32>,
    on_start_connection: EventHandler<u32>,
    on_finish_connection: EventHandler<u32>,
) -> Element {
    let armed = connecting == Some(node.id);
    let screen = world_to_screen(Point::new(node.x, node.y), viewport);
    let size = world_delta_to_screen(Point::new(node.width, node.height), viewport);
    rsx! {
        PointerDraggable::<NodeDrag> {
            payload: NodeDrag::Existing(node.id),
            zone: CANVAS,
            label: node.kind.label(),
            style: format!(
                "position: absolute; z-index: 10; left: {}px; top: {}px; width: {}px; height: {}px;",
                screen.x,
                screen.y,
                size.x,
                size.y
            ),
            "data-world-x": format!("{:.3}", node.x),
            "data-world-y": format!("{:.3}", node.y),
            "data-world-width": format!("{:.3}", node.width),
            "data-world-height": format!("{:.3}", node.height),
            class: "cursor-grab select-none rounded-lg border border-neutral-800 bg-black px-4 py-4 text-center shadow-2xl shadow-black/40 transition hover:border-neutral-700 data-dragging:opacity-40",
            button {
                class: if connecting.is_some() {
                    "absolute -left-2 top-1/2 z-20 h-4 w-4 -translate-y-1/2 rounded-full border border-neutral-700 bg-white shadow-[0_0_0_3px_rgba(0,0,0,0.8)] hover:scale-110 focus-visible:outline-2 focus-visible:outline-white"
                } else {
                    "absolute -left-2 top-1/2 z-20 h-4 w-4 -translate-y-1/2 rounded-full border border-neutral-700 bg-white shadow-[0_0_0_3px_rgba(0,0,0,0.8)] hover:scale-110 focus-visible:outline-2 focus-visible:outline-white"
                },
                aria_label: format!("Connect into {}", node.kind.label()),
                onpointerdown: move |evt| evt.stop_propagation(),
                onclick: move |evt| {
                    evt.stop_propagation();
                    on_finish_connection.call(node.id);
                },
            }
            div { class: "flex h-full flex-col items-center justify-center gap-2.5",
                NodeIcon { kind: node.kind }
                div {
                    div { class: "mx-auto max-w-[150px] text-balance text-sm font-semibold leading-5 text-white", "{node.kind.label()}" }
                    div { class: "mt-1 text-xs font-medium text-neutral-500", "{node.kind.category()}" }
                }
            }
            button {
                class: if armed {
                    "absolute -right-2 top-1/2 z-20 h-4 w-4 -translate-y-1/2 rounded-full border border-neutral-700 bg-white shadow-[0_0_0_3px_rgba(0,0,0,0.8)] ring-4 ring-white/20 hover:scale-110 focus-visible:outline-2 focus-visible:outline-white"
                } else {
                    "absolute -right-2 top-1/2 z-20 h-4 w-4 -translate-y-1/2 rounded-full border border-neutral-700 bg-white shadow-[0_0_0_3px_rgba(0,0,0,0.8)] hover:scale-110 focus-visible:outline-2 focus-visible:outline-white"
                },
                aria_label: format!("Connect from {}", node.kind.label()),
                onpointerdown: move |evt| evt.stop_propagation(),
                onclick: move |evt| {
                    evt.stop_propagation();
                    on_start_connection.call(node.id);
                },
            }
        }
    }
}

#[component]
fn NodeIcon(kind: NodeKind) -> Element {
    match kind {
        NodeKind::Source => rsx! {
            svg {
                class: "h-9 w-9 text-pink-300",
                view_box: "0 0 64 64",
                fill: "none",
                path {
                    d: "M18 12v28a8 8 0 1 0 8 8h11a8 8 0 1 0 8-8h-1V24",
                    stroke: "currentColor",
                    stroke_width: "4",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                }
                circle { cx: "18", cy: "48", r: "7", stroke: "currentColor", stroke_width: "4" }
                circle { cx: "45", cy: "24", r: "7", stroke: "currentColor", stroke_width: "4" }
            }
        },
        NodeKind::Transform => rsx! {
            svg {
                class: "h-9 w-9 text-sky-300",
                view_box: "0 0 64 64",
                fill: "none",
                ellipse { cx: "32", cy: "16", rx: "20", ry: "8", stroke: "currentColor", stroke_width: "4" }
                path {
                    d: "M12 16v30c0 4.5 9 8 20 8s20-3.5 20-8V16",
                    stroke: "currentColor",
                    stroke_width: "4",
                    stroke_linejoin: "round",
                }
                path {
                    d: "M12 31c0 4.5 9 8 20 8s20-3.5 20-8",
                    stroke: "currentColor",
                    stroke_width: "4",
                    stroke_linecap: "round",
                }
            }
        },
        NodeKind::Output => rsx! {
            svg {
                class: "h-9 w-9 text-emerald-300",
                view_box: "0 0 64 64",
                fill: "none",
                path {
                    d: "M16 34l10 10 22-24",
                    stroke: "currentColor",
                    stroke_width: "5",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                }
                circle { cx: "32", cy: "32", r: "24", stroke: "currentColor", stroke_width: "4" }
            }
        },
    }
}

#[component]
fn EdgePath(edge: Edge, nodes: Vec<Node>) -> Element {
    let Some(from) = nodes.iter().find(|node| node.id == edge.from) else {
        return rsx! {};
    };
    let Some(to) = nodes.iter().find(|node| node.id == edge.to) else {
        return rsx! {};
    };
    let x1 = from.x + from.width;
    let y1 = from.y + from.height / 2.0;
    let x2 = to.x;
    let y2 = to.y + to.height / 2.0;
    let dx = ((x2 - x1).abs() * 0.5).clamp(36.0, 80.0);
    let d = format!(
        "M {x1:.1} {y1:.1} C {:.1} {y1:.1}, {:.1} {y2:.1}, {x2:.1} {y2:.1}",
        x1 + dx,
        x2 - dx
    );
    rsx! {
        path {
            d,
            fill: "none",
            stroke: "#52525b",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_dasharray: "4 8",
            opacity: "0.9",
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
        div { class: "rounded-md border border-white/20 bg-neutral-900 px-3 py-2 text-sm text-neutral-50 shadow-2xl shadow-black/40", "{label}" }
    }
}

fn default_size(kind: NodeKind) -> (f64, f64) {
    match kind {
        NodeKind::Source => (160.0, 128.0),
        NodeKind::Transform => (190.0, 128.0),
        NodeKind::Output => (150.0, 128.0),
    }
}

fn constrained(position: Point, width: f64, height: f64) -> Point {
    Bounds {
        width: CANVAS_W,
        height: CANVAS_H,
    }
    .clamp_item(position, width, height)
}

fn keyboard_policy_class(active: bool) -> &'static str {
    if active {
        "rounded bg-white px-2 py-1 font-medium text-neutral-950"
    } else {
        "rounded px-2 py-1 text-neutral-300 hover:bg-white/10"
    }
}

fn keyboard_preview_point(policy: CanvasKeyboardPlacement) -> Point {
    match policy {
        CanvasKeyboardPlacement::Center => Point::new(CANVAS_W / 2.0, CANVAS_H / 2.0),
        CanvasKeyboardPlacement::Origin => Point::default(),
        CanvasKeyboardPlacement::Fixed(point) => point,
    }
}

fn keyboard_policy_label(policy: CanvasKeyboardPlacement) -> &'static str {
    match policy {
        CanvasKeyboardPlacement::Center => "Center",
        CanvasKeyboardPlacement::Origin => "Origin",
        CanvasKeyboardPlacement::Fixed(_) => "Fixed",
    }
}

fn boundary_label(drop: &ExternalDrop) -> String {
    if let Some(file) = drop.files.first() {
        if drop.files.len() == 1 {
            return format!("File {}", compact(&file.name(), 42));
        }
        return format!(
            "{} files, first {}",
            drop.files.len(),
            compact(&file.name(), 32)
        );
    }
    if let Some(url) = drop.url() {
        return format!("Link {}", compact(url, 42));
    }
    if let Some(text) = drop.text() {
        return format!("Text {}", compact(text, 42));
    }
    if let Some(ExternalPayload::Html(html)) = drop.best() {
        return format!("HTML {}", compact(html, 42));
    }
    "Native payload".to_string()
}

fn compact(value: &str, max: usize) -> String {
    let mut out = value.chars().take(max).collect::<String>();
    if value.chars().count() > max {
        out.push_str("...");
    }
    out
}

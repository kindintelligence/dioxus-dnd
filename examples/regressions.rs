//! Headless fixtures for the browser suite. This is not a showcase: each
//! block exercises one library behavior in the smallest form a real browser
//! can drive, with stable DOM hooks (headings, `id`s, `data-*`) the
//! Playwright specs assert against. See
//! `tests/browser/web-pointer-regressions.spec.js`.
//!
//! ```sh
//! dx serve --example regressions --platform web --features web
//! ```

use std::collections::HashMap;

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
        SortableFixture {}
        GridFixture {}
        AutoScrollFixture {}
        CanvasGrabFixture {}
        CopyMoveFixture {}
        AccessibleReorderFixture {}
        NativeBoundaryFixture {}
        BridgeFixture {}
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

// --- sortable: overlay ghost geometry, and release-outside cancels ------------

/// A five-row sortable with the floating-overlay ghost enabled. Drives:
/// overlay matches the source row and cleans up after drop; a release
/// outside the list commits no reorder; a release inside still does.
#[component]
fn SortableFixture() -> Element {
    let mut items = use_signal(|| {
        ["Research", "Draft", "Review", "Revise", "Publish"]
            .map(String::from)
            .to_vec()
    });
    rsx! {
        section {
            h2 { "Sortable list" }
            SortableList {
                len: items.read().len(),
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                overlay: move |ix: usize| rsx! {
                    "{items.read()[ix]}"
                },
                style: "width: 320px;",
                render: move |ix: usize| rsx! {
                    div { style: "padding: 10px; border-bottom: 1px solid #ddd; background: #fff;",
                        "{items.read()[ix]}"
                    }
                },
            }
        }
    }
}

// --- grid: release outside the tiles cancels ----------------------------------

#[component]
fn GridFixture() -> Element {
    let mut tiles = use_signal(|| (1..=6).map(|n| format!("Tile {n}")).collect::<Vec<_>>());
    rsx! {
        section {
            h2 { "Grid" }
            SortableGrid {
                len: tiles.read().len(),
                cols: 3,
                on_sort: move |ev: SortEvent| apply_sort(&mut tiles.write(), ev),
                style: "width: 360px; gap: 8px;",
                render: move |ix: usize| rsx! {
                    div { style: "padding: 16px; background: #eee; text-align: center;",
                        "{tiles.read()[ix]}"
                    }
                },
            }
        }
    }
}

// --- autoscroll: edge drags scroll, leaving the container stops it ------------

/// A scrollable queue with touch grips. Drives: autoscroll follows a mouse
/// pointer drag near the edge (and never scrolls on passive hover); scrolling
/// stops when the captured pointer leaves the container.
#[component]
fn AutoScrollFixture() -> Element {
    let mut rows = use_signal(|| {
        let mut v = vec!["Unload the truck".to_string()];
        v.extend((2..=14).map(|n| format!("Task {n}")));
        v
    });
    rsx! {
        section {
            h2 { "Autoscroll" }
            AutoScroll {
                class: "list-scroll",
                style: "max-height: 200px; overflow-y: auto; width: 320px; border: 1px solid #ccc;",
                SortableList {
                    len: rows.read().len(),
                    touch_handle: true,
                    on_sort: move |ev: SortEvent| apply_sort(&mut rows.write(), ev),
                    render: move |ix: usize| rsx! {
                        div { style: "padding: 10px; border-bottom: 1px solid #eee;",
                            "{rows.read()[ix]}"
                        }
                    },
                }
            }
        }
    }
}

// --- canvas: pointer drops land corrected by the grab offset -------------------

#[derive(Clone, PartialEq)]
struct CanvasNode {
    id: u32,
    x: f64,
    y: f64,
}

#[component]
fn CanvasGrabFixture() -> Element {
    let mut node = use_signal(|| CanvasNode {
        id: 1,
        x: 40.0,
        y: 40.0,
    });
    rsx! {
        section {
            h2 { "Canvas" }
            DndProvider::<CanvasNode> {
                CanvasDropZone::<CanvasNode> {
                    bounds: Bounds {
                        width: 520.0,
                        height: 260.0,
                    },
                    on_drop: move |d: CanvasDrop<CanvasNode>| {
                        let mut n = node.write();
                        n.x = d.position.x;
                        n.y = d.position.y;
                    },
                    class: "relative",
                    style: "width: 520px; height: 260px; border: 1px solid #ccc;",
                    Draggable::<CanvasNode> {
                        payload: node(),
                        label: "Input",
                        style: "position: absolute; left: {node().x}px; top: {node().y}px;",
                        div { style: "padding: 8px 14px; background: #fff; border: 1px solid #333;",
                            "Input"
                        }
                    }
                }
            }
        }
    }
}

// --- copy vs move: Ctrl at release resolves the pointer drop to Copy ----------

#[derive(Clone, PartialEq)]
struct Block {
    id: u32,
    name: String,
}

const PALETTE: ZoneId = ZoneId(2001);
const STAGE: ZoneId = ZoneId(2002);

#[component]
fn CopyMoveFixture() -> Element {
    let mut zones = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Block>> = HashMap::new();
        m.insert(
            PALETTE,
            ["Button", "Input", "Chart"]
                .iter()
                .enumerate()
                .map(|(i, n)| Block {
                    id: i as u32 + 1,
                    name: n.to_string(),
                })
                .collect(),
        );
        m.insert(STAGE, vec![]);
        m
    });
    let mut next_id = use_signal(|| 100u32);
    let on_drop = move |o: DropOutcome<Block>| {
        apply_clone_or_move(
            &mut zones.write(),
            o,
            |b| b.id,
            move |mut b| {
                b.id = next_id();
                next_id += 1;
                b
            },
        );
    };
    rsx! {
        section {
            h2 { "Copy vs move" }
            DndProvider::<Block> {
                div { style: "display: flex; gap: 16px;",
                    for (label, zone) in [("Palette", PALETTE), ("Stage", STAGE)] {
                        DropZone::<Block> {
                            id: zone,
                            on_drop,
                            style: "width: 200px; min-height: 140px; border: 1px dashed #999; padding: 8px;",
                            span { "{label}" }
                            for block in zones.read().get(&zone).cloned().unwrap_or_default() {
                                Draggable::<Block> {
                                    payload: block.clone(),
                                    zone,
                                    label: block.name.clone(),
                                    div { style: "padding: 8px; margin-top: 6px; background: #fff; border: 1px solid #333;",
                                        "{block.name}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- reorder buttons: clicks inside a sortable row stay clicks ----------------

#[component]
fn AccessibleReorderFixture() -> Element {
    let mut items = use_signal(|| {
        ["Wake up", "Ship code", "Touch grass", "Sleep"]
            .map(String::from)
            .to_vec()
    });
    rsx! {
        section {
            h2 { "Accessible reorder" }
            SortableList {
                len: items.read().len(),
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                style: "width: 320px;",
                render: move |ix: usize| rsx! {
                    div { style: "display: flex; justify-content: space-between; padding: 8px; border-bottom: 1px solid #eee;",
                        span { "{items.read()[ix]}" }
                        ReorderButtons {
                            index: ix,
                            total: items.read().len(),
                            label: items.read()[ix].clone(),
                            on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                        }
                    }
                },
            }
        }
    }
}

// --- bridge: the same ZoneId registered in two payload worlds -----------------
// The documented cross-type pattern (README "Mixing payload types", gallery
// "Standup"): tickets (&str) and people (u32) drag in separate providers; one
// shared box registers in both registries and each drop arrives through its
// own typed callback. A world's other zones stay dark for the foreign drag.

#[component]
fn DualZone(
    on_ticket: EventHandler<DropOutcome<&'static str>>,
    on_person: EventHandler<DropOutcome<u32>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let dnd_a = use_dnd::<&'static str>();
    let dnd_b = use_dnd::<u32>();
    let mut reg_a = use_zone_registry::<&'static str>();
    let mut reg_b = use_zone_registry::<u32>();
    let zone_id = use_zone_id();
    let mounted = use_signal(|| None::<std::rc::Rc<dioxus::html::MountedData>>);
    let rect = use_signal(|| None::<Rect>);
    use_hook(move || {
        reg_a.register(ZoneRecord {
            id: zone_id,
            parent: None,
            label: Some("bridge".into()),
            on_drop: Callback::new(move |o| on_ticket.call(o)),
            accepts: None,
            mounted,
            rect,
        });
        reg_b.register(ZoneRecord {
            id: zone_id,
            parent: None,
            label: Some("bridge".into()),
            on_drop: Callback::new(move |o| on_person.call(o)),
            accepts: None,
            mounted,
            rect,
        });
    });
    use_drop(move || {
        reg_a.unregister(zone_id);
        reg_b.unregister(zone_id);
    });
    rsx! {
        div {
            "data-active": if dnd_a.dragging() || dnd_b.dragging() { "true" },
            "data-over": if dnd_a.over() == Some(zone_id) || dnd_b.over() == Some(zone_id) { "true" },
            onmounted: move |evt: Event<dioxus::html::MountedData>| {
                let mut mounted = mounted;
                mounted.set(Some(evt.data()));
            },
            ..attributes,
            {children}
        }
    }
}

#[component]
fn BridgeFixture() -> Element {
    let mut log = use_signal(Vec::<String>::new);
    rsx! {
        section {
            h2 { "Bridge zone" }
            DndProvider::<&'static str> {
                DndProvider::<u32> {
                    div { style: "display:flex; gap:12px;",
                        Draggable::<&'static str> {
                            payload: "DND-41",
                            label: "ticket",
                            id: "bridge-ticket",
                            style: "width:120px; padding:10px; border:1px solid #333; \
                                    background:#fff; cursor:grab; user-select:none;",
                            "ticket DND-41"
                        }
                        Draggable::<u32> {
                            payload: 7u32,
                            label: "person",
                            id: "bridge-person",
                            style: "width:120px; padding:10px; border:1px solid #333; \
                                    background:#fff; cursor:grab; user-select:none;",
                            "person #7"
                        }
                    }
                    // A ticket-world-only zone: lights for ticket drags, stays
                    // dark (and unreachable) for person drags.
                    DropZone::<&'static str> {
                        id: ZoneId(2001),
                        on_drop: move |o: DropOutcome<&'static str>| {
                            log.write().push(format!("shipped:{}", o.payload));
                        },
                        class: "ticket-only",
                        style: "margin-top:16px; width:260px; min-height:50px; \
                                border:2px dashed #999; padding:8px;",
                        "tickets only"
                    }
                    // The bridge: one box, both worlds.
                    DualZone {
                        id: "bridge-zone",
                        on_ticket: move |o: DropOutcome<&'static str>| {
                            log.write().push(format!("ticket:{}", o.payload));
                        },
                        on_person: move |o: DropOutcome<u32>| {
                            log.write().push(format!("person:{}", o.payload));
                        },
                        style: "margin-top:16px; width:260px; min-height:50px; \
                                border:2px dashed #393; padding:8px;",
                        "agenda (both worlds)"
                    }
                    div {
                        id: "bridge-status",
                        "data-log": log.read().join(","),
                        "log: {log.read().join(\",\")}"
                    }
                }
            }
        }
    }
}

// --- native boundary: files in, external content in, links out ----------------

#[component]
fn NativeBoundaryFixture() -> Element {
    let mut files = use_signal(Vec::<String>::new);
    let mut external = use_signal(String::new);
    rsx! {
        section {
            h2 { "File drop" }
            FileDropZone {
                on_files: move |drop: FileDrop| {
                    files.write().extend(drop.files.iter().map(|f| f.name()));
                },
                style: "width: 320px; min-height: 80px; border: 2px dashed #999; padding: 12px;",
                p { "Drop files from your desktop here" }
                for name in files.read().clone() {
                    span { "{name}" }
                }
            }
        }
        section {
            h2 { "In & out" }
            ExternalDropZone {
                on_drop: move |d: ExternalDrop| {
                    external.set(format!("{} payload(s), {} file(s)", d.payloads.len(), d.files.len()));
                },
                style: "width: 320px; min-height: 60px; border: 2px dashed #999; padding: 12px;",
                p { "Drop text or a link here" }
                if !external.read().is_empty() {
                    span { "{external}" }
                }
            }
            ExternalDragSource {
                content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
                style: "display: block; width: 320px; margin-top: 12px; border: 1px solid #333; padding: 10px; cursor: grab;",
                "Drag this link out to another app"
            }
        }
    }
}

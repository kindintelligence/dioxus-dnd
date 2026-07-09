//! Headless fixtures for the browser suite. This is not a showcase: each
//! block exercises one library behavior in the smallest form a real browser
//! can drive, with stable DOM hooks (headings, `id`s, `data-*`) the
//! Playwright specs assert against. See
//! `tests/browser/web-pointer-regressions.spec.js`.
//!
//! ```sh
//! dx serve --example regressions --platform web --features web,serde
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
        StaleRectsFixture {}
        SettleFixture {}
        EdgeFixture {}
        VoiceFixture {}
        VirtualFixture {}
        TouchPreventFixture {}
        TouchSenseFixture {}
        FlipFixture {}
        MatchSourceFixture {}
        TypedFixture {}
    }
}

// --- typed DataTransfer transport (serde) ---------------------------------
// TypedDragSource serializes its payload (JSON under application/json plus
// a text/plain fallback) at dragstart; TypedDropZone decodes drops back to
// the type, ignores untyped drags, and reports undecodable JSON through
// on_invalid. The spec drives it with real DragEvents carrying a real
// DataTransfer - the boundary a headless VirtualDom cannot cross.

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TypedCard {
    id: u32,
    name: String,
}

#[component]
fn TypedFixture() -> Element {
    let mut landed = use_signal(String::new);
    let mut invalid = use_signal(|| 0u32);
    rsx! {
        section {
            h2 { "Typed transport" }
            TypedDragSource::<TypedCard> {
                payload: TypedCard { id: 7, name: "seven".into() },
                id: "typed-source",
                "typed source"
            }
            TypedDragSource::<TypedCard> {
                payload: TypedCard { id: 9, name: "nine".into() },
                fallback_text: "card nine".to_string(),
                id: "typed-source-fallback",
                "typed source with fallback"
            }
            TypedDropZone::<TypedCard> {
                id: "typed-zone",
                on_drop: move |d: TypedDrop<TypedCard>| {
                    landed.set(format!("{}:{}", d.payload.id, d.payload.name));
                },
                on_invalid: move |_| invalid += 1,
                "typed zone"
            }
            div {
                id: "typed-status",
                "data-landed": landed(),
                "data-invalid": "{invalid}",
            }
        }
    }
}

// --- match_source ghost + on_settled + SettleSlot ------------------------------
// DragOverlay { match_source: true }: the ghost wears the grabbed element's
// measured rect, so grabbing a wide row anywhere keeps the cursor inside the
// ghost (the pointer - grab anchor is exact when sizes agree). on_settled
// fires once when the settle glide lands - after release, never racing it.
// And the landed element, wrapped in SettleSlot { active }, holds its space
// invisibly (data-settling) until the ghost unmounts - never two copies.

#[component]
fn MatchSourceFixture() -> Element {
    let mut settled = use_signal(|| 0u32);
    let mut landed = use_signal(|| false);
    rsx! {
        section {
            h2 { "Match source" }
            DndProvider::<u32> {
                Draggable::<u32> {
                    payload: 11u32,
                    label: "wide row",
                    id: "ms-drag",
                    style: "display:block; width:260px; padding:10px; border:1px solid #333; \
                            background:#fff; cursor:grab; user-select:none;",
                    "a deliberately wide row"
                }
                DropZone::<u32> {
                    on_drop: move |_o: DropOutcome<u32>| landed.set(true),
                    class: "ms-zone",
                    style: "margin-top:90px; width:260px; min-height:60px; \
                            border:2px dashed #999; padding:8px;",
                    "target"
                    if landed() {
                        SettleSlot::<u32> {
                            active: true,
                            id: "ms-landed",
                            div { style: "padding:6px; border:1px solid #393; background:#efe;",
                                "delivered"
                            }
                        }
                    }
                }
                DragOverlay::<u32> {
                    match_source: true,
                    settle: true,
                    on_settled: move |_| settled += 1,
                    class: "ms-ghost",
                    style: "border:1px solid #339; background:#eef; padding:10px;",
                    "ghost"
                }
                div {
                    id: "ms-status",
                    "data-settled": "{settled}",
                    "settled: {settled}"
                }
            }
        }
    }
}

// --- FLIP: the reorder glide is armed synchronously on the real element -------
// With the `web` feature, `FlipItem` hands invert-flush-release to the DOM in
// one step, so by the time anything can observe the swap there is a live CSS
// transition carrying the tile home - no dependency on a painted in-between
// frame. Drives: a running animation exists right after the swap commits,
// and the tiles land at exchanged positions.

#[component]
fn FlipFixture() -> Element {
    let mut order = use_signal(|| vec!["A".to_string(), "B".to_string()]);
    let mut epoch = use_signal(|| 0usize);
    rsx! {
        section {
            h2 { "Flip" }
            button {
                id: "flip-swap",
                onclick: move |_| {
                    order.write().reverse();
                    epoch += 1;
                },
                "swap"
            }
            div { style: "display: flex; gap: 12px; width: 300px; margin-top: 8px;",
                for name in order.read().clone() {
                    FlipItem {
                        key: "{name}",
                        epoch: epoch(),
                        duration: 600.0,
                        div {
                            id: "flip-{name}",
                            style: "width: 80px; height: 40px; background: #eee; \
                                    display: grid; place-items: center;",
                            "{name}"
                        }
                    }
                }
            }
        }
    }
}

// --- touch auto-sense: scroll by swipe, drag by hold or sideways pull ---------
// Whole-row SortableList under the default `TouchSense::Auto` inside a
// scrollable container - the exact setup that used to trap mobile scrolling.
// Drives: a vertical swipe scrolls the container and reorders nothing; a
// 250ms hold picks the row up and the container stays put; a sideways pull
// picks it up with no hold at all.

#[component]
fn TouchSenseFixture() -> Element {
    let mut rows = use_signal(|| (1..=10).map(|n| format!("Item {n}")).collect::<Vec<_>>());
    rsx! {
        section {
            h2 { "Touch sense" }
            div {
                id: "ts-scroll",
                style: "height: 160px; overflow-y: auto; width: 300px; border: 1px solid #ccc;",
                SortableList {
                    len: rows.read().len(),
                    on_sort: move |ev: SortEvent| apply_sort(&mut rows.write(), ev),
                    render: move |ix: usize| rsx! {
                        div { style: "height: 40px; padding: 10px; box-sizing: border-box; \
                                      border-bottom: 1px solid #eee; background: #fff;",
                            "{rows.read()[ix]}"
                        }
                    },
                }
            }
            div {
                id: "ts-status",
                "data-order": rows.read().join(","),
                "order: {rows.read().join(\",\")}"
            }
        }
    }
}

// --- touch: prevent_default() on ontouchmove blocks native scroll -------------
// The touch auto-sensor rests on this: dioxus-web registers its delegated
// listener without `passive`, and the default mount root `#main` is a plain
// element (browsers force passive only on window/document/body), so a
// synchronous handler's prevent_default() still reaches the native event in
// time to cancel the pan. Two swipe lanes in one scroller pin it from both
// sides: the blocking lane must not scroll, the free lane must.

#[component]
fn TouchPreventFixture() -> Element {
    rsx! {
        section {
            h2 { "Touch preventDefault" }
            div {
                id: "tp-scroll",
                style: "height:150px; overflow-y:auto; width:300px; border:1px solid #ccc;",
                div {
                    id: "tp-blocker",
                    style: "height:70px; background:#fdd; touch-action: pan-y;",
                    ontouchmove: move |evt: TouchEvent| evt.prevent_default(),
                    "swipe here: no scroll"
                }
                div {
                    id: "tp-free",
                    style: "height:70px; background:#dfd;",
                    "swipe here: scrolls"
                }
                div { style: "height:600px;", "filler" }
            }
        }
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
// The cross-type pattern (README "Mixing payload types", gallery "Standup"),
// now `BridgeDropZone` in core: tickets (&str) and people (u32) drag in
// separate providers; one shared box registers in both registries and each
// drop arrives through its own typed callback. A world's other zones stay
// dark for the foreign drag.

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
                    // The bridge: one box, both worlds. (`id` is the ZoneId
                    // prop here, so the DOM hook is a class.)
                    BridgeDropZone::<&'static str, u32> {
                        class: "bridge-zone",
                        on_drop_a: move |o: DropOutcome<&'static str>| {
                            log.write().push(format!("ticket:{}", o.payload));
                        },
                        on_drop_b: move |o: DropOutcome<u32>| {
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

// --- stale rects: hit-testing tracks zones that auto-scroll under a drag ------
// Zone rects are cached at drag start; AutoScroll moves the zones mid-drag.
// The rect-refresh channel re-measures after every scroll, so hover and the
// drop land on the zone the user actually sees - not where it sat at pickup.

#[component]
fn StaleRectsFixture() -> Element {
    let mut landed = use_signal(|| "none".to_string());
    rsx! {
        section {
            h2 { "Stale rects" }
            DndProvider::<u32> {
                AutoScroll {
                    class: "stale-scroll",
                    style: "margin-top:12px; max-height:180px; overflow-y:auto; \
                            width:300px; border:1px solid #ccc;",
                    // The drag source lives inside the scroll container (as a
                    // list item would), so captured pointer moves bubble
                    // through the container and drive its autoscroll.
                    Draggable::<u32> {
                        payload: 1u32,
                        label: "parcel",
                        id: "stale-drag",
                        style: "height:50px; padding:10px; border-bottom:1px solid #333; \
                                background:#fff; cursor:grab; user-select:none; \
                                display:flex; align-items:center;",
                        "drag the parcel"
                    }
                    for i in 0..8u32 {
                        DropZone::<u32> {
                            id: ZoneId(3000 + i as u64),
                            on_drop: move |_o: DropOutcome<u32>| landed.set(format!("zone {i}")),
                            class: "stale-zone",
                            style: "height:70px; border-bottom:1px solid #eee; \
                                    display:flex; align-items:center; justify-content:center;",
                            "zone {i}"
                        }
                    }
                }
                div { id: "stale-status", "data-landed": "{landed}", "landed: {landed}" }
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

// --- drop-settle: the ghost glides into the receiving zone on drop -----------
// DragOverlay { settle: true }: after a successful pointer drop the overlay
// must survive the drop (while the zones unlight immediately), glide its
// center onto the zone's center, and unmount on transitionend. A cancelled
// drag just vanishes - settle is for completed drops only.

#[component]
fn SettleFixture() -> Element {
    let mut landed = use_signal(|| "none".to_string());
    rsx! {
        section {
            h2 { "Drop settle" }
            DndProvider::<u32> {
                Draggable::<u32> {
                    payload: 5u32,
                    label: "parcel",
                    id: "settle-drag",
                    style: "width:120px; padding:10px; border:1px solid #333; \
                            background:#fff; cursor:grab; user-select:none;",
                    "parcel #5"
                }
                DropZone::<u32> {
                    on_drop: move |o: DropOutcome<u32>| landed.set(format!("landed:{}", o.payload)),
                    class: "settle-zone",
                    style: "margin-top:120px; width:260px; min-height:60px; \
                            border:2px dashed #999; padding:8px;",
                    "settle target"
                }
                // Slowed down so the spec can observe the mid-glide ghost.
                DragOverlay::<u32> {
                    settle: true,
                    duration: 600.0,
                    class: "settle-ghost",
                    style: "width:120px; padding:10px; border:1px solid #339; background:#eef;",
                    "parcel #5"
                }
                div { id: "settle-status", "data-landed": landed(), "landed: {landed}" }
            }
        }
    }
}

// --- closest edge: data-edge tracks the pointer, the outcome carries it ------
// DropZone { edge: EdgeSet::Vertical }: while an acceptable pointer drag
// hovers the zone, data-edge reads the nearest allowed edge live on every
// move; the delivered DropOutcome records the edge at release; the attribute
// leaves with the drag.

#[component]
fn EdgeFixture() -> Element {
    let mut landed = use_signal(|| "none".to_string());
    rsx! {
        section {
            h2 { "Closest edge" }
            DndProvider::<u32> {
                Draggable::<u32> {
                    payload: 9u32,
                    label: "chip",
                    id: "edge-drag",
                    style: "width:120px; padding:10px; border:1px solid #333; \
                            background:#fff; cursor:grab; user-select:none;",
                    "chip #9"
                }
                DropZone::<u32> {
                    edge: EdgeSet::Vertical,
                    on_drop: move |o: DropOutcome<u32>| {
                        landed.set(match o.edge {
                            Some(e) => format!("edge:{}", e.as_str()),
                            None => "edge:none".to_string(),
                        });
                    },
                    class: "edge-zone",
                    style: "margin-top:40px; width:300px; height:120px; \
                            border:2px dashed #999; padding:8px;",
                    "vertical edges"
                }
                div { id: "edge-status", "data-landed": landed(), "landed: {landed}" }
            }
        }
    }
}

// --- localized voice: keyboard announcements read DndStrings ------------------
// A provided DndStrings context replaces every announcement the keyboard
// path voices, and closures that read a locale signal follow a live switch
// with no remount - the next phrase speaks the new language.

#[component]
fn VoiceFixture() -> Element {
    let locale = use_signal(|| "en");
    use_context_provider(|| DndStrings {
        picked_up: std::rc::Rc::new(move |name| match *locale.peek() {
            "es" => format!("Recogiste {name}. Usa las flechas, Enter para soltar."),
            _ => format!("Picked up {name}. Use arrow keys, Enter to drop."),
        }),
        over: std::rc::Rc::new(move |name| match *locale.peek() {
            "es" => format!("Sobre {name}."),
            _ => format!("Over {name}."),
        }),
        dropped_in: std::rc::Rc::new(move |name| match *locale.peek() {
            "es" => format!("Soltado en {name}."),
            _ => format!("Dropped in {name}."),
        }),
        cancelled: std::rc::Rc::new(move || match *locale.peek() {
            "es" => "Arrastre cancelado.".to_string(),
            _ => "Drag cancelled.".to_string(),
        }),
        ..Default::default()
    });
    let mut locale = locale;
    rsx! {
        section {
            h2 { "Localized voice" }
            button {
                id: "voice-toggle",
                onclick: move |_| {
                    let next = if *locale.peek() == "en" { "es" } else { "en" };
                    locale.set(next);
                },
                "toggle language"
            }
            DndProvider::<u32> {
                LiveRegion::<u32> {}
                Draggable::<u32> {
                    payload: 3u32,
                    label: "parcel",
                    id: "voice-drag",
                    style: "margin-top:12px; width:140px; padding:10px; border:1px solid #333; \
                            background:#fff; cursor:grab; user-select:none;",
                    "parcel"
                }
                DropZone::<u32> {
                    label: "shelf",
                    on_drop: move |_: DropOutcome<u32>| {},
                    class: "voice-zone",
                    style: "margin-top:12px; width:260px; min-height:50px; \
                            border:2px dashed #999; padding:8px;",
                    "shelf"
                }
            }
        }
    }
}

// --- virtual list: zones recycling mid-drag stay droppable --------------------
// A windowed list mounts/unmounts row zones as it scrolls. Rows that mount
// MID-DRAG missed the pickup measurement and the last scroll ping (which ran
// before they rendered), so they must measure themselves on mount to be
// hit-testable - the regression this fixture pins. AutoScroll's on_scroll
// drives the windowing; its internal ping keeps moved rows fresh.

const VROWS: usize = 1000;
const VROW_H: f64 = 30.0;
const VVIEW_H: f64 = 210.0;
const VBASE: u64 = 30_000;

#[component]
fn VirtualFixture() -> Element {
    let mut scroll_top = use_signal(|| 0.0f64);
    let mut scrolls = use_signal(|| 0u32);
    let mut landed = use_signal(|| "none".to_string());
    // The container's mounted handle, for turning a row's viewport position
    // back into a scroll offset (the wrapper shares the container's top).
    let container = use_signal(|| None::<std::rc::Rc<dioxus::html::MountedData>>);
    let first = ((scroll_top() / VROW_H) as usize).saturating_sub(4);
    let last = (first + (VVIEW_H / VROW_H).ceil() as usize + 8).min(VROWS);

    rsx! {
        section {
            h2 { "Virtual list" }
            DndProvider::<&'static str> {
                LiveRegion::<&'static str> {}
                Draggable::<&'static str> {
                    payload: "tag",
                    label: "tag",
                    id: "virtual-drag",
                    style: "width:120px; padding:10px; border:1px solid #333; \
                            background:#fff; cursor:grab; user-select:none;",
                    "tag"
                }
                div {
                    style: "margin-top:12px;",
                    onmounted: move |evt: Event<dioxus::html::MountedData>| {
                        let mut container = container;
                        container.set(Some(evt.data()));
                    },
                    AutoScroll {
                        class: "virtual-scroll",
                        style: "height:{VVIEW_H}px; overflow-y:auto; \
                                width:320px; border:1px solid #ccc; box-sizing:border-box;",
                        on_scroll: move |offset: Point| {
                            scrolls += 1;
                            scroll_top.set(offset.y);
                        },
                        div { style: "position: relative; height: {VROWS as f64 * VROW_H}px;",
                            div { style: "position: absolute; top: 0; left: 0; width: 100%; \
                                          transform: translateY({first as f64 * VROW_H}px);",
                                for ix in first..last {
                                    DropZone::<&'static str> {
                                        key: "{ix}",
                                        id: ZoneId(VBASE + ix as u64),
                                        label: format!("Row {ix}"),
                                        on_drop: move |o: DropOutcome<&'static str>| {
                                            landed.set(format!("row:{ix}:{}", o.payload));
                                        },
                                        style: "height:{VROW_H}px; box-sizing:border-box; \
                                                border-bottom:1px solid #eee; padding:4px 8px;",
                                        // Rendered rows are their own scroll sentinels: any
                                        // crossing of the container's clip edge fires, and the
                                        // entry rect + the row's canvas position recover the
                                        // offset - IntersectionObserver-driven, so it works for
                                        // wheel, scrollbar and programmatic scrolls alike, idle
                                        // or mid-drag (dioxus 0.7's documented virtual-list
                                        // pattern; scroll events never reach dioxus-web).
                                        div {
                                            style: "height:100%;",
                                            onvisible: move |evt: VisibleEvent| {
                                                let Ok(r) = evt.data().get_bounding_client_rect() else {
                                                    return;
                                                };
                                                let row_y = r.origin.y;
                                                let mut scroll_top = scroll_top;
                                                spawn(async move {
                                                    let Some(c) = container.peek().clone() else { return };
                                                    if let Ok(cr) = c.get_client_rect().await {
                                                        let s = (ix as f64 * VROW_H) - (row_y - cr.origin.y);
                                                        scroll_top.set(s.max(0.0));
                                                    }
                                                });
                                            },
                                            "Row {ix}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div {
                    id: "virtual-status",
                    "data-landed": landed(),
                    "data-window": "{first}..{last}",
                    "data-scrolls": "{scrolls}",
                    "landed: {landed} window: {first}..{last}"
                }
            }
        }
    }
}

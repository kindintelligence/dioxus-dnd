//! The dioxus-dnd website: a landing page whose centerpiece is a live
//! playground, one interactive demo per drop pattern, plus an "outcome
//! tape" that prints every `DropOutcome` the library delivers.
//!
//! Run with:
//! ```sh
//! dx serve --example showcase --platform web
//! ```
//!
//! When deploying as the website, set the page `<title>` and meta
//! description in your `index.html` / `Dioxus.toml`; the crate stays on
//! `minimal` features, so it doesn't pull the `document` machinery in.

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

fn main() {
    dioxus::launch(App);
}

// --- shared outcome tape --------------------------------------------------

#[derive(Clone, PartialEq)]
struct TapeEntry {
    demo: &'static str,
    line: String,
}

#[derive(Clone, Copy, PartialEq)]
struct Tape(Signal<Vec<TapeEntry>>);

impl Tape {
    fn print(&mut self, demo: &'static str, line: impl Into<String>) {
        let mut entries = self.0.write();
        entries.push(TapeEntry {
            demo,
            line: line.into(),
        });
        let overflow = entries.len().saturating_sub(30);
        if overflow > 0 {
            entries.drain(..overflow);
        }
    }
}

fn use_tape() -> Tape {
    use_context()
}

// --- demo registry ----------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Demo {
    Core,
    Sortable,
    Board,
    Tree,
    Canvas,
    Grid,
    MultiSelect,
    Files,
    InOut,
    Keyboard,
}

impl Demo {
    const ALL: [Demo; 10] = [
        Demo::Core,
        Demo::Sortable,
        Demo::Board,
        Demo::Tree,
        Demo::Canvas,
        Demo::Grid,
        Demo::MultiSelect,
        Demo::Files,
        Demo::InOut,
        Demo::Keyboard,
    ];

    fn title(&self) -> &'static str {
        match self {
            Demo::Core => "Pick up & drop",
            Demo::Sortable => "Sort a list",
            Demo::Board => "Board",
            Demo::Tree => "Tree",
            Demo::Canvas => "Canvas",
            Demo::Grid => "Grid",
            Demo::MultiSelect => "Multi-select",
            Demo::Files => "Files",
            Demo::InOut => "In & out",
            Demo::Keyboard => "Keyboard",
        }
    }

    fn module(&self) -> &'static str {
        match self {
            Demo::Core => "core",
            Demo::Sortable => "sortable + autoscroll",
            Demo::Board => "board",
            Demo::Tree => "tree",
            Demo::Canvas => "canvas",
            Demo::Grid => "grid",
            Demo::MultiSelect => "multiselect",
            Demo::Files => "files",
            Demo::InOut => "external + dragout",
            Demo::Keyboard => "a11y",
        }
    }

    fn hint(&self) -> &'static str {
        match self {
            Demo::Core => "Drag crates between the shelf and the bays. They really move. Hold Ctrl or Cmd while dropping to copy instead. Works with touch.",
            Demo::Sortable => "Drag a row: it slides toward its landing slot while the others make room. The arrows reorder without any dragging, and the list scrolls itself near its edges.",
            Demo::Board => "Move cards between columns. Drop on the thin slots to insert at an exact position.",
            Demo::Tree => "Drop on the top of a row to place before it, the bottom to place after, the middle to nest inside. The tree restructures for real.",
            Demo::Canvas => "Drag parts from the shelf onto the floor. Positions snap to the grid and stay inside the walls.",
            Demo::Grid => "Drag one tile onto another and they trade places.",
            Demo::MultiSelect => "Click selects one crate; Ctrl+click selects more. Drag any selected crate and the whole group ships together.",
            Demo::Files => "Drop image files from your computer. Anything over 2 MB or not an image gets refused with a reason.",
            Demo::InOut => "Drag a link or selected text from another tab into the bay, or drag our tag out into your URL bar.",
            Demo::Keyboard => "No mouse needed: Tab to a crate, press Space, pick a bay with the arrows, press Enter, and watch it land. Crates in bays stay focusable, so you can keep moving them.",
        }
    }
}

// --- app shell --------------------------------------------------------------

#[component]
fn App() -> Element {
    use_context_provider(|| Tape(Signal::new(Vec::new())));

    rsx! {
        style { {CSS} }
        div { class: "page",
            SiteHeader {}
            Hero {}
            Playground {}
            SiteFooter {}
        }
    }
}

#[component]
fn SiteHeader() -> Element {
    rsx! {
        header { class: "masthead",
            div { class: "shell masthead-row",
                a { class: "brand", href: "#top", "DIOXUS·DND" }
                nav { class: "topnav", aria_label: "Site",
                    a { href: "#playground", "Playground" }
                    a { href: "https://docs.rs/dioxus-dnd", "Docs" }
                    a { href: "https://crates.io/crates/dioxus-dnd", "crates.io" }
                    a { href: "https://github.com/kindintelligence/dioxus-dnd", "GitHub" }
                }
            }
        }
    }
}

#[component]
fn Hero() -> Element {
    rsx! {
        section { class: "hero", id: "top",
            div { class: "shell",
                p { class: "eyebrow", "A DRAG-AND-DROP LIBRARY FOR DIOXUS 0.8+" }
                h1 { class: "hero-title", "PICK IT UP." br {} "PUT IT ANYWHERE." }
                p { class: "lede",
                    "Sortable lists, boards, trees, grids, canvases, OS file drops, "
                    "multi-select, drag-out to other apps, with touch and keyboard "
                    "handled for you. Any payload type, no serialization, no JS."
                }
                div { class: "install-row",
                    // `user-select: all` lets one click select the whole command.
                    code { class: "install", title: "Click to select, then copy", "cargo add dioxus-dnd" }
                    span { class: "install-hint", "click to select" }
                    a { class: "cta", href: "#playground", "Try it live ↓" }
                }
                ul { class: "ticks", aria_label: "Highlights",
                    li { span { class: "tick-key", "12" } " drop patterns, one small core" }
                    li { span { class: "tick-key", "3" } " input methods: mouse, touch, keyboard" }
                    li { span { class: "tick-key", "38" } " tests, zero warnings, zero extra deps" }
                    li { span { class: "tick-key", "OS" } " file drops in, links dragged out" }
                }
            }
        }
    }
}

#[component]
fn Playground() -> Element {
    let mut current = use_signal(|| Demo::Core);

    rsx! {
        section { class: "playground", id: "playground",
            div { class: "shell-wide",
                div { class: "playground-head",
                    h2 { "THE LOADING DOCK" }
                    p { "Every demo is the real library. Drag something, and its DropOutcome prints on the tape below." }
                }
                div { class: "unit",
                    div { class: "unit-body",
                        nav { class: "manifest", aria_label: "Demos",
                            span { class: "manifest-head", "MANIFEST" }
                            for (ix, demo) in Demo::ALL.iter().enumerate() {
                                button {
                                    class: "manifest-line",
                                    "data-active": current() == *demo,
                                    onclick: {
                                        let demo = *demo;
                                        move |_| current.set(demo)
                                    },
                                    span { class: "manifest-no", {format!("{:02}", ix + 1)} }
                                    span { class: "manifest-title", "{demo.title()}" }
                                    span { class: "manifest-module", "{demo.module()}" }
                                }
                            }
                        }
                        main { class: "stage",
                            div { class: "stage-head",
                                h3 { "{current().title()}" }
                                code { class: "stage-module", "dioxus_dnd::{current().module()}" }
                            }
                            p { class: "stage-hint", "{current().hint()}" }
                            div { class: "stage-body",
                                match current() {
                                    Demo::Core => rsx! { CoreDemo {} },
                                    Demo::Sortable => rsx! { SortableDemo {} },
                                    Demo::Board => rsx! { BoardDemo {} },
                                    Demo::Tree => rsx! { TreeDemo {} },
                                    Demo::Canvas => rsx! { CanvasDemo {} },
                                    Demo::Grid => rsx! { GridDemo {} },
                                    Demo::MultiSelect => rsx! { MultiSelectDemo {} },
                                    Demo::Files => rsx! { FilesDemo {} },
                                    Demo::InOut => rsx! { InOutDemo {} },
                                    Demo::Keyboard => rsx! { KeyboardDemo {} },
                                }
                            }
                        }
                    }
                    OutcomeTape {}
                }
            }
        }
    }
}

#[component]
fn SiteFooter() -> Element {
    rsx! {
        footer { class: "site-footer",
            div { class: "shell footer-row",
                div { class: "footer-brand",
                    span { class: "brand", "DIOXUS·DND" }
                    p { "Modular, accessible drag-and-drop for Dioxus." }
                }
                nav { class: "footer-links", aria_label: "Footer",
                    a { href: "https://crates.io/crates/dioxus-dnd", "crates.io" }
                    a { href: "https://docs.rs/dioxus-dnd", "docs.rs" }
                    a { href: "https://github.com/kindintelligence/dioxus-dnd", "GitHub" }
                    a { href: "https://github.com/kindintelligence/dioxus-dnd/blob/main/CHANGELOG.md", "Changelog" }
                }
                p { class: "footer-meta",
                    "MIT · built with Dioxus 0.8 · no cookies, no tracking, just crates"
                }
            }
        }
    }
}

/// The signature element: a receipt of every drop the library delivered.
#[component]
fn OutcomeTape() -> Element {
    let tape = use_tape();
    let entries = tape.0.read();

    rsx! {
        footer { class: "tape", aria_label: "Drop outcomes",
            div { class: "tape-label", "OUTCOME TAPE" }
            div { class: "tape-roll",
                if entries.is_empty() {
                    span { class: "tape-empty", "No drops yet. Drag anything above and its DropOutcome prints here." }
                } else {
                    for (ix, e) in entries.iter().enumerate().rev() {
                        div { class: "tape-line", key: "{ix}",
                            span { class: "tape-demo", "[{e.demo}]" }
                            span { "{e.line}" }
                        }
                    }
                }
            }
        }
    }
}

// --- 01 core: stateful shelf & bays ----------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Crate {
    id: u32,
    name: &'static str,
    fragile: bool,
}

const SHELF: ZoneId = ZoneId(11);
const BAY_A: ZoneId = ZoneId(12);
const BAY_B: ZoneId = ZoneId(13);

#[component]
fn CoreDemo() -> Element {
    let mut tape = use_tape();
    let mut stock = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Crate>> = HashMap::new();
        m.insert(
            SHELF,
            vec![
                Crate {
                    id: 1,
                    name: "GEARS",
                    fragile: false,
                },
                Crate {
                    id: 2,
                    name: "GLASS",
                    fragile: true,
                },
                Crate {
                    id: 3,
                    name: "BOLTS",
                    fragile: false,
                },
            ],
        );
        m.insert(BAY_A, vec![]);
        m.insert(BAY_B, vec![]);
        m
    });
    let mut next_id = use_signal(|| 100u32);

    // One landing routine for every zone: Copy duplicates, Move relocates.
    let mut land = move |o: DropOutcome<Crate>, to_name: &'static str| {
        let name = o.payload.name;
        if o.effect == DropEffect::Copy {
            tape.print("core", format!("{name} copied → {to_name} (Ctrl/Cmd held)"));
        } else {
            tape.print("core", format!("{name} moved → {to_name}"));
        }
        apply_clone_or_move(
            &mut stock.write(),
            o,
            |c| c.id,
            |mut c| {
                c.id = next_id();
                next_id += 1;
                c
            },
        );
    };

    let crates_in = move |zone: ZoneId| stock.read().get(&zone).cloned().unwrap_or_default();

    rsx! {
        DndProvider::<Crate> {
            LiveRegion::<Crate> {}
            div { class: "helper-note",
                code { "apply_clone_or_move" }
                span { "Move removes by id. Copy runs the clone hook and assigns a fresh id." }
            }
            div { class: "dock",
                DropZone::<Crate> {
                    id: SHELF,
                    label: "the shelf",
                    class: "bay bay-shelf",
                    on_drop: move |o: DropOutcome<Crate>| land(o, "the shelf"),
                    span { class: "bay-name", "SHELF" }
                    div { class: "bay-stack",
                        for c in crates_in(SHELF) {
                            PointerDraggable::<Crate> {
                                key: "{c.id}",
                                payload: c.clone(),
                                zone: SHELF,
                                label: c.name,
                                class: "crate",
                                span { class: "crate-label", "{c.name}" }
                                if c.fragile { span { class: "crate-tag", "FRAGILE" } }
                            }
                        }
                    }
                }
                DropZone::<Crate> {
                    id: BAY_A,
                    label: "Bay A",
                    class: "bay",
                    on_drop: move |o: DropOutcome<Crate>| land(o, "Bay A"),
                    span { class: "bay-name", "BAY A" }
                    span { class: "bay-rule", "accepts anything" }
                    div { class: "bay-stack",
                        for c in crates_in(BAY_A) {
                            PointerDraggable::<Crate> {
                                key: "{c.id}",
                                payload: c.clone(),
                                zone: BAY_A,
                                label: c.name,
                                class: "crate",
                                span { class: "crate-label", "{c.name}" }
                            }
                        }
                    }
                }
                DropZone::<Crate> {
                    id: BAY_B,
                    label: "Bay B, padded",
                    accepts: move |c: Crate| c.fragile,
                    class: "bay",
                    on_drop: move |o: DropOutcome<Crate>| land(o, "Bay B"),
                    span { class: "bay-name", "BAY B" }
                    span { class: "bay-rule", "padded, fragile only" }
                    div { class: "bay-stack",
                        for c in crates_in(BAY_B) {
                            PointerDraggable::<Crate> {
                                key: "{c.id}",
                                payload: c.clone(),
                                zone: BAY_B,
                                label: c.name,
                                class: "crate",
                                span { class: "crate-label", "{c.name}" }
                                span { class: "crate-tag", "FRAGILE" }
                            }
                        }
                    }
                }
            }
            DragOverlay::<Crate> { CrateDragPreview {} }
        }
    }
}

#[component]
fn CrateDragPreview() -> Element {
    let dnd = use_dnd::<Crate>();
    let Some(c) = dnd.payload() else {
        return rsx! {};
    };

    rsx! {
        div { class: "crate drag-preview",
            span { class: "crate-label", "{c.name}" }
            if c.fragile { span { class: "crate-tag", "FRAGILE" } }
        }
    }
}

// --- 02 sortable (live displacement + buttons + autoscroll) -----------------

#[component]
fn SortableDemo() -> Element {
    let mut tape = use_tape();
    let mut items = use_signal(|| {
        vec![
            "Unload the truck",
            "Scan every label",
            "Weigh the pallets",
            "Stack by destination",
            "Wrap the fragile row",
            "Book the courier",
            "Print the manifest",
            "Seal the container",
            "Chalk the door number",
            "Sign the release",
            "Sweep the bay",
            "Lights out",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>()
    });

    rsx! {
        AutoScroll { class: "list-scroll",
            SortableList {
                len: items.read().len(),
                touch_handle: true,
                render: move |ix: usize| rsx! {
                    div { class: "row",
                        span { class: "row-text", "{items.read()[ix]}" }
                        ReorderButtons {
                            class: "row-buttons",
                            index: ix,
                            total: items.read().len(),
                            label: items.read()[ix].clone(),
                            on_sort: move |ev: SortEvent| {
                                let name = items.read()[ev.from].clone();
                                apply_sort(&mut items.write(), ev);
                                tape.print("sortable", format!("\"{name}\" moved {} → {} (buttons)", ev.from + 1, ev.to + 1));
                            },
                        }
                    }
                },
                on_sort: move |ev: SortEvent| {
                    let name = items.read()[ev.from].clone();
                    apply_sort(&mut items.write(), ev);
                    tape.print("sortable", format!("\"{name}\" moved {} → {}", ev.from + 1, ev.to + 1));
                },
            }
        }
    }
}

// --- 03 board -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Card {
    id: u32,
    title: String,
}

const INBOUND: ZoneId = ZoneId(101);
const PACKING: ZoneId = ZoneId(102);
const SHIPPED: ZoneId = ZoneId(103);

#[component]
fn BoardDemo() -> Element {
    let mut tape = use_tape();
    let mut board = use_signal(|| {
        let mut m: HashMap<ContainerId, Vec<Card>> = HashMap::new();
        m.insert(
            INBOUND,
            vec![
                Card {
                    id: 1,
                    title: "Order #4411".into(),
                },
                Card {
                    id: 2,
                    title: "Order #4412".into(),
                },
                Card {
                    id: 3,
                    title: "Order #4415".into(),
                },
            ],
        );
        m.insert(
            PACKING,
            vec![Card {
                id: 4,
                title: "Order #4408".into(),
            }],
        );
        m.insert(SHIPPED, vec![]);
        m
    });

    let col_name = |c: ContainerId| match c {
        INBOUND => "Inbound",
        PACKING => "Packing",
        _ => "Shipped",
    };
    let on_move = move |mv: MoveEvent<Card>| {
        tape.print(
            "board",
            format!(
                "{} · {} → {}{}",
                mv.item.title,
                col_name(mv.from.0),
                col_name(mv.to.0),
                mv.to
                    .1
                    .map(|ix| format!(" @ slot {}", ix + 1))
                    .unwrap_or_default(),
            ),
        );
        apply_move(&mut board.write(), mv);
    };

    rsx! {
        DndProvider::<BoardPayload<Card>> {
            LiveRegion::<BoardPayload<Card>> {}
            div { class: "board",
                for col in [INBOUND, PACKING, SHIPPED] {
                    BoardColumn::<Card> {
                        id: col,
                        label: col_name(col),
                        on_move,
                        class: "column",
                        h3 { class: "column-name", "{col_name(col)}" }
                        for (ix, card) in board.read().get(&col).cloned().unwrap_or_default().into_iter().enumerate() {
                            BoardSlot::<Card> { column: col, index: ix, on_move, class: "slot" }
                            BoardItem::<Card> {
                                item: card.clone(),
                                column: col,
                                index: ix,
                                class: "card",
                                "{card.title}"
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- 04 tree: real restructuring ---------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct TreeRow {
    id: u64,
    name: String,
    depth: usize,
}

/// Naive single-row tree move (demo-scoped: children don't travel with
/// their parent; a real app would move the subtree slice).
fn apply_tree_move(rows: &mut Vec<TreeRow>, dragged: u64, target: u64, intent: DropIntent) {
    let Some(dpos) = rows.iter().position(|r| r.id == dragged) else {
        return;
    };
    let mut row = rows.remove(dpos);
    let Some(tpos) = rows.iter().position(|r| r.id == target) else {
        rows.insert(dpos.min(rows.len()), row);
        return;
    };
    let t_depth = rows[tpos].depth;
    let (insert_at, depth) = match intent {
        DropIntent::Before => (tpos, t_depth),
        DropIntent::After => (tpos + 1, t_depth),
        DropIntent::Into => (tpos + 1, t_depth + 1),
    };
    row.depth = depth;
    rows.insert(insert_at.min(rows.len()), row);
}

#[component]
fn TreeDemo() -> Element {
    let mut tape = use_tape();
    let mut rows = use_signal(|| {
        vec![
            TreeRow {
                id: 1,
                name: "warehouse/".into(),
                depth: 0,
            },
            TreeRow {
                id: 2,
                name: "aisle-1/".into(),
                depth: 1,
            },
            TreeRow {
                id: 3,
                name: "pallet-a".into(),
                depth: 2,
            },
            TreeRow {
                id: 4,
                name: "aisle-2/".into(),
                depth: 1,
            },
            TreeRow {
                id: 5,
                name: "loose-box".into(),
                depth: 0,
            },
        ]
    });

    rsx! {
        DndProvider::<u64> {
            LiveRegion::<u64> {}
            div { class: "tree",
                for row in rows.read().iter().cloned() {
                    TreeNodeTarget::<u64> {
                        key: "{row.id}",
                        node: NodeId(row.id),
                        row_height: 34.0,
                        accepts: {
                            let id = row.id;
                            move |(dragged, _intent): (u64, DropIntent)| dragged != id
                        },
                        on_drop: {
                            let target_id = row.id;
                            let target_name = row.name.clone();
                            move |ev: TreeDropEvent<u64>| {
                                let dragged_name = rows.read().iter()
                                    .find(|r| r.id == ev.payload)
                                    .map(|r| r.name.clone())
                                    .unwrap_or_default();
                                apply_tree_move(&mut rows.write(), ev.payload, target_id, ev.intent);
                                let place = match ev.intent {
                                    DropIntent::Before => "before",
                                    DropIntent::After => "after",
                                    DropIntent::Into => "into",
                                };
                                tape.print("tree", format!("{dragged_name} → {place} {target_name}"));
                            }
                        },
                        class: "tree-row",
                        style: "padding-left: {row.depth as f64 * 1.4 + 0.6}rem;",
                        PointerDraggable::<u64> {
                            payload: row.id,
                            label: row.name.clone(),
                            class: "tree-grab",
                            "{row.name}"
                        }
                    }
                }
            }
        }
    }
}

// --- 05 canvas -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Part {
    glyph: &'static str,
    name: &'static str,
}

#[component]
fn CanvasDemo() -> Element {
    let mut tape = use_tape();
    let mut placed = use_signal(Vec::<(Part, Point)>::new);
    let parts = [
        Part {
            glyph: "▲",
            name: "cone",
        },
        Part {
            glyph: "■",
            name: "pallet",
        },
        Part {
            glyph: "●",
            name: "drum",
        },
    ];

    rsx! {
        DndProvider::<Part> {
            LiveRegion::<Part> {}
            div { class: "canvas-wrap",
                div { class: "shelf shelf-side",
                    for p in parts {
                        PointerDraggable::<Part> {
                            payload: p.clone(),
                            label: p.name,
                            class: "part",
                            span { "{p.glyph}" }
                            span { class: "part-name", "{p.name}" }
                        }
                    }
                }
                CanvasDropZone::<Part> {
                    snap: SnapGrid(24.0),
                    bounds: Bounds { width: 456.0, height: 264.0 },
                    class: "floor",
                    on_drop: move |d: CanvasDrop<Part>| {
                        tape.print("canvas", format!(
                            "{} placed at ({:.0}, {:.0}), snapped to 24px grid",
                            d.payload.name, d.position.x, d.position.y,
                        ));
                        placed.write().push((d.payload, d.position));
                    },
                    for (part, pos) in placed.read().iter() {
                        span {
                            class: "placed",
                            style: "left: {pos.x}px; top: {pos.y}px;",
                            "{part.glyph}"
                        }
                    }
                    if placed.read().is_empty() {
                        span { class: "floor-empty", "The floor is empty. Place a part." }
                    }
                }
            }
        }
    }
}

// --- 06 grid ---------------------------------------------------------------------

#[component]
fn GridDemo() -> Element {
    let mut tape = use_tape();
    let mut tiles = use_signal(|| ('A'..='H').collect::<Vec<char>>());

    rsx! {
        SortableGrid {
            class: "tiles",
            len: tiles.read().len(),
            cols: 4,
            mode: ReorderMode::Swap,
            render: move |ix: usize| rsx! {
                div { class: "tile", "{tiles.read()[ix]}" }
            },
            on_sort: move |ev: SortEvent| {
                let (a, b) = (tiles.read()[ev.from], tiles.read()[ev.to]);
                apply_swap(&mut tiles.write(), ev);
                tape.print("grid", format!("{a} ⇄ {b} (cells {} and {})", ev.from + 1, ev.to + 1));
            },
        }
    }
}

// --- 07 multi-select: loaded boxes stay visible --------------------------------

#[component]
fn MultiSelectDemo() -> Element {
    let mut tape = use_tape();
    let selection = use_selection::<u32>();
    let mut shipped = use_signal(Vec::<u32>::new);
    let boxes: Vec<u32> = (1..=8).collect();

    rsx! {
        DndProvider::<Vec<u32>> {
            LiveRegion::<Vec<u32>> {}
            div { class: "dock",
                div { class: "boxes",
                    for id in boxes.iter().copied().filter(|id| !shipped.read().contains(id)) {
                        SelectableDraggable::<u32> {
                            key: "{id}",
                            item: id,
                            selection,
                            label: format!("box {id}"),
                            class: "box",
                            "#{id}"
                        }
                    }
                }
                DropZone::<Vec<u32>> {
                    label: "Outbound truck",
                    class: "bay bay-wide",
                    on_drop: move |o: DropOutcome<Vec<u32>>| {
                        tape.print("multiselect", format!(
                            "{} box(es) loaded: {:?}",
                            o.payload.len(), o.payload,
                        ));
                        shipped.write().extend(o.payload);
                        let mut selection = selection;
                        selection.clear();
                    },
                    span { class: "bay-name", "OUTBOUND TRUCK" }
                    span { class: "bay-rule",
                        if shipped.read().is_empty() { "click to select, Ctrl+click for more, then drag the group here" }
                        else { {format!("{} loaded", shipped.read().len())} }
                    }
                    div { class: "bay-stack",
                        for id in shipped.read().iter() {
                            span { class: "chip", key: "{id}", "#{id}" }
                        }
                    }
                }
            }
            DragOverlay::<Vec<u32>> { SelectionDragPreview {} }
        }
    }
}

#[component]
fn SelectionDragPreview() -> Element {
    let dnd = use_dnd::<Vec<u32>>();
    let Some(ids) = dnd.payload() else {
        return rsx! {};
    };

    rsx! {
        div { class: "selection-preview drag-preview",
            for id in ids {
                span { class: "box preview-box", key: "{id}", "#{id}" }
            }
        }
    }
}

// --- 08 files --------------------------------------------------------------------

#[component]
fn FilesDemo() -> Element {
    let mut tape = use_tape();
    let mut received = use_signal(Vec::<String>::new);
    let mut hovering = use_signal(|| false);

    rsx! {
        FileDropZone {
            class: "filebay",
            "data-hover": hovering(),
            filter: FileFilter::new()
                .content_types(["image/*"])
                .max_size(2_000_000)
                .max_files(6),
            on_hover: move |over| hovering.set(over),
            on_files: move |drop: FileDrop| {
                for f in &drop.files {
                    tape.print("files", format!("accepted {} ({} KB)", f.name(), f.size() / 1024));
                    received.write().push(f.name());
                }
            },
            on_rejected: move |bad: Vec<(dioxus::html::FileData, FileRejection)>| {
                for (f, why) in bad {
                    let why = match why {
                        FileRejection::ContentType => "not an image",
                        FileRejection::TooLarge => "over 2 MB",
                        FileRejection::TooMany => "over the 6-file limit",
                        FileRejection::Extension => "wrong extension",
                    };
                    tape.print("files", format!("refused {}: {why}", f.name()));
                }
            },
            if received.read().is_empty() {
                span { class: "filebay-prompt", "Drop images here, up to 6, 2 MB each" }
            } else {
                ul { class: "filebay-list",
                    for name in received.read().iter() {
                        li { "{name}" }
                    }
                }
            }
        }
    }
}

// --- 09 in & out: received items stay visible -----------------------------------

#[component]
fn InOutDemo() -> Element {
    let mut tape = use_tape();
    let mut received = use_signal(Vec::<String>::new);

    rsx! {
        div { class: "inout",
            ExternalDropZone {
                class: "bay bay-wide",
                on_drop: move |d: ExternalDrop| {
                    if let Some(url) = d.url() {
                        tape.print("external", format!("link received: {url}"));
                        received.write().push(url.to_string());
                    } else if let Some(text) = d.text() {
                        let short: String = text.chars().take(60).collect();
                        tape.print("external", format!("text received: \"{short}\""));
                        received.write().push(short);
                    }
                    if !d.files.is_empty() {
                        tape.print("external", format!("{} file(s) received", d.files.len()));
                    }
                },
                span { class: "bay-name", "INBOUND" }
                span { class: "bay-rule", "drop a link or text from another window" }
                div { class: "bay-stack",
                    for (ix, item) in received.read().iter().enumerate() {
                        span { class: "chip chip-wide", key: "{ix}", "{item}" }
                    }
                }
            }
            ExternalDragSource {
                content: OutboundContent::url("https://crates.io/crates/dioxus-dnd", Some("dioxus-dnd")),
                class: "outbound-tag",
                "⇱ crates.io/crates/dioxus-dnd, drag me to another tab"
            }
        }
    }
}

// --- 10 keyboard: drops land visibly ---------------------------------------------

const K_SHELF: ZoneId = ZoneId(201);
const K_NORTH: ZoneId = ZoneId(202);
const K_SOUTH: ZoneId = ZoneId(203);

#[component]
fn KeyboardDemo() -> Element {
    let mut tape = use_tape();
    let mut stock = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Crate>> = HashMap::new();
        m.insert(
            K_SHELF,
            vec![
                Crate {
                    id: 9,
                    name: "PARTS",
                    fragile: false,
                },
                Crate {
                    id: 10,
                    name: "TOOLS",
                    fragile: false,
                },
            ],
        );
        m.insert(K_NORTH, vec![]);
        m.insert(K_SOUTH, vec![]);
        m
    });
    let zone_name = |z: ZoneId| match z {
        K_SHELF => "the shelf",
        K_NORTH => "Bay North",
        _ => "Bay South",
    };
    let mut land = move |o: DropOutcome<Crate>| {
        if let Some(from) = o.from {
            if let Some(v) = stock.write().get_mut(&from) {
                v.retain(|x| x.id != o.payload.id);
            }
        }
        tape.print("a11y", format!("{} → {}", o.payload.name, zone_name(o.to)));
        stock.write().entry(o.to).or_default().push(o.payload);
    };
    let crates_in = move |zone: ZoneId| stock.read().get(&zone).cloned().unwrap_or_default();
    let steps = [
        ("Tab", "focus a crate"),
        ("Space", "pick it up"),
        ("← → ↑ ↓", "choose a bay"),
        ("Enter", "drop it"),
        ("Esc", "cancel"),
    ];

    rsx! {
        DndProvider::<Crate> {
            LiveRegion::<Crate> {}
            div { class: "keys",
                for (key, what) in steps {
                    span { class: "keycap-pair",
                        kbd { "{key}" }
                        span { "{what}" }
                    }
                }
            }
            div { class: "dock",
                for (zone, title, rule) in [
                    (K_SHELF, "SHELF", "start here"),
                    (K_NORTH, "BAY NORTH", ""),
                    (K_SOUTH, "BAY SOUTH", ""),
                ] {
                    DropZone::<Crate> {
                        id: zone,
                        label: zone_name(zone),
                        class: "bay",
                        on_drop: move |o: DropOutcome<Crate>| land(o),
                        span { class: "bay-name", "{title}" }
                        if !rule.is_empty() { span { class: "bay-rule", "{rule}" } }
                        div { class: "bay-stack",
                            for c in crates_in(zone) {
                                PointerDraggable::<Crate> {
                                    key: "{c.id}",
                                    payload: c.clone(),
                                    zone,
                                    label: c.name,
                                    class: "crate",
                                    span { class: "crate-label", "{c.name}" }
                                }
                            }
                        }
                    }
                }
            }
            KeyboardEcho {}
        }
    }
}

/// Makes the (normally screen-reader-only) announcements visible, so sighted
/// visitors can follow what assistive tech hears.
#[component]
fn KeyboardEcho() -> Element {
    let dnd = use_dnd::<Crate>();
    let msg = dnd.announcement();
    rsx! {
        p { class: "echo", "aria-hidden": "true",
            if msg.is_empty() { "Screen-reader announcements appear here." } else { "🔊 {msg}" }
        }
    }
}

// --- styles ----------------------------------------------------------------------

const CSS: &str = r#"
@import url('https://fonts.googleapis.com/css2?family=Big+Shoulders+Stencil+Display:wght@700&family=Archivo:wght@400;600&family=IBM+Plex+Mono:wght@400;500&display=swap');

:root {
    --floor: #E7E5DC;
    --bench: #FFFFFF;
    --panel: #F1EFE8;
    --ink: #23281F;
    --line: rgba(35, 40, 31, 0.18);
    --safety: #F5C518;
    --oxide: #8C3B2E;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
html, body { margin: 0; }
body { background: var(--floor); color: var(--ink); font: 15px/1.55 'Archivo', sans-serif; }
h1, h2, h3 { font-family: 'Big Shoulders Stencil Display', sans-serif; letter-spacing: 0.04em; margin: 0; }
button { font: inherit; color: inherit; }
a { color: inherit; }
:focus-visible { outline: 3px solid var(--ink); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) { * { transition: none !important; animation: none !important; scroll-behavior: auto !important; } }

/* outcome tape colour scrollbars, everywhere */
* { scrollbar-width: thin; scrollbar-color: var(--ink) transparent; }
::-webkit-scrollbar { width: 10px; height: 10px; }
::-webkit-scrollbar-thumb { background: var(--ink); border-radius: 6px; border: 2px solid var(--floor); }
::-webkit-scrollbar-thumb:hover { background: #11140F; }
::-webkit-scrollbar-track { background: transparent; }

/* page scaffolding */
.shell { max-width: 1080px; margin: 0 auto; padding: 0 1.5rem; }
.shell-wide { max-width: 1280px; margin: 0 auto; padding: 0 1.5rem; }

/* header */
.masthead { position: sticky; top: 0; z-index: 50; background: var(--bench); border-bottom: 2px solid var(--ink); }
.masthead-row { display: flex; align-items: center; justify-content: space-between; padding-top: 0.8rem; padding-bottom: 0.8rem; }
.brand { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.4rem; letter-spacing: 0.06em; text-decoration: none; }
.topnav { display: flex; gap: 1.4rem; }
.topnav a { text-decoration: none; font-weight: 600; font-size: 0.9rem; border-bottom: 2px solid transparent; padding-bottom: 2px; }
.topnav a:hover { border-bottom-color: var(--safety); }

/* hero */
.hero { padding: 4.5rem 0 3.5rem; }
.eyebrow { font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; letter-spacing: 0.25em; color: var(--ink); margin: 0 0 1rem; }
.hero-title { font-size: clamp(3rem, 8vw, 5.5rem); line-height: 0.95; }
.lede { max-width: 58ch; font-size: 1.1rem; margin: 1.4rem 0 2rem; opacity: 0.85; }
.install-row { display: flex; gap: 0.8rem; align-items: center; flex-wrap: wrap; }
.install { background: var(--ink); color: var(--floor); font-family: 'IBM Plex Mono', monospace; font-size: 0.95rem; padding: 0.7rem 1.1rem; border-radius: 6px; user-select: all; }
.install-hint { font-family: 'IBM Plex Mono', monospace; font-size: 0.7rem; opacity: 0.55; }
.cta { font-weight: 600; text-decoration: none; border-bottom: 2px solid var(--safety); margin-left: 0.4rem; }
.ticks { list-style: none; display: flex; gap: 2rem; flex-wrap: wrap; padding: 0; margin: 2.4rem 0 0; }
.ticks li { font-size: 0.9rem; opacity: 0.85; }
.tick-key { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.5rem; color: var(--ink); margin-right: 0.35rem; }

/* playground */
.playground { padding: 1rem 0 4rem; }
.playground-head { margin-bottom: 1.2rem; }
.playground-head h2 { font-size: 2rem; }
.playground-head p { margin: 0.3rem 0 0; opacity: 0.75; max-width: 60ch; }
.unit { background: var(--bench); border: 2px solid var(--ink); border-radius: 12px; box-shadow: 6px 6px 0 rgba(35, 40, 31, 0.15); overflow: hidden; }
.unit-body { display: flex; min-height: 560px; }
.manifest { width: 250px; flex-shrink: 0; border-right: 2px solid var(--ink); padding: 1rem 0; display: flex; flex-direction: column; gap: 1px; background: var(--panel); }
.manifest-head { font-family: 'IBM Plex Mono', monospace; font-size: 0.7rem; letter-spacing: 0.2em; padding: 0 1.2rem 0.6rem; opacity: 0.6; }
.manifest-line { display: grid; grid-template-columns: 2rem 1fr; gap: 0 0.4rem; text-align: left; background: none; border: none; border-left: 4px solid transparent; padding: 0.55rem 1.2rem 0.55rem 0.8rem; cursor: pointer; }
.manifest-line:hover { background: var(--bench); }
.manifest-line[data-active="true"] { border-left-color: var(--safety); background: var(--bench); }
.manifest-no { font-family: 'IBM Plex Mono', monospace; font-size: 0.8rem; opacity: 0.55; }
.manifest-title { font-weight: 600; }
.manifest-module { grid-column: 2; font-family: 'IBM Plex Mono', monospace; font-size: 0.68rem; opacity: 0.55; }

.stage { flex: 1; min-width: 0; padding: 1.4rem 1.8rem 1.8rem; }
.stage-head { display: flex; align-items: baseline; gap: 1rem; flex-wrap: wrap; }
.stage-head h3 { font-size: 1.7rem; }
.stage-module { font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; background: var(--ink); color: var(--floor); padding: 0.15rem 0.5rem; border-radius: 3px; }
.stage-hint { max-width: 62ch; opacity: 0.8; margin: 0.4rem 0 1.1rem; }
/* fixed working height: switching demos never jumps the page */
.stage-body { min-height: 380px; }
.helper-note { display: flex; align-items: center; gap: 0.65rem; flex-wrap: wrap; max-width: 680px; margin: 0 0 1rem; padding: 0.55rem 0.7rem; background: var(--panel); border-left: 4px solid var(--ink); font-size: 0.82rem; }
.helper-note code { font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; background: var(--ink); color: var(--floor); border-radius: 3px; padding: 0.12rem 0.42rem; }
.helper-note span { opacity: 0.76; }

/* tape: docked to the bottom of the unit */
.tape { border-top: 2px solid var(--ink); background: var(--ink); color: #EDEDE6; display: flex; height: 6rem; }
.tape-label { writing-mode: vertical-rl; transform: rotate(180deg); font-family: 'IBM Plex Mono', monospace; font-size: 0.62rem; letter-spacing: 0.25em; padding: 0.4rem 0.3rem; color: var(--safety); border-right: 1px dashed rgba(237,237,230,0.3); }
.tape-roll { flex: 1; overflow-y: auto; padding: 0.5rem 1rem; font-family: 'IBM Plex Mono', monospace; font-size: 0.78rem; scrollbar-color: var(--safety) var(--ink); }
.tape-roll::-webkit-scrollbar-thumb { border-color: var(--ink); }
.tape-line { display: flex; gap: 0.6rem; padding: 0.1rem 0; border-bottom: 1px dotted rgba(237,237,230,0.12); }
.tape-demo { color: var(--safety); }
.tape-empty { opacity: 0.55; }

/* footer */
.site-footer { background: var(--ink); color: var(--floor); padding: 2.4rem 0; }
.footer-row { display: flex; gap: 2.5rem; flex-wrap: wrap; align-items: flex-start; justify-content: space-between; }
.footer-brand p { margin: 0.4rem 0 0; opacity: 0.7; font-size: 0.9rem; }
.footer-links { display: flex; gap: 1.4rem; flex-wrap: wrap; }
.footer-links a { font-weight: 600; font-size: 0.9rem; text-decoration: none; border-bottom: 2px solid var(--ink); padding-bottom: 2px; }
.footer-links a:hover { border-bottom-color: var(--safety); }
.footer-meta { margin: 0; font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; opacity: 0.6; width: 100%; }

/* shared demo furniture */
.dock { display: flex; gap: 1.2rem; flex-wrap: wrap; align-items: stretch; }
.shelf { display: flex; gap: 0.8rem; flex-wrap: wrap; }
.shelf-side { flex-direction: column; }
.crate { background: var(--bench); border: 2px solid var(--ink); border-radius: 4px; padding: 0.6rem 0.9rem; cursor: grab; display: inline-flex; flex-direction: column; gap: 0.15rem; box-shadow: 3px 3px 0 var(--ink); }
.crate:active { cursor: grabbing; }
.crate-label { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.15rem; letter-spacing: 0.08em; }
.crate-tag { font-family: 'IBM Plex Mono', monospace; font-size: 0.6rem; color: var(--oxide); letter-spacing: 0.15em; }
.bay { flex: 1; min-width: 180px; min-height: 150px; border: 2px dashed var(--ink); border-radius: 6px; padding: 0.8rem; display: flex; flex-direction: column; gap: 0.3rem; background: var(--panel); }
.bay-shelf { border-style: solid; }
.bay-wide { min-width: 300px; }
.bay-name { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.1rem; letter-spacing: 0.1em; }
.bay-rule { font-family: 'IBM Plex Mono', monospace; font-size: 0.68rem; opacity: 0.6; }
.bay-stack { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.5rem; align-content: flex-start; }
.chip { background: var(--bench); border: 1.5px solid var(--ink); border-radius: 999px; padding: 0.15rem 0.6rem; font-family: 'IBM Plex Mono', monospace; font-size: 0.75rem; }
.chip-wide { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.drag-preview { opacity: 0.82; transform: rotate(-1deg); }
.selection-preview { display: flex; gap: 0.35rem; align-items: center; }
.preview-box { min-width: 48px; padding-left: 0.65rem; padding-right: 0.65rem; background: var(--bench); }

/* sortable */
.list-scroll { max-height: 320px; overflow-y: auto; max-width: 440px; border: 2px solid var(--ink); border-radius: 6px; }
.row { display: flex; align-items: center; gap: 0.6rem; padding: 0.55rem 0.9rem; background: var(--bench); cursor: grab; box-shadow: inset 0 -1px 0 var(--line); }
[data-sort-handle] { opacity: 0.4; }
.row-text { flex: 1; }
.row-buttons { display: flex; gap: 2px; opacity: 0; }
.row:hover .row-buttons, .row-buttons:focus-within { opacity: 1; }
.row-buttons button { border: 1.5px solid var(--ink); background: var(--panel); border-radius: 3px; cursor: pointer; line-height: 1; padding: 0.1rem 0.3rem; }
.row-buttons button:disabled { opacity: 0.3; cursor: default; }
[data-dragging="true"] .row { opacity: 0.68; background: var(--bench); box-shadow: inset 0 -1px 0 var(--line); }
[data-drop-target="true"] .row { background: var(--panel); }

/* board */
.board { display: flex; gap: 1rem; flex-wrap: wrap; }
.column { background: var(--panel); border: 2px solid var(--ink); border-radius: 6px; padding: 0.7rem; width: 200px; min-height: 220px; }
.column-name { font-size: 1.1rem; letter-spacing: 0.08em; margin-bottom: 0.4rem; }
.card { background: var(--bench); border: 1.5px solid var(--ink); border-radius: 4px; padding: 0.5rem 0.7rem; cursor: grab; box-shadow: 2px 2px 0 var(--ink); }
.slot { height: 8px; border-radius: 4px; margin: 2px 0; }
.slot[data-active="true"] { height: 16px; background: var(--safety); }

/* tree */
.tree { max-width: 400px; border: 2px solid var(--ink); border-radius: 6px; background: var(--bench); overflow: hidden; }
.tree-row { border-bottom: 1px solid var(--line); position: relative; }
.tree-grab { padding: 0.45rem 0.6rem; cursor: grab; font-family: 'IBM Plex Mono', monospace; font-size: 0.85rem; }
.tree-row[data-intent="before"] { box-shadow: inset 0 3px 0 var(--safety); }
.tree-row[data-intent="after"] { box-shadow: inset 0 -3px 0 var(--safety); }
.tree-row[data-intent="into"] { background: rgba(245, 197, 24, 0.25); }

/* canvas */
.canvas-wrap { display: flex; gap: 1.2rem; flex-wrap: wrap; }
.part { display: flex; gap: 0.5rem; align-items: center; background: var(--panel); border: 2px solid var(--ink); border-radius: 4px; padding: 0.4rem 0.8rem; cursor: grab; }
.part-name { font-family: 'IBM Plex Mono', monospace; font-size: 0.75rem; }
.floor { position: relative; width: 480px; height: 288px; border: 2px solid var(--ink); border-radius: 6px; background-image: radial-gradient(var(--line) 1px, transparent 1px); background-size: 24px 24px; background-color: var(--panel); }
.placed { position: absolute; font-size: 1.3rem; }
.floor-empty { position: absolute; inset: 0; display: grid; place-items: center; opacity: 0.5; font-family: 'IBM Plex Mono', monospace; font-size: 0.8rem; }

/* grid */
.tiles { gap: 10px; max-width: 380px; }
.tile { background: var(--panel); border: 2px solid var(--ink); border-radius: 6px; padding: 1.2rem 0; text-align: center; font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.5rem; cursor: grab; box-shadow: 3px 3px 0 var(--ink); }
[data-drop-target="true"] > .tile { background: var(--safety); }
[data-dragging="true"] > .tile { opacity: 0.35; }

/* multi-select */
.boxes { display: grid; grid-template-columns: repeat(4, 64px); gap: 10px; align-content: flex-start; }
.box { text-align: center; padding: 0.9rem 0; background: var(--panel); border: 2px solid var(--ink); border-radius: 4px; cursor: grab; font-family: 'IBM Plex Mono', monospace; }
[data-selected="true"] .box, [data-selected="true"].box { background: var(--ink); color: var(--floor); }

/* files */
.filebay { border: 3px dashed var(--ink); border-radius: 8px; min-height: 160px; max-width: 480px; display: grid; place-items: center; padding: 1rem; background: var(--panel); }
.filebay[data-hover="true"] { border-color: var(--ink); background: rgba(35, 40, 31, 0.08); }
.filebay-prompt { opacity: 0.6; }
.filebay-list { margin: 0; font-family: 'IBM Plex Mono', monospace; font-size: 0.85rem; }

/* in & out */
.inout { display: flex; flex-direction: column; gap: 1.2rem; align-items: flex-start; }
.outbound-tag { display: inline-block; background: var(--safety); border: 2px solid var(--ink); border-radius: 999px; padding: 0.5rem 1.1rem; cursor: grab; font-family: 'IBM Plex Mono', monospace; font-size: 0.8rem; box-shadow: 3px 3px 0 var(--ink); }

/* keyboard */
.keys { display: flex; gap: 1rem; flex-wrap: wrap; margin-bottom: 1.2rem; }
.keycap-pair { display: flex; gap: 0.4rem; align-items: center; font-size: 0.8rem; }
kbd { background: var(--ink); color: var(--floor); border-radius: 4px; padding: 0.15rem 0.5rem; font-family: 'IBM Plex Mono', monospace; font-size: 0.75rem; box-shadow: 0 2px 0 rgba(35,40,31,0.5); }
.echo { font-family: 'IBM Plex Mono', monospace; font-size: 0.85rem; background: var(--panel); border-left: 4px solid var(--ink); padding: 0.6rem 0.9rem; max-width: 480px; margin-top: 1.2rem; }

@media (max-width: 820px) {
    .unit-body { flex-direction: column; min-height: 0; }
    .manifest { width: 100%; flex-direction: row; overflow-x: auto; border-right: none; border-bottom: 2px solid var(--ink); padding: 0.4rem; }
    .manifest-head { display: none; }
    .manifest-line { border-left: none; border-bottom: 4px solid transparent; white-space: nowrap; }
    .manifest-line[data-active="true"] { border-bottom-color: var(--safety); }
    .manifest-module { display: none; }
    .stage { padding: 1rem; }
    .stage-body { min-height: 0; }
    .hero { padding: 3rem 0 2.5rem; }
    .topnav { gap: 0.9rem; }
    .floor { width: 100%; }
}
"#;

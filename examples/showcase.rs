//! The dioxus-dnd website: a landing page whose centerpiece is a live
//! playground with one interactive demo per drop pattern, plus an "outcome
//! tape" that prints every `DropOutcome` the library delivers.
//!
//! Run with:
//! ```sh
//! dx serve --example showcase --platform web --features web
//! ```
//!
//! When deploying as the website, set the page `<title>` and meta
//! description in your `index.html` / `Dioxus.toml`. The crate stays on
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
    /// docket number: unique per printed slip, including milestone notices
    seq: usize,
}

#[derive(Clone, Copy, PartialEq)]
struct Tape(Signal<Vec<TapeEntry>>, Signal<usize>, Signal<usize>);

impl Tape {
    fn print(&mut self, demo: &'static str, line: impl Into<String>) {
        let mut entries = self.0.write();
        *self.2.write() += 1;
        entries.push(TapeEntry {
            demo,
            line: line.into(),
            seq: *self.2.peek(),
        });
        *self.1.write() += 1;
        // milestone lines print into whichever section earned them
        let milestone = match *self.1.peek() {
            10 => Some("milestone: 10 drops. OSHA would like a word."),
            25 => Some("milestone: 25 drops. that crate has frequent flyer status."),
            50 => Some("milestone: 50 drops. employee of the month. it's you."),
            _ => None,
        };
        if let Some(m) = milestone {
            *self.2.write() += 1;
            entries.push(TapeEntry {
                demo,
                line: m.into(),
                seq: *self.2.peek(),
            });
        }
        let overflow = entries.len().saturating_sub(60);
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

    /// The tag each demo prints tape lines under.
    fn tag(&self) -> &'static str {
        match self {
            Demo::Core => "core",
            Demo::Sortable => "sortable",
            Demo::Board => "board",
            Demo::Tree => "tree",
            Demo::Canvas => "canvas",
            Demo::Grid => "grid",
            Demo::MultiSelect => "multiselect",
            Demo::Files => "files",
            Demo::InOut => "external",
            Demo::Keyboard => "a11y",
        }
    }

    fn hint(&self) -> &'static str {
        match self {
            Demo::Core => "Drag crates between the shelf and the bays. They really move. Hold Ctrl or Cmd while dropping to copy instead. Works with touch.",
            Demo::Sortable => "Drag a row and it slides toward its landing slot while the others make room. Twelve rows, strong opinions about their order. The arrows reorder without dragging, and the list scrolls itself near its edges.",
            Demo::Board => "Move cards between columns. Drop on the thin slots to insert at an exact position.",
            Demo::Tree => "Drop on the top of a row to place before it, the bottom to place after, the middle to nest inside. The tree restructures for real.",
            Demo::Canvas => "Drag parts from the shelf onto the floor. Positions snap to the grid and stay inside the walls.",
            Demo::Grid => "Drag one tile onto another and they trade places. The tiles have accepted their fate.",
            Demo::MultiSelect => "Click selects one crate; Ctrl+click selects more. Drag any selected crate and the whole group ships together.",
            Demo::Files => "Drop image files from your computer. It feeds on PNGs and politely declines anything over 2 MB, with the reason on the tape.",
            Demo::InOut => "Drag a link or selected text from another tab into the bay, or drag our tag out into your URL bar. The original AirDrop was a loading dock.",
            Demo::Keyboard => "No mouse needed: Tab to a crate, press Space, pick a bay with the arrows, press Enter, and watch it land. Crates in bays stay focusable, so you can keep moving them. The forklift has arrow keys.",
        }
    }
}

// --- app shell --------------------------------------------------------------

#[component]
fn App() -> Element {
    use_context_provider(|| Tape(Signal::new(Vec::new()), Signal::new(0), Signal::new(0)));

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
                    a {
                        class: "ver-chip",
                        href: "https://github.com/kindintelligence/dioxus-dnd/releases",
                        "v1.0.0"
                    }
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
            div { class: "shell hero-grid",
                p { class: "eyebrow", "A DRAG-AND-DROP LIBRARY FOR DIOXUS 0.8+" }
                h1 { class: "hero-title", "PICK, DROP & SHIP" }
                div { class: "hero-copy",
                p { class: "lede",
                    "Sortable lists, boards, trees, grids, canvases, OS file drops, "
                    "multi-select, drag-out to other apps, with touch and keyboard "
                    "handled for you. Any payload type, no serialization, no JS."
                }
                div { class: "install-row",
                    // `user-select: all`: one click selects the whole command
                    code { class: "install", title: "Click to select, then copy", "cargo add dioxus-dnd" }
                    span { class: "install-hint", "click to select" }
                }
                }
                div { class: "hero-cta-panel",
                    a { class: "cta cta-large", href: "#playground", "Try it live ↓" }
                }
                ul { class: "ticks", aria_label: "Highlights",
                    li { span { class: "tick-key", "12" } " drop patterns, one small core" }
                    li { span { class: "tick-key", "3" } " input methods: mouse, touch, keyboard" }
                    li { span { class: "tick-key", "50" } " tests, zero warnings, zero extra deps" }
                    li { span { class: "tick-key", "OS" } " file drops in, links dragged out" }
                }
            }
        }
    }
}

#[component]
fn Playground() -> Element {
    rsx! {
        section { class: "playground", id: "playground",
            div { class: "shell-wide",
                div { class: "playground-head",
                    div { class: "playground-head-copy",
                    h2 { "THE LOADING DOCK" }
                    p { "Every demo is the real library. Drag something, and its DropOutcome prints on that demo's tape." }
                    nav { class: "demo-index", aria_label: "Demos",
                        for (ix, demo) in Demo::ALL.iter().enumerate() {
                            a { class: "demo-index-pill", href: "#demo-{ix + 1}",
                                span { class: "demo-index-no", {format!("{:02}", ix + 1)} }
                                "{demo.title()}"
                            }
                        }
                        span { class: "demo-index-kbd", "⌨ most demos work with Tab + Space too" }
                    }
                    }
                    DropSign {}
                }
            }
            for (ix, demo) in Demo::ALL.iter().enumerate() {
                DemoSection { demo: *demo, index: ix }
            }
        }
    }
}

/// One full-viewport demo panel. Copy and stage alternate sides down the
/// page; each section owns a generation counter so RESET remounts just its
/// demo (keyed single-item list, so the keyed diff replaces the subtree).
#[component]
fn DemoSection(demo: Demo, index: usize) -> Element {
    let mut generation = use_signal(|| 0u32);

    rsx! {
        section { class: "demo-section", id: "demo-{index + 1}",
            div { class: "shell-wide demo-grid",
                div { class: "demo-copy",
                    span { class: "demo-no", {format!("{:02}", index + 1)} }
                    h3 { "{demo.title()}" }
                    code { class: "stage-module", "dioxus_dnd::{demo.module()}" }
                    p { class: "stage-hint", "{demo.hint()}" }
                    DocketPrinter { tag: demo.tag() }
                }
                div { class: "demo-stage",
                    button {
                        class: "stage-reset",
                        title: "Restart this demo with fresh state",
                        onclick: move |_| generation += 1,
                        "RESET"
                    }
                    for stage_key in [format!("{}-{}", index, generation())] {
                        div { key: "{stage_key}",
                            match demo {
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
                    "MIT or Apache-2.0 · built with Dioxus 0.8 · no cookies, no tracking, just crates"
                }
            }
        }
    }
}

/// The signature element: a receipt of every drop the library delivered.
#[component]
fn DocketPrinter(tag: &'static str) -> Element {
    let tape = use_tape();
    let recent: Vec<(usize, String)> = tape
        .0
        .read()
        .iter()
        .rev()
        .filter(|e| e.demo == tag)
        .take(2)
        .map(|e| (e.seq, e.line.clone()))
        .collect();

    rsx! {
        div { class: "docket", aria_live: "polite",
            div { class: "docket-head",
                span { class: "docket-screw" }
                span { class: "docket-brand", "DOCKETS" }
                span { class: "docket-screw" }
            }
            div { class: "docket-out",
                // the pile: previous slip peeks out behind the fresh one
                if let Some((seq, line)) = recent.get(1) {
                    div { class: "slip slip-back",
                        div { class: "slip-top",
                            span { class: "slip-no", {format!("No. {seq:04}")} }
                            span { class: "slip-tag", "{tag}" }
                        }
                        span { class: "slip-line", "{line}" }
                        span { class: "slip-code" }
                    }
                }
                if let Some((seq, line)) = recent.first() {
                    for k in [*seq] {
                        div { class: "slip slip-front", key: "{k}",
                            div { class: "slip-top",
                                span { class: "slip-no", {format!("No. {seq:04}")} }
                                span { class: "slip-tag", "{tag}" }
                            }
                            span { class: "slip-line", "{line}" }
                            span { class: "slip-code" }
                        }
                    }
                } else {
                    div { class: "slip slip-front",
                        div { class: "slip-top",
                            span { class: "slip-no", "No. 0000" }
                            span { class: "slip-tag", "{tag}" }
                        }
                        span { class: "slip-line", "test print. printer ok. awaiting first drop." }
                        span { class: "slip-code" }
                    }
                }
            }
        }
    }
}

/// The classic factory safety sign, proudly resetting on every drop.
#[component]
fn DropSign() -> Element {
    let tape = use_tape();
    let total = *tape.1.read();
    let days = if total == 0 { "1" } else { "0" };

    rsx! {
        div { class: "drop-sign", aria_label: "Days without a dropped crate: {days}",
            span { class: "drop-sign-caption", "DAYS WITHOUT" }
            span { class: "drop-sign-caption", "A DROPPED CRATE" }
            // keyed by the running total so the digit re-stamps on each drop
            for k in [total] {
                span { class: "drop-sign-digit", key: "{k}", "{days}" }
            }
            span { class: "drop-sign-total", "TOTAL DROPS: {total}" }
        }
    }
}

// --- the core demo's freight ------------------------------------------------

const SHELF: ZoneId = ZoneId(11);
const BAY_A: ZoneId = ZoneId(12);
const BAY_B: ZoneId = ZoneId(13);

#[derive(Clone, PartialEq)]
struct Crate {
    id: u32,
    name: &'static str,
    fragile: bool,
}

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

    // (seq, zone, word): a rubber stamp lands on the receiving zone per drop.
    let mut stamp = use_signal(|| (0u64, ZoneId(0), ""));
    let stamp_for = move |zone: ZoneId| -> Element {
        let (seq, z, word) = stamp();
        if z == zone && seq > 0 {
            rsx! {
                for k in [seq] {
                    span { class: "stamp", key: "{k}", "{word}" }
                }
            }
        } else {
            rsx! {}
        }
    };

    // One landing routine for every zone: Copy duplicates, Move relocates.
    let mut land = move |o: DropOutcome<Crate>, to_name: &'static str| {
        let effect_was_copy = o.effect == DropEffect::Copy;
        let mut c = o.payload.clone();
        if o.effect == DropEffect::Copy {
            c.id = next_id();
            next_id += 1;
            tape.print(
                "core",
                format!("{} copied → {to_name} (Ctrl/Cmd held)", c.name),
            );
        } else {
            if let Some(from) = o.from {
                if let Some(v) = stock.write().get_mut(&from) {
                    v.retain(|x| x.id != o.payload.id);
                }
            }
            tape.print("core", format!("{} moved → {to_name}", c.name));
        }
        stock.write().entry(o.to).or_default().push(c);
        let word = if effect_was_copy {
            "COPIED"
        } else {
            "DELIVERED"
        };
        let next_seq = stamp.peek().0 + 1;
        stamp.set((next_seq, o.to, word));
    };

    let crates_in = move |zone: ZoneId| stock.read().get(&zone).cloned().unwrap_or_default();

    rsx! {
        DndProvider::<Crate> {
            LiveRegion::<Crate> {}
            div { class: "dock",
                DropZone::<Crate> {
                    id: SHELF,
                    label: "the shelf",
                    class: "bay bay-shelf",
                    on_drop: move |o: DropOutcome<Crate>| land(o, "the shelf"),
                    span { class: "bay-name", "SHELF" }
                    {stamp_for(SHELF)}
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
                    {stamp_for(BAY_A)}
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
                    span { class: "bay-rule", "padded walls, precious things only" }
                    {stamp_for(BAY_B)}
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
            DragOverlay::<Crate> { div { class: "ghost", "▣" } }
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
            DragOverlay::<Vec<u32>> {
                div { class: "ghost", SelectionCount::<u32> {} }
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
    let mut refuse_seq = use_signal(|| 0u64);

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
                    refuse_seq += 1;
                }
            },
            if received.read().is_empty() {
                span { class: "filebay-prompt", "Drop images here, up to 6, 2 MB each" }
                if refuse_seq() > 0 {
                    for k in [refuse_seq()] {
                        span { class: "stamp stamp-refused", key: "{k}", "REFUSED" }
                    }
                }
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
            div { class: "keyboard-layout",
                for (zone, title, rule) in [
                    (K_SHELF, "SHELF", "start here"),
                    (K_NORTH, "BAY NORTH", ""),
                    (K_SOUTH, "BAY SOUTH", ""),
                ] {
                    DropZone::<Crate> {
                        id: zone,
                        label: zone_name(zone),
                        class: if zone == K_SHELF { "bay keyboard-bay keyboard-shelf" } else { "bay keyboard-bay" },
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
    --teal: #23281F;
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
:focus-visible { outline: 3px solid var(--teal); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) { * { transition: none !important; animation: none !important; scroll-behavior: auto !important; } }

/* teal scrollbars, everywhere */
* { scrollbar-width: thin; scrollbar-color: var(--teal) transparent; }
::-webkit-scrollbar { width: 10px; height: 10px; }
::-webkit-scrollbar-thumb { background: var(--teal); border-radius: 6px; border: 2px solid var(--floor); }
::-webkit-scrollbar-thumb:hover { background: #0a5850; }
::-webkit-scrollbar-track { background: transparent; }

/* page scaffolding */
.shell { max-width: 1080px; margin: 0 auto; padding: 0 1.5rem; }
.shell-wide { max-width: 1280px; margin: 0 auto; padding: 0 1.5rem; }

/* header */
.masthead { position: sticky; top: 0; z-index: 50; background: var(--bench); border-bottom: 2px solid var(--ink); }
.masthead-row { display: flex; align-items: center; justify-content: space-between; padding-top: 1.1rem; padding-bottom: 1.1rem; }
.brand { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.4rem; letter-spacing: 0.06em; text-decoration: none; }
.topnav { display: flex; gap: 1.75rem; align-items: center; }
.topnav a { text-decoration: none; font-weight: 600; font-size: 0.9rem; border-bottom: 2px solid transparent; padding-bottom: 2px; }
.topnav a:hover { border-bottom-color: var(--safety); }

/* hero */
.hero { padding: 4rem 0 4.5rem; }
.hero-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(300px, 0.9fr);
  grid-template-areas:
    "eyebrow eyebrow"
    "title   title"
    "copy    action"
    "ticks   ticks";
  column-gap: 4.5rem;
  align-items: start;
}
.hero-grid > .eyebrow { grid-area: eyebrow; }
.hero-grid > .hero-title { grid-area: title; margin: 0 0 2.25rem; }
.hero-copy { grid-area: copy; min-width: 0; display: flex; flex-direction: column; }
.hero-cta-panel { grid-area: action; align-self: end; justify-self: center; padding-bottom: 0.75rem; }
.hero-crate { padding: 0.9rem 1.2rem; gap: 0.3rem; }
.hero-crate .crate-label { font-size: 1.3rem; }
.crate-sub { font-family: 'IBM Plex Mono', monospace; font-size: 0.6rem; letter-spacing: 0.08em; opacity: 0.6; }
section[id] { scroll-margin-top: 5.5rem; }
.ver-chip { font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; border: 1px solid var(--ink); border-radius: 999px; padding: 0.15rem 0.6rem; }
.stage-reset { position: absolute; top: 0.85rem; left: 0.85rem; z-index: 10; font-family: 'IBM Plex Mono', monospace; font-size: 0.68rem; letter-spacing: 0.15em; background: none; border: 1px solid var(--ink); border-radius: 4px; padding: 0.25rem 0.7rem; cursor: pointer; }
.stage-reset:hover { background: var(--safety); }
.demo-stage { position: relative; padding: 3.1rem 0.85rem 0.85rem; user-select: none; -webkit-user-select: none; }
.eyebrow { font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; letter-spacing: 0.25em; color: var(--teal); margin: 0 0 1.25rem; }
.hero-title { font-size: clamp(2.75rem, 7vw, 4.75rem); line-height: 0.92; }
.lede { max-width: 52ch; font-size: 1.12rem; line-height: 1.6; margin: 0 0 auto; opacity: 0.85; padding-bottom: 2rem; }
.install-row { display: flex; gap: 0.8rem; align-items: center; flex-wrap: wrap; }
.install { background: var(--ink); color: var(--floor); font-family: 'IBM Plex Mono', monospace; font-size: 0.95rem; padding: 0.7rem 1.1rem; border-radius: 6px; user-select: all; }
.install-hint { font-family: 'IBM Plex Mono', monospace; font-size: 0.7rem; opacity: 0.55; }
.cta { font-weight: 600; text-decoration: none; border-bottom: 2px solid var(--safety); margin-left: 0.4rem; }
.cta-large { font-size: 1.12rem; }
.ticks { grid-area: ticks; list-style: none; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 1.25rem 2rem; padding: 1.75rem 0 0; margin: 3rem 0 0; border-top: 1px dashed rgba(35, 40, 31, 0.3); }
.ticks li { display: grid; grid-template-columns: 2.4rem 1fr; column-gap: 0.65rem; align-items: baseline; }
.ticks li { font-size: 0.9rem; opacity: 0.85; }
.tick-key { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.5rem; color: var(--teal); text-align: right; line-height: 1; }

/* playground */
.playground { padding: 4rem 0 0; }
.demo-index { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; margin-top: 1.75rem; }
.demo-index-pill { display: inline-flex; gap: 0.4rem; align-items: baseline; font-size: 0.82rem; font-weight: 600; text-decoration: none; border: 1px solid var(--ink); border-radius: 999px; padding: 0.3rem 0.85rem; background: var(--bench); }
.demo-index-pill:hover { background: var(--safety); }
.demo-index-no { font-family: 'IBM Plex Mono', monospace; font-size: 0.68rem; opacity: 0.55; }
.demo-index-kbd { font-family: 'IBM Plex Mono', monospace; font-size: 0.62rem; opacity: 0.5; flex-basis: 100%; margin-top: 0.4rem; }
.demo-section { padding: 4rem 0; border-top: 1px dashed rgba(35, 40, 31, 0.25); }
.demo-section:first-of-type { border-top: none; margin-top: 2.5rem; }
.demo-section:nth-of-type(even) { background: var(--bench); }
.demo-grid { display: grid; grid-template-columns: minmax(300px, 0.72fr) minmax(0, 1.28fr); gap: 4rem; align-items: center; width: 100%; }
/* criss-cross: copy flips to the right on even sections */
.demo-section:nth-of-type(even) .demo-grid { grid-template-columns: minmax(0, 1.28fr) minmax(300px, 0.72fr); }
.demo-section:nth-of-type(even) .demo-copy { order: 2; }
.demo-copy { display: flex; flex-direction: column; }
.demo-copy h3 { font-size: 2.1rem; margin: 0.5rem 0 0.75rem; }
.demo-no { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 3.5rem; line-height: 1; color: var(--safety); -webkit-text-stroke: 1px var(--ink); display: block; }
.demo-copy .stage-hint { margin: 1rem 0 1.5rem; }
.demo-stage { min-width: 0; }
.playground-head { margin-bottom: 1rem; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 3rem 4rem; align-items: start; }
.playground-head-copy { min-width: 0; }
.playground-head h2 { font-size: clamp(2.2rem, 4.5vw, 3rem); }
.playground-head p { margin: 0.9rem 0 0; opacity: 0.75; max-width: 56ch; font-size: 1.05rem; }

.stage-module { font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; background: var(--ink); color: var(--floor); padding: 0.15rem 0.5rem; border-radius: 3px; align-self: flex-start; }
.stage-hint { max-width: 46ch; opacity: 0.8; margin: 1rem 0 1.5rem; }
/* fixed working height: switching demos never jumps the page */

/* tape: docked to the bottom of the unit */
/* the docket printer: every drop prints a numbered receipt */
.docket { margin-top: 1.75rem; max-width: 300px; }
.docket-head { position: relative; z-index: 2; display: flex; align-items: center; justify-content: space-between; background: var(--ink); color: #EDEDE6; border-radius: 8px 8px 3px 3px; padding: 0.5rem 0.8rem; box-shadow: 0 3px 0 rgba(35, 40, 31, 0.35), 5px 5px 0 rgba(35, 40, 31, 0.15); }
.docket-head::after { content: ""; position: absolute; left: 10%; right: 10%; bottom: -2px; height: 3px; background: #0f120d; border-radius: 2px; }
.docket-brand { font-family: 'IBM Plex Mono', monospace; font-size: 0.58rem; letter-spacing: 0.3em; color: var(--safety); }
.docket-screw { width: 6px; height: 6px; border-radius: 50%; background: rgba(237, 237, 230, 0.35); box-shadow: inset 0 1px 1px rgba(0, 0, 0, 0.6); }
.docket-out { position: relative; display: grid; padding: 0 12px; overflow: hidden; }
.slip { grid-area: 1 / 1; background: var(--bench); border: 1px solid rgba(35, 40, 31, 0.22); border-top: none; padding: 0.65rem 0.85rem 1.1rem; font-family: 'IBM Plex Mono', monospace; font-size: 0.7rem; display: grid; gap: 0.45rem; box-shadow: 0 5px 10px rgba(35, 40, 31, 0.12); clip-path: polygon(0 0, 100% 0, 100% calc(100% - 7px), 95.8% 100%, 91.6% calc(100% - 7px), 87.5% 100%, 83.3% calc(100% - 7px), 79.1% 100%, 75% calc(100% - 7px), 70.8% 100%, 66.6% calc(100% - 7px), 62.5% 100%, 58.3% calc(100% - 7px), 54.1% 100%, 50% calc(100% - 7px), 45.8% 100%, 41.6% calc(100% - 7px), 37.5% 100%, 33.3% calc(100% - 7px), 29.1% 100%, 25% calc(100% - 7px), 20.8% 100%, 16.6% calc(100% - 7px), 12.5% 100%, 8.3% calc(100% - 7px), 4.1% 100%, 0 calc(100% - 7px)); }
.slip-back { transform: translateY(10px) rotate(1.8deg) scale(0.97); opacity: 0.45; z-index: 0; }
.slip-front { position: relative; z-index: 1; animation: print-out 480ms cubic-bezier(0.2, 0.85, 0.3, 1); }
@keyframes print-out { from { transform: translateY(-60%); } to { transform: translateY(0); } }
.slip-top { display: flex; justify-content: space-between; gap: 0.6rem; padding-bottom: 0.4rem; border-bottom: 1px dashed rgba(35, 40, 31, 0.35); }
.slip-no { font-weight: 700; letter-spacing: 0.05em; }
.slip-tag { text-transform: uppercase; font-size: 0.58rem; letter-spacing: 0.2em; opacity: 0.55; }
.slip-line { overflow-wrap: anywhere; line-height: 1.5; }
.slip-code { height: 13px; width: 62%; background: repeating-linear-gradient(90deg, var(--ink) 0 2px, transparent 2px 5px, var(--ink) 5px 6px, transparent 6px 10px, var(--ink) 10px 13px, transparent 13px 16px); opacity: 0.85; }

/* the safety sign that proudly resets */
.drop-sign { justify-self: end; flex: none; display: grid; justify-items: center; background: var(--ink); color: #EDEDE6; border-radius: 8px; padding: 1.1rem 1.6rem 0.9rem; box-shadow: 6px 6px 0 rgba(35, 40, 31, 0.2); transform: rotate(-1.2deg); }
.drop-sign-caption { font-family: 'IBM Plex Mono', monospace; font-size: 0.58rem; letter-spacing: 0.22em; opacity: 0.75; }
.drop-sign-digit { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 3.4rem; line-height: 1.05; color: var(--safety); animation: sign-pop 320ms ease-out; }
.drop-sign-total { font-family: 'IBM Plex Mono', monospace; font-size: 0.58rem; letter-spacing: 0.15em; opacity: 0.6; margin-top: 0.45rem; padding-top: 0.45rem; border-top: 1px dashed rgba(237, 237, 230, 0.3); }
@keyframes sign-pop { from { transform: scale(1.7); } to { transform: scale(1); } }

/* rubber stamps on receiving zones */
.stamp { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%) rotate(-14deg); font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.7rem; letter-spacing: 0.12em; color: var(--oxide); border: 3px solid var(--oxide); border-radius: 4px; padding: 0.05rem 0.55rem; background: rgba(255, 255, 255, 0.7); pointer-events: none; opacity: 0; animation: stamp-in 1.8s ease-out forwards; z-index: 5; }
@keyframes stamp-in {
  0% { opacity: 0; transform: translate(-50%, -50%) rotate(-14deg) scale(2.3); }
  22% { opacity: 1; transform: translate(-50%, -50%) rotate(-14deg) scale(1); }
  72% { opacity: 1; }
  100% { opacity: 0; }
}

/* footer */
.site-footer { background: var(--ink); color: var(--floor); padding: 4rem 0; margin-top: 4rem; }
.footer-row { display: flex; gap: 2.5rem; flex-wrap: wrap; align-items: flex-start; justify-content: space-between; }
.footer-brand p { margin: 0.4rem 0 0; opacity: 0.7; font-size: 0.9rem; }
.footer-links { display: flex; gap: 1.4rem; flex-wrap: wrap; }
.footer-links a { font-weight: 600; font-size: 0.9rem; text-decoration: none; border-bottom: 2px solid var(--teal); padding-bottom: 2px; }
.footer-links a:hover { border-bottom-color: var(--safety); }
.footer-meta { margin: 1.5rem 0 0; font-family: 'IBM Plex Mono', monospace; font-size: 0.72rem; opacity: 0.6; width: 100%; }

/* shared demo furniture */
.dock { display: flex; gap: 1.2rem; flex-wrap: wrap; align-items: stretch; }
.shelf { display: flex; gap: 0.8rem; flex-wrap: wrap; }
.shelf-side { flex-direction: column; }
.crate { background: var(--bench); border: 2px solid var(--ink); border-radius: 4px; padding: 0.6rem 0.9rem; cursor: grab; display: inline-flex; flex-direction: column; gap: 0.15rem; box-shadow: 3px 3px 0 var(--ink); }
.crate:active { cursor: grabbing; }
.crate-label { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.15rem; letter-spacing: 0.08em; }
.crate-tag { font-family: 'IBM Plex Mono', monospace; font-size: 0.6rem; color: var(--oxide); letter-spacing: 0.15em; margin-left: 0.45em; }
.bay { position: relative; flex: 1; min-width: 180px; min-height: 150px; border: 2px dashed var(--ink); border-radius: 6px; padding: 0.8rem; display: flex; flex-direction: column; gap: 0.3rem; background: var(--panel); }
.bay-shelf { border-style: solid; }
.bay-wide { min-width: 300px; }
.bay-name { font-family: 'Big Shoulders Stencil Display', sans-serif; font-size: 1.1rem; letter-spacing: 0.1em; }
.bay-rule { font-family: 'IBM Plex Mono', monospace; font-size: 0.68rem; opacity: 0.6; }
.bay-stack { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.5rem; align-content: flex-start; }
.chip { background: var(--bench); border: 1.5px solid var(--ink); border-radius: 999px; padding: 0.15rem 0.6rem; font-family: 'IBM Plex Mono', monospace; font-size: 0.75rem; }
.chip-wide { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ghost { background: var(--safety); border: 2px solid var(--ink); border-radius: 4px; padding: 0.3rem 0.7rem; font-weight: 600; box-shadow: 5px 7px 0 var(--ink); transform: rotate(-4deg); }

/* sortable */
.list-scroll { max-height: 320px; overflow-y: auto; max-width: 440px; border: 2px solid var(--ink); border-radius: 6px; }
.row { display: flex; flex: 1; align-items: center; gap: 0.6rem; padding: 0.55rem 0.9rem; border-bottom: 1px solid var(--line); background: var(--bench); cursor: grab; }
[data-sort-handle] { opacity: 0.55; background: var(--bench); border-bottom: 1px solid var(--line); }
.row-text { flex: 1; }
.row-buttons { display: flex; gap: 2px; opacity: 0; }
.row:hover .row-buttons, .row-buttons:focus-within { opacity: 1; }
.row-buttons button { border: 1.5px solid var(--ink); background: var(--panel); border-radius: 3px; cursor: pointer; line-height: 1; padding: 0.1rem 0.3rem; }
.row-buttons button:disabled { opacity: 0.3; cursor: default; }
[data-dragging="true"] .row { opacity: 0.45; background: var(--panel); box-shadow: inset 0 0 0 2px var(--safety); }
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
[data-selected="true"] .box, [data-selected="true"].box { background: var(--teal); color: var(--floor); }

/* files */
.filebay { position: relative; border: 3px dashed var(--ink); border-radius: 8px; min-height: 160px; max-width: 480px; display: grid; place-items: center; padding: 1rem; background: var(--panel); }
.filebay[data-hover="true"] { border-color: var(--teal); background: rgba(14, 107, 99, 0.08); }
.filebay-prompt { opacity: 0.6; }
.filebay-list { margin: 0; font-family: 'IBM Plex Mono', monospace; font-size: 0.85rem; }

/* in & out */
.inout { display: flex; flex-direction: column; gap: 1.2rem; align-items: flex-start; }
.outbound-tag { display: inline-block; background: var(--safety); border: 2px solid var(--ink); border-radius: 999px; padding: 0.5rem 1.1rem; cursor: grab; font-family: 'IBM Plex Mono', monospace; font-size: 0.8rem; box-shadow: 3px 3px 0 var(--ink); }

/* keyboard */
.keys { display: flex; flex-wrap: wrap; gap: 0.45rem 0.8rem; align-items: center; margin-bottom: 0.9rem; max-width: 680px; }
.keycap-pair { display: inline-flex; gap: 0.35rem; align-items: center; font-size: 0.76rem; white-space: nowrap; }
kbd { background: var(--ink); color: var(--floor); border-radius: 4px; padding: 0.15rem 0.5rem; font-family: 'IBM Plex Mono', monospace; font-size: 0.75rem; box-shadow: 0 2px 0 rgba(35,40,31,0.5); }
.keyboard-layout { display: grid; grid-template-columns: minmax(190px, 0.72fr) minmax(250px, 1fr); grid-template-rows: repeat(2, minmax(108px, auto)); gap: 0.85rem; align-items: stretch; max-width: 680px; }
.keyboard-shelf { grid-column: 1; grid-row: 1 / span 2; min-height: 0; }
.keyboard-bay { min-width: 0; min-height: 108px; }
.keyboard-bay .bay-stack { min-height: 42px; }
.echo { font-family: 'IBM Plex Mono', monospace; font-size: 0.82rem; background: var(--panel); border-left: 4px solid var(--teal); padding: 0.65rem 0.85rem; max-width: 680px; min-height: 42px; margin-top: 0.95rem; }

@media (max-width: 900px) {
    .demo-grid { grid-template-columns: 1fr; gap: 2rem; }
    .demo-section:nth-of-type(even) .demo-copy { order: 0; }
    .demo-section { padding: 3rem 0; }
    .hero-grid { grid-template-columns: 1fr; grid-template-areas: "eyebrow" "title" "copy" "action" "ticks"; row-gap: 2rem; }
    .ticks { grid-template-columns: 1fr 1fr; margin-top: 0; }
    .hero-grid > .hero-title { margin-bottom: 0; }
    .lede { padding-bottom: 0; }
    .playground-head { grid-template-columns: 1fr; }
    .drop-sign { justify-self: start; }
    .topnav { gap: 0.9rem; flex-wrap: wrap; }
    .keyboard-layout { grid-template-columns: 1fr; }
    .keyboard-shelf { grid-column: auto; grid-row: auto; }
    .floor { width: 100%; }
}
"#;

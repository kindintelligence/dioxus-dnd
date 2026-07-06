//! A clean, Tailwind (shadcn-flavoured) gallery of every dioxus-dnd pattern -
//! all on the **web pointer path** (`input: DragInputMode::Pointer`), with
//! standard, no-frills drag behaviour: the dragged item dims, drop targets
//! highlight, and the sortable list shows a caller-composed overlay.
//!
//! Run:
//! ```sh
//! dx serve --example gallery --platform web --features web
//! ```
//! (The `web` feature enables native pointer capture so mouse drags stay glued
//! to the pointer. Touch and pen work either way.)

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

fn main() {
    dioxus::launch(App);
}

// shadcn-ish tokens, reused everywhere.
const ITEM: &str = "flex items-center gap-2 cursor-grab select-none rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 shadow-sm transition data-dragging:opacity-50 data-dragging:ring-2 data-dragging:ring-slate-300";
const ZONE: &str = "rounded-lg border-2 border-dashed border-slate-200 p-3 min-h-24 transition space-y-2 data-active:border-slate-300 data-over:border-slate-900 data-over:bg-slate-100/60";

#[component]
fn App() -> Element {
    rsx! {
        document::Script { src: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" }
        div { class: "min-h-screen bg-slate-50 text-slate-900 antialiased",
            div { class: "mx-auto max-w-5xl px-6 py-14 space-y-8",
                header { class: "space-y-1",
                    p { class: "text-sm font-medium text-slate-500", "dioxus-dnd" }
                    h1 { class: "text-3xl font-semibold tracking-tight", "Drag & drop gallery" }
                    p { class: "text-sm text-slate-500",
                        "Every pattern, on the web pointer path. Clean, standard behaviour - grab, drag, drop."
                    }
                }
                CardsDemo {}
                SortableDemo {}
                GridDemo {}
                BoardDemo {}
                TreeDemo {}
                CanvasDemo {}
                MultiSelectDemo {}
                FilesDemo {}
                ExternalDemo {}
            }
        }
    }
}

#[component]
fn Section(title: String, note: String, children: Element) -> Element {
    rsx! {
        section { class: "rounded-xl border border-slate-200 bg-white p-6 shadow-sm",
            div { class: "mb-4",
                h2 { class: "text-base font-semibold", "{title}" }
                p { class: "mt-0.5 text-sm text-slate-500", "{note}" }
            }
            {children}
        }
    }
}

// --- 1. cards between zones (core Draggable/DropZone + overlay) ---------------

#[derive(Clone, PartialEq)]
struct Card {
    id: u32,
    title: String,
}

const TODO: ZoneId = ZoneId(1);
const DONE: ZoneId = ZoneId(2);

#[component]
fn CardsDemo() -> Element {
    let mut bins = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Card>> = HashMap::new();
        m.insert(
            TODO,
            vec![
                Card {
                    id: 1,
                    title: "Design the API".into(),
                },
                Card {
                    id: 2,
                    title: "Write the docs".into(),
                },
                Card {
                    id: 3,
                    title: "Ship it".into(),
                },
            ],
        );
        m.insert(DONE, vec![]);
        m
    });
    let move_card = move |o: DropOutcome<Card>| {
        let mut b = bins.write();
        for cards in b.values_mut() {
            cards.retain(|c| c.id != o.payload.id);
        }
        b.entry(o.to).or_default().push(o.payload);
    };

    rsx! {
        Section { title: "Cards", note: "Move cards between two zones. The ghost follows the cursor.",
            DndProvider::<Card> {
                div { class: "grid grid-cols-2 gap-4",
                    for (name, zone) in [("To do", TODO), ("Done", DONE)] {
                        DropZone::<Card> {
                            id: zone,
                            label: name,
                            on_drop: move_card,
                            class: ZONE,
                            p { class: "text-xs font-medium uppercase tracking-wide text-slate-400", "{name}" }
                            for card in bins.read().get(&zone).cloned().unwrap_or_default() {
                                PointerDraggable::<Card> {
                                    payload: card.clone(),
                                    zone,
                                    input: DragInputMode::Pointer,
                                    label: card.title.clone(),
                                    class: ITEM,
                                    "{card.title}"
                                }
                            }
                        }
                    }
                }
                DragOverlay::<Card> {
                    class: "pointer-events-none rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm shadow-lg",
                    CardGhost {}
                }
            }
        }
    }
}

#[component]
fn CardGhost() -> Element {
    let dnd = use_dnd::<Card>();
    let title = dnd.payload().map(|c| c.title).unwrap_or_default();
    rsx! { "{title}" }
}

// --- 2. sortable list --------------------------------------------------------

#[component]
fn SortableDemo() -> Element {
    let mut items = use_signal(|| {
        ["Research", "Draft", "Review", "Revise", "Publish"]
            .map(String::from)
            .to_vec()
    });
    rsx! {
        Section { title: "Sortable list", note: "Grab a row and drag to reorder. Rows slide to make room.",
            SortableList {
                len: items.read().len(),
                input: DragInputMode::Pointer,
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                class: "relative overflow-hidden [&>*]:mb-2 [&>*]:flex [&>*]:items-center [&>*]:rounded-lg [&>*]:border [&>*]:border-slate-200 [&>*]:bg-white [&>*]:px-3 [&>*]:py-2 [&>*]:text-sm [&>*]:cursor-grab [&>*]:select-none [&>*]:shadow-sm [&>*]:transition [&>[data-dragging]]:opacity-35 [&>[data-drop-target]]:border-slate-300 [&>[data-drop-target]]:bg-slate-50",
                overlay: move |ix: usize| rsx! { "{items.read()[ix]}" },
                render: move |ix: usize| rsx! { "{items.read()[ix]}" },
            }
        }
    }
}

// --- 3. sortable grid --------------------------------------------------------

#[component]
fn GridDemo() -> Element {
    let mut tiles = use_signal(|| (1..=9).map(|n| format!("Tile {n}")).collect::<Vec<_>>());
    rsx! {
        Section { title: "Grid", note: "Reorder tiles in two dimensions.",
            SortableGrid {
                len: tiles.read().len(),
                cols: 3,
                input: DragInputMode::Pointer,
                on_sort: move |ev: SortEvent| apply_sort(&mut tiles.write(), ev),
                class: "gap-2",
                item_class: "flex items-center justify-center rounded-lg border border-slate-200 bg-white p-6 text-sm text-slate-700 cursor-grab select-none shadow-sm transition data-dragging:opacity-50 data-drop-target:border-slate-900 data-drop-target:ring-2 data-drop-target:ring-slate-900".to_string(),
                render: move |ix: usize| rsx! { "{tiles.read()[ix]}" },
            }
        }
    }
}

// --- 4. board (kanban) -------------------------------------------------------

const BACKLOG: ContainerId = ZoneId(10);
const DOING: ContainerId = ZoneId(11);
const SHIPPED: ContainerId = ZoneId(12);

#[component]
fn BoardDemo() -> Element {
    let mut board = use_signal(|| {
        let mut m: HashMap<ContainerId, Vec<Card>> = HashMap::new();
        m.insert(
            BACKLOG,
            vec![
                Card {
                    id: 1,
                    title: "Sketch UI".into(),
                },
                Card {
                    id: 2,
                    title: "Model data".into(),
                },
            ],
        );
        m.insert(
            DOING,
            vec![Card {
                id: 3,
                title: "Wire events".into(),
            }],
        );
        m.insert(SHIPPED, vec![]);
        m
    });
    rsx! {
        Section { title: "Board", note: "Move cards across columns.",
            DndProvider::<BoardPayload<Card>> {
                div { class: "grid grid-cols-3 gap-4",
                    for (name, col) in [("Backlog", BACKLOG), ("Doing", DOING), ("Shipped", SHIPPED)] {
                        BoardColumn::<Card> {
                            id: col,
                            label: name,
                            on_move: move |mv: MoveEvent<Card>| apply_move(&mut board.write(), mv),
                            class: "rounded-lg border border-slate-200 bg-slate-50 p-3 min-h-32 space-y-2 data-active:bg-slate-100/70",
                            p { class: "text-xs font-medium uppercase tracking-wide text-slate-400", "{name}" }
                            for (ix, card) in board.read().get(&col).cloned().unwrap_or_default().into_iter().enumerate() {
                                BoardItem::<Card> {
                                    item: card.clone(),
                                    column: col,
                                    index: ix,
                                    input: DragInputMode::Pointer,
                                    label: card.title.clone(),
                                    class: ITEM,
                                    "{card.title}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- 5. tree (before / into / after) -----------------------------------------

#[component]
fn TreeDemo() -> Element {
    let mut msg = use_signal(String::new);
    rsx! {
        Section { title: "Tree", note: "Drag onto a row's top edge to insert before, middle to nest, bottom to insert after.",
            DndProvider::<String> {
                PointerDraggable::<String> {
                    payload: "New node".to_string(),
                    input: DragInputMode::Pointer,
                    label: "New node",
                    class: "mb-3 inline-flex {ITEM}",
                    "Drag me onto the rows"
                }
                div { class: "rounded-lg border border-slate-200 overflow-hidden",
                    for (n, name) in [(1u64, "Documents"), (2, "Pictures"), (3, "Projects")] {
                        TreeNodeTarget::<String> {
                            node: NodeId(n),
                            label: name,
                            on_drop: move |ev: TreeDropEvent<String>| {
                                msg.set(format!("{} → {:?} {}", ev.payload, ev.intent, ev.target.0));
                            },
                            class: "border-b border-slate-100 px-3 py-2 text-sm text-slate-700 transition
                                    data-[intent=before]:shadow-[inset_0_2px_0_0_#0f172a]
                                    data-[intent=after]:shadow-[inset_0_-2px_0_0_#0f172a]
                                    data-[intent=into]:bg-slate-100",
                            "{name}"
                        }
                    }
                }
                if !msg.read().is_empty() {
                    p { class: "mt-2 text-xs text-slate-500", "{msg}" }
                }
            }
        }
    }
}

// --- 6. canvas (free position) -----------------------------------------------

#[derive(Clone, PartialEq)]
struct Node {
    id: u32,
    label: String,
    x: f64,
    y: f64,
}

#[component]
fn CanvasDemo() -> Element {
    let mut nodes = use_signal(|| {
        vec![
            Node {
                id: 1,
                label: "Input".into(),
                x: 24.0,
                y: 24.0,
            },
            Node {
                id: 2,
                label: "Transform".into(),
                x: 180.0,
                y: 90.0,
            },
            Node {
                id: 3,
                label: "Output".into(),
                x: 60.0,
                y: 150.0,
            },
        ]
    });
    rsx! {
        Section { title: "Canvas", note: "Drop anywhere - the node lands where you release it.",
            DndProvider::<Node> {
                CanvasDropZone::<Node> {
                    bounds: Bounds { width: 640.0, height: 220.0 },
                    on_drop: move |d: CanvasDrop<Node>| {
                        let mut ns = nodes.write();
                        if let Some(n) = ns.iter_mut().find(|n| n.id == d.payload.id) {
                            n.x = d.position.x;
                            n.y = d.position.y;
                        }
                    },
                    class: "relative h-56 rounded-lg border border-slate-200 bg-[radial-gradient(#e2e8f0_1px,transparent_1px)] [background-size:16px_16px] data-active:border-slate-300",
                    for node in nodes.read().clone() {
                        PointerDraggable::<Node> {
                            payload: node.clone(),
                            input: DragInputMode::Pointer,
                            label: node.label.clone(),
                            style: "position: absolute; left: {node.x}px; top: {node.y}px;",
                            class: "cursor-grab select-none rounded-md border border-slate-300 bg-white px-3 py-1.5 text-sm shadow-sm data-dragging:opacity-50",
                            "{node.label}"
                        }
                    }
                }
            }
        }
    }
}

// --- 7. multi-select ---------------------------------------------------------

#[component]
fn MultiSelectDemo() -> Element {
    let selection = use_selection::<u32>();
    let mut trashed = use_signal(Vec::<String>::new);
    let files = [
        (1u32, "report.pdf"),
        (2, "photo.jpg"),
        (3, "notes.txt"),
        (4, "budget.xlsx"),
    ];
    rsx! {
        Section { title: "Multi-select", note: "Click to select, Ctrl/Cmd-click to add. Drag any selected item to move the whole set.",
            DndProvider::<Vec<u32>> {
                div { class: "grid grid-cols-2 gap-4",
                    div { class: "space-y-2",
                        for (id, name) in files {
                            SelectableDraggable::<u32> {
                                item: id,
                                selection,
                                input: DragInputMode::Pointer,
                                label: name,
                                class: "flex cursor-grab select-none items-center rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm shadow-sm transition data-selected:border-slate-900 data-selected:bg-slate-100 data-dragging:opacity-50",
                                "{name}"
                            }
                        }
                    }
                    DropZone::<Vec<u32>> {
                        on_drop: move |o: DropOutcome<Vec<u32>>| {
                            let names: Vec<String> = o.payload.iter().filter_map(|id| files.iter().find(|(fid, _)| fid == id).map(|(_, n)| n.to_string())).collect();
                            trashed.write().extend(names);
                        },
                        class: ZONE,
                        p { class: "text-xs font-medium uppercase tracking-wide text-slate-400", "Trash" }
                        if trashed.read().is_empty() {
                            p { class: "text-sm text-slate-400", "Drop selected files here" }
                        } else {
                            p { class: "text-sm text-slate-600", "{trashed.read().join(\", \")}" }
                        }
                    }
                }
                DragOverlay::<Vec<u32>> {
                    class: "pointer-events-none rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm shadow-lg",
                    SelectionCount::<u32> {}
                }
            }
        }
    }
}

// --- 8. OS file drop (native) ------------------------------------------------

#[component]
fn FilesDemo() -> Element {
    let mut names = use_signal(Vec::<String>::new);
    rsx! {
        Section { title: "File drop", note: "Drag files from your OS. (Native - pointer drags can't cross the app boundary.)",
            FileDropZone {
                on_files: move |drop: FileDrop| {
                    names.write().extend(drop.files.iter().map(|f| f.name()));
                },
                class: "flex min-h-24 items-center justify-center rounded-lg border-2 border-dashed border-slate-200 text-sm text-slate-400 transition data-over:border-slate-900 data-over:bg-slate-100/60 data-over:text-slate-600",
                if names.read().is_empty() {
                    "Drop files from your desktop here"
                } else {
                    "{names.read().join(\", \")}"
                }
            }
        }
    }
}

// --- 9. drag out / external in (native) --------------------------------------

#[component]
fn ExternalDemo() -> Element {
    let mut dropped = use_signal(String::new);
    rsx! {
        Section { title: "In & out", note: "Drag the link out to another tab; drop text/links from elsewhere in. (Native.)",
            div { class: "grid grid-cols-2 gap-4",
                ExternalDragSource {
                    content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
                    class: "flex cursor-grab items-center justify-center rounded-lg border border-slate-200 bg-white px-3 py-6 text-sm text-slate-700 shadow-sm",
                    "Drag this link out ↗"
                }
                ExternalDropZone {
                    on_drop: move |d: ExternalDrop| {
                        dropped.set(format!("{} payload(s), {} file(s)", d.payloads.len(), d.files.len()));
                    },
                    class: "flex min-h-24 items-center justify-center rounded-lg border-2 border-dashed border-slate-200 text-sm text-slate-400 transition data-over:border-slate-900 data-over:bg-slate-100/60",
                    if dropped.read().is_empty() {
                        "Drop text or a link here"
                    } else {
                        "{dropped}"
                    }
                }
            }
        }
    }
}

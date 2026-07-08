//! Kanban board + file drop demo - now with keyboard access (Tab to a card,
//! Space to pick up, arrows to choose a column, Enter to drop, Esc to
//! cancel), touch support, auto-scrolling columns, and screen-reader
//! announcements.
//!
//! Run with the Dioxus CLI:
//! ```sh
//! dx serve --example kanban --platform web --features web
//! ```

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct Card {
    id: u32,
    title: String,
}

const TODO: ZoneId = ZoneId(1);
const DOING: ZoneId = ZoneId(2);
const DONE: ZoneId = ZoneId(3);

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut board = use_signal(|| {
        let mut m: HashMap<ContainerId, Vec<Card>> = HashMap::new();
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
        m.insert(DOING, vec![]);
        m.insert(DONE, vec![]);
        m
    });
    let mut dropped_files = use_signal(Vec::<String>::new);
    let mut tiles = use_signal(|| (1..=8).collect::<Vec<u32>>());

    let on_move = move |mv: MoveEvent<Card>| {
        apply_move(&mut board.write(), mv);
    };

    rsx! {
        style { {CSS} }
        h1 { "dioxus-dnd demo" }

        DndProvider::<BoardPayload<Card>> {
            // One announcer per provider: voices pick-up/over/drop/cancel
            // for screen readers.
            LiveRegion::<BoardPayload<Card>> {}
            div { class: "board",
                for (name, col) in [("Todo", TODO), ("Doing", DOING), ("Done", DONE)] {
                    BoardColumn::<Card> {
                        id: col,
                        label: name,
                        on_move,
                        class: "column",
                        h2 { "{name}" }
                        AutoScroll {
                            class: "column-scroll",
                            for (ix, card) in board.read().get(&col).cloned().unwrap_or_default().into_iter().enumerate() {
                                BoardSlot::<Card> { column: col, index: ix, on_move, class: "slot" }
                                // Draggable defaults to pointer events
                                // for mouse, touch and pen. Keyboard works out
                                // of the box (Tab + Space/arrows).
                                Draggable::<BoardPayload<Card>> {
                                    payload: BoardPayload { item: card.clone(), from: col, index: ix },
                                    zone: col,
                                    label: card.title.clone(),
                                    class: "card",
                                    "{card.title}"
                                }
                            }
                        }
                    }
                }
            }
        }

        h2 { "Grid (swap mode - try Ctrl-drag for copy cursor)" }
        SortableGrid {
            class: "grid",
            len: tiles.read().len(),
            cols: 4,
            mode: ReorderMode::Swap,
            render: move |ix: usize| rsx! { div { class: "tile", "{tiles.read()[ix]}" } },
            on_sort: move |ev: SortEvent| apply_swap(&mut tiles.write(), ev),
        }

        h2 { "Drag out" }
        ExternalDragSource {
            class: "dragout",
            content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
            "Drag me into another tab or your URL bar"
        }

        h2 { "File drop" }
        FileDropZone {
            class: "filezone",
            filter: FileFilter::new().max_files(5),
            on_files: move |drop: FileDrop| {
                for f in drop.files {
                    dropped_files.write().push(format!("{} ({} bytes)", f.name(), f.size()));
                }
            },
            if dropped_files.read().is_empty() {
                "Drop up to 5 files here"
            } else {
                ul {
                    for name in dropped_files.read().iter() {
                        li { "{name}" }
                    }
                }
            }
        }
    }
}

const CSS: &str = r#"
body { font-family: sans-serif; margin: 2rem; }
.board { display: flex; gap: 1rem; }
.column { background: #f0f0f4; border-radius: 8px; padding: .75rem; width: 220px; }
.column-scroll { max-height: 300px; overflow-y: auto; }
.card { background: white; border-radius: 6px; padding: .6rem .8rem; margin: 2px 0;
        box-shadow: 0 1px 3px rgba(0,0,0,.15); cursor: grab; }
.card:focus-within { outline: 2px solid #4a7dff; outline-offset: 2px; }
.slot { height: 6px; border-radius: 3px; }
.slot[data-active="true"] { height: 14px; background: #c7d7ff; }
.filezone { border: 2px dashed #aaa; border-radius: 8px; padding: 2rem; margin-top: .5rem; }
.grid { gap: 8px; max-width: 400px; }
.tile { background: #dfe7ff; border-radius: 6px; padding: 1rem; text-align: center; cursor: grab; }
.tile:hover { background: #cdd9ff; }
[data-drop-target="true"] > .tile { outline: 2px solid #4a7dff; }
[data-dragging="true"] > .tile { opacity: .4; }
.dragout { display: inline-block; background: #ffe9c7; border-radius: 6px; padding: .6rem 1rem; cursor: grab; }
"#;

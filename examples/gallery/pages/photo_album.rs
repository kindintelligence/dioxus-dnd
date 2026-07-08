//! Photo album: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn PhotoAlbumPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Reorder",
            title: "Photo album",
            lead: "SortableGrid is the two-dimensional sibling of SortableList: drag a tile onto another and the album reflows around the insertion point. Same index-based contract, same SortEvent, one extra prop for the column count.",
        }
        AlbumDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "The grid owns layout.",
                        "It sets display: grid with equal columns on its container, and your forwarded class and style merge in afterwards, so gap utilities work and a custom grid-template-columns wins per property.",
                    ),
                    (
                        "Tiles are library wrappers.",
                        "Like the list, the grid renders a wrapper per tile; item_class styles them, and that wrapper is where data-dragging and data-drop-target appear. The hovered tile is simply the one under the pointer.",
                    ),
                    (
                        "Drop means insert or swap; you choose.",
                        "The drop emits the same SortEvent as a list. Pair the default ReorderMode::Insert with apply_sort for gallery-style reflow, or ReorderMode::Swap with apply_swap for dashboard tiles that trade places.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Nothing about the tiles is special: render draws whatever you like at index N, and the grid takes care of measuring, hit-testing and the reorder gesture. If you already integrated SortableList, this is the same code with cols added."
                }
            }
            DioxusNote {
                p {
                    "Notice the demo keys each tile's colors by the photo's own hue field, not by its index. In Dioxus (as in React), identity should travel with the data: key by something stable so a reorder moves the picture, not just the caption."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "SortableGrid props",
                rows: vec![
                    ("len", "usize, required", "Number of tiles."),
                    ("cols", "usize, required", "Number of columns; the grid renders repeat(cols, 1fr)."),
                    ("render", "Callback<usize, Element>, required", "Draws the tile at the given index."),
                    ("on_sort", "EventHandler<SortEvent>, required", "Fired when a tile is dropped on another."),
                    ("mode", "ReorderMode = Insert", "Semantic and styling hint: the root carries data-mode=\"insert\" or \"swap\" so the two feels can style differently."),
                    ("item_class", "Option<String>", "Classes for each tile's wrapper div, the element carrying data-dragging and data-drop-target."),
                ],
            }
            PropsTable {
                title: "Grid helpers",
                rows: vec![
                    ("cell_of(index, cols)", "-> (row, col)", "Where a flat index sits in the grid."),
                    ("index_of(row, col, cols, len)", "-> Option<usize>", "Back the other way, None outside the grid. Both are public for custom keyboard navigation or layout math."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "mode does not change the event.",
                        "Insert and Swap both emit SortEvent; the mode is for styling, and your choice of apply_sort or apply_swap decides the semantics.",
                    ),
                    (
                        "A drop off the tiles cancels,",
                        "so releasing in the page margin never commits an accidental reorder.",
                    ),
                    (
                        "Tiles set touch-action: none,",
                        "which is right for grids (they rarely scroll by dragging across their own tiles). For scrollable lists, see the Podcast queue page.",
                    ),
                    (
                        "No hysteresis needed:",
                        "tiles do not shift while you hover in a grid, so the target is always simply the tile under the pointer.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"SortableGrid {
    len: photos.read().len(),
    cols: 3,
    on_sort: move |ev: SortEvent| apply_sort(&mut photos.write(), ev),
    class: "gap-2.5",
    item_class: "rounded-xl overflow-hidden
                 data-dragging:opacity-40
                 data-drop-target:brightness-115",
    render: move |ix: usize| rsx! {
        PhotoTile { photo: photos.read()[ix].clone() }
    },
}"#;

// --- 6. photo album (sortable grid) ------------------------------------------

#[derive(Clone, PartialEq)]
struct Photo {
    label: &'static str,
    hue: usize,
}

/// A palette two-tone gradient, keyed by the photo's own hue (NOT its slot), so
/// the picture travels with its label when the grid reorders.
fn photo_gradient(h: usize) -> &'static str {
    const G: [&str; 9] = [
        "bg-gradient-to-br from-[#3E7558] to-[#D5B876]",
        "bg-gradient-to-br from-[#C9926B] to-[#E9DDB8]",
        "bg-gradient-to-br from-[#A6C1B0] to-[#F0F2E3]",
        "bg-gradient-to-br from-[#2A5E48] to-[#A6C1B0]",
        "bg-gradient-to-br from-[#D5B876] to-[#C9926B]",
        "bg-gradient-to-br from-[#6C9984] to-[#D9E4EC]",
        "bg-gradient-to-br from-[#B88B2F] to-[#E9DDB8]",
        "bg-gradient-to-br from-[#3E7558] to-[#A6C1B0]",
        "bg-gradient-to-br from-[#C9926B] to-[#B88B2F]",
    ];
    G[h % G.len()]
}

#[component]
fn AlbumDemo() -> Element {
    let mut photos = use_signal(|| {
        vec![
            Photo {
                label: "Sunday hike",
                hue: 0,
            },
            Photo {
                label: "Harbor at dusk",
                hue: 1,
            },
            Photo {
                label: "The studio",
                hue: 2,
            },
            Photo {
                label: "Roadtrip",
                hue: 3,
            },
            Photo {
                label: "Back garden",
                hue: 4,
            },
            Photo {
                label: "Rooftop",
                hue: 5,
            },
            Photo {
                label: "Corner cafe",
                hue: 6,
            },
            Photo {
                label: "Coastline",
                hue: 7,
            },
            Photo {
                label: "Market day",
                hue: 8,
            },
        ]
    });
    rsx! {
        Section {
            title: "Photo album",
            note: "Arrange an album in two dimensions. Drag a photo and the grid reflows around it; drop it and everything snaps to the new order.",
            tag: "SortableGrid",
            // SortableGrid puts `display: grid; grid-template-columns:
            // repeat(cols, 1fr)` on this container; `class` (the gap) merges in.
            SortableGrid {
                len: photos.read().len(),
                cols: 3,
                on_sort: move |ev: SortEvent| apply_sort(&mut photos.write(), ev),
                class: "gap-2.5",
                // `min-h` is a fallback so a tile can never collapse if the
                // arbitrary aspect ratio is unsupported.
                // Drop target brightens so the highlight reads on the light page;
                // the dragged tile fades.
                item_class: "group relative aspect-[4/3] min-h-[6rem] overflow-hidden rounded-xl cursor-grab select-none shadow-[0_2px_10px_-2px_rgba(26,24,21,0.10)] transition data-dragging:opacity-40 data-drop-target:brightness-115"
                    .to_string(),
                render: move |ix: usize| {
                    let p = photos.read()[ix].clone();
                    rsx! {
                        div { class: "absolute inset-0 {photo_gradient(p.hue)}" }
                        div { class: "absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/45 to-transparent p-2 pt-6",
                            span { class: "text-[11px] font-medium text-white", "{p.label}" }
                        }
                    }
                },
            }
        }
    }
}

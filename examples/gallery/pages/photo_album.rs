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
            lead: "SortableGrid is the 2D sibling of SortableList: drag a tile anywhere in the grid and the others reflow around the insertion point.",
        }
        AlbumDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "The grid owns layout: it sets display: grid with repeat(cols, 1fr) on its container, and your forwarded class and style merge in after, so gap utilities and custom grid-template-columns both work. Tiles are library-rendered wrappers; item_class styles them and is where data-dragging and data-drop-target live."
                }
                p {
                    "The default ReorderMode::Insert removes the tile and inserts it at the target index. ReorderMode::Swap exchanges the two tiles instead, for dashboard-style fixed slots, paired with apply_swap."
                }
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Key your visuals by item identity, not index:",
                        "a tile's colors should travel with it when the grid reorders.",
                    ),
                    (
                        "mode: ReorderMode::Swap + apply_swap",
                        "turns insert-and-reflow into trade-places.",
                    ),
                    (
                        "Custom tracks:",
                        "style: \"grid-template-columns: 2fr 1fr 1fr;\" wins per property over the functional grid style.",
                    ),
                    (
                        "cell_of and index_of are public helpers",
                        "when you build custom grid interactions.",
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
        "bg-gradient-to-br from-[#D97D55] to-[#F4E9D7]",
        "bg-gradient-to-br from-[#6FA4AF] to-[#B8C4A9]",
        "bg-gradient-to-br from-[#B8C4A9] to-[#F4E9D7]",
        "bg-gradient-to-br from-[#6FA4AF] to-[#F4E9D7]",
        "bg-gradient-to-br from-[#D97D55] to-[#B8C4A9]",
        "bg-gradient-to-br from-[#6FA4AF] to-[#D97D55]",
        "bg-gradient-to-br from-[#B8C4A9] to-[#6FA4AF]",
        "bg-gradient-to-br from-[#D97D55] to-[#6FA4AF]",
        "bg-gradient-to-br from-[#F4E9D7] to-[#D97D55]",
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
                // Drop target brightens (dimming reads poorly on a dark page);
                // the dragged tile fades.
                item_class: "group relative aspect-[4/3] min-h-[6rem] overflow-hidden rounded-xl cursor-grab select-none shadow-[0_2px_10px_-2px_rgba(0,0,0,0.5)] transition data-dragging:opacity-40 data-drop-target:brightness-115"
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

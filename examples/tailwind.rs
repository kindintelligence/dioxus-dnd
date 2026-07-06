//! Styling dioxus-dnd with Tailwind - no CSS file, no custom state wiring.
//! Every drag interaction below is styled purely through utility classes
//! reacting to the library's state data attributes:
//!
//! - `data-dragging:`     the element being dragged
//! - `data-active:`       a compatible drag is in flight (reveal targets)
//! - `data-over:`         that drag hovers this zone
//! - `data-drop-target:`  hovered slot in sortable lists/grids
//! - `data-[intent=…]:`   before/into/after bands on tree rows
//!
//! Uses the Tailwind v4 browser CDN so the example runs without a build
//! step (don't ship the CDN to production).
//!
//! Run with the Dioxus CLI:
//! ```sh
//! dx serve --example tailwind --platform web
//! ```
//!
//! Add `--features web` for native pointer capture, so a mouse reorder stays
//! glued to the row even when the cursor leaves the list:
//! ```sh
//! dx serve --example tailwind --platform web --features web
//! ```

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct Card {
    id: u32,
    title: String,
}

const BACKLOG: ZoneId = ZoneId(1);
const SPRINT: ZoneId = ZoneId(2);

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Script { src: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" }
        div { class: "mx-auto max-w-2xl p-8 font-sans text-slate-800",
            h1 { class: "mb-1 text-2xl font-bold", "dioxus-dnd × Tailwind" }
            p { class: "mb-8 text-sm text-slate-500",
                "All drag feedback below is utility classes on data attributes - no stylesheet, no signals."
            }
            CardsDemo {}
            SortableDemo {}
            TreeDemo {}
            FilesDemo {}
        }
    }
}

/// Draggable cards between two zones. The zones reveal themselves via
/// `data-active:` while a drag is in flight and highlight via `data-over:`;
/// the dragged card dims via `data-dragging:`; the ghost is a styled
/// `DragOverlay`.
#[component]
fn CardsDemo() -> Element {
    let mut bins = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Card>> = HashMap::new();
        m.insert(
            BACKLOG,
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
        m.insert(SPRINT, vec![]);
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
        section { class: "mb-10",
            h2 { class: "mb-3 text-lg font-semibold", "Cards" }
            DndProvider::<Card> {
                LiveRegion::<Card> {}
                div { class: "grid grid-cols-2 gap-4",
                    for (name, zone) in [("Backlog", BACKLOG), ("Sprint", SPRINT)] {
                        DropZone::<Card> {
                            id: zone,
                            label: name,
                            on_drop: move_card,
                            class: "min-h-40 rounded-xl border-2 border-dashed border-transparent bg-slate-50 p-3 transition
                                    data-active:border-slate-300
                                    data-over:border-blue-500 data-over:bg-blue-50",
                            h3 { class: "mb-2 text-xs font-semibold uppercase tracking-wide text-slate-400", "{name}" }
                            for card in bins.read().get(&zone).cloned().unwrap_or_default() {
                                PointerDraggable::<Card> {
                                    payload: card.clone(),
                                    zone,
                                    input: DragInputMode::Pointer,
                                    label: card.title.clone(),
                                    class: "mb-2 cursor-grab rounded-lg border border-slate-200 bg-white p-3 text-sm shadow-sm transition
                                            focus-visible:outline-2 focus-visible:outline-blue-500
                                            data-dragging:opacity-40",
                                    "{card.title}"
                                }
                            }
                        }
                    }
                }
                // The ghost that follows the cursor: class lands on the
                // overlay wrapper, positioning stays functional.
                DragOverlay::<Card> {
                    class: "rotate-3 scale-105",
                    GhostCard {}
                }
            }
        }
    }
}

/// Renders the in-flight card inside the overlay.
#[component]
fn GhostCard() -> Element {
    let dnd = use_dnd::<Card>();
    let title = dnd.payload().map(|c| c.title).unwrap_or_default();
    rsx! {
        div { class: "rounded-lg border border-blue-300 bg-white p-3 text-sm shadow-xl", "{title}" }
    }
}

/// Sortable list with a touch grip. The restored `SortableList` API puts
/// state attributes on its row wrappers, so the list class styles those
/// direct children.
#[component]
fn SortableDemo() -> Element {
    let mut items = use_signal(|| {
        vec![
            "Alpha".to_string(),
            "Bravo".into(),
            "Charlie".into(),
            "Delta".into(),
        ]
    });

    rsx! {
        section { class: "mb-10",
            h2 { class: "mb-3 text-lg font-semibold", "Sortable list" }
            SortableList {
                len: items.read().len(),
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                input: DragInputMode::Pointer,
                touch_handle: true,
                class: "[&>*]:mb-1 [&>*]:gap-2 [&>*]:rounded-lg [&>*]:border [&>*]:border-slate-200 [&>*]:bg-white [&>*]:p-2 [&>*]:transition [&>*]:will-change-transform [&>*]:select-none
                        [&_[data-sort-handle]]:w-6 [&_[data-sort-handle]]:cursor-grab [&_[data-sort-handle]]:text-slate-400 [&>[data-dragging]_[data-sort-handle]]:cursor-grabbing
                        [&>[data-dragging]]:shadow-lg [&>[data-dragging]]:ring-2 [&>[data-dragging]]:ring-blue-400
                        [&>[data-drop-target]]:border-blue-500 [&>[data-drop-target]]:bg-blue-50 [&>[data-drop-target]]:ring-2 [&>[data-drop-target]]:ring-blue-200",
                render: move |ix: usize| rsx! {
                    div { class: "text-sm", "{items.read()[ix]}" }
                },
            }
        }
    }
}

/// Tree rows styled by drop intent - value selectors on `data-intent`.
#[component]
fn TreeDemo() -> Element {
    let mut message = use_signal(String::new);

    rsx! {
        section { class: "mb-10",
            h2 { class: "mb-3 text-lg font-semibold", "Tree intents" }
            DndProvider::<String> {
                Draggable::<String> {
                    payload: "The item".to_string(),
                    label: "The item",
                    class: "mb-3 inline-block cursor-grab rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm shadow-sm
                            data-dragging:opacity-75",
                    "Drag me over the rows - edges insert, center nests"
                }
                for (n, name) in [(1, "Documents"), (2, "Pictures"), (3, "Projects")] {
                    TreeNodeTarget::<String> {
                        node: NodeId(n),
                        label: name,
                        on_drop: move |ev: TreeDropEvent<String>| {
                            message.set(format!("{} → {:?} node {}", ev.payload, ev.intent, ev.target.0));
                        },
                        class: "rounded px-3 py-2 text-sm
                                data-[intent=before]:shadow-[inset_0_3px_0_theme(colors.blue.500)]
                                data-[intent=after]:shadow-[inset_0_-3px_0_theme(colors.blue.500)]
                                data-[intent=into]:bg-blue-50",
                        "📁 {name}"
                    }
                }
            }
            if !message.read().is_empty() {
                p { class: "mt-2 text-xs text-slate-500", "{message}" }
            }
        }
    }
}

/// OS file drop - the classic highlight needs only `data-over:`.
#[component]
fn FilesDemo() -> Element {
    let mut names = use_signal(Vec::<String>::new);

    rsx! {
        section {
            h2 { class: "mb-3 text-lg font-semibold", "File drop" }
            FileDropZone {
                on_files: move |drop: FileDrop| {
                    names.write().extend(drop.files.iter().map(|f| f.name()));
                },
                class: "flex min-h-24 items-center justify-center rounded-xl border-2 border-dashed border-slate-300 text-sm text-slate-400 transition
                        data-over:border-blue-500 data-over:bg-blue-50 data-over:text-blue-600",
                if names.read().is_empty() {
                    "Drop files from your OS here"
                } else {
                    "{names.read().join(\", \")}"
                }
            }
        }
    }
}

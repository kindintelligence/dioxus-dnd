//! Moodboard: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn MoodboardPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Structure",
            title: "Moodboard",
            lead: "CanvasDropZone is the free-position primitive for whiteboards, node editors and floor planners: drop anywhere and you get back a corrected top-left position, not a slot index.",
        }
        MoodboardDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "A completed CanvasDrop gives you two points: pointer, the raw canvas-relative pointer position, and position, which is pointer minus the grab offset (where inside the element you picked it up), then snapped and clamped if you configured SnapGrid or Bounds. Writing position back into your model is the whole loop."
                }
                p {
                    "Bounds clamps the top-left point only; use Bounds::clamp_item with the element's size when the whole item must stay inside."
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
                        "The grab offset does the polish:",
                        "items land where you dropped them visually, not offset by where you grabbed.",
                    ),
                    (
                        "Keyboard placement is a policy:",
                        "Center (default), Origin, or Fixed(point) via the keyboard prop.",
                    ),
                    (
                        "client_to_canvas and friends are public",
                        "for wiring custom pan, zoom or preview interactions.",
                    ),
                    (
                        "Native boundary drops need their own zone:",
                        "layer a FileDropZone or ExternalDropZone over the canvas and place its element point with the same geometry helpers.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"CanvasDropZone::<Note> {
    snap: SnapGrid(16.0),
    bounds: Bounds { width: 640.0, height: 220.0 },
    on_drop: move |d: CanvasDrop<Note>| {
        if let Some(n) = notes.write().iter_mut().find(|n| n.id == d.payload.id) {
            n.x = d.position.x;
            n.y = d.position.y;
        }
    },
    for note in notes.read().clone() {
        Draggable::<Note> {
            payload: note.clone(),
            style: "position: absolute; left: {note.x}px; top: {note.y}px;",
            Sticky { note }
        }
    }
}"#;

// --- 10. moodboard (canvas: free position) -----------------------------------

#[derive(Clone, PartialEq)]
struct Note {
    id: u32,
    label: String,
    x: f64,
    y: f64,
}

/// Soft palette tints for sticky notes, keyed by id. Deliberately kept light
/// on the midnight page: real paper stickies, dark ink, pinned to a dark board.
fn note_color(id: u32) -> &'static str {
    const C: [&str; 4] = [
        "bg-[#f0d6c6]",
        "bg-[#cfe0e3]",
        "bg-[#dde5d1]",
        "bg-[#f4e9d7]",
    ];
    C[id as usize % C.len()]
}

#[component]
fn MoodboardDemo() -> Element {
    // Positions kept comfortably inside the 640x220 bounds so every note is
    // fully visible even before it is moved.
    let mut notes = use_signal(|| {
        vec![
            Note {
                id: 1,
                label: "Warm, not loud".into(),
                x: 20.0,
                y: 20.0,
            },
            Note {
                id: 2,
                label: "Fewer, better parts".into(),
                x: 220.0,
                y: 66.0,
            },
            Note {
                id: 3,
                label: "Should feel handmade".into(),
                x: 80.0,
                y: 128.0,
            },
            Note {
                id: 4,
                label: "Delight in the details".into(),
                x: 420.0,
                y: 30.0,
            },
        ]
    });
    rsx! {
        Section {
            title: "Moodboard",
            note: "A free-form board with no grid and no order. Drag a note anywhere and it stays where you drop it.",
            tag: "CanvasDropZone",
            DndProvider::<Note> {
                LiveRegion::<Note> {}
                CanvasDropZone::<Note> {
                    bounds: Bounds {
                        width: 640.0,
                        height: 220.0,
                    },
                    on_drop: move |d: CanvasDrop<Note>| {
                        let mut ns = notes.write();
                        if let Some(n) = ns.iter_mut().find(|n| n.id == d.payload.id) {
                            n.x = d.position.x;
                            n.y = d.position.y;
                        }
                    },
                    class: "relative h-56 overflow-hidden rounded-xl bg-[#26211a] bg-[radial-gradient(#3f372b_1px,transparent_1px)] [background-size:16px_16px] ring-1 ring-white/5 shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)] transition data-active:ring-[#B8C4A9]/60",
                    for note in notes.read().clone() {
                        Draggable::<Note> {
                            payload: note.clone(),
                            label: note.label.clone(),
                            style: "position: absolute; left: {note.x}px; top: {note.y}px;",
                            class: "w-36 cursor-grab select-none rounded-lg p-3 text-[12px] font-medium leading-snug text-[#4a4235] shadow-[0_6px_18px_-6px_rgba(0,0,0,0.5)] ring-1 ring-black/25 transition hover:-translate-y-0.5 data-dragging:opacity-60 {note_color(note.id)}",
                            span { class: "mb-1.5 block h-1.5 w-1.5 rounded-full bg-black/20" }
                            div { "{note.label}" }
                        }
                    }
                }
            }
        }
    }
}

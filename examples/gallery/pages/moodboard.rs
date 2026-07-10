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
            lead: "CanvasDropZone is the free-position primitive for whiteboards, node editors and floor planners. A drop doesn't mean \"slot 3\" here; it means \"exactly there\", and the zone hands you a corrected top-left position ready to write into your model.",
        }
        MoodboardDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Position, not order.",
                        "A completed drop delivers a CanvasDrop with two points, both relative to the canvas: pointer is the raw release position, and position is the corrected top-left where the element should land.",
                    ),
                    (
                        "The grab offset does the polish.",
                        "position starts as pointer minus grab: where inside the element you originally picked it up. That is why a note lands exactly where its ghost was, instead of jumping so its corner meets the cursor tip.",
                    ),
                    (
                        "Then snap, then clamp.",
                        "If you configure a SnapGrid the position rounds to the grid; if you configure Bounds it clamps inside. The order is fixed (grab correction, snap, clamp) so results are predictable. Writing position back into your model is the whole loop.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "The notes are ordinary Draggables positioned absolutely from your own model. The canvas doesn't own their layout; it only reports where each drop should put them, which keeps pan/zoom layers and custom rendering entirely in your hands."
                }
            }
            DioxusNote {
                p {
                    "The inline style interpolates model values straight into rsx: left and top come from note.x and note.y. There is no separate styling system to learn; a signal write to those fields moves the note on the next render."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "CanvasDropZone props",
                rows: vec![
                    ("on_drop", "EventHandler<CanvasDrop<T>>, required", "The completed drop with corrected position."),
                    ("snap", "Option<SnapGrid>", "Round positions to a square grid; SnapGrid(16.0) snaps to 16px."),
                    ("bounds", "Option<Bounds>", "Clamp the top-left into width by height. See clamp_item when the whole element must stay inside."),
                    ("keyboard", "CanvasKeyboardPlacement = Center", "Where keyboard-driven drops land: the zone center, the origin, or a fixed point."),
                    ("id / label", "Option<ZoneId> / Option<String>", "Identity and screen-reader name, as on every zone."),
                ],
            }
            PropsTable {
                title: "CanvasDrop<T> fields",
                rows: vec![
                    ("payload", "T", "The dragged value."),
                    ("position", "Point", "Corrected top-left: pointer minus grab, snapped and clamped. Write this into your model."),
                    ("pointer", "Point", "The raw canvas-relative release position, untouched, when you need your own math."),
                ],
            }
            PropsTable {
                title: "Geometry helpers",
                rows: vec![
                    ("client_to_canvas / canvas_to_client", "(Point, Rect) -> Point", "Convert between viewport and canvas-local coordinates."),
                    ("canvas_position(pointer, grab, snap, bounds)", "-> Point", "The exact correction pipeline the zone applies, public for previews and custom flows."),
                    ("Bounds::clamp_item(p, w, h)", "-> Point", "Clamp so a whole w by h element stays inside, not just its corner."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Keyboard placement is a policy:",
                        "Center (default), Origin, or Fixed(point) via the keyboard prop, so canvas drops stay keyboard-accessible.",
                    ),
                    (
                        "Pan and zoom stay yours:",
                        "CanvasViewport and the screen/world helpers in core convert coordinates for zoomed planes; the zone itself stays deliberately simple.",
                    ),
                    (
                        "data-active marks the canvas",
                        "while any drag is in flight, for the dashed \"you can drop here\" outline this demo shows.",
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

const BOARD: ZoneId = ZoneId(7100);
const BOUNDS: Bounds = Bounds {
    width: 640.0,
    height: 220.0,
};

#[derive(Clone, PartialEq)]
struct Note {
    id: u32,
    label: String,
    x: f64,
    y: f64,
}

/// Soft paper tints for sticky notes, keyed by id: warm pastels with dark
/// ink, pinned to the parchment board.
fn note_color(id: u32) -> &'static str {
    const C: [&str; 4] = [
        "bg-[#E8D4BE]",
        "bg-[#D9E4EC]",
        "bg-[#E4ECDD]",
        "bg-[#E9DDB8]",
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
                    id: BOARD,
                    bounds: BOUNDS,
                    on_drop: move |d: CanvasDrop<Note>| {
                        let mut ns = notes.write();
                        if let Some(n) = ns.iter_mut().find(|n| n.id == d.payload.id) {
                            n.x = d.position.x;
                            n.y = d.position.y;
                        }
                    },
                    class: "relative h-56 overflow-hidden rounded-xl bg-[#EEEADF] bg-[radial-gradient(#D7D4C9_1px,transparent_1px)] [background-size:16px_16px] ring-1 ring-[#E8E5D9] shadow-[inset_0_1px_2px_rgba(26,24,21,0.07)] transition data-active:ring-[#6C9984]/60",
                    MoodNotes { notes }
                }
            }
        }
    }
}

/// The notes themselves. The one in flight rides the pointer through the
/// exact corrected placement its drop will use (same `canvas_position`
/// math, same bounds), so the element simply travels across the board and
/// stops where you let go - fully normal appearance, nothing left behind,
/// no overlay.
#[component]
fn MoodNotes(notes: Signal<Vec<Note>>) -> Element {
    let dnd = use_dnd::<Note>();
    let registry = use_zone_registry::<Note>();
    let live_pos = move |note: &Note| -> Point {
        let in_flight = dnd.dragging()
            && dnd.mode() == DragMode::Pointer
            && dnd.payload().map(|p| p.id) == Some(note.id);
        if in_flight {
            if let Some(rect) = registry.cached_rect(BOARD) {
                let pointer = client_to_canvas(dnd.pointer(), rect);
                return canvas_position(pointer, dnd.grab(), None, Some(BOUNDS));
            }
        }
        Point::new(note.x, note.y)
    };
    rsx! {
        for note in notes.read().clone() {
            Draggable::<Note> {
                key: "{note.id}",
                payload: note.clone(),
                label: note.label.clone(),
                style: {
                    let p = live_pos(&note);
                    format!("position: absolute; left: {}px; top: {}px;", p.x, p.y)
                },
                class: "w-36 cursor-grab select-none rounded-lg p-3 text-[12px] font-medium leading-snug text-[#2C2A25] shadow-[0_6px_18px_-6px_rgba(26,24,21,0.10)] ring-1 ring-black/25 hover:-translate-y-0.5 data-dragging:z-10 {note_color(note.id)}",
                span { class: "mb-1.5 block h-1.5 w-1.5 rounded-full bg-black/20" }
                div { "{note.label}" }
            }
        }
    }
}

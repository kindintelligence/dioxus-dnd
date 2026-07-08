//! Project files: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn ProjectFilesPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Structure",
            title: "Project files",
            lead: "The classic tree problem: dropping on a node can mean three different things. dioxus-dnd splits each row into three bands - top edge places before, middle nests inside, bottom edge places after - and a cycle guard keeps folders out of their own subtrees.",
        }
        FilesTreeDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Three meanings per row.",
                        "TreeNodeTarget resolves the pointer's vertical position against row_height into a DropIntent: the top quarter is Before, the bottom quarter After, the middle half Into. While hovered it exposes the live value as a data-intent attribute for styling insertion indicators.",
                    ),
                    (
                        "The drop is yours to interpret.",
                        "A completed drop hands you a TreeDropEvent: the payload, the target NodeId and the intent. With a parent-pointer model like this demo's, a whole subtree moves with one field write, because children keep pointing at the dragged node.",
                    ),
                    (
                        "The tree stays a tree.",
                        "The accepts callback receives the payload and the intent together, so rules like \"files refuse Into\" are one comparison. would_create_cycle walks the target's ancestors and refuses when the dragged node appears, which covers dropping a folder into itself or any descendant.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Each row is both a target (TreeNodeTarget) and a source (the Draggable inside it). The payload here is just the node's id: the drop handler looks the node up in the model, so the tree structure lives in exactly one place."
                }
            }
            DioxusNote {
                p {
                    "The accepts closure captures per-row data by wrapping the move closure in a block that first copies what it needs (the target id, whether it's a folder). That block-then-closure shape is the standard Rust answer whenever each row of a loop needs its own captured values."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "TreeNodeTarget props",
                rows: vec![
                    ("node", "NodeId, required", "The node this row represents; handed back in the drop event."),
                    ("row_height", "f64 = 28.0", "Height used for the three bands. Keep it close to the rendered height; keyboard drops resolve their intent against it."),
                    ("accepts", "Callback<(T, DropIntent), bool>", "Refuse combinations: cycle prevention, files rejecting Into, permission rules."),
                    ("on_drop", "EventHandler<TreeDropEvent<T>>, required", "The completed drop with payload, target and intent."),
                    ("label", "Option<String>", "Screen-reader name announced during keyboard navigation."),
                ],
            }
            PropsTable {
                title: "Types and helpers",
                rows: vec![
                    ("DropIntent", "Before | After | Into", "Where, relative to the target, the payload should land."),
                    ("TreeDropEvent<T>", "payload, target, intent", "Everything your model needs to perform the move."),
                    ("intent_from_offset(y, row_height)", "-> DropIntent", "The public band math, for custom tree interactions."),
                    ("would_create_cycle(parent_of, dragged, target)", "-> bool", "Walks ancestors via your lookup closure; true when the drop would make a node its own ancestor."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Value selectors style the bands:",
                        "data-[intent=before] draws a top edge, data-[intent=into] tints the row, data-[intent=after] draws a bottom edge, all with no extra state.",
                    ),
                    (
                        "Keyboard drops land in the middle band,",
                        "resolving to Into, which is what nesting into the focused row should mean.",
                    ),
                    (
                        "The chevron trick is free:",
                        "this demo's folder chevrons swing open on data-intent=into via pure CSS, signalling \"this will go inside\" with zero wiring.",
                    ),
                    (
                        "would_create_cycle is defensive",
                        "about broken parent maps too: a cycle in your own data reads as unsafe rather than looping forever.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"TreeNodeTarget::<u64> {
    node: NodeId(n.id),
    row_height: 38.0,
    accepts: move |(dragged, intent): (u64, DropIntent)| {
        if intent == DropIntent::Into && !n.folder { return false; }
        !would_create_cycle(parent_of, NodeId(dragged), NodeId(target))
    },
    on_drop: move |ev: TreeDropEvent<u64>| reparent(ev.payload, target, ev.intent),
    Draggable::<u64> { payload: n.id, RowFace {} }
}"#;

// --- 9. project files (real tree: reparenting + cycle guard) -----------------

#[derive(Clone, Copy, PartialEq)]
struct FsNode {
    id: u64,
    parent: Option<u64>,
    name: &'static str,
    folder: bool,
}

/// Depth-first flatten in display order. Sibling order is the storage order,
/// so a reorder is just a `Vec` move and a reparent is one field write.
fn flatten_tree(
    nodes: &[FsNode],
    parent: Option<u64>,
    depth: usize,
    out: &mut Vec<(usize, FsNode)>,
) {
    for n in nodes.iter().filter(|n| n.parent == parent) {
        out.push((depth, *n));
        if n.folder {
            flatten_tree(nodes, Some(n.id), depth + 1, out);
        }
    }
}

/// A chevron that swings open when the row is about to receive a drop
/// *inside* - pure CSS off the row's `data-intent`, zero wiring.
#[component]
fn Chevron() -> Element {
    rsx! {
        svg {
            class: "h-3 w-3 shrink-0 text-[#BBB8AE] transition-transform duration-150 in-data-[intent=into]:rotate-90 in-data-[intent=into]:text-[#1C4A38]",
            "viewBox": "0 0 12 12",
            fill: "none",
            stroke: "currentColor",
            "stroke-width": "1.8",
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            "aria-hidden": "true",
            path { d: "M4.5 2.5 8 6l-3.5 3.5" }
        }
    }
}

#[component]
fn FilesTreeDemo() -> Element {
    let mut nodes = use_signal(|| {
        vec![
            FsNode {
                id: 1,
                parent: None,
                name: "src",
                folder: true,
            },
            FsNode {
                id: 2,
                parent: Some(1),
                name: "components",
                folder: true,
            },
            FsNode {
                id: 3,
                parent: Some(2),
                name: "button.rs",
                folder: false,
            },
            FsNode {
                id: 4,
                parent: Some(2),
                name: "card.rs",
                folder: false,
            },
            FsNode {
                id: 5,
                parent: Some(1),
                name: "main.rs",
                folder: false,
            },
            FsNode {
                id: 6,
                parent: None,
                name: "assets",
                folder: true,
            },
            FsNode {
                id: 7,
                parent: Some(6),
                name: "logo.svg",
                folder: false,
            },
            FsNode {
                id: 8,
                parent: None,
                name: "README.md",
                folder: false,
            },
        ]
    });
    let mut msg = use_signal(String::new);
    let mut flat = Vec::new();
    flatten_tree(&nodes.read(), None, 0, &mut flat);

    rsx! {
        Section {
            title: "Project files",
            note: "A real tree: every row drags and every row is a target. Top edge places before, the middle drops inside a folder (files refuse it), the bottom places after. Try dropping src into its own components folder: the cycle guard keeps the tree a tree.",
            tag: "would_create_cycle",
            DndProvider::<u64> {
                LiveRegion::<u64> {}
                div { class: "overflow-hidden rounded-xl bg-[#EEEADF] ring-1 ring-[#E8E5D9]",
                    for (depth, n) in flat {
                        TreeNodeTarget::<u64> {
                            key: "{n.id}",
                            node: NodeId(n.id),
                            row_height: 38.0,
                            label: n.name,
                            accepts: {
                                let target = n.id;
                                let folder = n.folder;
                                move |(dragged, intent): (u64, DropIntent)| {
                                    // Only folders can contain things.
                                    if intent == DropIntent::Into && !folder {
                                        return false;
                                    }
                                    // And nothing may land inside its own subtree.
                                    let ns = nodes.read();
                                    !would_create_cycle(
                                        |id: NodeId| {
                                            ns.iter().find(|x| x.id == id.0).and_then(|x| x.parent).map(NodeId)
                                        },
                                        NodeId(dragged),
                                        NodeId(target),
                                    )
                                }
                            },
                            on_drop: {
                                let target_id = n.id;
                                let target_name = n.name;
                                move |ev: TreeDropEvent<u64>| {
                                    let mut ns = nodes.write();
                                    let Some(drag_pos) = ns.iter().position(|x| x.id == ev.payload) else {
                                        return;
                                    };
                                    let mut dragged = ns.remove(drag_pos);
                                    let Some(tpos) = ns.iter().position(|x| x.id == target_id) else {
                                        ns.insert(drag_pos, dragged);
                                        return;
                                    };
                                    // Children keep pointing at the dragged node,
                                    // so the whole subtree travels with one write.
                                    let (new_parent, at) = match ev.intent {
                                        DropIntent::Into => (Some(target_id), ns.len()),
                                        DropIntent::Before => (ns[tpos].parent, tpos),
                                        DropIntent::After => (ns[tpos].parent, tpos + 1),
                                    };
                                    dragged.parent = new_parent;
                                    let name = dragged.name;
                                    ns.insert(at, dragged);
                                    drop(ns);
                                    let verb = match ev.intent {
                                        DropIntent::Before => "before",
                                        DropIntent::Into => "into",
                                        DropIntent::After => "after",
                                    };
                                    msg.set(format!("Moved {name} {verb} {target_name}"));
                                }
                            },
                            class: "border-b border-[#E8E5D9] transition last:border-0
                                    data-[intent=before]:shadow-[inset_0_2px_0_0_#1C4A38]
                                    data-[intent=after]:shadow-[inset_0_-2px_0_0_#1C4A38]
                                    data-[intent=into]:bg-[#6C9984]/18",
                            Draggable::<u64> {
                                payload: n.id,
                                label: n.name,
                                class: "block cursor-grab select-none transition hover:bg-[#E1DDCE]/50 active:cursor-grabbing data-dragging:opacity-40",
                                div { class: "flex items-center gap-2 py-2.5 pl-3 pr-3.5 text-[13px] font-medium text-[#2C2A25]",
                                    for _ in 0..depth {
                                        span { class: "ml-1 h-5 w-3.5 shrink-0 border-l border-[#D7D4C9]" }
                                    }
                                    if n.folder {
                                        Chevron {}
                                        FolderIcon {}
                                        span { class: "font-mono text-[12px]", "{n.name}/" }
                                    } else {
                                        span { class: "w-3 shrink-0" }
                                        span { class: "text-[#B88B2F]", DocGlyph {} }
                                        span { class: "font-mono text-[12px]", "{n.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
                if !msg.read().is_empty() {
                    p { class: "mt-2 text-xs text-[#45423B]", "{msg}" }
                }
            }
        }
    }
}

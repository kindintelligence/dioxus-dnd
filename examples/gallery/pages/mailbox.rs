//! Mailbox: live demo plus how the pattern works.

use std::collections::HashSet;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn MailboxPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Organize",
            title: "Mailbox",
            lead: "Click to select, Cmd-click to build a stack, then drag any selected row and the whole selection travels as one payload. The Receipts label files a copy when Cmd is held: one on_drop, two behaviors, decided by DropOutcome::effect.",
        }
        MailboxDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "use_selection hands you a shared Selection<K>; SelectableDraggable wires the click conventions (click selects one, Cmd or Ctrl click toggles) and carries Vec<K> as the drag payload. Dragging a selected row brings the whole selection; dragging an unselected one brings just itself."
                }
                p {
                    "Because the payload is a plain Vec of keys, the drop side is ordinary Rust: retain to remove, extend a HashSet to label, and read o.effect to decide whether the originals stay."
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
                        "data-selected",
                        "sits on each selected row for styling; it follows the shared Selection, not component-local state.",
                    ),
                    (
                        "Selection is a Copy handle:",
                        "clear(), len(), is_empty() and items() read and write the same shared state from anywhere.",
                    ),
                    (
                        "Ghosts stay cheap.",
                        "Render a count in the DragOverlay instead of cloning rows; dragging forty messages costs the same as one.",
                    ),
                    (
                        "SelectionCount",
                        "is a ready-made \"n item(s)\" ghost if you do not need custom copy.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"let mut selection = use_selection::<u32>();
rsx! {
    DndProvider::<Vec<u32>> {
        for mail in inbox() {
            SelectableDraggable::<u32> { item: mail.id, selection, MailRow { mail } }
        }
        DropZone::<Vec<u32>> {
            label: "Receipts",
            on_drop: move |o: DropOutcome<Vec<u32>>| {
                if o.effect == DropEffect::Copy {
                    labeled.write().extend(o.payload);   // originals stay
                } else {
                    inbox.write().retain(|m| !o.payload.contains(&m.id));
                }
            },
        }
        DragOverlay::<Vec<u32>> { StackGhost {} }
    }
}"#;

// --- 3. mailbox (multi-select + modifier copy/move, dark skin) ----------------

#[derive(Clone, Copy, PartialEq)]
struct Email {
    id: u32,
    from: &'static str,
    subject: &'static str,
    time: &'static str,
    unread: bool,
}

/// How many messages, worded for the status line.
fn messages(n: usize) -> String {
    if n == 1 {
        "1 message".to_string()
    } else {
        format!("{n} messages")
    }
}

/// The floating stack that follows the pointer: a count, not a copy of the
/// rows, so dragging forty messages costs the same as dragging one.
#[component]
fn MailGhost() -> Element {
    let dnd = use_dnd::<Vec<u32>>();
    let n = dnd.payload().map(|p| p.len()).unwrap_or(0);
    let word = if n == 1 { "message" } else { "messages" };
    rsx! { "{n} {word}" }
}

#[component]
fn MailboxDemo() -> Element {
    let mut selection = use_selection::<u32>();
    let mut inbox = use_signal(|| {
        vec![
            Email {
                id: 1,
                from: "Stripe",
                subject: "Your March invoice is ready",
                time: "9:12",
                unread: true,
            },
            Email {
                id: 2,
                from: "Mara Chen",
                subject: "Re: offsite agenda",
                time: "8:47",
                unread: true,
            },
            Email {
                id: 3,
                from: "GitHub",
                subject: "dioxus-dnd v1.0 released",
                time: "8:02",
                unread: false,
            },
            Email {
                id: 4,
                from: "Aeropress Club",
                subject: "Order #1180 has shipped",
                time: "7:31",
                unread: false,
            },
            Email {
                id: 5,
                from: "Linear",
                subject: "Weekly digest: 12 issues closed",
                time: "6:58",
                unread: false,
            },
        ]
    });
    // Ids that carry the Receipts label (a Cmd-drop keeps them in the inbox).
    let mut labeled = use_signal(HashSet::<u32>::new);
    let mut archived = use_signal(|| 0usize);
    let mut filed = use_signal(|| 0usize);
    let mut trashed = use_signal(|| 0usize);
    let mut status = use_signal(String::new);

    // Shared dark-skin zone styling: the same data attributes as everywhere
    // else, just dressed for a midnight panel.
    const DARK_ZONE: &str = "flex items-center justify-between gap-2 rounded-lg border border-dashed border-white/15 px-3 py-2.5 text-[12px] font-medium text-[#b8ab93] transition data-active:border-[#B8C4A9]/70 data-active:bg-[#B8C4A9]/10 data-over:border-solid data-over:border-[#D97D55] data-over:bg-[#D97D55]/15 data-over:text-[#f4e9d7]";

    rsx! {
        Section {
            title: "Mailbox",
            note: "Click to select, Cmd or Ctrl click to build a stack, then drag it. Archive and Trash move the messages out; drop on Receipts with Cmd held and they're filed as a copy, staying in your inbox. This is the panel that talked the rest of the gallery into going dark.",
            tag: "DropOutcome::effect",
            DndProvider::<Vec<u32>> {
                LiveRegion::<Vec<u32>> {}
                // No extra panel wrapper: since the whole page adopted this
                // demo's midnight language, the inbox sits on the section card
                // like every other well.
                div { class: "grid grid-cols-1 gap-3 sm:grid-cols-3",
                    div { class: "sm:col-span-2",
                        div { class: "mb-2 flex items-baseline justify-between px-1",
                            p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#8d8069]",
                                "Inbox · {inbox.read().len()}"
                            }
                            if !selection.is_empty() {
                                button {
                                    class: "rounded-md px-1.5 py-0.5 text-[11px] font-medium text-[#D97D55] transition hover:bg-white/5",
                                    onclick: move |_| selection.clear(),
                                    "Clear {selection.len()} selected"
                                }
                            }
                        }
                        div { class: "overflow-hidden rounded-lg bg-white/[0.03] ring-1 ring-white/5",
                            if inbox.read().is_empty() {
                                p { class: "py-8 text-center text-[12px] text-[#8d8069]",
                                    "Inbox zero. Beautiful."
                                }
                            }
                            for e in inbox.read().clone() {
                                SelectableDraggable::<u32> {
                                    key: "{e.id}",
                                    item: e.id,
                                    selection,
                                    label: e.subject,
                                    class: "block cursor-grab select-none border-b border-white/5 px-3 py-2.5 text-[13px] transition last:border-0 hover:bg-white/[0.04] active:cursor-grabbing data-selected:bg-[#D97D55]/15 data-dragging:opacity-40",
                                    div { class: "flex w-full items-center gap-2.5",
                                        span { class: if e.unread { "h-1.5 w-1.5 shrink-0 rounded-full bg-[#D97D55]" } else { "h-1.5 w-1.5 shrink-0 rounded-full bg-transparent" } }
                                        span { class: if e.unread { "w-24 shrink-0 truncate font-semibold text-[#f4e9d7]" } else { "w-24 shrink-0 truncate font-medium text-[#b8ab93]" },
                                            "{e.from}"
                                        }
                                        span { class: if e.unread { "min-w-0 flex-1 truncate text-[#d9cfbc]" } else { "min-w-0 flex-1 truncate text-[#9c8f77]" },
                                            "{e.subject}"
                                        }
                                        if labeled.read().contains(&e.id) {
                                            span { class: "shrink-0 rounded bg-[#B8C4A9]/20 px-1.5 py-0.5 text-[10px] font-semibold text-[#B8C4A9]",
                                                "Receipts"
                                            }
                                        }
                                        span { class: "shrink-0 text-[11px] tabular-nums text-[#8d8069]",
                                            "{e.time}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        p { class: "px-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#8d8069]",
                            "Drop to triage"
                        }
                        DropZone::<Vec<u32>> {
                            label: "Archive",
                            on_drop: move |o: DropOutcome<Vec<u32>>| {
                                let n = o.payload.len();
                                inbox.write().retain(|e| !o.payload.contains(&e.id));
                                selection.clear();
                                archived += n;
                                status.set(format!("Archived {}.", messages(n)));
                            },
                            class: DARK_ZONE,
                            span { "Archive" }
                            span { class: "min-w-5 rounded-full bg-white/8 px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums",
                                "{archived}"
                            }
                        }
                        DropZone::<Vec<u32>> {
                            label: "Receipts",
                            // The one zone where the drop *effect* matters:
                            // Ctrl/Cmd at release resolves to Copy, so the
                            // originals stay put and only the label lands.
                            on_drop: move |o: DropOutcome<Vec<u32>>| {
                                let n = o.payload.len();
                                if o.effect == DropEffect::Copy {
                                    labeled.write().extend(o.payload.iter().copied());
                                    selection.clear();
                                    status
                                        .set(
                                            format!(
                                                "Filed a copy of {}. The originals kept their seat.",
                                                messages(n),
                                            ),
                                        );
                                } else {
                                    inbox.write().retain(|e| !o.payload.contains(&e.id));
                                    selection.clear();
                                    filed += n;
                                    status
                                        .set(
                                            format!(
                                                "Moved {} to Receipts. Hold Cmd or Ctrl to file a copy instead.",
                                                messages(n),
                                            ),
                                        );
                                }
                            },
                            class: DARK_ZONE,
                            span {
                                "Receipts"
                                span { class: "ml-1.5 text-[10px] font-normal text-[#8d8069]",
                                    "⌘ copies"
                                }
                            }
                            span { class: "min-w-5 rounded-full bg-white/8 px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums",
                                "{filed}"
                            }
                        }
                        DropZone::<Vec<u32>> {
                            label: "Trash",
                            on_drop: move |o: DropOutcome<Vec<u32>>| {
                                let n = o.payload.len();
                                inbox.write().retain(|e| !o.payload.contains(&e.id));
                                labeled.write().retain(|id| !o.payload.contains(id));
                                selection.clear();
                                trashed += n;
                                status.set(format!("Deleted {}.", messages(n)));
                            },
                            class: DARK_ZONE,
                            span { "Trash" }
                            span { class: "min-w-5 rounded-full bg-white/8 px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums",
                                "{trashed}"
                            }
                        }
                        if !status.read().is_empty() {
                            p { class: "mt-auto px-1 pt-2 text-[11px] leading-relaxed text-[#B8C4A9]",
                                "{status}"
                            }
                        }
                    }
                }
                DragOverlay::<Vec<u32>> { class: "pointer-events-none rotate-2 rounded-lg bg-[#3d352a] px-3.5 py-2 text-[12px] font-semibold text-[#f4e9d7] shadow-[inset_0_1px_0_rgba(255,255,255,0.09),inset_0_0_0_1px_rgba(255,255,255,0.04),0_20px_44px_-12px_rgba(0,0,0,0.65)]",
                    MailGhost {}
                }
            }
        }
    }
}

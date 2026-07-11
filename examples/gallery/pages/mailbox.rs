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
            lead: "Multi-select triage, the way mail apps do it: click to select one, Cmd or Ctrl click to build a stack, then drag any selected row and the whole selection travels as a single payload of keys.",
        }
        MailboxDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Selection is shared state.",
                        "use_selection::<u32>() returns a Selection handle: a cheap-to-copy value you can read and write from anywhere in the tree. It is the single source of truth for which rows are selected.",
                    ),
                    (
                        "Rows wire the click conventions.",
                        "SelectableDraggable handles the platform behavior for you: a plain click selects only that row, Ctrl or Cmd click toggles it into the stack, and each selected row carries data-selected for styling.",
                    ),
                    (
                        "The payload is a Vec of keys.",
                        "The provider's type is Vec<u32>, not the whole email struct. Dragging a selected row carries every selected key; dragging an unselected row carries just that one. Keys keep the payload tiny and your model authoritative.",
                    ),
                    (
                        "Drops are ordinary Rust.",
                        "A zone receives DropOutcome<Vec<u32>>. Remove rows with retain, file labels by extending a HashSet, and branch on the effect: here, Cmd at release means Copy, so Receipts labels the originals instead of moving them.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Note what is absent: no selection bookkeeping in your model, no per-row event handlers, no special multi-drag mode. The selection handle and the Vec payload carry all of it."
                }
            }
            DioxusNote {
                p {
                    "Selection is Copy, like all Dioxus signal handles: passing it into a component or closure hands out another key to the same shared state, not a duplicate of the data. That is why every row receives the same selection and they all stay in sync."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "SelectableDraggable props",
                rows: vec![
                    ("item", "K, required", "This row's key. K is any Clone + PartialEq type; ids are typical."),
                    ("selection", "Selection<K>, required", "The shared selection from use_selection."),
                    ("zone", "Option<ZoneId>", "The zone this row lives in, forwarded to DropOutcome::from."),
                    ("effect", "DropEffect = Move", "Base effect for drags starting here; modifiers can still override at release."),
                    ("label", "Option<String>", "Screen-reader name for pickup announcements."),
                ],
            }
            PropsTable {
                title: "Selection<K> methods",
                rows: vec![
                    ("click(key, modifiers)", "", "The standard convention in one call: plain click selects only this key, Ctrl or Cmd click toggles it. SelectableDraggable calls this for you."),
                    ("select_only / toggle / clear", "", "Direct control when you build custom interactions (a select-all checkbox, shift ranges)."),
                    ("is_selected(&key)", "-> bool", "Membership test; drives data-selected."),
                    ("items / len / is_empty", "", "Snapshot of the selected keys in selection order, and the usual size queries."),
                ],
            }
            PropsTable {
                title: "Ghost helpers",
                rows: vec![
                    ("SelectionCount::<K>", "component", "A ready-made \"n item(s)\" badge for DragOverlay::<Vec<K>> when you don't need custom copy."),
                    ("use_dnd::<Vec<K>>()", "hook", "Inside your own ghost, read the in-flight payload to render a count or a preview, as MailGhost does here."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Ghosts stay cheap.",
                        "Render a count in the DragOverlay instead of cloning rows; dragging forty messages costs the same as dragging one.",
                    ),
                    (
                        "Dragging an unselected row carries just that row.",
                        "The selection stays as it was, so a stray drag never dismantles a carefully built stack.",
                    ),
                    (
                        "Clear the selection in your drop handler.",
                        "The library doesn't guess whether a completed triage should keep the stack; this demo clears it after every drop.",
                    ),
                    (
                        "data-selected follows the shared Selection,",
                        "not component-local state, so a clear-all button anywhere updates every row.",
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

    // Shared triage-zone styling: the same data attributes as everywhere
    // else, dressed as compact rows beside the inbox well.
    const TRIAGE_ZONE: &str = "flex items-center justify-between gap-2 rounded-lg border border-dashed border-[#7A776C]/30 px-3 py-2.5 text-[12px] font-medium text-[#45423B] transition data-active:border-[#6C9984]/70 data-active:bg-[#6C9984]/10 data-over:border-solid data-over:border-[#1C4A38] data-over:bg-[#1C4A38]/15 data-over:text-[#1A1815]";

    rsx! {
        Section {
            title: "Mailbox",
            note: "Click to select, Cmd or Ctrl click to build a stack, then drag it. Archive and Trash move the messages out; drop on Receipts with Cmd held and they're filed as a copy, staying in your inbox.",
            tag: "DropOutcome::effect",
            DndProvider::<Vec<u32>> {
                LiveRegion::<Vec<u32>> {}
                // No extra panel wrapper: the inbox sits on the section card
                // as a recessed well, like every other list in the gallery.
                div { class: "grid grid-cols-1 gap-3 sm:grid-cols-3",
                    div { class: "sm:col-span-2",
                        div { class: "mb-2 flex items-baseline justify-between px-1",
                            p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9B988D]",
                                "Inbox · {inbox.read().len()}"
                            }
                            if !selection.is_empty() {
                                button {
                                    class: "rounded-md px-1.5 py-0.5 text-[11px] font-medium text-[#1C4A38] transition hover:bg-[#E1DDCE]/50",
                                    onclick: move |_| selection.clear(),
                                    "Clear {selection.len()} selected"
                                }
                            }
                        }
                        div { class: "overflow-hidden rounded-lg bg-[#EEEADF] ring-1 ring-[#E8E5D9]",
                            if inbox.read().is_empty() {
                                p { class: "py-8 text-center text-[12px] text-[#9B988D]",
                                    "Inbox zero. Beautiful."
                                }
                            }
                            for e in inbox.read().clone() {
                                SelectableDraggable::<u32> {
                                    key: "{e.id}",
                                    item: e.id,
                                    selection,
                                    label: e.subject,
                                    class: "block cursor-grab select-none border-b border-[#E8E5D9] px-3 py-2.5 text-[13px] transition last:border-0 hover:bg-[#E1DDCE]/50 active:cursor-grabbing data-selected:bg-[#1C4A38]/15 [&_[data-dragging]]:opacity-40",
                                    div { class: "flex w-full items-center gap-2.5",
                                        span { class: if e.unread { "h-1.5 w-1.5 shrink-0 rounded-full bg-[#1C4A38]" } else { "h-1.5 w-1.5 shrink-0 rounded-full bg-transparent" } }
                                        span { class: if e.unread { "w-24 shrink-0 truncate font-semibold text-[#1A1815]" } else { "w-24 shrink-0 truncate font-medium text-[#45423B]" },
                                            "{e.from}"
                                        }
                                        span { class: if e.unread { "min-w-0 flex-1 truncate text-[#2C2A25]" } else { "min-w-0 flex-1 truncate text-[#7A776C]" },
                                            "{e.subject}"
                                        }
                                        if labeled.read().contains(&e.id) {
                                            span { class: "shrink-0 rounded bg-[#6C9984]/20 px-1.5 py-0.5 text-[10px] font-semibold text-[#6C9984]",
                                                "Receipts"
                                            }
                                        }
                                        span { class: "shrink-0 text-[11px] tabular-nums text-[#9B988D]",
                                            "{e.time}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flex flex-col gap-2",
                        p { class: "px-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9B988D]",
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
                            class: TRIAGE_ZONE,
                            span { "Archive" }
                            span { class: "min-w-5 rounded-full bg-[#E4ECDD] px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums text-[#1C4A38] ring-1 ring-[#CFDDCF]",
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
                            class: TRIAGE_ZONE,
                            span {
                                "Receipts"
                                span { class: "ml-1.5 text-[10px] font-normal text-[#9B988D]",
                                    "⌘ copies"
                                }
                            }
                            span { class: "min-w-5 rounded-full bg-[#E4ECDD] px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums text-[#1C4A38] ring-1 ring-[#CFDDCF]",
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
                            class: TRIAGE_ZONE,
                            span { "Trash" }
                            span { class: "min-w-5 rounded-full bg-[#E4ECDD] px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums text-[#1C4A38] ring-1 ring-[#CFDDCF]",
                                "{trashed}"
                            }
                        }
                        if !status.read().is_empty() {
                            p { class: "mt-auto px-1 pt-2 text-[11px] leading-relaxed text-[#6C9984]",
                                "{status}"
                            }
                        }
                    }
                }
                DragOverlay::<Vec<u32>> {
                    settle: true,
                    duration: 160.0,
                    easing: "cubic-bezier(0.22, 1, 0.36, 1)",
                    class: "pointer-events-none rotate-2 rounded-lg bg-[#FBFAF6] px-3.5 py-2 text-[12px] font-semibold text-[#1A1815] shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.06),0_2px_4px_rgba(26,24,21,0.10),0_12px_24px_-8px_rgba(26,24,21,0.12)]",
                    MailGhost {}
                }
            }
        }
    }
}

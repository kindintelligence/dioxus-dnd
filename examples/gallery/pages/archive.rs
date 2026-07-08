//! Archive: drag-and-drop inside a 10,000-row virtualized list.

use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn ArchivePage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Scale",
            title: "Archive",
            lead: "Ten thousand rows, one scrollbar, and every row is a drop zone - but only the visible slice exists. Zones register and unregister as the list recycles, measure themselves the moment they mount, and drops land on what you see, even after auto-scrolling half the archive mid-drag.",
        }
        ArchiveDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Windowing is plain math.",
                        "Fixed-height rows make the visible slice scroll_top / row_height with a buffer on each side, drawn inside a full-height canvas with a translateY offset - the standard virtualization shape. The 10,000-row list mounts about forty DropZones at any moment.",
                    ),
                    (
                        "Zones churn; the registry doesn't care.",
                        "A row scrolling out unregisters its zone; a row scrolling in registers one and measures itself on mount, so it's hit-testable the moment it exists - even if it appeared mid-drag, after the pickup measurement and the last scroll ping both ran.",
                    ),
                    (
                        "Rows are their own scroll sentinels.",
                        "dioxus-web 0.7 delivers no element scroll events, so the rows carry onvisible: crossing the container's clip fires an IntersectionObserver entry whose rect, plus the row's canvas position, recovers the scroll offset - for wheel, scrollbar and programmatic scrolls alike, idle or mid-drag. AutoScroll's on_scroll adds the edge-auto-scroll reports (sampled through MountedData after its own scrolling) and pings the rect-refresh channel.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "The library needs nothing special for virtualization - rows are ordinary DropZones with stable index-derived ids, and remounting one replaces its registration in place. What the demo adds is the windowing itself; swap in any virtual list, keep the zones."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "The pieces at play",
                rows: vec![
                    ("onvisible (on each row)", "IntersectionObserver entry", "The windowing signal: any row crossing the container's clip reports, and entry rect + row index recover scroll_top. Dioxus's documented virtual-list tool."),
                    ("AutoScroll::on_scroll", "EventHandler<Point>", "The offset after events AutoScroll can observe - its own edge scrolling above all - following the rect-refresh ping."),
                    ("DropZone id", "ZoneId(BASE + index)", "Stable per row index, so a recycled row re-registers as itself and handlers can name rows."),
                    ("aria-setsize / aria-posinset", "forwarded attributes", "Screen readers announce position in the full 10,000 even though ~40 rows exist."),
                    ("use_rect_refresh()", "-> RectRefresh", "Only needed when your scroll container is not an AutoScroll: ping refresh_all() from your own onscroll."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Keyboard navigation walks the mounted window:",
                        "arrows step through the ~40 registered rows in spatial order. Scroll first, then pick up - the zones under the viewport are the ones a keyboard drag can reach.",
                    ),
                    (
                        "Drop handlers capture the row index,",
                        "so the model update is an ordinary map insert - no bookkeeping about which DOM node happened to host the row.",
                    ),
                    (
                        "The mounted counter is the registry itself:",
                        "it renders from ZoneRegistry::records(), the same subscribing read the debug overlay uses.",
                    ),
                    (
                        "Hovering the top or bottom edge mid-drag auto-scrolls",
                        "the archive under the pointer; releases land on the row that scrolled into place (the stale-rects machinery from 2.3.0 at work).",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"AutoScroll {
    style: "height: 420px; overflow-y: auto;",
    on_scroll: move |offset: Point| scroll_top.set(offset.y), // edge auto-scroll
    div { style: "position: relative; height: {ROWS as f64 * ROW_H}px;",
        div { style: "position: absolute; top: 0; width: 100%;
                      transform: translateY({first as f64 * ROW_H}px);",
            for ix in first..last {
                DropZone::<Tag> {
                    key: "{ix}",
                    id: ZoneId(BASE + ix as u64),   // stable per row
                    label: format!("Record {}", ix + 1),
                    on_drop: move |o: DropOutcome<Tag>| tag_row(ix, o.payload),
                    "aria-setsize": "{ROWS}",
                    "aria-posinset": "{ix + 1}",
                    div {
                        // rows double as scroll sentinels (see below)
                        onvisible: move |evt| resync_window(ix, evt),
                        Row { ix }
                    }
                }
            }
        }
    }
}"#;

// --- 18. archive (drag-and-drop in a 10k-row virtual list) -------------------

const ROWS: usize = 10_000;
const ROW_H: f64 = 44.0;
const VIEW_H: f64 = 420.0;
const BUFFER: usize = 6;
/// Row zone ids: 20_000..30_000, clear of every other page's explicit ids.
const ROW_BASE: u64 = 20_000;

const DEPARTMENTS: [&str; 8] = [
    "Cartography",
    "Botany",
    "Acquisitions",
    "Restoration",
    "Field notes",
    "Correspondence",
    "Instruments",
    "Expeditions",
];

const TAGS: [(&str, &str); 3] = [
    ("Urgent", "bg-[#B84A39]"),
    ("Review", "bg-[#B88B2F]"),
    ("Digitize", "bg-[#3E7558]"),
];

fn tag_color(tag: &str) -> &'static str {
    TAGS.iter()
        .find(|(name, _)| *name == tag)
        .map(|(_, c)| *c)
        .unwrap_or("bg-[#7A776C]")
}

#[component]
fn ArchiveDemo() -> Element {
    let mut scroll_top = use_signal(|| 0.0f64);
    let mut tagged = use_signal(HashMap::<usize, &'static str>::new);
    let mut status = use_signal(String::new);
    // The container's mounted handle, for turning a row's viewport position
    // back into a scroll offset (the wrapper shares the container's top).
    let container = use_signal(|| None::<std::rc::Rc<dioxus::html::MountedData>>);

    let first = ((scroll_top() / ROW_H) as usize).saturating_sub(BUFFER);
    let visible = (VIEW_H / ROW_H).ceil() as usize + 2 * BUFFER;
    let last = (first + visible).min(ROWS);

    rsx! {
        Section {
            title: "Archive",
            note: "Tag records by dropping a chip anywhere in the list - scroll first, or let the drag itself scroll by hovering the edges. The counter shows how few zones actually exist.",
            tag: "AutoScroll::on_scroll",
            DndProvider::<&'static str> {
                LiveRegion::<&'static str> {}
                MountedCounter { window: last - first }
                div { class: "mb-3 flex flex-wrap items-center gap-1.5",
                    for (name , color) in TAGS {
                        Draggable::<&'static str> {
                            key: "{name}",
                            payload: name,
                            label: name,
                            class: "cursor-grab select-none rounded-md px-2.5 py-1.5 text-[12px] font-medium text-[#F6F3EC] {color} shadow-[0_1px_2px_rgba(26,24,21,0.15)] transition hover:-translate-y-px active:cursor-grabbing data-dragging:opacity-40",
                            "{name}"
                        }
                    }
                    if !status.read().is_empty() {
                        span { class: "ml-1 font-mono text-[11px] text-[#7A776C]", "{status}" }
                    }
                }
                div {
                    onmounted: move |evt: Event<dioxus::html::MountedData>| {
                        let mut container = container;
                        container.set(Some(evt.data()));
                    },
                    AutoScroll {
                    class: "rounded-xl bg-[#F6F3EC] ring-1 ring-[#E8E5D9]",
                    style: "height: {VIEW_H}px; overflow-y: auto;",
                    on_scroll: move |offset: Point| scroll_top.set(offset.y),
                    div {
                        style: "position: relative; height: {ROWS as f64 * ROW_H}px;",
                        role: "list",
                        div {
                            style: "position: absolute; top: 0; left: 0; width: 100%; transform: translateY({first as f64 * ROW_H}px);",
                            for ix in first..last {
                                DropZone::<&'static str> {
                                    key: "{ix}",
                                    id: ZoneId(ROW_BASE + ix as u64),
                                    label: format!("Record {}", ix + 1),
                                    on_drop: move |o: DropOutcome<&'static str>| {
                                        tagged.write().insert(ix, o.payload);
                                        status.set(format!("{} → record {}", o.payload, ix + 1));
                                    },
                                    class: "block px-3 transition-colors data-over:bg-[#E4ECDD]",
                                    style: "height: {ROW_H}px;",
                                    role: "listitem",
                                    aria_setsize: "{ROWS}",
                                    aria_posinset: "{ix + 1}",
                                    // Rendered rows are their own scroll sentinels: crossing
                                    // the container's clip fires onvisible, and the entry rect
                                    // plus the row's canvas position recover the offset. This
                                    // is what re-slices the window for wheel, scrollbar and
                                    // programmatic scrolls - scroll events never reach
                                    // dioxus-web 0.7, so observers carry the signal.
                                    div {
                                        class: "flex h-full items-center gap-3 border-b border-[#E8E5D9] text-[12.5px]",
                                        onvisible: move |evt: VisibleEvent| {
                                            let Ok(r) = evt.data().get_bounding_client_rect() else {
                                                return;
                                            };
                                            let row_y = r.origin.y;
                                            let mut scroll_top = scroll_top;
                                            spawn(async move {
                                                let Some(c) = container.peek().clone() else { return };
                                                if let Ok(cr) = c.get_client_rect().await {
                                                    let s = (ix as f64 * ROW_H) - (row_y - cr.origin.y);
                                                    scroll_top.set(s.max(0.0));
                                                }
                                            });
                                        },
                                        span { class: "w-14 shrink-0 text-right font-mono text-[11px] tabular-nums text-[#BBB8AE]",
                                            {format!("{:05}", ix + 1)}
                                        }
                                        span { class: "min-w-0 flex-1 truncate text-[#45423B]",
                                            {format!("Record of {}", DEPARTMENTS[ix % DEPARTMENTS.len()])}
                                        }
                                        if let Some(tag) = tagged.read().get(&ix) {
                                            span { class: "shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium text-[#F6F3EC] {tag_color(tag)}",
                                                "{tag}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                }
            }
        }
    }
}

/// The registry's live census, rendered where visitors can see the point:
/// ten thousand rows, a few dozen zones.
#[component]
fn MountedCounter(window: usize) -> Element {
    let registry = use_zone_registry::<&'static str>();
    let mounted = registry.records().len();
    rsx! {
        p { class: "mb-2 text-[11px] text-[#9B988D]",
            span { class: "font-mono tabular-nums text-[#45423B]", "{mounted}" }
            " zones registered for "
            span { class: "font-mono tabular-nums text-[#45423B]", "10,000" }
            " rows (window of {window})"
        }
    }
}

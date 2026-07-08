//! A clean gallery of every dioxus-dnd pattern, each framed as a small, real
//! interface (a reading list, a playlist, a kanban board, a moodboard) rather
//! than an abstract "Tile 1 / Tile 2" demo. Everything runs on the web pointer
//! path (`input: DragInputMode::Pointer`): the dragged item dims, drop targets
//! highlight, and completed drops flash into place.
//!
//! Palette: midnight umber surfaces (#211c15 page, #2b2620 cards, #362f26
//! items) under clay #D97D55, cream #F4E9D7, sage #B8C4A9 and teal #6FA4AF
//! accents. The dark mailbox panel set the mood; the page followed it.
//!
//! Run:
//! ```sh
//! dx serve --example gallery --platform web --features web
//! ```
//! (The `web` feature enables native pointer capture so mouse drags stay glued
//! to the pointer. Touch and pen work either way.)

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

fn main() {
    dioxus::launch(App);
}

// Shared tokens. `ITEM` is the draggable box (border/background/padding);
// `ROW` is the flex layout for its contents. They're split because
// `PointerDraggable` forwards `class` to an outer wrapper and renders
// `children` inside a nested block element, so a `flex` on `ITEM` would never
// reach the contents. Wrapping them in an explicit `ROW` div lays them out
// correctly regardless.
// Card recipe: top-lit gradient surface, a 1px inset edge highlight (the
// "machined" line every dark card needs), a real ambient shadow, and a
// one-pixel hover lift that presses back down on grab.
const ITEM: &str = "group block cursor-grab select-none rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] px-3.5 py-2.5 text-[13px] text-[#f4e9d7] shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.03),0_1px_2px_rgba(0,0,0,0.5),0_4px_12px_-4px_rgba(0,0,0,0.4)] transition hover:-translate-y-px hover:brightness-[1.06] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.09),inset_0_0_0_1px_rgba(255,255,255,0.04),0_2px_4px_rgba(0,0,0,0.5),0_12px_24px_-8px_rgba(0,0,0,0.55)] active:translate-y-0 active:cursor-grabbing data-dragging:opacity-50";
const ROW: &str = "flex w-full items-center gap-2.5";
const ZONE: &str = "rounded-xl border border-dashed border-white/15 p-3.5 min-h-24 transition space-y-2 data-active:border-[#B8C4A9] data-active:bg-[#B8C4A9]/12 data-over:border-solid data-over:border-[#D97D55] data-over:bg-[#D97D55]/15";

// Base type is Inter. `:focus:not(:focus-visible)` removes the mouse/drag focus
// outline (the ugly ring while dragging) while preserving keyboard focus rings.
// `drop-flash` is a one-shot ring + lifted shadow that pulses and settles, so a
// completed drop reads clearly even when the layout barely moves; it is shadow
// only (no transform) so it composes with FLIP transforms.
const BASE_CSS: &str = r#"
html {
  font-family: 'Inter', ui-sans-serif, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
  -webkit-tap-highlight-color: transparent;
  /* Midnight theme: paint the root too (overscroll, rubber-banding) and let
     the browser render native scrollbars and controls in their dark forms. */
  background: #211c15;
  color-scheme: dark;
  /* Sidebar anchor navigation glides instead of jumping. */
  scroll-behavior: smooth;
}
*:focus:not(:focus-visible) { outline: none; }
@keyframes drop-flash {
  0%   { box-shadow: 0 0 0 3px rgba(217,125,85,0.35), 0 20px 40px -12px rgba(0,0,0,0.38); }
  55%  { box-shadow: 0 0 0 1px rgba(217,125,85,0.12), 0 6px 16px -6px rgba(0,0,0,0.16); }
  100% { box-shadow: 0 0 0 0 rgba(217,125,85,0),      0 1px 2px 0 rgba(0,0,0,0); }
}
.drop-flash { animation: drop-flash 600ms cubic-bezier(0.22, 1, 0.36, 1); }
/* First-class keyboard focus: every draggable (core Draggable renders
   aria-roledescription) gets a clay ring on focus-visible instead of the
   browser default outline. */
[aria-roledescription="draggable"]:focus-visible {
  outline: 2px solid rgba(217,125,85,0.75);
  outline-offset: 2px;
  border-radius: 10px;
}
"#;

#[component]
fn App() -> Element {
    // Off-canvas drawer state for the mobile sidebar. On lg+ screens the
    // sidebar is pinned open and this signal is ignored.
    let mut nav_open = use_signal(|| false);
    rsx! {
        document::Script { src: "https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4" }
        document::Link { rel: "preconnect", href: "https://fonts.googleapis.com" }
        document::Link {
            rel: "preconnect",
            href: "https://fonts.gstatic.com",
            crossorigin: "",
        }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap",
        }
        style { {BASE_CSS} }
        div { class: "min-h-screen bg-[#211c15] text-[#f4e9d7] antialiased selection:bg-[#D97D55] selection:text-white lg:pl-64",
            Sidebar { open: nav_open }
            // The mobile drawer toggle: double chevrons pointing the way the
            // drawer will move. Hidden on lg+, where the sidebar is pinned.
            button {
                class: "fixed left-4 top-4 z-50 grid h-10 w-10 place-items-center rounded-xl bg-[#2b2620]/90 text-[#e0a37f] shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_8px_20px_-6px_rgba(0,0,0,0.6)] ring-1 ring-white/10 backdrop-blur transition active:scale-95 lg:hidden",
                aria_label: if nav_open() { "Close navigation" } else { "Open navigation" },
                onclick: move |_| {
                    let v = nav_open();
                    nav_open.set(!v);
                },
                DoubleChevron { open: nav_open() }
            }
            div { class: "mx-auto max-w-3xl px-5 py-14 sm:px-6 sm:py-20",
                Header {}

                GroupLabel {
                    kicker: "Organize",
                    title: "Move things where they belong.",
                }
                div { class: "space-y-4",
                    ReadingListDemo {}
                    NewsletterDemo {}
                    MailboxDemo {}
                }

                GroupLabel {
                    kicker: "Reorder",
                    title: "Put things in the right order.",
                }
                div { class: "space-y-4",
                    PlaylistDemo {}
                    PriorityDemo {}
                    AlbumDemo {}
                    QueueDemo {}
                }

                GroupLabel { kicker: "Structure", title: "Give it shape." }
                div { class: "space-y-4",
                    SprintDemo {}
                    FilesTreeDemo {}
                    MoodboardDemo {}
                }

                GroupLabel { kicker: "Motion", title: "Animate the change." }
                div { class: "space-y-4",
                    ShuffleDemo {}
                    MenuDemo {}
                }

                GroupLabel {
                    kicker: "Beyond the window",
                    title: "Cross the app boundary.",
                }
                div { class: "space-y-4",
                    UploadDemo {}
                    ShareDemo {}
                }

                footer { class: "mt-14 border-t border-white/8 pt-7 text-[12px] text-[#8d8069]",
                    "Built with "
                    span { class: "font-medium text-[#b8ab93]", "dioxus-dnd" }
                    ". Every interface above is a screenful of code, styled by you."
                }
            }
        }
    }
}

// --- scaffolding -------------------------------------------------------------

/// Kebab-case anchor id derived from a title ("Weekly focus" -> "weekly-focus").
/// `GroupLabel` and `Section` derive their ids with this, so sidebar links
/// need nothing but the visible title.
fn slug(s: &str) -> String {
    s.to_lowercase().replace(' ', "-")
}

/// A double chevron that points into the drawer's direction of travel:
/// >> to open, << (rotated) to close.
#[component]
fn DoubleChevron(open: bool) -> Element {
    rsx! {
        svg {
            class: if open { "h-4 w-4 rotate-180 transition-transform duration-300" } else { "h-4 w-4 transition-transform duration-300" },
            "viewBox": "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            "stroke-width": "1.8",
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            "aria-hidden": "true",
            path { d: "M3.5 3.5 8 8l-4.5 4.5" }
            path { d: "M8.5 3.5 13 8l-4.5 4.5" }
        }
    }
}

/// Site navigation: a detached floating card pinned on the left from lg up,
/// and an off-canvas drawer below that, opened by the double-chevron button
/// in `App` and dismissed by the scrim, the flipped chevrons, or any link.
#[component]
fn Sidebar(open: Signal<bool>) -> Element {
    let mut open = open;
    // Mirrors the page: group kickers and demo titles. Hrefs are derived
    // from the titles with `slug`, matching the ids `Section` renders.
    const NAV: &[(&str, &[&str])] = &[
        ("Organize", &["Reading list", "Newsletter builder", "Mailbox"]),
        ("Reorder", &["Playlist", "Weekly focus", "Photo album", "Podcast queue"]),
        ("Structure", &["Sprint board", "Project files", "Moodboard"]),
        ("Motion", &["Shuffle", "Menu"]),
        ("Beyond the window", &["Upload", "Share"]),
    ];
    // Mobile: slide fully off-canvas (including shadow) when closed; the
    // drawer starts below the fixed toggle button. Desktop: always pinned,
    // full height, detached from every edge.
    let shell = if open() {
        "translate-x-0"
    } else {
        "-translate-x-[120%] lg:translate-x-0"
    };
    rsx! {
        // Scrim: tap anywhere off the drawer to close it. Mobile only.
        if open() {
            div {
                class: "fixed inset-0 z-30 bg-black/60 backdrop-blur-[2px] lg:hidden",
                onclick: move |_| open.set(false),
            }
        }
        aside { class: "fixed bottom-4 left-4 top-16 z-40 flex w-64 flex-col overflow-y-auto rounded-2xl bg-[#26211a]/95 p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_24px_60px_-24px_rgba(0,0,0,0.8)] ring-1 ring-white/8 backdrop-blur transition-transform duration-300 lg:bottom-5 lg:left-5 lg:top-5 lg:w-56 {shell}",
            div { class: "mb-2 flex items-center gap-2 px-2.5 pt-1",
                span { class: "h-2 w-2 shrink-0 rounded-full bg-[#D97D55] shadow-[0_0_10px_rgba(217,125,85,0.6)]" }
                span { class: "text-[14px] font-semibold tracking-tight text-[#f4e9d7]",
                    "dioxus-dnd"
                }
                span { class: "ml-auto rounded-full bg-white/8 px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-[#8d8069]",
                    "14"
                }
            }
            nav { aria_label: "Sections", class: "flex-1",
                for (group, items) in NAV.iter() {
                    p { class: "px-2.5 pb-1 pt-4 text-[10px] font-semibold uppercase tracking-[0.16em] text-[#6FA4AF]",
                        "{group}"
                    }
                    for item in items.iter() {
                        a {
                            href: "#{slug(item)}",
                            class: "block rounded-lg px-2.5 py-1.5 text-[13px] font-medium text-[#9c8f77] transition hover:bg-white/5 hover:text-[#f4e9d7]",
                            onclick: move |_| open.set(false),
                            "{item}"
                        }
                    }
                }
            }
            p { class: "px-2.5 pb-1 pt-3 text-[11px] leading-relaxed text-[#6d6150]",
                "Every pattern, one library."
            }
        }
    }
}

#[component]
fn Header() -> Element {
    rsx! {
        header { class: "mb-12",
            p { class: "text-[12px] font-semibold uppercase tracking-[0.18em] text-[#D97D55]",
                "Dioxus · Drag & Drop"
            }
            h1 { class: "mt-3 text-3xl font-semibold tracking-tight text-[#f4e9d7] sm:text-4xl",
                "Pick it up, put it anywhere."
            }
            p { class: "mt-3 max-w-xl text-[14px] leading-relaxed text-[#b8ab93]",
                "Fourteen real interfaces, each a few lines over one library pattern. A mailbox, a sprint board, a file tree that reparents, a moodboard, all on the web pointer path."
            }
            div { class: "mt-5 flex flex-wrap gap-2",
                for chip in ["Pointer-native", "Keyboard-accessible", "Bring your own styles"] {
                    span { class: "rounded-full bg-white/8 px-2.5 py-1 text-[11px] font-medium text-[#b8ab93]",
                        "{chip}"
                    }
                }
            }
        }
    }
}

/// A light section divider: a small uppercase label and a one-line lead.
/// Carries a `slug(kicker)` id so sidebar group labels can link to it.
#[component]
fn GroupLabel(kicker: String, title: String) -> Element {
    rsx! {
        div {
            id: slug(&kicker),
            class: "mb-4 mt-14 flex flex-wrap items-baseline gap-x-3 gap-y-1 scroll-mt-20 first:mt-2 lg:scroll-mt-8",
            h2 { class: "text-[12px] font-semibold uppercase tracking-[0.18em] text-[#6FA4AF]",
                "{kicker}"
            }
            p { class: "text-[13px] text-[#9c8f77]", "{title}" }
        }
    }
}

#[component]
fn Section(title: String, note: String, tag: String, children: Element) -> Element {
    rsx! {
        section {
            // The sidebar links here by the slug of the visible title.
            id: slug(&title),
            class: "rounded-2xl bg-[#2b2620] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_1px_2px_rgba(0,0,0,0.3),0_18px_36px_-24px_rgba(0,0,0,0.6)] scroll-mt-20 sm:p-6 lg:scroll-mt-8",
            div { class: "mb-5 flex items-start justify-between gap-4",
                div { class: "min-w-0",
                    h3 { class: "text-[15px] font-semibold tracking-tight text-[#f4e9d7]",
                        "{title}"
                    }
                    p { class: "mt-1 text-[13px] leading-relaxed text-[#b8ab93]",
                        "{note}"
                    }
                }
                code { class: "mt-0.5 hidden shrink-0 rounded-md bg-white/10 px-2 py-1 font-mono text-[11px] text-[#e0a37f] ring-1 ring-white/10 sm:block",
                    "{tag}"
                }
            }
            {children}
        }
    }
}

/// A small folder glyph for tree rows.
#[component]
fn FolderIcon() -> Element {
    rsx! {
        svg {
            class: "h-4 w-4 shrink-0 text-[#B8A98C]",
            "viewBox": "0 0 20 20",
            fill: "currentColor",
            "aria-hidden": "true",
            path { d: "M3 5.75A1.75 1.75 0 0 1 4.75 4h2.8c.46 0 .9.18 1.24.51l.9.9c.13.12.3.19.48.19h4.08A1.75 1.75 0 0 1 18 7.35v6.9A1.75 1.75 0 0 1 16.25 16H4.75A1.75 1.75 0 0 1 3 14.25v-8.5Z" }
        }
    }
}

/// A dog-eared document glyph for file rows.
#[component]
fn DocGlyph() -> Element {
    rsx! {
        svg {
            class: "h-4 w-4 shrink-0",
            "viewBox": "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            "stroke-width": "1.7",
            "stroke-linecap": "round",
            "stroke-linejoin": "round",
            "aria-hidden": "true",
            path { d: "M6 3.5h7l5 5V20a1.5 1.5 0 0 1-1.5 1.5H6A1.5 1.5 0 0 1 4.5 20V5A1.5 1.5 0 0 1 6 3.5Z" }
            path { d: "M13 3.5V9h5" }
        }
    }
}

// --- shared card model (reading list, newsletter blocks, calendar) -----------

#[derive(Clone, PartialEq)]
struct Card {
    id: u32,
    title: String,
    sub: String,
}

impl Card {
    fn new(id: u32, title: &str, sub: &str) -> Self {
        Card { id, title: title.into(), sub: sub.into() }
    }
}

/// A palette accent bar, derived from a card's id so it stays stable across
/// moves. Cycles clay, teal, sage.
fn swatch(id: u32) -> &'static str {
    // Each bar blooms softly in its own color: accents that glow read as
    // premium on a dark surface where a flat bar would just sit there.
    const C: [&str; 3] = [
        "bg-[#D97D55] shadow-[0_0_10px_rgba(217,125,85,0.5)]",
        "bg-[#6FA4AF] shadow-[0_0_10px_rgba(111,164,175,0.5)]",
        "bg-[#B8C4A9] shadow-[0_0_10px_rgba(184,196,169,0.45)]",
    ];
    C[id as usize % C.len()]
}

/// The visible face of a card: a thin colour bar, a title, and an optional
/// subtitle.
#[component]
fn CardFace(card: Card) -> Element {
    rsx! {
        span { class: "h-7 w-1 shrink-0 rounded-full {swatch(card.id)}" }
        div { class: "min-w-0 flex-1",
            div { class: "truncate font-medium text-[#f4e9d7]", "{card.title}" }
            if !card.sub.is_empty() {
                div { class: "truncate text-[11px] text-[#9c8f77]", "{card.sub}" }
            }
        }
    }
}

// --- 1. reading list (core Draggable / DropZone + overlay) -------------------

const TODO: ZoneId = ZoneId(9001);
const DONE: ZoneId = ZoneId(9002);

#[component]
fn ReadingListDemo() -> Element {
    let mut bins = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Card>> = HashMap::new();
        m.insert(
            TODO,
            vec![
                Card::new(1, "The Creative Act", "Rick Rubin"),
                Card::new(2, "Piranesi", "Susanna Clarke"),
                Card::new(3, "Klara and the Sun", "Kazuo Ishiguro"),
            ],
        );
        m.insert(
            DONE,
            vec![Card::new(4, "Tomorrow, and Tomorrow", "Gabrielle Zevin")],
        );
        m
    });
    // Book that just landed, so it flashes onto its new shelf.
    let mut flashed = use_signal(|| None::<u32>);
    let move_card = move |o: DropOutcome<Card>| {
        let id = o.payload.id;
        let mut b = bins.write();
        for cards in b.values_mut() {
            cards.retain(|c| c.id != id);
        }
        b.entry(o.to).or_default().push(o.payload);
        drop(b);
        flashed.set(Some(id));
    };

    rsx! {
        Section {
            title: "Reading list",
            note: "Two shelves: what you're reading, and what you've finished. Move a book across and it flashes onto its new shelf.",
            tag: "DropZone",
            DndProvider::<Card> {
                LiveRegion::<Card> {}
                div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                    for (name, zone, hint) in [("Reading", TODO, "In progress"), ("Finished", DONE, "Done")] {
                        DropZone::<Card> {
                            id: zone,
                            label: name,
                            on_drop: move_card,
                            class: ZONE,
                            div { class: "mb-1 flex items-baseline justify-between",
                                p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9c8f77]",
                                    "{name}"
                                }
                                span { class: "text-[10px] text-[#6d6150]", "{hint}" }
                            }
                            for card in bins.read().get(&zone).cloned().unwrap_or_default() {
                                PointerDraggable::<Card> {
                                    payload: card.clone(),
                                    zone,
                                    input: DragInputMode::Pointer,
                                    label: card.title.clone(),
                                    class: if flashed() == Some(card.id) { format!("{ITEM} drop-flash") } else { ITEM.to_string() },
                                    // Clear the flash when any card is picked up, so the
                                    // next drop re-triggers the animation cleanly.
                                    on_drag_start: move |_| flashed.set(None),
                                    div { class: ROW,
                                        CardFace { card: card.clone() }
                                    }
                                }
                            }
                        }
                    }
                }
                DragOverlay::<Card> { class: "pointer-events-none flex items-center gap-2 rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] px-3.5 py-2.5 text-[13px] font-medium text-[#f4e9d7] shadow-[inset_0_1px_0_rgba(255,255,255,0.09),inset_0_0_0_1px_rgba(255,255,255,0.04),0_20px_44px_-12px_rgba(0,0,0,0.65)]",
                    CardGhost {}
                }
            }
        }
    }
}

#[component]
fn CardGhost() -> Element {
    let dnd = use_dnd::<Card>();
    rsx! {
        if let Some(c) = dnd.payload() {
            span { class: "h-4 w-1 shrink-0 rounded-full {swatch(c.id)}" }
            span { "{c.title}" }
        }
    }
}

// --- 2. newsletter builder (modifier keys + apply_clone_or_move) -------------

const PALETTE: ZoneId = ZoneId(9011);
const STAGE: ZoneId = ZoneId(9012);

#[component]
fn NewsletterDemo() -> Element {
    let mut zones = use_signal(|| {
        let mut m: HashMap<ZoneId, Vec<Card>> = HashMap::new();
        m.insert(
            PALETTE,
            vec![
                Card::new(1, "Heading", "Big section title"),
                Card::new(2, "Image", "Full-width photo"),
                Card::new(3, "Button", "Call to action"),
                Card::new(4, "Quote", "Pull quote"),
            ],
        );
        m.insert(STAGE, vec![]);
        m
    });
    let mut next_id = use_signal(|| 100u32);
    let on_drop = move |o: DropOutcome<Card>| {
        // Ctrl/Cmd forces a copy (new id, source kept); a plain drag moves.
        apply_clone_or_move(
            &mut zones.write(),
            o,
            |c| c.id,
            move |mut c| {
                c.id = next_id();
                next_id += 1;
                c
            },
        );
    };
    rsx! {
        Section {
            title: "Newsletter builder",
            note: "Drag blocks in to move them. Hold Cmd or Ctrl to drop a copy instead, and build a whole email from a few pieces.",
            tag: "apply_clone_or_move",
            DndProvider::<Card> {
                LiveRegion::<Card> {}
                div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                    for (name, zone) in [("Blocks", PALETTE), ("Your email", STAGE)] {
                        DropZone::<Card> {
                            id: zone,
                            label: name,
                            on_drop,
                            class: ZONE,
                            p { class: "mb-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9c8f77]",
                                "{name}"
                            }
                            for card in zones.read().get(&zone).cloned().unwrap_or_default() {
                                PointerDraggable::<Card> {
                                    payload: card.clone(),
                                    zone,
                                    input: DragInputMode::Pointer,
                                    label: card.title.clone(),
                                    class: ITEM,
                                    div { class: ROW,
                                        CardFace { card: card.clone() }
                                    }
                                }
                            }
                            if zone == STAGE && zones.read().get(&zone).map(|v| v.is_empty()).unwrap_or(true) {
                                p { class: "py-3 text-center text-[12px] text-[#6d6150]",
                                    "Drop blocks to compose your email"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

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
            Email { id: 1, from: "Stripe", subject: "Your March invoice is ready", time: "9:12", unread: true },
            Email { id: 2, from: "Mara Chen", subject: "Re: offsite agenda", time: "8:47", unread: true },
            Email { id: 3, from: "GitHub", subject: "dioxus-dnd v1.0 released", time: "8:02", unread: false },
            Email { id: 4, from: "Aeropress Club", subject: "Order #1180 has shipped", time: "7:31", unread: false },
            Email { id: 5, from: "Linear", subject: "Weekly digest: 12 issues closed", time: "6:58", unread: false },
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
                                    input: DragInputMode::Pointer,
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

// --- 4. playlist (sortable list) ---------------------------------------------

#[derive(Clone, PartialEq)]
struct Track {
    title: &'static str,
    artist: &'static str,
    dur: &'static str,
}

#[component]
fn PlaylistDemo() -> Element {
    let mut items = use_signal(|| {
        vec![
            Track { title: "Nightcall", artist: "Kavinsky", dur: "4:18" },
            Track { title: "Redbone", artist: "Childish Gambino", dur: "5:27" },
            Track { title: "Midnight City", artist: "M83", dur: "4:03" },
            Track { title: "Teardrop", artist: "Massive Attack", dur: "5:29" },
            Track { title: "Weird Fishes", artist: "Radiohead", dur: "5:18" },
        ]
    });
    rsx! {
        Section {
            title: "Playlist",
            note: "Reorder tonight's set. Grab a track and the others slide to make room; drop it and the ghost settles into its slot.",
            tag: "SortableList",
            SortableList {
                len: items.read().len(),
                input: DragInputMode::Pointer,
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                // No floating overlay: the grabbed row lifts in place and the
                // others slide to make room. No ghost, no ring.
                // The mailbox's list language: one contained well, hairline
                // dividers, and the grabbed row lifts out of it as a card.
                class: "relative rounded-xl bg-white/[0.03] ring-1 ring-white/5 [&>*]:flex [&>*]:items-center [&>*]:gap-3 [&>*]:px-3.5 [&>*]:py-2.5 [&>*]:text-[13px] [&>*]:cursor-grab [&>*]:select-none [&>*]:transition [&>*+*]:border-t [&>*+*]:border-white/5 [&>*:first-child]:rounded-t-xl [&>*:last-child]:rounded-b-xl [&>*:hover]:bg-white/[0.04] [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:rounded-lg [&>[data-dragging]]:bg-[#3d352a] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.04),0_16px_34px_-12px_rgba(0,0,0,0.65)]",
                render: move |ix: usize| {
                    let t = items.read()[ix].clone();
                    rsx! {
                        span { class: "w-4 shrink-0 text-center text-[11px] font-semibold tabular-nums text-[#6d6150]",
                            "{ix + 1}"
                        }
                        div { class: "min-w-0 flex-1",
                            div { class: "truncate font-medium text-[#f4e9d7]", "{t.title}" }
                            div { class: "truncate text-[11px] text-[#9c8f77]", "{t.artist}" }
                        }
                        span { class: "shrink-0 text-[11px] tabular-nums text-[#9c8f77]", "{t.dur}" }
                    }
                },
            }
        }
    }
}

// --- 5. weekly focus (accessible reorder, headless ReorderButtons) -----------

#[component]
fn PriorityDemo() -> Element {
    let mut items = use_signal(|| {
        ["Ship the redesign", "Reply to investors", "1:1 with Sam", "Book the venue"]
            .map(String::from)
            .to_vec()
    });
    rsx! {
        Section {
            title: "Weekly focus",
            note: "Rank your week with the mouse or the arrow buttons. Both emit the same reorder event, so keyboard users are covered too.",
            tag: "ReorderButtons",
            SortableList {
                len: items.read().len(),
                input: DragInputMode::Pointer,
                on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                class: "rounded-xl bg-white/[0.03] ring-1 ring-white/5 [&>*]:flex [&>*]:items-center [&>*]:justify-between [&>*]:gap-3 [&>*]:px-3.5 [&>*]:py-2.5 [&>*]:text-[13px] [&>*]:cursor-grab [&>*]:select-none [&>*]:transition [&>*+*]:border-t [&>*+*]:border-white/5 [&>*:first-child]:rounded-t-xl [&>*:last-child]:rounded-b-xl [&>*:hover]:bg-white/[0.04] [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:rounded-lg [&>[data-dragging]]:bg-[#3d352a] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.04),0_16px_34px_-12px_rgba(0,0,0,0.65)]",
                render: move |ix: usize| rsx! {
                    div { class: "flex min-w-0 items-center gap-2.5",
                        span { class: "grid h-6 w-6 shrink-0 place-items-center rounded-md bg-[#D97D55] text-[11px] font-semibold tabular-nums text-white",
                            "{ix + 1}"
                        }
                        span { class: "truncate font-medium text-[#f4e9d7]", "{items.read()[ix]}" }
                    }
                    ReorderButtons {
                        index: ix,
                        total: items.read().len(),
                        label: items.read()[ix].clone(),
                        on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
                        class: "flex shrink-0 gap-1 [&_button]:grid [&_button]:h-6 [&_button]:w-6 [&_button]:place-items-center [&_button]:rounded-md [&_button]:bg-white/8 [&_button]:text-[#9c8f77] [&_button]:transition [&_button:not(:disabled)]:hover:bg-white/15 [&_button:not(:disabled)]:hover:text-[#D97D55] [&_button:disabled]:opacity-30",
                    }
                },
            }
        }
    }
}

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
            Photo { label: "Sunday hike", hue: 0 },
            Photo { label: "Harbor at dusk", hue: 1 },
            Photo { label: "The studio", hue: 2 },
            Photo { label: "Roadtrip", hue: 3 },
            Photo { label: "Back garden", hue: 4 },
            Photo { label: "Rooftop", hue: 5 },
            Photo { label: "Corner cafe", hue: 6 },
            Photo { label: "Coastline", hue: 7 },
            Photo { label: "Market day", hue: 8 },
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
                input: DragInputMode::Pointer,
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

// --- 7. podcast queue (auto-scrolling container) -----------------------------

#[component]
fn QueueDemo() -> Element {
    let mut rows = use_signal(|| {
        const T: [&str; 8] = [
            "The long game",
            "Small teams",
            "Design debt",
            "Shipping fast",
            "The art of saying no",
            "On craft",
            "Growth loops",
            "Rest as strategy",
        ];
        (1..=18)
            .map(|n| format!("Ep {n:02}  ·  {}", T[(n - 1) as usize % T.len()]))
            .collect::<Vec<_>>()
    });
    // Index of the row that just landed, so it can flash.
    let mut dropped = use_signal(|| None::<usize>);
    rsx! {
        Section {
            title: "Podcast queue",
            note: "A queue longer than the window. Drag toward the top or bottom edge and it scrolls itself; the episode flashes where it lands. On a phone the dotted grip does the dragging, so a finger on the rows still scrolls the list.",
            tag: "AutoScroll",
            // The scroll container *is* the well: flat rows, hairline
            // dividers, and the grabbed row lifts out of the surface.
            AutoScroll { class: "max-h-52 overflow-y-auto rounded-xl bg-white/[0.03] ring-1 ring-white/5",
                SortableList {
                    len: rows.read().len(),
                    input: DragInputMode::Pointer,
                    // Inside a scroll container, claim only the grip for touch
                    // drags - the rows themselves keep scrolling by finger.
                    touch_handle: true,
                    on_sort: move |ev: SortEvent| {
                        apply_sort(&mut rows.write(), ev);
                        dropped.set(Some(ev.to));
                    },
                    class: "[&>*]:px-1.5 [&>*]:transition [&>*+*]:border-t [&>*+*]:border-white/5 [&>*:hover]:bg-white/[0.04] [&>[data-dragging]]:relative [&>[data-dragging]]:z-10 [&>[data-dragging]]:bg-[#3d352a] [&>[data-dragging]]:shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.04),0_12px_26px_-10px_rgba(0,0,0,0.65)] [&_[data-sort-handle]]:w-6 [&_[data-sort-handle]]:shrink-0 [&_[data-sort-handle]]:cursor-grab [&_[data-sort-handle]]:text-[13px] [&_[data-sort-handle]]:text-[#6d6150] [&_[data-sort-handle]]:transition [&_[data-sort-handle]:hover]:text-[#D97D55]",
                    render: move |ix: usize| {
                        let flash = if dropped() == Some(ix) { "drop-flash" } else { "" };
                        rsx! {
                            div {
                                class: "cursor-grab select-none rounded-md px-2 py-2.5 text-[13px] text-[#d9cfbc] transition {flash}",
                                // Reset once the flash finishes so the same row
                                // can flash again on its next drop.
                                onanimationend: move |_| {
                                    if dropped() == Some(ix) {
                                        dropped.set(None);
                                    }
                                },
                                "{rows.read()[ix]}"
                            }
                        }
                    },
                }
            }
        }
    }
}

// --- 8. sprint board (kanban: insertion slots + a WIP limit that refuses) ----

// High ids, far above the `use_zone_id` auto counter: `BoardSlot`s draw
// auto ids (11, 12, ...) from the same process-wide sequence, and the zone
// registry replaces by id, so a low explicit column id can collide with a
// slot's auto id and silently knock it out of the registry.
const BACKLOG: ContainerId = ZoneId(9101);
const DOING: ContainerId = ZoneId(9102);
const SHIPPED: ContainerId = ZoneId(9103);

/// In progress holds this many cards, no more.
const WIP: usize = 2;

/// Two-letter initials for the assignee chip.
fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect()
}

#[component]
fn SprintDemo() -> Element {
    let board = use_signal(|| {
        let mut m: HashMap<ContainerId, Vec<Card>> = HashMap::new();
        m.insert(
            BACKLOG,
            vec![
                Card::new(1, "Dark mode tokens", "Priya Nair"),
                Card::new(2, "Fix drop flicker", "Sam Ortiz"),
                Card::new(3, "Touch handles", "Mara Chen"),
            ],
        );
        m.insert(DOING, vec![Card::new(4, "Keyboard traversal", "Chad N")]);
        m.insert(SHIPPED, vec![Card::new(5, "Pointer capture", "Sam Ortiz")]);
        m
    });
    rsx! {
        Section {
            title: "Sprint board",
            note: "Point between two cards and a clay line marks the exact insert, not just an append. In progress is capped at two: once full it stops lighting up and refuses the drop until something ships.",
            tag: "BoardSlot",
            DndProvider::<BoardPayload<Card>> {
                LiveRegion::<BoardPayload<Card>> {}
                SprintColumns { board }
            }
        }
    }
}

/// The columns live in their own component so `use_dnd` runs *inside* the
/// `DndProvider` above (context is provided to children, not siblings).
#[component]
fn SprintColumns(board: Signal<HashMap<ContainerId, Vec<Card>>>) -> Element {
    let mut board = board;
    let dnd = use_dnd::<BoardPayload<Card>>();
    let count = move |col: ContainerId| board.read().get(&col).map(|v| v.len()).unwrap_or(0);
    let on_move = move |mv: MoveEvent<Card>| apply_move(&mut board.write(), mv);
    // The WIP rule, enforced by the library: when In progress is full, neither
    // the column nor its slots light up, and the drop is refused outright.
    // Moves *within* the column stay allowed - the count doesn't change.
    let wip_gate = move |col: ContainerId, p: BoardPayload<Card>| {
        col != DOING || p.from == DOING || count(DOING) < WIP
    };
    // The two slots hugging the dragged card are no-op drops (the card would
    // land where it already is), so their indicator is suppressed: only slots
    // that actually move something light up.
    let is_noop = move |col: ContainerId, ix: usize| {
        dnd.payload()
            .map(|p| p.from == col && (ix == p.index || ix == p.index + 1))
            .unwrap_or(false)
    };
    // Slot geometry never changes mid-drag: the pointer path hit-tests rects
    // cached at drag start, so a slot that grows in the layout would shift
    // every card below it and strand the highlight on stale geometry. The
    // open state is a clay insertion line scaling in, with zero reflow.
    //
    // The visible gap stays 12px, but the slot's *element* is a 32px band
    // (h-8 pulled back by -my-2.5) overlapping the card edges: the library
    // hit-tests the measured rect, so pointing anywhere near the seam
    // resolves to the slot. pointer-events-none keeps that invisible overlap
    // from stealing pointerdown on the cards themselves.
    const SLOT: &str = "pointer-events-none relative -my-2.5 flex h-8 items-center px-1 [&[data-over]>span]:scale-x-100 [&[data-over]>span]:opacity-100";
    const SLOT_LINE: &str = "h-[3px] w-full origin-center scale-x-50 rounded-full bg-[#D97D55] opacity-0 shadow-[0_0_12px_rgba(217,125,85,0.7)] transition-all duration-150";

    rsx! {
        div { class: "grid grid-cols-1 gap-3 sm:grid-cols-3",
            for (name, col) in [("Backlog", BACKLOG), ("In progress", DOING), ("Shipped", SHIPPED)] {
                BoardColumn::<Card> {
                    id: col,
                    label: name,
                    on_move,
                    accepts: move |p: BoardPayload<Card>| wip_gate(col, p),
                    class: "rounded-xl bg-[#26211a] p-2.5 min-h-36 shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)] transition data-active:ring-1 data-active:ring-[#B8C4A9]/60 data-active:bg-[#B8C4A9]/12",
                    div { class: "mb-1 flex items-center justify-between px-1",
                        p { class: "text-[11px] font-semibold uppercase tracking-[0.12em] text-[#9c8f77]",
                            "{name}"
                        }
                        if col == DOING {
                            span { class: if count(DOING) >= WIP { "rounded-full bg-[#D97D55] px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-white" } else { "rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-[#b8ab93] ring-1 ring-white/10" },
                                "{count(DOING)}/{WIP}"
                            }
                        } else {
                            span { class: "min-w-5 rounded-full bg-white/10 px-1.5 py-0.5 text-center text-[10px] font-semibold tabular-nums text-[#b8ab93] ring-1 ring-white/10",
                                "{count(col)}"
                            }
                        }
                    }
                    BoardSlot::<Card> {
                        column: col,
                        index: 0,
                        on_move,
                        class: SLOT,
                        if !is_noop(col, 0) {
                            span { class: SLOT_LINE }
                        }
                    }
                    for (ix, card) in board.read().get(&col).cloned().unwrap_or_default().into_iter().enumerate() {
                        BoardItem::<Card> {
                            item: card.clone(),
                            column: col,
                            index: ix,
                            input: DragInputMode::Pointer,
                            label: card.title.clone(),
                            class: ITEM,
                            div { class: ROW,
                                span { class: "h-7 w-1 shrink-0 rounded-full {swatch(card.id)}" }
                                div { class: "min-w-0 flex-1",
                                    div { class: "truncate font-medium text-[#f4e9d7]",
                                        "{card.title}"
                                    }
                                    div { class: "truncate text-[11px] text-[#9c8f77]",
                                        "{card.sub}"
                                    }
                                }
                                span { class: "grid h-6 w-6 shrink-0 place-items-center rounded-full bg-white/10 text-[9px] font-bold uppercase text-[#e0a37f] ring-1 ring-white/10",
                                    "{initials(&card.sub)}"
                                }
                            }
                        }
                        BoardSlot::<Card> {
                            column: col,
                            index: ix + 1,
                            on_move,
                            class: SLOT,
                            if !is_noop(col, ix + 1) {
                                span { class: SLOT_LINE }
                            }
                        }
                    }
                }
            }
        }
    }
}

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
fn flatten_tree(nodes: &[FsNode], parent: Option<u64>, depth: usize, out: &mut Vec<(usize, FsNode)>) {
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
            class: "h-3 w-3 shrink-0 text-[#6d6150] transition-transform duration-150 in-data-[intent=into]:rotate-90 in-data-[intent=into]:text-[#D97D55]",
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
            FsNode { id: 1, parent: None, name: "src", folder: true },
            FsNode { id: 2, parent: Some(1), name: "components", folder: true },
            FsNode { id: 3, parent: Some(2), name: "button.rs", folder: false },
            FsNode { id: 4, parent: Some(2), name: "card.rs", folder: false },
            FsNode { id: 5, parent: Some(1), name: "main.rs", folder: false },
            FsNode { id: 6, parent: None, name: "assets", folder: true },
            FsNode { id: 7, parent: Some(6), name: "logo.svg", folder: false },
            FsNode { id: 8, parent: None, name: "README.md", folder: false },
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
                div { class: "overflow-hidden rounded-xl bg-white/[0.03] ring-1 ring-white/5",
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
                            class: "border-b border-white/6 transition last:border-0
                                    data-[intent=before]:shadow-[inset_0_2px_0_0_#D97D55]
                                    data-[intent=after]:shadow-[inset_0_-2px_0_0_#D97D55]
                                    data-[intent=into]:bg-[#B8C4A9]/18",
                            PointerDraggable::<u64> {
                                payload: n.id,
                                input: DragInputMode::Pointer,
                                label: n.name,
                                class: "block cursor-grab select-none transition hover:bg-white/[0.04] active:cursor-grabbing data-dragging:opacity-40",
                                div { class: "flex items-center gap-2 py-2.5 pl-3 pr-3.5 text-[13px] font-medium text-[#d9cfbc]",
                                    for _ in 0..depth {
                                        span { class: "ml-1 h-5 w-3.5 shrink-0 border-l border-white/10" }
                                    }
                                    if n.folder {
                                        Chevron {}
                                        FolderIcon {}
                                        span { class: "font-mono text-[12px]", "{n.name}/" }
                                    } else {
                                        span { class: "w-3 shrink-0" }
                                        span { class: "text-[#B8A98C]", DocGlyph {} }
                                        span { class: "font-mono text-[12px]", "{n.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
                if !msg.read().is_empty() {
                    p { class: "mt-2 text-xs text-[#b8ab93]", "{msg}" }
                }
            }
        }
    }
}

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
            Note { id: 1, label: "Warm, not loud".into(), x: 20.0, y: 20.0 },
            Note { id: 2, label: "Fewer, better parts".into(), x: 220.0, y: 66.0 },
            Note { id: 3, label: "Should feel handmade".into(), x: 80.0, y: 128.0 },
            Note { id: 4, label: "Delight in the details".into(), x: 420.0, y: 30.0 },
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
                        PointerDraggable::<Note> {
                            payload: note.clone(),
                            input: DragInputMode::Pointer,
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

// --- 11. shuffle (FLIP reorder transitions) ----------------------------------

/// A translucent accent tint for FLIP tiles, keyed by index: the palette
/// colors at low opacity over the dark card, with a brightened text tone.
fn soft_tint(i: usize) -> &'static str {
    const C: [&str; 6] = [
        "bg-[#D97D55]/25 text-[#eda87f]",
        "bg-[#6FA4AF]/25 text-[#9ecad3]",
        "bg-[#B8C4A9]/25 text-[#c9d4ba]",
        "bg-[#F4E9D7]/15 text-[#e5d5b8]",
        "bg-[#D97D55]/25 text-[#eda87f]",
        "bg-[#6FA4AF]/25 text-[#9ecad3]",
    ];
    C[i % C.len()]
}

#[component]
fn ShuffleDemo() -> Element {
    let mut tiles = use_signal(|| (1..=6).collect::<Vec<u32>>());
    let mut epoch = use_signal(|| 0usize);
    let shuffle = move |_| {
        tiles.write().rotate_left(1);
        epoch += 1;
    };
    rsx! {
        Section {
            title: "Shuffle",
            note: "Change the order and every tile glides from its old slot to the new one. (Experimental; depends on browser paint timing.)",
            tag: "FlipItem",
            div { class: "space-y-3",
                button {
                    class: "rounded-lg bg-[#D97D55] px-3.5 py-1.5 text-[13px] font-medium text-white shadow-[0_2px_6px_-2px_rgba(217,125,85,0.5)] transition hover:bg-[#c96b45] active:scale-[0.98]",
                    onclick: shuffle,
                    "Shuffle"
                }
                div { class: "grid grid-cols-6 gap-2",
                    for n in tiles.read().iter().copied() {
                        // A stable key per tile lets Dioxus reuse the DOM node
                        // across reorders, so FlipItem can measure the move.
                        FlipItem {
                            key: "{n}",
                            epoch: epoch(),
                            class: "flex aspect-square items-center justify-center rounded-xl text-base font-semibold {soft_tint(n as usize - 1)}",
                            "{n}"
                        }
                    }
                }
            }
        }
    }
}

// --- 12. menu filter (FLIP on a filter change, survivors reflow) -------------

#[component]
fn MenuDemo() -> Element {
    #[derive(Clone, PartialEq)]
    struct Dish {
        id: u32,
        name: &'static str,
        cat: &'static str,
    }
    // Interleaved so filtering to one course pulls dishes from scattered cells.
    let all = use_signal(|| {
        vec![
            Dish { id: 1, name: "Wood-fired margherita", cat: "Mains" },
            Dish { id: 2, name: "Burrata and peach", cat: "Small plates" },
            Dish { id: 3, name: "Olive oil cake", cat: "Sweets" },
            Dish { id: 4, name: "Rigatoni al ragu", cat: "Mains" },
            Dish { id: 5, name: "Charred shishitos", cat: "Small plates" },
            Dish { id: 6, name: "Affogato", cat: "Sweets" },
            Dish { id: 7, name: "Roast chicken", cat: "Mains" },
            Dish { id: 8, name: "Warm focaccia", cat: "Small plates" },
            Dish { id: 9, name: "Tiramisu", cat: "Sweets" },
        ]
    });
    let mut filter = use_signal(|| "All");
    let mut epoch = use_signal(|| 0usize);
    let dot = |cat: &str| match cat {
        "Mains" => "bg-[#D97D55]",
        "Small plates" => "bg-[#B8C4A9]",
        _ => "bg-[#6FA4AF]",
    };
    rsx! {
        Section {
            title: "Menu",
            note: "Filter the menu and the remaining dishes glide up to fill the gaps: the same animation, driven by a filter instead of a drag.",
            tag: "FlipItem",
            div { class: "space-y-3",
                div { class: "flex flex-wrap gap-2",
                    for t in ["All", "Mains", "Small plates", "Sweets"] {
                        button {
                            class: if filter() == t { "rounded-full bg-[#D97D55] px-3 py-1 text-[12px] font-medium text-white shadow-[0_2px_6px_-2px_rgba(217,125,85,0.5)] transition" } else { "rounded-full bg-white/8 px-3 py-1 text-[12px] font-medium text-[#b8ab93] transition hover:bg-white/12 hover:text-[#D97D55]" },
                            onclick: move |_| {
                                if filter() != t {
                                    filter.set(t);
                                    epoch += 1;
                                }
                            },
                            "{t}"
                        }
                    }
                }
                div { class: "grid grid-cols-2 gap-2 sm:grid-cols-3",
                    for d in all.read().iter().filter(|d| filter() == "All" || d.cat == filter()).cloned() {
                        // Stable key per dish so a survivor keeps its DOM node
                        // across the filter change and FlipItem can glide it.
                        FlipItem {
                            key: "{d.id}",
                            epoch: epoch(),
                            class: "flex items-center gap-2 rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] px-3 py-2.5 text-[12px] text-[#d9cfbc] shadow-[inset_0_1px_0_rgba(255,255,255,0.07),inset_0_0_0_1px_rgba(255,255,255,0.03),0_1px_2px_rgba(0,0,0,0.4)]",
                            span { class: "inline-block h-2 w-2 shrink-0 rounded-full {dot(d.cat)}" }
                            span { class: "min-w-0 truncate", "{d.name}" }
                        }
                    }
                }
            }
        }
    }
}

// --- 13. upload (OS file drop, native) ---------------------------------------

#[component]
fn UploadDemo() -> Element {
    let mut accepted = use_signal(Vec::<String>::new);
    let mut refused = use_signal(Vec::<String>::new);
    rsx! {
        Section {
            title: "Upload",
            note: "Drag images from your desktop. Anything that isn't an image, or weighs over 5 MB, bounces with the reason on its chip. (Native OS drag; in-page pointer drags can't cross the app boundary.)",
            tag: "FileFilter",
            FileDropZone {
                filter: FileFilter::new()
                                                                    .content_types(["image/*"])
                                                                    .max_size(5_000_000)
                                                                    .max_files(6),
                on_files: move |drop: FileDrop| {
                    accepted.write().extend(drop.files.iter().map(|f| f.name()));
                },
                on_rejected: move |bad: Vec<(dioxus::html::FileData, FileRejection)>| {
                    refused
                        .write()
                        .extend(
                            bad
                                .into_iter()
                                .map(|(f, why)| {
                                    let reason = match why {
                                        FileRejection::ContentType => "not an image",
                                        FileRejection::TooLarge => "over 5 MB",
                                        FileRejection::TooMany => "past the 6-file limit",
                                        FileRejection::Extension => "wrong extension",
                                    };
                                    format!("{} · {reason}", f.name())
                                }),
                        );
                },
                class: "flex min-h-28 flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-white/15 p-4 text-center transition data-over:border-[#D97D55] data-over:bg-[#D97D55]/15",
                if accepted.read().is_empty() && refused.read().is_empty() {
                    p { class: "text-sm font-medium text-[#b8ab93]", "Drop images here to upload" }
                    p { class: "text-[12px] text-[#9c8f77]", "Up to 6 images, 5 MB each" }
                } else {
                    if !accepted.read().is_empty() {
                        div { class: "flex flex-wrap justify-center gap-1.5",
                            for n in accepted.read().clone() {
                                span { class: "inline-flex items-center gap-1.5 rounded-md bg-[#B8C4A9]/20 px-2 py-1 text-[11px] font-medium text-[#c9d4ba]",
                                    span { DocGlyph {} }
                                    "{n}"
                                }
                            }
                        }
                    }
                    if !refused.read().is_empty() {
                        div { class: "flex flex-wrap justify-center gap-1.5",
                            for m in refused.read().clone() {
                                span { class: "inline-flex items-center rounded-md bg-[#D97D55]/20 px-2 py-1 text-[11px] font-medium text-[#eda87f]",
                                    "{m}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- 14. share (drag out / external in, native) ------------------------------

#[component]
fn ShareDemo() -> Element {
    let mut dropped = use_signal(String::new);
    rsx! {
        Section {
            title: "Share",
            note: "Drag the link out to another tab or app, or drop a link or text back in. (Native drag across the app boundary.)",
            tag: "ExternalDragSource",
            div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                ExternalDragSource {
                    content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
                    class: "flex cursor-grab items-center justify-between gap-3 rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] px-3.5 py-4 text-[13px] text-[#d9cfbc] shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.03),0_1px_2px_rgba(0,0,0,0.5),0_4px_12px_-4px_rgba(0,0,0,0.4)] transition hover:-translate-y-px hover:brightness-[1.06]",
                    div { class: "min-w-0",
                        div { class: "font-medium text-[#f4e9d7]", "Dioxus" }
                        div { class: "truncate text-[11px] text-[#9c8f77]", "dioxuslabs.com" }
                    }
                    span { class: "text-[#B8A98C]", "↗" }
                }
                ExternalDropZone {
                    on_drop: move |d: ExternalDrop| {
                        dropped
                            .set(format!("{} payload(s), {} file(s)", d.payloads.len(), d.files.len()));
                    },
                    class: "flex min-h-24 items-center justify-center rounded-xl border-2 border-dashed border-white/15 p-3 text-center text-sm text-[#9c8f77] transition data-over:border-[#D97D55] data-over:bg-[#D97D55]/15 data-over:text-[#b8ab93]",
                    if dropped.read().is_empty() {
                        "Drop a link or text here"
                    } else {
                        "{dropped}"
                    }
                }
            }
        }
    }
}

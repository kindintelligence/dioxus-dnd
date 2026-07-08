//! Shared chrome for the gallery site: the midnight theme tokens, the demo
//! section frame, icons, the shared card model, and the teaching scaffold
//! every pattern page is built from.

use dioxus::prelude::*;

// Shared tokens. `ITEM` is the draggable box (border/background/padding);
// `ROW` is the flex layout for its contents. They're split because
// `Draggable` forwards `class` to an outer wrapper and renders
// `children` inside a nested block element, so a `flex` on `ITEM` would never
// reach the contents. Wrapping them in an explicit `ROW` div lays them out
// correctly regardless.
// Card recipe: top-lit gradient surface, a 1px inset edge highlight (the
// "machined" line every dark card needs), a real ambient shadow, and a
// one-pixel hover lift that presses back down on grab.
pub const ITEM: &str = "group block cursor-grab select-none rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] px-3.5 py-2.5 text-[13px] text-[#f4e9d7] shadow-[inset_0_1px_0_rgba(255,255,255,0.08),inset_0_0_0_1px_rgba(255,255,255,0.03),0_1px_2px_rgba(0,0,0,0.5),0_4px_12px_-4px_rgba(0,0,0,0.4)] transition hover:-translate-y-px hover:brightness-[1.06] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.09),inset_0_0_0_1px_rgba(255,255,255,0.04),0_2px_4px_rgba(0,0,0,0.5),0_12px_24px_-8px_rgba(0,0,0,0.55)] active:translate-y-0 active:cursor-grabbing data-dragging:opacity-50";
pub const ROW: &str = "flex w-full items-center gap-2.5";
pub const ZONE: &str = "rounded-xl border border-dashed border-white/15 p-3.5 min-h-24 transition space-y-2 data-active:border-[#B8C4A9] data-active:bg-[#B8C4A9]/12 data-over:border-solid data-over:border-[#D97D55] data-over:bg-[#D97D55]/15";

pub const BASE_CSS: &str = r#"
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

/// Kebab-case anchor id derived from a title ("Weekly focus" -> "weekly-focus").
/// `GroupLabel` and `Section` derive their ids with this, so sidebar links
/// need nothing but the visible title.
pub fn slug(s: &str) -> String {
    s.to_lowercase().replace(' ', "-")
}

/// A double chevron that points into the drawer's direction of travel:
/// >> to open, << (rotated) to close.
#[component]
pub fn DoubleChevron(open: bool) -> Element {
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

/// A light section divider: a small uppercase label and a one-line lead.
/// Carries a `slug(kicker)` id so sidebar group labels can link to it.
#[component]
pub fn GroupLabel(kicker: String, title: String) -> Element {
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
pub fn Section(title: String, note: String, tag: String, children: Element) -> Element {
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
pub fn FolderIcon() -> Element {
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
pub fn DocGlyph() -> Element {
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
pub struct Card {
    pub id: u32,
    pub title: String,
    pub sub: String,
}

impl Card {
    pub fn new(id: u32, title: &str, sub: &str) -> Self {
        Card {
            id,
            title: title.into(),
            sub: sub.into(),
        }
    }
}

/// A palette accent bar, derived from a card's id so it stays stable across
/// moves. Cycles clay, teal, sage.
pub fn swatch(id: u32) -> &'static str {
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
pub fn CardFace(card: Card) -> Element {
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

// --- teaching scaffold (shared by every pattern page) --------------------

/// Page header: group kicker, title, one-paragraph lead.
#[component]
pub fn PageIntro(kicker: String, title: String, lead: String) -> Element {
    rsx! {
        header { class: "mb-8",
            p { class: "text-[11px] font-semibold uppercase tracking-[0.18em] text-[#6FA4AF]",
                "{kicker}"
            }
            h1 { class: "mt-2 text-2xl font-semibold tracking-tight text-[#f4e9d7] sm:text-3xl",
                "{title}"
            }
            p { class: "mt-3 max-w-xl text-[14px] leading-relaxed text-[#b8ab93]",
                "{lead}"
            }
        }
    }
}

/// A titled block under the demo: "How it works", "Use it", "Good to know".
#[component]
pub fn DocBlock(title: String, children: Element) -> Element {
    rsx! {
        section { class: "mt-10",
            h2 { class: "mb-3 text-[15px] font-semibold tracking-tight text-[#f4e9d7]",
                "{title}"
            }
            {children}
        }
    }
}

/// Body copy column for DocBlock prose.
#[component]
pub fn Prose(children: Element) -> Element {
    rsx! {
        div { class: "max-w-xl space-y-3 text-[14px] leading-relaxed text-[#b8ab93]",
            {children}
        }
    }
}

/// A recessed code well, matching the midnight insets.
#[component]
pub fn CodeBlock(code: String) -> Element {
    rsx! {
        pre { class: "overflow-x-auto rounded-xl bg-[#26211a] p-4 text-[12.5px] leading-relaxed text-[#d9cfbc] shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)] ring-1 ring-white/5",
            code { class: "font-mono", "{code}" }
        }
    }
}

/// Clay-dotted bullets: a bold lead phrase, then the rest of the sentence.
#[component]
pub fn ApiNotes(notes: Vec<(&'static str, &'static str)>) -> Element {
    rsx! {
        ul { class: "max-w-xl space-y-2.5",
            for (lead, rest) in notes {
                li { class: "flex gap-2.5 text-[13.5px] leading-relaxed text-[#b8ab93]",
                    span { class: "mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-[#D97D55]" }
                    span {
                        span { class: "font-semibold text-[#f4e9d7]", "{lead} " }
                        "{rest}"
                    }
                }
            }
        }
    }
}

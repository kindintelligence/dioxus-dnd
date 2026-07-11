//! Shared chrome for the gallery site: the KI-U paper theme tokens, the demo
//! section frame, icons, the shared card model, and the teaching scaffold
//! every pattern page is built from.

use dioxus::prelude::*;

// Shared tokens. `ITEM` is the draggable box (border/background/padding);
// `ROW` is the flex layout for its contents. They're split because
// `Draggable` forwards `class` to an outer wrapper and renders
// `children` inside a nested block element, so a `flex` on `ITEM` would never
// reach the contents. Wrapping them in an explicit `ROW` div lays them out
// correctly regardless.
// Card recipe: top-lit gradient surface, a 1px inset edge line for
// definition on the light ground, a soft ambient shadow, and a one-pixel
// hover lift that presses back down on grab.
pub const ITEM: &str = "group block cursor-grab select-none rounded-xl bg-gradient-to-b from-[#FBFAF6] to-[#F6F3EC] px-3.5 py-2.5 text-[13px] text-[#1A1815] shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.05),0_1px_2px_rgba(26,24,21,0.10),0_4px_12px_-4px_rgba(26,24,21,0.08)] transition hover:-translate-y-px hover:brightness-[1.06] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.06),0_2px_4px_rgba(26,24,21,0.10),0_12px_24px_-8px_rgba(26,24,21,0.12)] active:translate-y-0 active:cursor-grabbing data-dragging:opacity-40";
pub const ROW: &str = "flex w-full items-center gap-2.5";

/// The floating card ghost every Card-based page dresses its `DragOverlay`
/// in: the ITEM face, lifted (bigger drop shadow, no hover states - it IS
/// the hover). Pair with `match_source: true` so it wears the grabbed
/// card's exact rect.
pub const GHOST: &str = "pointer-events-none flex items-center gap-2.5 rounded-xl bg-gradient-to-b from-[#FBFAF6] to-[#F6F3EC] px-3.5 py-2.5 text-[13px] text-[#1A1815] shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.06),0_2px_4px_rgba(26,24,21,0.10),0_12px_24px_-8px_rgba(26,24,21,0.12)]";
pub const ZONE: &str = "rounded-xl border border-dashed border-[#7A776C]/30 p-3.5 min-h-24 transition space-y-2 data-active:border-[#6C9984] data-active:bg-[#6C9984]/12 data-over:border-solid data-over:border-[#1C4A38] data-over:bg-[#1C4A38]/15";

pub const BASE_CSS: &str = r#"
html {
  font-family: 'Poppins', ui-sans-serif, system-ui, -apple-system, 'Segoe UI', sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-rendering: optimizeLegibility;
  -webkit-tap-highlight-color: transparent;
  /* Paper theme: paint the root too (overscroll, rubber-banding) and let
     the browser render native scrollbars and controls in their light forms. */
  background: #FBFAF6;
  color-scheme: light;
  /* Sidebar anchor navigation glides instead of jumping. */
  scroll-behavior: smooth;
}
/* Mono is for metadata and code only. */
pre, code, kbd {
  font-family: 'Geist Mono', ui-monospace, 'SF Mono', Menlo, monospace;
}
*:focus:not(:focus-visible) { outline: none; }
/* First-class keyboard focus: every draggable (core Draggable renders
   aria-roledescription) gets a forest ring on focus-visible instead of the
   browser default outline. */
[aria-roledescription="draggable"]:focus-visible {
  outline: 2px solid rgba(28,74,56,0.75);
  outline-offset: 2px;
  border-radius: 10px;
}
/* Homegrown syntax tint for CodeBlock (see `highlight` in ui.rs), tuned for
   the inverse ink panel: components/types in forest-300 lead, props in the
   info fill, keywords warm clay, strings ochre, numbers clay-300, while
   punctuation and comments recede into ink-500. */
.code-ty   { color: #A6C1B0; font-weight: 600; }
.code-prop { color: #D9E4EC; }
.code-kw   { color: #E8D4BE; font-weight: 500; }
.code-str  { color: #D5B876; }
.code-num  { color: #C9926B; }
.code-com  { color: #7A776C; font-style: italic; }
.code-pun  { color: #7A776C; }
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
            h2 { class: "text-[12px] font-medium uppercase tracking-[0.12em] text-[#7A776C]",
                "{kicker}"
            }
            p { class: "text-[13px] text-[#9B988D]", "{title}" }
        }
    }
}

/// The demo frame. The page title lives in `PageIntro`, so this header
/// doesn't repeat it: a "Live demo" eyebrow with a forest dot, the API tag,
/// and the one-line try-it instruction.
#[component]
pub fn Section(title: String, note: String, tag: String, children: Element) -> Element {
    rsx! {
        section {
            id: slug(&title),
            class: "rounded-2xl bg-[#F6F3EC] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_1px_2px_rgba(26,24,21,0.07),0_18px_36px_-24px_rgba(26,24,21,0.14)] scroll-mt-20 sm:p-6 lg:scroll-mt-8",
            div { class: "mb-2 flex items-center justify-between gap-4",
                div { class: "flex items-center gap-2",
                    span { class: "h-2 w-2 rounded-full bg-[#3E7558]" }
                    p { class: "text-[11px] font-medium uppercase tracking-[0.12em] text-[#7A776C]",
                        "Live demo"
                    }
                }
                code { class: "hidden shrink-0 rounded-md bg-[#E4ECDD] px-2 py-1 font-mono text-[11px] text-[#1C4A38] ring-1 ring-[#CFDDCF] sm:block",
                    "{tag}"
                }
            }
            p { class: "mb-5 max-w-xl text-[13px] leading-relaxed text-[#45423B]",
                "{note}"
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
            class: "h-4 w-4 shrink-0 text-[#B88B2F]",
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
/// moves. Cycles forest, brown, sage.
pub fn swatch(id: u32) -> &'static str {
    // Each bar blooms softly in its own color: a faint glow keeps the
    // accents lively where a flat bar would just sit there.
    const C: [&str; 3] = ["bg-[#3E7558]", "bg-[#A8613F]", "bg-[#B88B2F]"];
    C[id as usize % C.len()]
}

/// The visible face of a card: a thin colour bar, a title, and an optional
/// subtitle.
#[component]
pub fn CardFace(card: Card) -> Element {
    rsx! {
        span { class: "h-7 w-1 shrink-0 rounded-full {swatch(card.id)}" }
        div { class: "min-w-0 flex-1",
            div { class: "truncate font-medium text-[#1A1815]", "{card.title}" }
            if !card.sub.is_empty() {
                div { class: "truncate text-[11px] text-[#7A776C]", "{card.sub}" }
            }
        }
    }
}

// --- teaching scaffold (shared by every pattern page) --------------------

/// Page header: forest eyebrow, title, one-paragraph lead. Owns the page
/// title; the demo Section below deliberately does not repeat it.
#[component]
pub fn PageIntro(kicker: String, title: String, lead: String) -> Element {
    rsx! {
        header { class: "mb-8",
            p { class: "text-[11px] font-medium uppercase tracking-[0.12em] text-[#1C4A38]",
                "{kicker}"
            }
            h1 { class: "mt-2 text-2xl font-semibold tracking-tight text-[#1A1815] sm:text-3xl",
                "{title}"
            }
            p { class: "mt-3 max-w-xl text-[14px] leading-relaxed text-[#45423B]",
                "{lead}"
            }
        }
    }
}

/// A titled block under the demo: "How it works", "Use it", "Good to know".
/// A hairline rule plus an eyebrow gives every block the same editorial
/// rhythm, and the body wrapper spaces mixed children (prose, code, tables,
/// callouts) evenly so nothing cramps or gaps.
#[component]
pub fn DocBlock(title: String, children: Element) -> Element {
    rsx! {
        section { class: "mt-9 border-t border-[#E8E5D9] pt-6",
            h2 { class: "mb-4 text-[12px] font-medium uppercase tracking-[0.12em] text-[#7A776C]",
                "{title}"
            }
            div { class: "space-y-4", {children} }
        }
    }
}

/// Body copy column for DocBlock prose.
#[component]
pub fn Prose(children: Element) -> Element {
    rsx! {
        div { class: "max-w-xl space-y-3 text-[14px] leading-relaxed text-[#45423B]",
            {children}
        }
    }
}

/// A tiny Rust/rsx tokenizer behind `CodeBlock`: splits a snippet into
/// `(css class, text)` runs so the code panel can tint components, props,
/// keywords, strings, numbers, comments and punctuation from the system
/// palette without a JS highlighter. `None` runs render in the base ink.
/// Good enough for the gallery's hand-written snippets; not a real lexer.
fn highlight(src: &str) -> Vec<(Option<&'static str>, String)> {
    const KEYWORDS: [&str; 16] = [
        "let", "mut", "move", "for", "in", "if", "else", "fn", "pub", "use", "match", "return",
        "async", "await", "impl", "const",
    ];
    const PRIMITIVES: [&str; 8] = ["usize", "u32", "u64", "f64", "i32", "i64", "bool", "str"];

    fn flush(out: &mut Vec<(Option<&'static str>, String)>, plain: &mut String) {
        if !plain.is_empty() {
            out.push((None, std::mem::take(plain)));
        }
    }

    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            flush(&mut out, &mut plain);
            let mut s = String::new();
            while i < chars.len() && chars[i] != '\n' {
                s.push(chars[i]);
                i += 1;
            }
            out.push((Some("code-com"), s));
        } else if c == '"' {
            flush(&mut out, &mut plain);
            let mut s = String::from('"');
            i += 1;
            while i < chars.len() {
                let ch = chars[i];
                s.push(ch);
                i += 1;
                if ch == '\\' && i < chars.len() {
                    s.push(chars[i]);
                    i += 1;
                } else if ch == '"' {
                    break;
                }
            }
            out.push((Some("code-str"), s));
        } else if c.is_ascii_digit() {
            flush(&mut out, &mut plain);
            let mut s = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || "._".contains(chars[i])) {
                s.push(chars[i]);
                i += 1;
            }
            out.push((Some("code-num"), s));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let mut s = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                s.push(chars[i]);
                i += 1;
            }
            // A lone `:` right after an identifier marks an rsx prop or a
            // struct field (`::` paths stay plain).
            let is_prop = chars.get(i) == Some(&':') && chars.get(i + 1) != Some(&':');
            let class = if KEYWORDS.contains(&s.as_str()) {
                Some("code-kw")
            } else if PRIMITIVES.contains(&s.as_str())
                || s.chars().next().is_some_and(|f| f.is_ascii_uppercase())
            {
                Some("code-ty")
            } else if is_prop {
                Some("code-prop")
            } else {
                None
            };
            match class {
                Some(cl) => {
                    flush(&mut out, &mut plain);
                    out.push((Some(cl), s));
                }
                None => plain.push_str(&s),
            }
        } else if c.is_whitespace() {
            plain.push(c);
            i += 1;
        } else {
            // Punctuation runs get their own (receding) tone so the names
            // carry the hierarchy, not the braces.
            flush(&mut out, &mut plain);
            let mut s = String::new();
            while i < chars.len()
                && !chars[i].is_whitespace()
                && !chars[i].is_ascii_alphanumeric()
                && chars[i] != '_'
                && chars[i] != '"'
                && !(chars[i] == '/' && chars.get(i + 1) == Some(&'/'))
            {
                s.push(chars[i]);
                i += 1;
            }
            out.push((Some("code-pun"), s));
        }
    }
    flush(&mut out, &mut plain);
    out
}

/// The code panel: inverse ink surface so snippets anchor each page, with
/// the homegrown syntax tint (see [`highlight`] and `.code-*` in `BASE_CSS`).
#[component]
pub fn CodeBlock(code: String) -> Element {
    rsx! {
        pre { class: "overflow-x-auto rounded-xl bg-[#1A1815] p-4 text-[12.5px] leading-[1.7] text-[#E8E5D9] shadow-[0_2px_0_rgba(26,24,21,0.03),0_8px_20px_rgba(26,24,21,0.08)]",
            code { class: "font-mono",
                for (class, text) in highlight(&code) {
                    if let Some(cl) = class {
                        span { class: cl, "{text}" }
                    } else {
                        "{text}"
                    }
                }
            }
        }
    }
}

/// Numbered walkthrough for "How it works": a forest number chip, a bold lead
/// phrase, then the explanation. Reads as a story, not a spec.
#[component]
pub fn Steps(steps: Vec<(&'static str, &'static str)>) -> Element {
    rsx! {
        ol { class: "max-w-xl space-y-4",
            for (i, (lead, rest)) in steps.into_iter().enumerate() {
                li { class: "flex gap-3 text-[14px] leading-relaxed text-[#45423B]",
                    span { class: "mt-0.5 grid h-5 w-5 shrink-0 place-items-center rounded-full bg-[#E4ECDD] text-[11px] font-semibold tabular-nums text-[#1C4A38] ring-1 ring-[#A6C1B0]",
                        "{i + 1}"
                    }
                    span {
                        span { class: "font-semibold text-[#1A1815]", "{lead} " }
                        "{rest}"
                    }
                }
            }
        }
    }
}

/// An API reference table: a forest header bar naming the item, then one
/// row per prop/field/method with the name and type in an aligned left
/// column and the description on the right, so the tables read like real
/// documentation rather than a list.
#[component]
pub fn PropsTable(title: String, rows: Vec<(&'static str, &'static str, &'static str)>) -> Element {
    rsx! {
        div { class: "overflow-hidden rounded-lg bg-[#FBFAF6] ring-1 ring-[#E8E5D9] shadow-[0_1px_0_rgba(26,24,21,0.04),0_1px_2px_rgba(26,24,21,0.04)]",
            div { class: "flex items-center gap-2 border-b border-[#E8E5D9] bg-[#F0F2E3]/70 px-4 py-2",
                span { class: "h-1.5 w-1.5 shrink-0 rounded-full bg-[#3E7558]" }
                p { class: "text-[11.5px] font-medium uppercase tracking-[0.1em] text-[#45423B]",
                    "{title}"
                }
            }
            div { class: "divide-y divide-[#E8E5D9]",
                for (name, ty, desc) in rows {
                    div { class: "px-4 py-2.5 sm:grid sm:grid-cols-[minmax(0,15rem)_1fr] sm:gap-x-6",
                        div { class: "min-w-0",
                            code { class: "break-words font-mono text-[12px] font-semibold text-[#1C4A38]",
                                "{name}"
                            }
                            if !ty.is_empty() {
                                div {
                                    code { class: "font-mono text-[10.5px] text-[#9B988D]",
                                        "{ty}"
                                    }
                                }
                            }
                        }
                        p { class: "mt-1 text-[13px] leading-relaxed text-[#45423B] sm:mt-0",
                            "{desc}"
                        }
                    }
                }
            }
        }
    }
}

/// An info-toned "New to Dioxus?" callout that decodes the framework
/// concepts a page's snippet leans on, so the gallery teaches Dioxus
/// alongside the library. Keep it to two or three sentences, page-specific.
#[component]
pub fn DioxusNote(children: Element) -> Element {
    rsx! {
        aside { class: "max-w-xl rounded-lg border-l-2 border-[#2D4F6B] bg-[#D9E4EC]/50 px-4 py-3",
            p { class: "mb-1 text-[11px] font-semibold uppercase tracking-[0.14em] text-[#2D4F6B]",
                "New to Dioxus?"
            }
            div { class: "space-y-2 text-[13px] leading-relaxed text-[#45423B]", {children} }
        }
    }
}

/// Clay-dotted bullets: a bold lead phrase, then the rest of the sentence.
#[component]
pub fn ApiNotes(notes: Vec<(&'static str, &'static str)>) -> Element {
    rsx! {
        ul { class: "max-w-xl space-y-2.5",
            for (lead, rest) in notes {
                li { class: "flex gap-2.5 text-[13.5px] leading-relaxed text-[#45423B]",
                    span { class: "mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-[#1C4A38]" }
                    span {
                        span { class: "font-semibold text-[#1A1815]", "{lead} " }
                        "{rest}"
                    }
                }
            }
        }
    }
}

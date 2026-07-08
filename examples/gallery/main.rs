//! The gallery as a small multi-page site: every dioxus-dnd pattern gets its
//! own page with a live demo, how the pattern works, a usage snippet and API
//! notes. A detached sidebar (off-canvas drawer on mobile, opened with the
//! double chevrons) navigates between them via dioxus-router.
//!
//! Run:
//! ```sh
//! dx serve --example gallery --platform web --features web
//! ```

mod pages;
mod ui;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;
use pages::*;
use ui::*;

fn main() {
    dioxus::launch(App);
}

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Shell)]
        #[route("/")]
        Home {},
        #[route("/reading-list")]
        ReadingListPage {},
        #[route("/newsletter-builder")]
        NewsletterBuilderPage {},
        #[route("/mailbox")]
        MailboxPage {},
        #[route("/playlist")]
        PlaylistPage {},
        #[route("/weekly-focus")]
        WeeklyFocusPage {},
        #[route("/photo-album")]
        PhotoAlbumPage {},
        #[route("/podcast-queue")]
        PodcastQueuePage {},
        #[route("/sprint-board")]
        SprintBoardPage {},
        #[route("/project-files")]
        ProjectFilesPage {},
        #[route("/moodboard")]
        MoodboardPage {},
        #[route("/standup")]
        StandupPage {},
        #[route("/shuffle")]
        ShufflePage {},
        #[route("/menu")]
        MenuPage {},
        #[route("/upload")]
        UploadPage {},
        #[route("/share")]
        SharePage {},
}

/// One entry in the sidebar and on the home grid.
struct NavItem {
    title: &'static str,
    blurb: &'static str,
    route: Route,
}

/// The site map: (group kicker, group tagline, pages). The sidebar and the
/// home page grid both render from this, so adding a page is one line here
/// plus its route above.
fn nav() -> Vec<(&'static str, &'static str, Vec<NavItem>)> {
    vec![
        (
            "Organize",
            "Move things where they belong.",
            vec![
                NavItem {
                    title: "Reading list",
                    blurb: "Move items between two zones with a custom drag ghost.",
                    route: Route::ReadingListPage {},
                },
                NavItem {
                    title: "Newsletter builder",
                    blurb: "Copy or move between palettes with the platform modifier keys.",
                    route: Route::NewsletterBuilderPage {},
                },
                NavItem {
                    title: "Mailbox",
                    blurb: "Drag a multi-select stack and branch on copy versus move.",
                    route: Route::MailboxPage {},
                },
            ],
        ),
        (
            "Reorder",
            "Put things in the right order.",
            vec![
                NavItem {
                    title: "Playlist",
                    blurb: "Reorder a list with a live preview of the final order.",
                    route: Route::PlaylistPage {},
                },
                NavItem {
                    title: "Weekly focus",
                    blurb: "One reorder model serving drags and plain button presses.",
                    route: Route::WeeklyFocusPage {},
                },
                NavItem {
                    title: "Photo album",
                    blurb: "Reorder tiles in two dimensions with insert-and-reflow.",
                    route: Route::PhotoAlbumPage {},
                },
                NavItem {
                    title: "Podcast queue",
                    blurb: "Auto-scroll at the edges, with rows that still finger-scroll.",
                    route: Route::PodcastQueuePage {},
                },
            ],
        ),
        (
            "Structure",
            "Give it shape.",
            vec![
                NavItem {
                    title: "Sprint board",
                    blurb: "Kanban columns, precise insertion slots, and a WIP limit that refuses.",
                    route: Route::SprintBoardPage {},
                },
                NavItem {
                    title: "Project files",
                    blurb: "A reparenting tree with before, into and after intents.",
                    route: Route::ProjectFilesPage {},
                },
                NavItem {
                    title: "Moodboard",
                    blurb: "Free positioning on a canvas, with optional snap and bounds.",
                    route: Route::MoodboardPage {},
                },
                NavItem {
                    title: "Standup",
                    blurb: "Two payload worlds bridged by one shared drop zone.",
                    route: Route::StandupPage {},
                },
            ],
        ),
        (
            "Motion",
            "Animate the change.",
            vec![
                NavItem {
                    title: "Shuffle",
                    blurb: "FLIP transitions: tiles glide from old slot to new.",
                    route: Route::ShufflePage {},
                },
                NavItem {
                    title: "Menu",
                    blurb: "The same FLIP glide, driven by a filter instead of a drag.",
                    route: Route::MenuPage {},
                },
            ],
        ),
        (
            "Beyond the window",
            "Cross the app boundary.",
            vec![
                NavItem {
                    title: "Upload",
                    blurb: "OS file drops with declarative acceptance and honest rejections.",
                    route: Route::UploadPage {},
                },
                NavItem {
                    title: "Share",
                    blurb: "Drag content out to other apps, and accept text or links back in.",
                    route: Route::SharePage {},
                },
            ],
        ),
    ]
}

#[component]
fn App() -> Element {
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
            href: "https://fonts.googleapis.com/css2?family=Poppins:wght@300;400;500;600;700&display=swap",
        }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=Geist+Mono:wght@400;500;600&display=swap",
        }
        style { {BASE_CSS} }
        Router::<Route> {}
    }
}

/// The frame around every page. The landing page stands alone, full-bleed;
/// the sidebar (a drawer on mobile, with the double-chevron toggle) appears
/// once you step into a pattern page.
#[component]
fn Shell() -> Element {
    let mut nav_open = use_signal(|| false);
    let on_home = use_route::<Route>() == (Route::Home {});
    rsx! {
        div { class: if on_home { "min-h-screen bg-[#FBFAF6] text-[#1A1815] antialiased selection:bg-[#1C4A38] selection:text-white" } else { "min-h-screen bg-[#FBFAF6] text-[#1A1815] antialiased selection:bg-[#1C4A38] selection:text-white lg:pl-72" },
            if !on_home {
                Sidebar { open: nav_open }
                button {
                    class: "fixed left-4 top-4 z-50 grid h-10 w-10 place-items-center rounded-xl bg-[#F6F3EC]/90 text-[#12362A] shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_20px_-6px_rgba(26,24,21,0.14)] ring-1 ring-[#D7D4C9] backdrop-blur transition active:scale-95 lg:hidden",
                    aria_label: if nav_open() { "Close navigation" } else { "Open navigation" },
                    onclick: move |_| {
                        let v = nav_open();
                        nav_open.set(!v);
                    },
                    DoubleChevron { open: nav_open() }
                }
            }
            div { class: "mx-auto max-w-3xl px-5 py-14 sm:px-6 sm:py-16",
                Outlet::<Route> {}
                footer { class: "mt-14 border-t border-[#E8E5D9] pt-7 text-[12px] text-[#9B988D]",
                    "Built with "
                    span { class: "font-medium text-[#45423B]", "dioxus-dnd" }
                    ". Every page is a screenful of code, styled by you."
                }
            }
        }
    }
}

/// Site navigation: a detached floating card pinned on the left from lg up,
/// an off-canvas drawer below that. A brand lockup with a drag-handle mark,
/// hairline-ruled group labels, entries numbered in the mono gutter with a
/// forest indicator on the active page, and a version footer. Links close
/// the drawer.
#[component]
fn Sidebar(open: Signal<bool>) -> Element {
    let mut open = open;
    let current = use_route::<Route>();
    // Number the patterns 01..15 across all groups for the mono gutter.
    let mut n = 0usize;
    let groups: Vec<(&'static str, Vec<(usize, NavItem)>)> = nav()
        .into_iter()
        .map(|(group, _tagline, items)| {
            (
                group,
                items
                    .into_iter()
                    .map(|item| {
                        n += 1;
                        (n, item)
                    })
                    .collect(),
            )
        })
        .collect();
    let shell = if open() {
        "translate-x-0"
    } else {
        "-translate-x-[120%] lg:translate-x-0"
    };
    rsx! {
        if open() {
            div {
                class: "fixed inset-0 z-30 bg-black/30 backdrop-blur-[2px] lg:hidden",
                onclick: move |_| open.set(false),
            }
        }
        aside { class: "fixed bottom-4 left-4 top-16 z-40 flex w-64 flex-col overflow-y-auto rounded-2xl bg-[#F6F3EC]/95 p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_2px_0_rgba(26,24,21,0.03),0_24px_60px_-24px_rgba(26,24,21,0.16)] ring-1 ring-[#E8E5D9] backdrop-blur transition-transform duration-300 lg:bottom-5 lg:left-5 lg:top-5 lg:w-60 {shell}",
            Link {
                to: Route::Home {},
                onclick: move |_| open.set(false),
                class: "flex items-center gap-2.5 rounded-lg px-2 py-1.5 transition-colors duration-200 hover:bg-[#EEEADF]/70",
                span { class: "grid h-7 w-7 shrink-0 place-items-center rounded-lg bg-[#1C4A38] text-[13px] leading-none text-[#F0F2E3] shadow-[inset_0_1px_0_rgba(255,255,255,0.15)]",
                    "⠿"
                }
                div { class: "min-w-0",
                    p { class: "text-[13.5px] font-semibold leading-tight tracking-tight text-[#1A1815]",
                        "dioxus-dnd"
                    }
                    p { class: "text-[9.5px] font-medium uppercase tracking-[0.1em] text-[#9B988D]",
                        "Drag & drop for Dioxus"
                    }
                }
            }
            div { class: "mx-2 mb-1 mt-2.5 h-px bg-[#E8E5D9]" }
            nav { aria_label: "Patterns", class: "flex-1",
                for (group, items) in groups {
                    div { class: "flex items-center gap-2.5 px-2.5 pb-1.5 pt-5",
                        p { class: "text-[10px] font-medium uppercase tracking-[0.14em] text-[#9B988D]",
                            "{group}"
                        }
                        span { class: "h-px flex-1 bg-[#E8E5D9]" }
                    }
                    for (n, item) in items {
                        {
                            let active = current == item.route;
                            rsx! {
                                Link {
                                    to: item.route.clone(),
                                    onclick: move |_| open.set(false),
                                    class: if active { "relative flex items-center gap-2.5 rounded-md bg-[#E4ECDD] px-2.5 py-1.5 text-[13px] font-medium text-[#12362A]" } else { "relative flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-[13px] font-medium text-[#5E5B52] transition-colors duration-200 hover:bg-[#EEEADF]/70 hover:text-[#1A1815]" },
                                    if active {
                                        span { class: "absolute left-0 top-1/2 h-4 w-[3px] -translate-y-1/2 rounded-full bg-[#1C4A38]" }
                                    }
                                    code { class: if active { "text-[10px] tabular-nums text-[#3E7558]" } else { "text-[10px] tabular-nums text-[#BBB8AE]" },
                                        {format!("{n:02}")}
                                    }
                                    span { class: "truncate", "{item.title}" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "mt-3 border-t border-[#E8E5D9] px-2 pb-1 pt-3",
                p { class: "text-[11px] leading-relaxed text-[#BBB8AE]",
                    "Every pattern, one library."
                }
                div { class: "mt-2 flex items-center gap-2.5",
                    code { class: "rounded bg-[#EEEADF] px-1.5 py-0.5 text-[10px] text-[#7A776C]",
                        "v1.0.0"
                    }
                    a {
                        href: "https://github.com/kindintelligence/dioxus-dnd",
                        target: "_blank",
                        class: "text-[11px] font-medium text-[#1C4A38] transition-colors duration-200 hover:text-[#12362A]",
                        "GitHub ↗"
                    }
                    a {
                        href: "https://crates.io/crates/dioxus-dnd",
                        target: "_blank",
                        class: "text-[11px] font-medium text-[#1C4A38] transition-colors duration-200 hover:text-[#12362A]",
                        "crates.io ↗"
                    }
                }
            }
        }
    }
}

/// One paper scrap on the hero board: a label pinned at a jaunty angle,
/// draggable for real.
#[derive(Clone, PartialEq)]
struct Scrap {
    id: u32,
    label: &'static str,
    x: f64,
    y: f64,
    rot: &'static str,
    tone: &'static str,
}

/// The hero's proof-of-work: a dot-grid board where every scrap is a real
/// `Draggable` on a real `CanvasDropZone`. The landing page runs on the
/// library it advertises.
#[component]
fn HeroBoard() -> Element {
    let mut scraps = use_signal(|| {
        vec![
            Scrap {
                id: 1,
                label: "sortable",
                x: 26.0,
                y: 22.0,
                rot: "-rotate-2",
                tone: "bg-[#E4ECDD] text-[#1C4A38]",
            },
            Scrap {
                id: 2,
                label: "kanban",
                x: 132.0,
                y: 96.0,
                rot: "rotate-1",
                tone: "bg-[#E8D4BE] text-[#7A3E25]",
            },
            Scrap {
                id: 3,
                label: "tree",
                x: 240.0,
                y: 34.0,
                rot: "rotate-2",
                tone: "bg-[#E9DDB8] text-[#8A6A1F]",
            },
            Scrap {
                id: 4,
                label: "canvas",
                x: 318.0,
                y: 108.0,
                rot: "-rotate-1",
                tone: "bg-[#D9E4EC] text-[#2D4F6B]",
            },
            Scrap {
                id: 5,
                label: "multi-select",
                x: 420.0,
                y: 44.0,
                rot: "rotate-3",
                tone: "bg-[#F0F2E3] text-[#2A5E48]",
            },
            Scrap {
                id: 6,
                label: "files",
                x: 60.0,
                y: 132.0,
                rot: "rotate-1",
                tone: "bg-[#F1D9D1] text-[#8B3A2E]",
            },
        ]
    });
    rsx! {
        DndProvider::<Scrap> {
            LiveRegion::<Scrap> {}
            CanvasDropZone::<Scrap> {
                bounds: Bounds {
                    width: 640.0,
                    height: 150.0,
                },
                label: "Hero board",
                on_drop: move |d: CanvasDrop<Scrap>| {
                    let mut s = scraps.write();
                    if let Some(scrap) = s.iter_mut().find(|s| s.id == d.payload.id) {
                        scrap.x = d.position.x;
                        scrap.y = d.position.y;
                    }
                },
                class: "relative h-52 overflow-hidden rounded-2xl bg-[#F6F3EC] bg-[radial-gradient(#E1DDCE_1px,transparent_1px)] [background-size:18px_18px] ring-1 ring-[#E8E5D9] shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_1px_2px_rgba(26,24,21,0.04)] transition data-active:ring-[#A6C1B0]",
                for scrap in scraps.read().clone() {
                    Draggable::<Scrap> {
                        payload: scrap.clone(),
                        label: scrap.label,
                        style: "position: absolute; left: {scrap.x}px; top: {scrap.y}px;",
                        class: "cursor-grab select-none rounded-md px-2.5 py-1.5 font-mono text-[11.5px] font-medium ring-1 ring-[#1A1815]/10 shadow-[0_1px_2px_rgba(26,24,21,0.08),0_6px_14px_-6px_rgba(26,24,21,0.16)] transition hover:-translate-y-px active:cursor-grabbing data-dragging:opacity-40 {scrap.rot} {scrap.tone}",
                        "{scrap.label}"
                    }
                }
            }
            p { class: "mt-3 text-[12.5px] leading-relaxed text-[#7A776C]",
                "Every scrap up there is a "
                span { class: "font-mono text-[11.5px] text-[#1C4A38]", "Draggable" }
                " on a "
                span { class: "font-mono text-[11.5px] text-[#1C4A38]", "CanvasDropZone" }
                ". Go on, rearrange them."
            }
        }
    }
}

/// A nav group prepared for the home grid: group number, kicker, tagline,
/// and its pages numbered continuously across all groups.
type NumberedGroup = (usize, &'static str, &'static str, Vec<(usize, NavItem)>);

/// The landing page: a two-tone display headline, the live hero board, a
/// mono stat strip, numbered group sections matching the sidebar, and a
/// closing statement band.
#[component]
fn Home() -> Element {
    // Continue the sidebar's 01..15 numbering onto the cards.
    let mut n = 0usize;
    let groups: Vec<NumberedGroup> = nav()
        .into_iter()
        .enumerate()
        .map(|(gi, (group, tagline, items))| {
            (
                gi + 1,
                group,
                tagline,
                items
                    .into_iter()
                    .map(|item| {
                        n += 1;
                        (n, item)
                    })
                    .collect(),
            )
        })
        .collect();
    rsx! {
        header { class: "mb-8",
            p { class: "text-[11px] font-medium uppercase tracking-[0.12em] text-[#1C4A38]",
                "Dioxus · Drag & Drop"
            }
            h1 { class: "mt-3 text-4xl font-semibold leading-[1.05] tracking-[-0.03em] sm:text-5xl",
                span { class: "block text-[#1A1815]", "Pick it up." }
                span { class: "block text-[#BBB8AE]", "Put it anywhere." }
            }
            p { class: "mt-5 max-w-xl text-[15px] leading-relaxed text-[#45423B]",
                "Fifteen drag and drop patterns for Dioxus, each on its own page: a live interface you can grab, how it works underneath, and the API that drives it."
            }
            div { class: "mt-6 flex flex-wrap items-center gap-3",
                Link {
                    to: Route::ReadingListPage {},
                    class: "rounded-lg bg-[#1C4A38] px-4 py-2 text-[13px] font-medium text-[#F0F2E3] shadow-[0_1px_0_rgba(26,24,21,0.04),0_2px_6px_rgba(26,24,21,0.10)] transition-colors duration-200 hover:bg-[#12362A]",
                    "Start with the basics"
                }
                a {
                    href: "https://github.com/kindintelligence/dioxus-dnd",
                    target: "_blank",
                    class: "rounded-lg px-4 py-2 text-[13px] font-medium text-[#45423B] ring-1 ring-[#D7D4C9] transition-colors duration-200 hover:bg-[#EEEADF]/70 hover:text-[#1A1815]",
                    "GitHub ↗"
                }
                a {
                    href: "https://crates.io/crates/dioxus-dnd",
                    target: "_blank",
                    class: "rounded-lg px-4 py-2 text-[13px] font-medium text-[#45423B] ring-1 ring-[#D7D4C9] transition-colors duration-200 hover:bg-[#EEEADF]/70 hover:text-[#1A1815]",
                    "crates.io ↗"
                }
            }
        }
        HeroBoard {}
        div { class: "mt-8 grid grid-cols-2 gap-px overflow-hidden rounded-xl bg-[#E8E5D9] ring-1 ring-[#E8E5D9] shadow-[0_1px_0_rgba(26,24,21,0.04),0_1px_2px_rgba(26,24,21,0.04)] sm:grid-cols-4",
            for (value, label) in [
                ("15", "patterns, each its own page"),
                ("3", "inputs: mouse, touch, keyboard"),
                ("0", "JavaScript in the library itself"),
                ("MIT", "licensed, on crates.io"),
            ] {
                div { class: "bg-[#FBFAF6] px-4 py-3",
                    p { class: "text-[16px] font-semibold tracking-tight text-[#1A1815]",
                        "{value}"
                    }
                    p { class: "mt-0.5 text-[11px] leading-snug text-[#7A776C]", "{label}" }
                }
            }
        }
        for (gi, group, tagline, items) in groups {
            div { class: "mb-4 mt-12 flex items-baseline justify-between gap-4 border-b border-[#E8E5D9] pb-2.5",
                div { class: "flex items-baseline gap-3",
                    code { class: "text-[13px] text-[#BBB8AE]", {format!("{gi:02}")} }
                    h2 { class: "text-[16px] font-semibold tracking-tight text-[#1A1815]",
                        "{group}"
                    }
                }
                p { class: "hidden text-[12px] text-[#9B988D] sm:block", "{tagline}" }
            }
            div { class: "grid grid-cols-1 gap-3 sm:grid-cols-2",
                for (num, item) in items {
                    Link {
                        to: item.route.clone(),
                        class: "group rounded-xl bg-[#F6F3EC] p-4 ring-1 ring-[#E8E5D9] shadow-[0_1px_0_rgba(26,24,21,0.04),0_1px_2px_rgba(26,24,21,0.04)] transition duration-200 hover:-translate-y-0.5 hover:shadow-[0_2px_0_rgba(26,24,21,0.03),0_8px_20px_rgba(26,24,21,0.08)]",
                        div { class: "flex items-center justify-between",
                            code { class: "text-[10px] tabular-nums text-[#BBB8AE] transition-colors duration-200 group-hover:text-[#3E7558]",
                                {format!("{num:02}")}
                            }
                            span { class: "text-[#BBB8AE] transition duration-200 group-hover:translate-x-0.5 group-hover:text-[#1C4A38]",
                                "→"
                            }
                        }
                        p { class: "mt-2 text-[14px] font-semibold tracking-tight text-[#1A1815]",
                            "{item.title}"
                        }
                        p { class: "mt-1 text-[12.5px] leading-relaxed text-[#7A776C]",
                            "{item.blurb}"
                        }
                    }
                }
            }
        }
        div { class: "mt-16 border-y border-[#E8E5D9] py-9 text-center",
            p { class: "mx-auto max-w-md text-[17px] font-light leading-relaxed text-[#45423B]",
                "The library moves the payload. "
                span { class: "font-medium text-[#1C4A38]", "What it means is yours." }
            }
        }
    }
}

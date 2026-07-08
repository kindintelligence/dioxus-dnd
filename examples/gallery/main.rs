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
            href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap",
        }
        style { {BASE_CSS} }
        Router::<Route> {}
    }
}

/// The persistent frame around every page: detached sidebar (drawer on
/// mobile), the double-chevron toggle, and the routed content column.
#[component]
fn Shell() -> Element {
    let mut nav_open = use_signal(|| false);
    rsx! {
        div { class: "min-h-screen bg-[#211c15] text-[#f4e9d7] antialiased selection:bg-[#D97D55] selection:text-white lg:pl-64",
            Sidebar { open: nav_open }
            button {
                class: "fixed left-4 top-4 z-50 grid h-10 w-10 place-items-center rounded-xl bg-[#2b2620]/90 text-[#e0a37f] shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_8px_20px_-6px_rgba(0,0,0,0.6)] ring-1 ring-white/10 backdrop-blur transition active:scale-95 lg:hidden",
                aria_label: if nav_open() { "Close navigation" } else { "Open navigation" },
                onclick: move |_| {
                    let v = nav_open();
                    nav_open.set(!v);
                },
                DoubleChevron { open: nav_open() }
            }
            div { class: "mx-auto max-w-3xl px-5 py-14 sm:px-6 sm:py-16",
                Outlet::<Route> {}
                footer { class: "mt-14 border-t border-white/8 pt-7 text-[12px] text-[#8d8069]",
                    "Built with "
                    span { class: "font-medium text-[#b8ab93]", "dioxus-dnd" }
                    ". Every page is a screenful of code, styled by you."
                }
            }
        }
    }
}

/// Site navigation: a detached floating card pinned on the left from lg up,
/// an off-canvas drawer below that. Links close the drawer and highlight the
/// current page.
#[component]
fn Sidebar(open: Signal<bool>) -> Element {
    let mut open = open;
    let current = use_route::<Route>();
    let shell = if open() {
        "translate-x-0"
    } else {
        "-translate-x-[120%] lg:translate-x-0"
    };
    rsx! {
        if open() {
            div {
                class: "fixed inset-0 z-30 bg-black/60 backdrop-blur-[2px] lg:hidden",
                onclick: move |_| open.set(false),
            }
        }
        aside { class: "fixed bottom-4 left-4 top-16 z-40 flex w-64 flex-col overflow-y-auto rounded-2xl bg-[#26211a]/95 p-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.05),0_24px_60px_-24px_rgba(0,0,0,0.8)] ring-1 ring-white/8 backdrop-blur transition-transform duration-300 lg:bottom-5 lg:left-5 lg:top-5 lg:w-56 {shell}",
            Link {
                to: Route::Home {},
                onclick: move |_| open.set(false),
                class: "mb-2 flex items-center gap-2 rounded-lg px-2.5 pt-1 transition hover:bg-white/5",
                span { class: "h-2 w-2 shrink-0 rounded-full bg-[#D97D55] shadow-[0_0_10px_rgba(217,125,85,0.6)]" }
                span { class: "text-[14px] font-semibold tracking-tight text-[#f4e9d7]",
                    "dioxus-dnd"
                }
                span { class: "ml-auto rounded-full bg-white/8 px-1.5 py-0.5 text-[10px] font-semibold tabular-nums text-[#8d8069]",
                    "14"
                }
            }
            nav { aria_label: "Patterns", class: "flex-1",
                for (group, _tagline, items) in nav() {
                    p { class: "px-2.5 pb-1 pt-4 text-[10px] font-semibold uppercase tracking-[0.16em] text-[#6FA4AF]",
                        "{group}"
                    }
                    for item in items {
                        Link {
                            to: item.route.clone(),
                            onclick: move |_| open.set(false),
                            class: if current == item.route { "block rounded-lg bg-white/8 px-2.5 py-1.5 text-[13px] font-medium text-[#f4e9d7]" } else { "block rounded-lg px-2.5 py-1.5 text-[13px] font-medium text-[#9c8f77] transition hover:bg-white/5 hover:text-[#f4e9d7]" },
                            "{item.title}"
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

/// The landing page: hero plus a linked card per pattern, grouped like the
/// sidebar.
#[component]
fn Home() -> Element {
    rsx! {
        header { class: "mb-10",
            p { class: "text-[12px] font-semibold uppercase tracking-[0.18em] text-[#D97D55]",
                "Dioxus · Drag & Drop"
            }
            h1 { class: "mt-3 text-3xl font-semibold tracking-tight text-[#f4e9d7] sm:text-4xl",
                "Pick it up, put it anywhere."
            }
            p { class: "mt-3 max-w-xl text-[14px] leading-relaxed text-[#b8ab93]",
                "Fourteen drag and drop patterns, each on its own page: a live interface you can grab, how it works underneath, and the API that drives it."
            }
            div { class: "mt-5 flex flex-wrap gap-2",
                for chip in ["Pointer-native", "Keyboard-accessible", "Bring your own styles"] {
                    span { class: "rounded-full bg-white/8 px-2.5 py-1 text-[11px] font-medium text-[#b8ab93]",
                        "{chip}"
                    }
                }
            }
        }
        for (group, tagline, items) in nav() {
            GroupLabel { kicker: group, title: tagline }
            div { class: "grid grid-cols-1 gap-3 sm:grid-cols-2",
                for item in items {
                    Link {
                        to: item.route.clone(),
                        class: "group rounded-xl bg-gradient-to-b from-[#3d352a] to-[#332c23] p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.06),inset_0_0_0_1px_rgba(255,255,255,0.03),0_1px_2px_rgba(0,0,0,0.5),0_4px_12px_-4px_rgba(0,0,0,0.4)] transition hover:-translate-y-px hover:brightness-[1.06]",
                        div { class: "flex items-center justify-between gap-3",
                            span { class: "text-[14px] font-semibold text-[#f4e9d7]",
                                "{item.title}"
                            }
                            span { class: "text-[#6d6150] transition group-hover:translate-x-0.5 group-hover:text-[#e0a37f]",
                                "→"
                            }
                        }
                        p { class: "mt-1.5 text-[12.5px] leading-relaxed text-[#9c8f77]",
                            "{item.blurb}"
                        }
                    }
                }
            }
        }
    }
}

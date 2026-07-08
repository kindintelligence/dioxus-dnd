//! Menu: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn MenuPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Motion",
            title: "Menu",
            lead: "Filter the dishes and the survivors glide into the gaps: proof that FlipItem animates layout changes, whatever caused them.",
        }
        MenuDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "Filtering removes some keyed nodes; the remaining ones keep their identity, so when the grid reflows FlipItem measures each survivor's jump and glides it. The pattern is identical to the shuffle: stable keys, one epoch bump per filter change."
                }
                p {
                    "This is the cheapest way to make list and grid state changes feel physical across your whole app, with no per-item animation code."
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
                        "Keys are the contract:",
                        "filter by retaining identity, never by rebuilding items with new keys.",
                    ),
                    (
                        "Interleave your data",
                        "so a filter pulls survivors from scattered cells; the glide is what sells it.",
                    ),
                    (
                        "Entering items do not animate,",
                        "only survivors that moved; pair with your own enter transition if needed.",
                    ),
                    ("One epoch signal can serve several", "FlipItem groups that change together."),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"for dish in all.read().iter().filter(|d| filter() == "All" || d.cat == filter()) {
    FlipItem {
        key: "{dish.id}",
        epoch: epoch(),
        DishChip { dish: dish.clone() }
    }
}"#;

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
            Dish {
                id: 1,
                name: "Wood-fired margherita",
                cat: "Mains",
            },
            Dish {
                id: 2,
                name: "Burrata and peach",
                cat: "Small plates",
            },
            Dish {
                id: 3,
                name: "Olive oil cake",
                cat: "Sweets",
            },
            Dish {
                id: 4,
                name: "Rigatoni al ragu",
                cat: "Mains",
            },
            Dish {
                id: 5,
                name: "Charred shishitos",
                cat: "Small plates",
            },
            Dish {
                id: 6,
                name: "Affogato",
                cat: "Sweets",
            },
            Dish {
                id: 7,
                name: "Roast chicken",
                cat: "Mains",
            },
            Dish {
                id: 8,
                name: "Warm focaccia",
                cat: "Small plates",
            },
            Dish {
                id: 9,
                name: "Tiramisu",
                cat: "Sweets",
            },
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

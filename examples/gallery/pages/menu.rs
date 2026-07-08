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
            lead: "The same FlipItem as the shuffle, driven by a filter instead of a drag. Hide some dishes and the survivors glide into the gaps: proof that the animation belongs to the layout change, not to any particular gesture.",
        }
        MenuDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Filtering removes keyed nodes.",
                        "The for loop simply skips dishes that fail the filter. Because each survivor keeps its key, Dioxus keeps its DOM node and only its grid position changes.",
                    ),
                    (
                        "Survivors measure their jump.",
                        "One epoch bump per filter change tells every FlipItem to compare rectangles. A dish that moved three cells up glides three cells up; a dish that stayed put does nothing.",
                    ),
                    (
                        "The recipe generalizes.",
                        "Stable keys plus one epoch bump is the entire integration, whatever caused the reflow: filters, sorting controls, drag reorders, or data arriving. It is the cheapest way to make state changes feel physical across an app.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "Compare with the Shuffle page: the FlipItem usage is character-for-character the same. Only the thing bumping the epoch changed, from a shuffle button to filter chips."
                }
            }
            DioxusNote {
                p {
                    "Conditional rendering in Dioxus is ordinary Rust control flow: this page filters with a plain iterator inside the for loop. There is no special directive to learn; if an expression yields the item, it renders."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "FlipItem props",
                rows: vec![
                    ("epoch", "usize, required", "Bump whenever the layout changes; triggers a re-measure."),
                    ("duration", "f64 = 200.0", "Transition duration in milliseconds."),
                    ("easing", "String = \"ease\"", "CSS easing function for the glide."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Keys are the contract:",
                        "filter by retaining identity, never by rebuilding items with new keys, or there is no move to measure.",
                    ),
                    (
                        "Entering items do not animate,",
                        "only survivors that moved; pair with your own enter transition if appearing dishes should fade in.",
                    ),
                    (
                        "One epoch signal can serve",
                        "several FlipItem groups that change together; they all re-measure on the same bump.",
                    ),
                    (
                        "Interleave your data",
                        "in demos and empty states so a filter pulls survivors from scattered cells; the glide is what sells it.",
                    ),
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
        "Mains" => "bg-[#3E7558]",
        "Small plates" => "bg-[#6C9984]",
        _ => "bg-[#B88B2F]",
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
                            class: if filter() == t { "rounded-full bg-[#1C4A38] px-3 py-1 text-[12px] font-medium text-white shadow-[0_2px_6px_-2px_rgba(28,74,56,0.5)] transition" } else { "rounded-full bg-[#7A776C]/10 px-3 py-1 text-[12px] font-medium text-[#45423B] transition hover:bg-[#7A776C]/15 hover:text-[#1C4A38]" },
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
                            class: "flex items-center gap-2 rounded-xl bg-gradient-to-b from-[#FBFAF6] to-[#F6F3EC] px-3 py-2.5 text-[12px] text-[#2C2A25] shadow-[inset_0_1px_0_rgba(255,255,255,0.4),inset_0_0_0_1px_rgba(26,24,21,0.05),0_1px_2px_rgba(26,24,21,0.08)]",
                            span { class: "inline-block h-2 w-2 shrink-0 rounded-full {dot(d.cat)}" }
                            span { class: "min-w-0 truncate", "{d.name}" }
                        }
                    }
                }
            }
        }
    }
}

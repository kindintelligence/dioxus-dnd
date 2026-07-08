//! Standup: two independent drag worlds sharing one drop target.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn StandupPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Structure",
            title: "Standup",
            lead: "Tickets drag in one provider, teammates in another - two type-worlds that can't see each other, by design. The agenda tray is a BridgeDropZone, registered in both: one DOM box, one ZoneId, two typed drop callbacks.",
        }
        StandupDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "Two providers, two worlds.",
                        "DndProvider::<Ticket> and DndProvider::<Person> nest without interfering: each carries its own context and its own zone registry, keyed by payload type. A ticket drag consults only ticket zones - the compiler already guaranteed a Person handler can never receive one.",
                    ),
                    (
                        "One box, two registrations.",
                        "Zone ids are process-global while registries are per-type, so the bridge calls use_zone_registry twice and registers the same ZoneId in each, sharing one pair of mounted/rect signals. Both worlds' hit-testing and keyboard navigation now find the same rectangle on their own.",
                    ),
                    (
                        "Each drop arrives typed.",
                        "A ticket drop can only reach the DropOutcome<Ticket> callback, a person drop only the DropOutcome<Person> one. There is no downcast and no shared erased channel - dispatch happened at the type level, before the app ever ran.",
                    ),
                ],
            }
        }
        DocBlock { title: "When an enum is enough",
            Prose {
                p {
                    "Reach for a bridge only when two providers genuinely exist. If one drag world simply carries several shapes - a tree of files and folders, a list of cards and separators - make the payload an enum. The zone's accepts filters variants, the drop handler matches on them, and everything stays in one provider:"
                }
            }
            CodeBlock { code: ENUM_SNIPPET }
            Prose {
                p {
                    "The bridge is for the other case: the tickets and the team here are separate features with separate providers, and only the agenda needs to hear from both."
                }
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "One component, two worlds. It behaves like a DropZone in each: per-world accepts filtering, keyboard reachability, and data-active / data-over lighting up for an acceptable drag from either side."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "BridgeDropZone<A, B> props",
                rows: vec![
                    ("id", "Option<ZoneId>", "Stable identity, valid in both worlds. Auto-generated if omitted."),
                    ("label", "Option<String>", "Screen-reader label, announced by both worlds' navigation."),
                    ("accepts_a / accepts_b", "Option<Callback<_, bool>>", "Per-world acceptance. Return false to reject a payload; the zone won't highlight or accept it from that world."),
                    ("on_drop_a / on_drop_b", "EventHandler<DropOutcome<_>>", "Typed drop callbacks. An A drag can only reach on_drop_a, a B drag only on_drop_b."),
                ],
            }
        }
        DocBlock { title: "Beyond two worlds",
            Prose {
                p {
                    "BridgeDropZone packages a recipe you can write yourself - and for three or more providers, you still do. Registries are per-type but zone ids are process-global, so one component registers the same ZoneId in each world's registry, sharing one mounted/rect pair:"
                }
            }
            CodeBlock { code: RECIPE_SNIPPET }
            DioxusNote {
                p {
                    "Signals are Copy handles, so passing the same mounted and rect into every record genuinely shares them: any world's refresh_rects() re-measures the one rectangle all registries see."
                }
            }
            PropsTable {
                title: "Registry pieces the recipe uses",
                rows: vec![
                    ("use_zone_registry::<T>()", "-> ZoneRegistry<T>", "The per-type registry the provider carries. Public precisely so custom zones like this one can exist."),
                    ("use_zone_id()", "-> ZoneId", "A stable, process-unique id. Unique across all types, which is what lets one id live in several registries."),
                    ("ZoneRecord<T>", "id, parent, label, on_drop, accepts, mounted, rect", "Everything a registry knows about a zone. register() adds or replaces by id; unregister() removes."),
                    ("ParentZone", "context marker", "Read it to discover an enclosing zone, provide it so zones nested in the bridge find their parent."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Keyboard drags reach the bridge from both worlds:",
                        "pick up a ticket or a teammate with Enter and arrow to the agenda - each world's spatial navigation lists the shared rectangle among its own zones.",
                    ),
                    (
                        "Per-world acceptance still works:",
                        "accepts_a and accepts_b are independent, so the bridge could refuse done tickets while welcoming every teammate.",
                    ),
                    (
                        "The bridge can host nested zones.",
                        "It provides one unambiguous ParentZone id that exists in both registries, so a DropZone of either type nested inside it ascends correctly.",
                    ),
                    (
                        "Don't reach for a bridge to erase types.",
                        "If everything belongs to one interaction, an enum payload in a single provider is simpler and keeps one registry doing the work.",
                    ),
                ],
            }
        }
    }
}

const ENUM_SNIPPET: &str = r#"#[derive(Clone, PartialEq)]
enum Node { File(u64), Folder(u64) }

DropZone::<Node> {
    accepts: move |n: Node| matches!(n, Node::Folder(_)),
    on_drop: move |o: DropOutcome<Node>| match o.payload {
        Node::File(id) => open(id),
        Node::Folder(id) => reveal(id),
    },
}"#;

const SNIPPET: &str = r#"BridgeDropZone::<Ticket, Person> {
    label: "Standup agenda",
    on_drop_a: move |o: DropOutcome<Ticket>| discuss(o.payload),
    on_drop_b: move |o: DropOutcome<Person>| update_from(o.payload),
    // per-world acceptance, like DropZone's accepts:
    accepts_a: move |t: Ticket| !t.done,
    "Drop a ticket or a teammate"
}"#;

const RECIPE_SNIPPET: &str = r#"let mut reg_a = use_zone_registry::<Ticket>();
let mut reg_b = use_zone_registry::<Person>();
let zone_id = use_zone_id();          // process-unique: valid in both worlds
let parent = try_use_context::<ParentZone>().map(|p| p.0);
let mounted = use_signal(|| None);    // one DOM box,
let rect = use_signal(|| None);       // one rectangle, shared by both records
use_hook(|| {
    reg_a.register(ZoneRecord {
        id: zone_id, parent, label: label.clone(),
        on_drop: Callback::new(move |o| on_ticket.call(o)),
        accepts: None, mounted, rect,
    });
    reg_b.register(ZoneRecord {
        id: zone_id, parent, label,   // same id, other registry
        on_drop: Callback::new(move |o| on_person.call(o)),
        accepts: None, mounted, rect,
    });
});
use_drop(move || {
    reg_a.unregister(zone_id);
    reg_b.unregister(zone_id);
});"#;

// --- 15. standup (two providers bridged by BridgeDropZone) -------------------

#[derive(Clone, PartialEq)]
struct Ticket {
    id: u32,
    key: &'static str,
    title: &'static str,
}

#[derive(Clone, PartialEq)]
struct Person {
    id: u32,
    name: &'static str,
    role: &'static str,
}

/// One agenda line - the enum lives in the *model*, not the drag machinery.
#[derive(Clone, PartialEq)]
enum AgendaItem {
    Discuss(Ticket),
    Update(Person),
}

#[component]
fn StandupDemo() -> Element {
    let mut tickets = use_signal(|| {
        vec![
            Ticket {
                id: 1,
                key: "DND-41",
                title: "Ghost lags on touch",
            },
            Ticket {
                id: 2,
                key: "DND-45",
                title: "Rects stale after resize",
            },
            Ticket {
                id: 3,
                key: "DND-52",
                title: "Keyboard skips columns",
            },
            Ticket {
                id: 4,
                key: "DND-57",
                title: "Autoscroll overshoots",
            },
        ]
    });
    let mut people = use_signal(|| {
        vec![
            Person {
                id: 1,
                name: "Mara",
                role: "core",
            },
            Person {
                id: 2,
                name: "Theo",
                role: "web",
            },
            Person {
                id: 3,
                name: "Iris",
                role: "a11y",
            },
            Person {
                id: 4,
                name: "Sam",
                role: "docs",
            },
        ]
    });
    let mut agenda = use_signal(Vec::<AgendaItem>::new);
    let mut shipped = use_signal(Vec::<&'static str>::new);
    let mut out = use_signal(Vec::<&'static str>::new);

    let chip = "cursor-grab select-none rounded-md px-2.5 py-1.5 text-[12.5px] font-medium ring-1 ring-[#1A1815]/10 shadow-[0_1px_2px_rgba(26,24,21,0.08)] transition hover:-translate-y-px active:cursor-grabbing data-dragging:opacity-40";
    let own_zone = "mt-3 grid min-h-11 place-items-center rounded-lg border border-dashed border-[#D7D4C9] px-3 py-2 text-[11.5px] font-medium text-[#9B988D] transition data-active:border-[#6C9984] data-active:text-[#45423B] data-over:border-solid data-over:border-[#1C4A38] data-over:bg-[#E4ECDD] data-over:text-[#12362A]";

    rsx! {
        Section {
            title: "Standup",
            note: "Tickets and teammates live in separate providers: a ticket never lights the team's zone, and vice versa. Only the agenda tray is registered in both worlds, so it accepts either - and each drop arrives through its own typed callback. Keyboard works too: Enter to pick up, arrows to the tray.",
            tag: "BridgeDropZone",
            DndProvider::<Ticket> {
                DndProvider::<Person> {
                    LiveRegion::<Ticket> {}
                    LiveRegion::<Person> {}
                    div { class: "grid gap-3 sm:grid-cols-2",
                        // --- world A: tickets --------------------------------
                        div { class: "rounded-xl bg-[#EEEADF] p-3 ring-1 ring-[#E8E5D9]",
                            p { class: "text-[10px] font-medium uppercase tracking-[0.14em] text-[#7A776C]",
                                "Open tickets"
                            }
                            div { class: "mt-2 flex flex-wrap gap-1.5",
                                for t in tickets.read().clone() {
                                    Draggable::<Ticket> {
                                        key: "{t.id}",
                                        payload: t.clone(),
                                        label: t.key,
                                        class: "{chip} bg-[#E4ECDD] text-[#1C4A38]",
                                        span { class: "font-mono text-[11px]", "{t.key}" }
                                    }
                                }
                                if tickets.read().is_empty() {
                                    p { class: "py-1 text-[12px] text-[#9B988D]", "All shipped." }
                                }
                            }
                            DropZone::<Ticket> {
                                label: "Shipped",
                                on_drop: move |o: DropOutcome<Ticket>| {
                                    tickets.write().retain(|t| t.id != o.payload.id);
                                    shipped.write().push(o.payload.key);
                                },
                                class: own_zone,
                                if shipped.read().is_empty() {
                                    span { "Shipped" }
                                } else {
                                    span { class: "font-mono text-[11px] text-[#1C4A38]",
                                        {format!("Shipped: {}", shipped.read().join(", "))}
                                    }
                                }
                            }
                        }
                        // --- world B: the team -------------------------------
                        div { class: "rounded-xl bg-[#EEEADF] p-3 ring-1 ring-[#E8E5D9]",
                            p { class: "text-[10px] font-medium uppercase tracking-[0.14em] text-[#7A776C]",
                                "Team"
                            }
                            div { class: "mt-2 flex flex-wrap gap-1.5",
                                for p in people.read().clone() {
                                    Draggable::<Person> {
                                        key: "{p.id}",
                                        payload: p.clone(),
                                        label: p.name,
                                        class: "{chip} bg-[#D9E4EC] text-[#2D4F6B]",
                                        "{p.name}"
                                        span { class: "ml-1 font-mono text-[10px] opacity-60", "{p.role}" }
                                    }
                                }
                                if people.read().is_empty() {
                                    p { class: "py-1 text-[12px] text-[#9B988D]", "Everyone's out." }
                                }
                            }
                            DropZone::<Person> {
                                label: "Out today",
                                on_drop: move |o: DropOutcome<Person>| {
                                    people.write().retain(|p| p.id != o.payload.id);
                                    out.write().push(o.payload.name);
                                },
                                class: own_zone,
                                if out.read().is_empty() {
                                    span { "Out today" }
                                } else {
                                    span { class: "font-mono text-[11px] text-[#2D4F6B]",
                                        {format!("Out: {}", out.read().join(", "))}
                                    }
                                }
                            }
                        }
                    }
                    // --- the bridge: one box, both worlds --------------------
                    BridgeDropZone::<Ticket, Person> {
                        label: "Standup agenda",
                        on_drop_a: move |o: DropOutcome<Ticket>| {
                            let mut a = agenda.write();
                            if !a.iter().any(|i| matches!(i, AgendaItem::Discuss(t) if t.id == o.payload.id)) {
                                a.push(AgendaItem::Discuss(o.payload));
                            }
                        },
                        on_drop_b: move |o: DropOutcome<Person>| {
                            let mut a = agenda.write();
                            if !a.iter().any(|i| matches!(i, AgendaItem::Update(p) if p.id == o.payload.id)) {
                                a.push(AgendaItem::Update(o.payload));
                            }
                        },
                        class: "mt-3 rounded-xl border border-dashed border-[#D7D4C9] bg-[#F6F3EC] p-3 transition data-active:border-[#6C9984] data-over:border-solid data-over:border-[#1C4A38] data-over:bg-[#E4ECDD]/60",
                        div { class: "flex items-center justify-between",
                            p { class: "text-[10px] font-medium uppercase tracking-[0.14em] text-[#7A776C]",
                                "Standup agenda"
                            }
                            code { class: "text-[10px] text-[#9B988D]", "accepts both worlds" }
                        }
                        if agenda.read().is_empty() {
                            p { class: "mt-2 py-2 text-center text-[12.5px] text-[#9B988D]",
                                "Drop a ticket to discuss it, or a teammate for their update."
                            }
                        } else {
                            ol { class: "mt-2 space-y-1",
                                for (ix , item) in agenda.read().iter().enumerate() {
                                    li {
                                        key: "{ix}",
                                        class: "flex items-baseline gap-2.5 rounded-md bg-[#FBFAF6] px-2.5 py-1.5 text-[12.5px] ring-1 ring-[#E8E5D9]",
                                        code { class: "text-[10px] tabular-nums text-[#BBB8AE]",
                                            {format!("{:02}", ix + 1)}
                                        }
                                        match item {
                                            AgendaItem::Discuss(t) => rsx! {
                                                span { class: "font-mono text-[11px] text-[#1C4A38]", "{t.key}" }
                                                span { class: "text-[#45423B]", "{t.title}" }
                                            },
                                            AgendaItem::Update(p) => rsx! {
                                                span { class: "font-medium text-[#2D4F6B]", "{p.name}" }
                                                span { class: "text-[#45423B]", "gives an update" }
                                            },
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

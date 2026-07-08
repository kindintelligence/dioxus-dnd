//! Packing list: the crate's voice, localized with dioxus-i18n.

use std::rc::Rc;

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;
use dioxus_i18n::{prelude::*, t};
use unic_langid::langid;

use crate::ui::*;

#[component]
pub fn PackingListPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Voice",
            title: "Packing list",
            lead: "Every phrase the library speaks - keyboard announcements, button labels, the selection badge - reads a DndStrings from context, with English built in. Provide one and the whole voice follows your app's locale; here it follows dioxus-i18n, live.",
        }
        PackingDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "One struct of phrase functions.",
                        "DndStrings has one field per phrase (picked_up, over, dropped_in, cancelled...), each a plain function from its parameters to a String. Translations reorder and inflect freely because they own the whole sentence - nothing is concatenated for them.",
                    ),
                    (
                        "Provide it above your drag UI.",
                        "Components capture the struct from context once, falling back to English when none is there. Build it with struct-update syntax over Default::default() and override only what you translate.",
                    ),
                    (
                        "The closures read your locale.",
                        "Localization systems are stateful, so the fields are functions, not strings: each call goes through dioxus-i18n's t! macro, which resolves against the language selected at that moment. Switch languages and the very next announcement speaks it - no re-providing, no remounting.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "That is the entire integration: dioxus-i18n owns the catalog and the selected language, DndStrings carries the lookups to every draggable, zone, reorder button and selection badge below it. No feature flag, no dependency in the library - any i18n system that can produce a String plugs in the same way, including a plain match on your own locale signal."
                }
            }
            DioxusNote {
                p {
                    "use_init_i18n provides an I18n context; the t! macro resolves through it from wherever it's called - render or event handler - because context lookup walks up from the current scope. Fluent's syntax puts arguments in the message, so translators control word order."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "DndStrings fields (all with English defaults)",
                rows: vec![
                    ("picked_up(name)", "keyboard pickup", "Also the user's manual: keep the arrows/Enter/Escape instructions in the translation."),
                    ("over(name) / over_inside(name, parent)", "keyboard navigation", "Voiced as arrows move across zones, flat or nested."),
                    ("dropped_in(name) / cancelled()", "drag end", "The landing and the Escape hatch."),
                    ("no_targets() / no_target_selected()", "dead ends", "Nowhere to go; Enter with nothing selected."),
                    ("item() / zone(n) / row(n)", "fallback names", "Used when a Draggable, DropZone or ReorderButtons row has no label."),
                    ("move_up(name) / move_down(name)", "ReorderButtons", "The buttons' aria-labels."),
                    ("selection_count(n)", "SelectionCount", "The badge text - your chance at real plural rules."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Labels are yours to localize too:",
                        "the library voices the names you pass as label props, so pass them through t! as well - this demo's zones and items all do.",
                    ),
                    (
                        "Don't re-provide on switch.",
                        "Components capture DndStrings once at mount. Have the closures read the locale (t! does) and every phrase follows the switch automatically.",
                    ),
                    (
                        "use_dnd_strings() is public,",
                        "so a custom zone or source you build from the hooks can voice itself in the same language as the built-ins.",
                    ),
                    (
                        "Pair with dir for RTL locales:",
                        "strings localize the voice; DndProvider's dir: Direction::Rtl mirrors the keyboard's spatial navigation to match the mirrored layout.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"use dioxus_i18n::{prelude::*, t};
use unic_langid::langid;

let mut i18n = use_init_i18n(|| {
    I18nConfig::new(langid!("en"))
        .with_locale((langid!("en"), EN_FTL))   // picked-up = Picked up {$name}. ...
        .with_locale((langid!("es"), ES_FTL))   // picked-up = Recogiste {$name}. ...
});
use_context_provider(|| DndStrings {
    picked_up: Rc::new(|name| t!("picked-up", name: name)),
    over: Rc::new(|name| t!("over", name: name)),
    dropped_in: Rc::new(|name| t!("dropped-in", name: name)),
    cancelled: Rc::new(|| t!("cancelled")),
    ..Default::default()
});

// anywhere: switch the language, the next phrase speaks it
i18n.set_language(langid!("es"));"#;

// --- 17. packing list (DndStrings wired to dioxus-i18n) ----------------------

const EN_FTL: &str = r#"
picked-up = Picked up { $name }. Use arrow keys to choose a drop target, Enter to drop, Escape to cancel.
over = Over { $name }.
over-inside = Over { $name }, inside { $parent }.
no-targets = No drop targets available.
no-target-selected = No drop target selected.
dropped-in = Dropped in { $name }.
cancelled = Drag cancelled.
item = item
zone-n = zone { $n }
pile = Not packed
suitcase = Suitcase
passport = Passport
camera = Camera
sunscreen = Sunscreen
boots = Hiking boots
phrasebook = Phrasebook
try-it = Drag things into the suitcase - or focus one and press Enter, arrows, Enter. The box below shows what a screen reader hears.
quiet = (nothing voiced yet)
"#;

const ES_FTL: &str = r#"
picked-up = Recogiste { $name }. Usa las flechas para elegir un destino, Enter para soltar, Escape para cancelar.
over = Sobre { $name }.
over-inside = Sobre { $name }, dentro de { $parent }.
no-targets = No hay destinos disponibles.
no-target-selected = Ningún destino seleccionado.
dropped-in = Soltado en { $name }.
cancelled = Arrastre cancelado.
item = elemento
zone-n = zona { $n }
pile = Sin empacar
suitcase = Maleta
passport = Pasaporte
camera = Cámara
sunscreen = Protector solar
boots = Botas de montaña
phrasebook = Libro de frases
try-it = Arrastra cosas a la maleta - o enfoca una y pulsa Enter, flechas, Enter. La caja de abajo muestra lo que oye un lector de pantalla.
quiet = (todavía no se ha dicho nada)
"#;

const PILE: ZoneId = ZoneId(9401);
const CASE: ZoneId = ZoneId(9402);

#[component]
fn PackingDemo() -> Element {
    let mut i18n = use_init_i18n(|| {
        I18nConfig::new(langid!("en"))
            .with_locale((langid!("en"), EN_FTL))
            .with_locale((langid!("es"), ES_FTL))
            .with_fallback(langid!("en"))
    });
    // The whole integration: every phrase the library voices goes through
    // the Fluent catalog above, resolved at call time against the selected
    // language.
    use_context_provider(|| DndStrings {
        picked_up: Rc::new(|name| t!("picked-up", name: name)),
        over: Rc::new(|name| t!("over", name: name)),
        over_inside: Rc::new(|name, parent| t!("over-inside", name: name, parent: parent)),
        no_targets: Rc::new(|| t!("no-targets")),
        no_target_selected: Rc::new(|| t!("no-target-selected")),
        dropped_in: Rc::new(|name| t!("dropped-in", name: name)),
        cancelled: Rc::new(|| t!("cancelled")),
        item: Rc::new(|| t!("item")),
        zone: Rc::new(|n| t!("zone-n", n: n)),
        ..Default::default()
    });

    // (key, id) - names resolve through t! per render, so they follow the
    // language switch like everything else.
    let mut pile = use_signal(|| {
        vec![
            ("passport", 1u32),
            ("camera", 2),
            ("sunscreen", 3),
            ("boots", 4),
        ]
    });
    let mut packed = use_signal(Vec::<(&'static str, u32)>::new);
    let language = i18n.language();
    let lang_btn = "rounded-md px-2.5 py-1 text-[11.5px] font-medium ring-1 ring-[#1A1815]/10 transition hover:-translate-y-px aria-pressed:bg-[#1C4A38] aria-pressed:text-[#F6F3EC]";

    rsx! {
        Section {
            title: "Packing list",
            note: "Switch the language and pick something up with the keyboard: the announcements - mirrored in the box below so you can see them - change tongue mid-session, as do the labels they name.",
            tag: "DndStrings",
            div { class: "mb-3 flex items-center gap-1.5",
                for (label , id) in [("English", langid!("en")), ("Español", langid!("es"))] {
                    button {
                        class: lang_btn,
                        aria_pressed: if language == id { "true" },
                        onclick: {
                            let id = id.clone();
                            move |_| i18n.set_language(id.clone())
                        },
                        "{label}"
                    }
                }
            }
            DndProvider::<u32> {
                LiveRegion::<u32> {}
                div { class: "grid grid-cols-1 gap-4 sm:grid-cols-2",
                    for (zone , title_key , items) in [(PILE, "pile", pile), (CASE, "suitcase", packed)] {
                        DropZone::<u32> {
                            id: zone,
                            label: t!(title_key),
                            on_drop: move |o: DropOutcome<u32>| {
                                let all: Vec<_> = pile.read().iter().chain(packed.read().iter()).cloned().collect();
                                if let Some(it) = all.into_iter().find(|(_, id)| *id == o.payload) {
                                    pile.write().retain(|(_, id)| *id != o.payload);
                                    packed.write().retain(|(_, id)| *id != o.payload);
                                    if zone == CASE { packed.write().push(it) } else { pile.write().push(it) }
                                }
                            },
                            class: ZONE,
                            p { class: "mb-1 text-[11px] font-semibold uppercase tracking-[0.12em] text-[#7A776C]",
                                {t!(title_key)}
                            }
                            for (key , id) in items.read().clone() {
                                Draggable::<u32> {
                                    key: "{id}",
                                    payload: id,
                                    zone,
                                    label: t!(key),
                                    class: ITEM,
                                    div { class: ROW,
                                        span { class: "h-4 w-1 shrink-0 rounded-full {swatch(id)}" }
                                        span { {t!(key)} }
                                    }
                                }
                            }
                        }
                    }
                }
                p { class: "mt-3 text-[12px] text-[#7A776C]", {t!("try-it")} }
                VoiceMirror {}
            }
        }
    }
}

/// The announcement channel, made visible so sighted visitors can watch the
/// localized voice. Hidden from the accessibility tree - `LiveRegion`
/// already speaks it, and hearing everything twice helps no one.
#[component]
fn VoiceMirror() -> Element {
    let dnd = use_dnd::<u32>();
    let text = dnd.announcement();
    rsx! {
        div {
            aria_hidden: "true",
            class: "mt-2 min-h-10 rounded-lg bg-[#1A1815] px-3 py-2 font-mono text-[12px] text-[#E4ECDD]",
            if text.is_empty() {
                span { class: "opacity-50", {t!("quiet")} }
            } else {
                "{text}"
            }
        }
    }
}

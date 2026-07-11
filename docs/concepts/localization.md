# Localization

Every phrase the crate voices - the keyboard-drag announcements,
`ReorderButtons` labels and row fallbacks, the `SelectionCount` badge -
reads a `DndStrings` from context, with English built in. Localizing the
whole voice is one context provide, and the crate takes no i18n dependency
to make it work.

API reference: [api/localization.md](../api/localization.md).
Live demo: the
[Packing list](https://kindintelligence.github.io/dioxus-dnd/packing-list)
page shows the full dioxus-i18n wiring, with a live English/Spanish toggle
and a visible mirror of the announcement channel.

## The mental model

`DndStrings` has one field per phrase (`picked_up`, `over`, `dropped_in`,
`cancelled`, ...), and every field is a function from its parameters to a
`String`, not a template string. Each translation owns its whole sentence,
so it can reorder the arguments, inflect around them, and apply real
plural rules; the crate never concatenates fragments on a translation's
behalf.

The struct travels through Dioxus context. The components that speak
(`Draggable`'s keyboard path, `ReorderButtons`, `SelectionCount`) read it
with `use_dnd_strings()` and fall back to the built-in English when no
ancestor provided one, so there is nothing to configure when English is
fine.

## A worked example

The full dioxus-i18n integration (the Fluent-based crate the Dioxus docs
recommend), exactly as the Packing list demo wires it:

```rust,ignore
use std::rc::Rc;
use dioxus_i18n::{prelude::*, t};
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
i18n.set_language(langid!("es"));
```

That is the entire integration: dioxus-i18n owns the catalog and the
selected language, `DndStrings` carries the lookups to every draggable,
zone, reorder button and selection badge below it. Any i18n system that
produces a `String` plugs in the same way, including a plain `match` on
your own locale signal.

## Provide once, override what you translate

Provide the struct anywhere above your drag UI; everything below it is
localized. Build it with struct-update syntax over `Default::default()`
and override only the phrases you have translations for - the rest keep
speaking the built-in English instead of breaking. The defaults are pinned
by the crate's tests as user-facing contracts, so translations keyed off
the English wording do not drift silently.

## The closures read the locale

Components capture `DndStrings` once, at mount. Re-providing the struct on
a language switch therefore reaches only components mounted afterwards,
which is why switching works the other way around: every phrase is a fresh
function call, so a closure that resolves against the locale selected at
that moment (as `t!` does) speaks the new language on the very next
announcement. No re-providing, no remounting. Render-time phrases follow
too: a closure that reads a signal re-renders its readers when the locale
changes, so `ReorderButtons` labels and the `SelectionCount` badge repaint
on switch.

## What to pair with it

Two things live outside `DndStrings`:

- **Your `label` props.** The crate voices the names you give it
  ("Dropped in {label}"), so pass item and zone labels through the same
  translation layer. The Packing list demo resolves every label through
  `t!` per render, and they change tongue with the phrases that name them.
- **Layout direction.** Strings localize the voice; `dir: Direction::Rtl`
  on `DndProvider` mirrors the keyboard's spatial navigation to match a
  right-to-left layout. Set both for RTL locales.

## Custom components

`use_dnd_strings()` is public, so a custom drag source or zone built from
the hooks voices itself in the same language as the built-ins:

```rust,ignore
let strings = use_dnd_strings();
let name = label.clone().unwrap_or_else(|| (strings.item)());
dnd.announce((strings.picked_up)(&name));
```

Messages pushed through `dnd.announce(...)` come out of the provider's
`LiveRegion` like every built-in phrase.

## Gotchas

- **`picked_up` is also the user's manual.** The English default ends with
  "Use arrow keys to choose a drop target, Enter to drop, Escape to
  cancel." Keep the key instructions in the translation, or keyboard users
  in that locale never learn the controls.
- **`selection_count` is where plural rules go.** The English default is
  the deliberately lazy "{n} item(s)"; the field receives the count, so
  Fluent selectors or your own `match` can do it properly.
- **Do not re-provide on switch.** Capture-once means a freshly provided
  struct misses everything already mounted. Have the closures read the
  locale instead; that path updates everything.
- **Announcements still need a `LiveRegion`.** `DndStrings` writes the
  phrase; one `LiveRegion` per provider speaks it. Without one, keyboard
  drags work but say nothing. See [Accessibility](accessibility.md).
- **`DndDebugOverlay` is intentionally not localized.** It is a dev-only
  inspector that renders debug chrome; gate it out of release builds
  rather than translating it.

## Related

- [Accessibility](accessibility.md): the `LiveRegion` that speaks these
  phrases, `ReorderButtons`, and the WCAG mapping.
- [Multi-select](multiselect.md): `SelectionCount`, the badge that
  `selection_count` fills.
- [Drag and drop](drag-and-drop.md): the `label` props these phrases name,
  and `DndProvider`'s `dir`.

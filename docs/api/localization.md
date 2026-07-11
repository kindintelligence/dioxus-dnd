# Localization API reference

Localizable strings - every phrase the crate voices, in one place:
screen-reader announcements ("Picked up Piranesi. ..."), `ReorderButtons`
labels and the `SelectionCount` badge all read a `DndStrings` from context,
falling back to built-in English when none is provided.

Concept guide: [docs/concepts/localization.md](../concepts/localization.md).
Live demo with full dioxus-i18n wiring:
[Packing list](https://kindintelligence.github.io/dioxus-dnd/packing-list).

Provide one anywhere above your drag UI to localize everything below it:

```rust,ignore
use_context_provider(|| DndStrings {
    dropped_in: Rc::new(|name| t!("dropped-in", name: name)),
    ..Default::default()
});
```

The fields are plain `Rc<dyn Fn(..) -> String>`, so the crate stays
dependency-free while any i18n system plugs in - the closure above calls
dioxus-i18n's `t!` (the Fluent-based crate the Dioxus docs recommend), but
a `match` on your own locale signal works just as well. Have the closures
*read* your locale state rather than re-providing the struct on switch:
components capture `DndStrings` once at mount, and a closure that reads a
signal re-renders its readers when the locale changes.

## `DndStrings`

The crate's voice, one field per phrase. Every field is a function so
translations can reorder, inflect or pluralize freely - each owns its
whole sentence and nothing is concatenated for it. Build it with
struct-update syntax over `Default::default()` to override only what you
translate; the defaults are the built-in English.

| Field | Signature | English default | Used for |
|---|---|---|---|
| `picked_up` | `Rc<dyn Fn(&str) -> String>` | `Picked up {name}. Use arrow keys to choose a drop target, Enter to drop, Escape to cancel.` | Voiced when a keyboard drag picks an item up. Receives the draggable's `label`. This is also the user's manual - keep the key instructions (arrows, Enter, Escape) in the translation. |
| `over` | `Rc<dyn Fn(&str) -> String>` | `Over {name}.` | Voiced when keyboard navigation reaches a zone. Receives the zone's name. |
| `over_inside` | `TwoNamePhrase` | `Over {name}, inside {parent}.` | Voiced when keyboard navigation reaches a zone nested in a labeled parent. Receives the zone's name, then the parent's. |
| `no_targets` | `Rc<dyn Fn() -> String>` | `No drop targets available.` | Voiced when an arrow key finds nowhere to go. |
| `no_target_selected` | `Rc<dyn Fn() -> String>` | `No drop target selected.` | Voiced when Enter is pressed with no zone selected. |
| `dropped_in` | `Rc<dyn Fn(&str) -> String>` | `Dropped in {name}.` | Voiced when a keyboard drop lands. Receives the zone's name. |
| `cancelled` | `Rc<dyn Fn() -> String>` | `Drag cancelled.` | Voiced when Escape cancels the drag. |
| `item` | `Rc<dyn Fn() -> String>` | `item` | Fallback name for a draggable with no `label`. |
| `zone` | `Rc<dyn Fn(u64) -> String>` | `zone {n}` | Fallback name for a zone with no `label`. Receives the zone id's number. |
| `move_up` | `Rc<dyn Fn(&str) -> String>` | `Move {name} up` | `ReorderButtons`: the up button's `aria-label`. Receives the row's name. |
| `move_down` | `Rc<dyn Fn(&str) -> String>` | `Move {name} down` | `ReorderButtons`: the down button's `aria-label`. Receives the row's name. |
| `row` | `Rc<dyn Fn(usize) -> String>` | `item {n}` | `ReorderButtons`: fallback name for a row with no `label`. Receives the 1-based row number. |
| `selection_count` | `Rc<dyn Fn(usize) -> String>` | `{n} item(s)` | `SelectionCount`: the badge text. Receives how many items are in flight - your chance at real plural rules. |

Behavior notes:

- The first nine fields are the keyboard-drag phrases. `Draggable`'s
  keyboard path calls them and pushes the result through the context's
  announcement channel; one `LiveRegion` per provider speaks it. Custom
  flows push their own via `dnd.announce(...)`.
- `move_up`, `move_down` and `row` label the real buttons that
  `a11y::ReorderButtons` renders; `selection_count` fills the multiselect
  `SelectionCount` badge. Both components read the struct through
  `use_dnd_strings`, so one provide covers them all.
- `DndStrings` is `Clone`. Its `Debug` impl prints just `DndStrings`; the
  closures are opaque.
- `Default::default()` is the built-in English, pinned by the crate's
  tests - the exact wordings are user-facing contracts apps may key
  translations off.

## `TwoNamePhrase`

A phrase taking two names (the zone, then its parent):
`Rc<dyn Fn(&str, &str) -> String>`. The type of `over_inside`. It is not
re-exported in the prelude; reach it as
`dioxus_dnd::core::strings::TwoNamePhrase`.

## `use_dnd_strings`

```rust,ignore
pub fn use_dnd_strings() -> DndStrings
```

The subtree's `DndStrings`, or the English defaults when no ancestor
provided one. Captured once per component instance - localize by having
the provided closures read your locale state, not by re-providing the
struct. Public so custom components voice themselves consistently with the
built-ins.

## Where the rest lives

`LiveRegion` and `ReorderButtons`:
[docs/api/accessibility.md](accessibility.md). `SelectionCount`:
[docs/api/multiselect.md](multiselect.md). The `announce` and
`announcement` methods on the context: [docs/api/core.md](core.md).
`DndProvider`'s `dir: Direction::Rtl`, the layout half of localization:
[docs/api/drag-and-drop.md](drag-and-drop.md). `DndDebugOverlay` is
intentionally not localized (a dev-only tool):
[docs/api/debugging.md](debugging.md).

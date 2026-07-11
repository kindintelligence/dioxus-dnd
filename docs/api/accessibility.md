# Accessibility API reference

Accessibility helpers: `LiveRegion`, the visually-hidden `aria-live` region
that voices the shared context's announcement channel to screen readers, and
`ReorderButtons`, the no-gesture reordering fallback.

Concept guide: [docs/concepts/accessibility.md](../concepts/accessibility.md).
The keyboard interaction itself is built into the core `Draggable` - every
draggable is focusable and operable with Space/Enter (pick up / drop), arrow
keys (choose a drop target from the registered zones) and Escape (cancel);
see [docs/api/drag-and-drop.md](drag-and-drop.md). This module adds the
voice and the fallback.

```rust,ignore
DndProvider::<Card> {
    LiveRegion::<Card> {}
    Draggable::<Card> { payload: card.clone(), label: "Ship it", "Ship it" }
    DropZone::<Card>  { label: "Done", on_drop, "Done" }
}
```

Give `Draggable` and `DropZone` a `label` for meaningful announcements
("Picked up Ship it. ...", "Over Done."). Custom flows push their own
messages with `DndContext::announce`.

## `LiveRegion`

A visually-hidden `aria-live="polite"` region voicing drag announcements.
Renders a `div` with `aria-live="polite"`, `aria-atomic="true"` and
`role="status"`, hidden by the standard visually-hidden recipe: present to
the accessibility tree, invisible on screen. It reads the context's
announcement channel, so it voices the built-in keyboard announcements and
anything pushed through `DndContext::announce` alike, without ever moving
focus.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `phantom` | `PhantomData<T>` | `PhantomData` | Internal marker carrying the generic; never set it. |

The type parameter is the only real configuration: `LiveRegion::<Card>`
voices the `DndProvider::<Card>` above it. Render exactly one per provider,
anywhere in its subtree; with multiple providers, render one each. The
element forwards no attributes and needs no styling.

## `ReorderButtons`

Headless move-up / move-down buttons - the most robust accessibility
fallback of all: reordering with plain button presses, no drag gesture
(pointer *or* keyboard-drag) required. Emits the same `SortEvent` your drag
path already handles, so one `on_sort` serves both inputs.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `index` | `usize` | required | This row's index. |
| `total` | `usize` | required | Total number of rows; drives the edge disabling. |
| `label` | `Option<String>` | `None` | Accessible name of the item, used in the button labels ("Move {label} up"). Falls back to the localized row phrase, "item {n}" (1-based). |
| `on_sort` | `EventHandler<SortEvent>` | required | Fired with the same event shape as drag-reordering: up emits `SortEvent { from: index, to: index - 1 }`, down emits `{ from: index, to: index + 1 }`. |

Extra attributes (`class`, `style`, ...) are forwarded to the wrapper
`span`. The two `<button type="button">` children are unstyled and render
plain arrow glyphs; style them with descendant selectors, or replace the
glyphs entirely via CSS.

| Attribute | Where |
|---|---|
| `data-reorder` | on each button, valued `"up"` \| `"down"`, so the two directions style independently |

Behavior:

- The up button is `disabled` on the first row (`index == 0`), the down
  button on the last (`index + 1 >= total`), so an out-of-range event can
  never be emitted and the affordance reflects what is possible.
- The wrapper stops `pointerdown` propagation and each click stops its own,
  so pressing a button never starts (or gets swallowed by pointer capture
  of) an enclosing drag surface such as a `SortableList` row. Taps stay
  taps; the row still drags everywhere else.
- The `aria-label`s come from `DndStrings` (`move_up`, `move_down`, with
  `row` as the unlabeled fallback), so they localize with everything else.
  See [docs/api/localization.md](localization.md).
- Nothing about it assumes `SortableList`: pair it with `SortableGrid` or
  any list of your own that handles `SortEvent`.

```rust,ignore
SortableList {
    len: items.read().len(),
    render: move |ix: usize| rsx! {
        span { "{items.read()[ix]}" }
        ReorderButtons { index: ix, total: items.read().len(), on_sort }
    },
    on_sort,
}
```

## The reduced-motion override

This module also owns the crate's `prefers-reduced-motion` stylesheet.
Every element the crate animates carries `data-dnd-motion`, and the
outermost animated component in a subtree renders one hidden `<style>`
forcing `transition-duration: 0.01ms` on those elements when the user asks
the OS for reduced motion. Near-zero rather than zero, so `transitionend`
still fires for any cleanup listening.

The public contract is the attribute: mark your own animated elements with
`data-dnd-motion` and they honor the setting too. The hooks and constant
behind the sheet (`use_reduced_motion_css`, `use_reduced_motion_css_if`,
`REDUCED_MOTION_CSS`) are crate-internal.

## Where the rest lives

The announcement channel (`DndContext::announce` to push,
`DndContext::announcement` to read): [docs/api/core.md](core.md). Every
announced phrase and button label, as localizable `DndStrings` fields:
[docs/api/localization.md](localization.md). The keyboard interaction and
the `label` props: [docs/api/drag-and-drop.md](drag-and-drop.md).
`SortEvent`: [docs/api/sortable-lists.md](sortable-lists.md).

# Accessibility

Drag-and-drop is where accessibility usually goes to die: pointer-only
interactions, silent state changes, motion nobody asked for. This crate
treats the accessible path as the same path. Every capability below works
on every `Draggable` and `DropZone` with no extra wiring, and the pieces
you do wire (labels, one `LiveRegion`) are one prop each.

API reference: [api/accessibility.md](../api/accessibility.md).
Live demo: the
[Weekly focus](https://kindintelligence.github.io/dioxus-dnd/weekly-focus)
page runs `ReorderButtons`; every gallery page voices its drags through a
`LiveRegion`.

## The mental model

A keyboard drag is not a parallel feature, it is the same drag. Space on a
focused `Draggable` pushes the same payload into the same context, zones
light the same `data-active`/`data-over` styling, and the drop delivers the
same `DropOutcome` to the same `on_drop`, with `mode: DragMode::Keyboard`.
Because both paths share one state machine, the accessible path cannot
drift out of sync with the pointer path.

Two things are yours to wire, one prop each: give `Draggable` and
`DropZone` a `label` so announcements name things, and render one
`LiveRegion` per provider so the announcements are voiced.

## A worked example

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

rsx! {
    DndProvider::<Card> {
        LiveRegion::<Card> {}
        Draggable::<Card> { payload: card.clone(), label: "Ship it", "Ship it" }
        DropZone::<Card> { label: "Done", on_drop, "Done" }
    }
}
// A keyboard drag, as a screen reader hears it:
// "Picked up Ship it. Use arrow keys to choose a drop target,
//  Enter to drop, Escape to cancel."
// "Over Done."          (arrow keys; nested zones say "Over Done,
//                        inside Sprint board.")
// "Dropped in Done."    (Enter)   or   "Drag cancelled."   (Escape)
```

## The keyboard model

Every `Draggable` is focusable (`tabindex="0"`, `role="button"`,
`aria-roledescription="draggable"`) and fully operable without a pointer:

- **Space or Enter** picks the focused item up.
- **Up and Down** cycle the acceptable zones at the current level, in
  spatial order: top-to-bottom, then left-to-right. Zones are measured at
  pickup, so the order matches what the eye sees, not mount order.
- **Right** descends into the hovered zone's nested zones and **Left**
  ascends to its parent - the WAI-ARIA tree convention, mirrored under
  RTL. In flat apps with no nesting they fall back to next and previous.
- **Space or Enter** drops into the selected zone.
- **Escape** cancels. There is no keyboard trap: focus never leaves the
  item, and every drag can be abandoned.

Zones whose `accepts` rejects the payload are skipped entirely, the same
zones a pointer drag falls through. And when a drop re-mounts the moved
item somewhere else in the tree, the crate restores focus: the browser
dumps focus on `<body>` when the old element unmounts, so the landing
`Draggable` claims it back on mount and Tab order continues from where the
item landed.

## What screen readers hear

Render one `LiveRegion::<T>` per provider, anywhere in its subtree. It is
a visually hidden `aria-live="polite"` region, so every step of a keyboard
drag is voiced without stealing focus, instructions included ("Picked up
Ship it. Use arrow keys to choose a drop target, Enter to drop, Escape to
cancel.").

Dead ends are voiced too: "No drop targets available." when an arrow key
finds nowhere to go, "No drop target selected." when Enter is pressed with
no zone chosen. Custom flows push their own messages through the same
channel with `dnd.announce("...")`; `LiveRegion` voices whatever arrives.

Every phrase is localizable through `DndStrings`, see
[Localization](localization.md). In virtualized lists, forward
`aria-setsize`/`aria-posinset` so position is announced against the full
list, not the rendered window (the gallery's
[Archive](https://kindintelligence.github.io/dioxus-dnd/archive) page
shows this).

## Reordering without any drag at all

`ReorderButtons` renders real move-up and move-down buttons with localized
`aria-label`s ("Move Piranesi up"), disabled at the list edges, emitting
the same `SortEvent` as drag-reordering. One `on_sort` serves pointer
drags, keyboard drags, and plain button presses; your handler cannot tell
which input produced the event, and does not need to. This is the
strongest fallback there is: no gesture of any kind required.

```rust,ignore
render: move |ix: usize| rsx! {
    span { "{items.read()[ix]}" }
    ReorderButtons {
        index: ix,
        total: items.read().len(),
        label: items.read()[ix].clone(),
        on_sort,
    }
},
```

## Right-to-left layouts

Pass `dir: Direction::Rtl` on the provider and keyboard navigation
mirrors: spatial order runs right-to-left within a row, and the
descend/ascend arrows swap so "into" is always the arrow pointing along
reading order.

```rust,ignore
DndProvider::<Card> { dir: Direction::Rtl, /* ... */ }
```

## Reduced motion

Everything the crate animates - `SortableList`'s live preview, `FlipItem`'s
glide, `DragOverlay`'s drop-settle - marks its moving elements with
`data-dnd-motion` and ships a `prefers-reduced-motion: reduce` override, so
drags snap instead of gliding when the user asks the OS for less motion.
Nothing to configure; mark your own animated elements with the same
attribute to opt them in. The override uses a near-zero duration rather
than zero, so `transitionend`-driven cleanup still runs.

## Motor forgiveness

- Presses become drags only after 8px of movement (the `threshold` prop),
  so clicks and tremors stay clicks.
- A release just outside every zone snaps to the closest acceptable zone
  whose edge is within 48px, so drops in gutters still land.
- Touch auto-senses by default: vertical swipes scroll, a short hold or
  sideways pull drags, so scrolling a list and dragging its rows do not
  fight. Explicit `touch_handle` grips remain for lists that want a
  visible affordance. See [Touch and input](touch-and-input.md).

## What this means for compliance

The crate covers the interaction-layer criteria a drag-and-drop feature
usually fails. Mapping to WCAG 2.2:

| Criterion | How it's met |
|---|---|
| 2.1.1 Keyboard (A) | complete keyboard path on every draggable, built in |
| 2.1.2 No Keyboard Trap (A) | focus stays on the item; Escape always cancels |
| 2.5.7 Dragging Movements (AA) | keyboard operation on everything, plus `ReorderButtons` as a no-gesture, single-pointer alternative for reordering |
| 4.1.2 Name, Role, Value (A) | `role="button"` + `aria-roledescription="draggable"`, accessible names from `label` props, real buttons in `ReorderButtons` |
| 4.1.3 Status Messages (AA) | `LiveRegion`'s polite live region announces every state change without moving focus |
| 2.3.3 Animation from Interactions (AAA) | `prefers-reduced-motion` honored across every built-in animation |

Honest scope: a library can't make your *app* compliant. You still own
focus visibility (2.4.7), contrast and target sizes for your rendered
items, and passing meaningful `label`s - the crate makes those the only
things left to do.

## Gotchas

- **Exactly one `LiveRegion` per provider.** Zero means keyboard drags
  work but say nothing. Its type parameter must match the provider's
  payload type; with multiple providers (see
  [Mixing payload types](mixing-payload-types.md)), render one each.
- **`picked_up` is the user's manual.** The pickup announcement carries
  the key instructions; when you localize `DndStrings`, keep the arrows,
  Enter, and Escape in the translation.
- **Unlabeled things announce as "item" and "zone {n}".** The machinery
  works, the voice is meaningless. Pass `label`s, and pass them through
  your translation layer if you have one.
- **Keyboard drops carry `edge: None` and a zero `grab`.** Treat `None`
  as your neutral intent (usually append), so keyboard users get sensible
  placement rather than a degenerate corner drop.

## Related

- [Drag and drop](drag-and-drop.md): the components the keyboard path
  lives on, and the `DropOutcome` both paths deliver.
- [Localization](localization.md): every announced phrase, translatable
  as whole sentences.
- [Styling](styling.md): the `data-*` contract keyboard drags light
  identically.
- [Sortable lists](sortable-lists.md): where `SortEvent` and `on_sort`
  come from.

# Animation API reference

Drop animations: `FlipItem` implements the FLIP technique for reorder
transitions, gliding items to their new slots whenever an epoch counter
reports a layout change.

Concept guide: [docs/concepts/animation.md](../concepts/animation.md).
Drop-settle (the ghost gliding into the receiving zone on drop) is built
into the overlay instead: set `settle: true` on `DragOverlay`, reference in
[docs/api/drag-and-drop.md](drag-and-drop.md).

You drive `FlipItem` with an `epoch` counter: bump it whenever order
changes.

```rust,ignore
let mut items = use_signal(|| vec![/* ... */]);
let mut epoch = use_signal(|| 0usize);
rsx! {
    SortableList {
        len: items.read().len(),
        render: move |ix: usize| rsx! {
            FlipItem { epoch: epoch(), Row { item: items.read()[ix].clone() } }
        },
        on_sort: move |ev: SortEvent| {
            apply_sort(&mut items.write(), ev);
            epoch += 1;
        },
    }
}
```

## `FlipItem`

Wraps one list or grid item and glides it to its new position whenever
`epoch` changes, via FLIP (First, Last, Invert, Play): on each bump the item
measures where it moved from, renders instantly *back* at its old position
via a transform, then releases the transform with a CSS transition, so tiles
appear to glide to their new slots. Needs no `DndProvider`; any keyed layout
change qualifies, not just drags.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `epoch` | `usize` | required | Bump whenever the surrounding order changes; triggers the re-measure. |
| `duration` | `f64` | `200.0` | Transition duration in milliseconds. |
| `easing` | `String` | `"ease"` | CSS easing function for the glide. |

Renders a wrapper `div` and forwards arbitrary attributes (`class`, `id`,
...) to it. The wrapper's inline style *is* the animation (the transform and
its transition), so style through `class` and children rather than a
competing inline `transform`.

Data attributes:

| Attribute | Present while |
|---|---|
| `data-dnd-motion` | always; marks the wrapper for the crate's reduced-motion override |

Behavior notes:

- **The measurement contract.** `FlipItem` watches the counter, not your
  data. It needs a stable key so Dioxus keeps the DOM node across the
  reorder; it can only measure a move the node survives. The first
  measurement is a baseline: nothing glides until the item has a previous
  rectangle to compare, and an item whose position did not change does
  nothing.
- **At rest it is inert.** Between epochs the wrapper carries no transform
  and an armed transition, nothing else, so it costs nothing while the
  layout is static. One epoch signal can drive several `FlipItem` groups
  that change together.

## The two glide paths

With the `web` feature (on a web renderer), the glide is armed synchronously
on the real DOM element - invert, forced style flush, release - so it cannot
race the browser's paint schedule, and the animation itself is a plain
compositor-driven CSS transition. The rest style written on the element
equals the style the component renders, so the virtual DOM's view of the
attribute stays truthful.

Without it (or on a non-web renderer), a render-twice fallback stands in:
one render commits the inverted frame, the next releases the transform.
That path is **experimental** - it depends on the browser painting the
inverted frame between two commits, so validate it in your target renderer
and tune `duration` to taste.

## Reduced motion

The wrapper's `data-dnd-motion` opts it into the crate's
`prefers-reduced-motion: reduce` override, which `FlipItem` ships itself
(one hidden `<style>` per subtree): when the user asks the OS for less
motion, the glide snaps near instantly. The override uses a near-zero
duration rather than zero, so `transitionend`-driven cleanup still runs.
Mark your own animated elements with the same attribute to opt them in.

## Snap-back on cancel

Needs no Rust at all; it is a CSS recipe: give the overlay's child
`transition: transform 150ms ease` and revert your item's `data-dragging`
styles with a transition.

## Where the rest lives

`DragOverlay`'s drop-settle and `SettleSlot`'s handoff:
[docs/api/drag-and-drop.md](drag-and-drop.md). The live preview that
animates during a sortable drag:
[docs/api/sortable-lists.md](sortable-lists.md). The reduced-motion
contract across the crate:
[docs/concepts/accessibility.md](../concepts/accessibility.md).

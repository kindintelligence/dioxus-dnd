# Animation

Drag and drop earns its motion in three places: the ghost gliding into the
receiving zone on drop, items gliding to their new slots after a reorder,
and a cancelled drag snapping back. Each has one owner, and only the reorder
glide needs a component from this module: `FlipItem`.

API reference: [api/animation.md](../api/animation.md).
Live demo: the
[Shuffle](https://kindintelligence.github.io/dioxus-dnd/shuffle) page drives
the glide from a button, the
[Menu](https://kindintelligence.github.io/dioxus-dnd/menu) page from a
filter.

## The mental model

`FlipItem` implements FLIP (First, Last, Invert, Play) for one wrapped item:

1. **First**: remember the item's rectangle from the last layout.
2. **Last**: let the new layout apply and measure where the item ended up.
3. **Invert**: instantly transform the item back to its old position, with
   no transition.
4. **Play**: release the transform with a CSS transition armed. The browser
   animates the release, so the item appears to glide to its new slot.

The component does not watch your data. It watches an `epoch` counter you
bump whenever the surrounding order changes, and on each bump it compares
rectangles. An item that moved three cells glides three cells; an item that
stayed put does nothing. The animation belongs to the layout change, not to
any particular gesture, so the same wrapper animates drag reorders, filters,
sorts and insertions. `FlipItem` needs no `DndProvider`.

## A worked example

Stable keys plus one epoch bump per change is the entire integration:

```rust,ignore
let mut tiles = use_signal(|| (1..=6).collect::<Vec<u32>>());
let mut epoch = use_signal(|| 0usize);

rsx! {
    button { onclick: move |_| { tiles.write().rotate_left(1); epoch += 1; }, "Shuffle" }
    div { class: "grid grid-cols-6 gap-2",
        for n in tiles.read().iter().copied() {
            FlipItem { key: "{n}", epoch: epoch(), Tile { n } }
        }
    }
}
```

The key matters as much as the epoch. Keys tell Dioxus which DOM node
belongs to which item, so a reorder moves nodes instead of rewriting their
contents; `FlipItem` can only measure a move if the node survives it. Key by
identity (the tile's number), never by position.

Wired to a drag reorder, the bump lives in `on_sort`:

```rust,ignore
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
```

One epoch signal can serve several `FlipItem` groups that change together;
they all re-measure on the same bump. Between epochs the wrapper is inert -
no transform, an armed transition, nothing else - so it costs nothing while
the layout is at rest.

## Two paths to the glide

The invert step is fragile by nature: the browser must start the transition
from the old position, not the new one. The crate has two ways to guarantee
that, chosen by the `web` cargo feature.

- **With `web`** (on a web renderer), the glide is armed synchronously on
  the real DOM element: write the inverted transform, force a style and
  layout flush, then write the rest style with its transition armed. The
  sequence cannot race the browser's paint schedule, and the animation
  itself is a plain compositor-driven CSS transition.
- **Without it**, a render-twice fallback stands in: one render commits the
  inverted frame, the next releases the transform. That path is
  **experimental** - it depends on the browser painting the inverted frame
  between two commits, so validate it in your target renderer and tune
  `duration` to taste.

Everything else about the component is identical on both paths.

## Reduced motion

`FlipItem` marks its wrapper with `data-dnd-motion` and ships a
`prefers-reduced-motion: reduce` override, so the glide snaps instead of
gliding when the user asks the OS for less motion. Nothing to configure, and
the same is true of every animation the crate owns. Mark your own animated
elements with the same attribute to opt them in. See
[Accessibility](accessibility.md).

## Drop-settle and snap-back

The other two animations need no component from this module:

- **Drop-settle** (the ghost gliding into the receiving zone on a
  successful drop) is built into the overlay: set `settle: true` on
  `DragOverlay`, and wrap the landed element in `SettleSlot` for a seamless
  handoff. Both are documented in
  [api/drag-and-drop.md](../api/drag-and-drop.md).
- **Snap-back on cancel** is a CSS recipe, no Rust at all: give the
  overlay's child `transition: transform 150ms ease` and revert your item's
  `data-dragging` styles with a transition.

## Gotchas

- **Key by identity, never by position.** A positional key makes the
  reorder rewrite contents in place, and there is no move to measure.
- **Entering items do not animate.** Only survivors that moved glide; pair
  with your own enter transition if appearing items should fade in.
- **Do not fight the transform.** The wrapper's inline style *is* the
  animation (a transform plus its transition), so keep drop-completion
  effects shadow-only or color-only, and never put a competing inline
  `transform` on the wrapper.
- **The fallback path is timing-dependent.** Without the `web` feature this
  is the one code path in the crate whose behavior depends on browser paint
  timing rather than pure logic. Enable `web` for web builds.

## Related

- [Drag and drop](drag-and-drop.md): `DragOverlay`, `SettleSlot` and the
  `data-dragging` styling hook.
- [Sortable lists](sortable-lists.md): the live-preview displacement that
  animates during the drag itself.
- [Accessibility](accessibility.md): the reduced-motion contract in full.

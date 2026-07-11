# Sortable lists and grids

`SortableList` reorders one collection by dragging its rows; `SortableGrid`
does the same for tiles laid out in columns. They share one contract: you
hand them a length and a way to render item N, and a completed drag hands
you back "move index 3 to index 0". The component owns the gesture; you
own the data.

API reference: [api/sortable-lists.md](../api/sortable-lists.md).
Live demos:
[Playlist](https://kindintelligence.github.io/dioxus-dnd/playlist) (the
list),
[Photo album](https://kindintelligence.github.io/dioxus-dnd/photo-album)
(the grid), and
[Podcast queue](https://kindintelligence.github.io/dioxus-dnd/podcast-queue)
(touch handles inside a scrollable list).

## The mental model

Three props make the whole contract:

- `len` says how many items there are.
- `render` draws the item at a given index.
- `on_sort` receives a `SortEvent { from, to }` when a drop commits.

The component never sees your data. It renders one wrapper per index,
measures the wrappers, runs the drag gesture, and reports the result as a
pair of indices. `apply_sort` applies that event to any `Vec` in one call;
`apply_swap` exchanges the two positions instead.

Both components are self-contained. Unlike everything built on `Draggable`
and `DropZone`, they need no `DndProvider`: drag state lives inside the
component and the payload transport is indices. The flip side: a sortable
reorders within itself only. Moving items between containers is the boards
pattern - see [Boards](boards.md).

Mouse, touch and pen all drive the same pointer-event gesture machine as
`Draggable`, so the browser never creates a native drag image, and presses
become drags only after 8px of travel, so clicks and taps stay what they
are.

## A complete example

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[component]
fn Playlist() -> Element {
    let mut items = use_signal(|| vec![
        "alpha".to_string(), "beta".into(), "gamma".into(),
    ]);
    rsx! {
        SortableList {
            len: items.read().len(),
            render: move |ix: usize| rsx! { "{items.read()[ix]}" },
            on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
        }
    }
}
```

While you drag, the picked-up row slides toward its landing slot and its
neighbors translate out of the way: a live preview of the final order. On
release, `on_sort` fires once and you mutate the `Vec`; the component
clears its preview before your handler runs, so the real order takes over
seamlessly.

## The live preview

The preview is pure `transform: translate`, no reparenting. Two movements
happen at once:

- Rows between the source and the hovered target shift by one slot pitch
  (the measured distance between consecutive row origins, so CSS gaps and
  margins count) to close the source's slot.
- The source row itself translates all the way to the target slot.

Every slot stays filled by exactly one box, which is what makes the
preview read as the final order rather than a hole plus a floater. A row
is adopted as the target only when the pointer crosses its midpoint in the
travel direction, so the gap never flickers at row boundaries; hovering
the source row or leaving the list keeps the previous target. Slides run
for `transition_ms` (default 160ms) and honor `prefers-reduced-motion` by
snapping. Set `live_preview: false` for highlight-only feedback via
`data-drop-target`.

## The overlay ghost

By default the row itself slides, visible and in flow. For the
floating-ghost feel (dnd-kit's default look), set `overlay`:

```rust,ignore
SortableList {
    len, render, on_sort,
    overlay: move |ix: usize| rsx! { RowGhost { ix } },
}
```

Your `overlay(ix)` element floats pinned to the pointer, sized to the
picked-up row's measured rect. The in-flow row turns invisible but keeps
translating, so its empty box is the gap the neighbors part around. Keep
the ghost lightweight: it is your content, not a clone of the row. If the
row's rect has not been measured yet, no ghost renders and the row simply
slides, so nothing ever disappears.

## Grids

`SortableGrid` is the two-dimensional sibling: a flat `Vec` displayed in
`cols` columns, for dashboards, tile galleries and icon views. Same
contract, one extra prop:

```rust,ignore
let mut tiles = use_signal(|| (0..12).collect::<Vec<u32>>());
rsx! {
    SortableGrid {
        len: tiles.read().len(),
        cols: 4,
        mode: ReorderMode::Swap,
        render: move |ix: usize| rsx! { Tile { n: tiles.read()[ix] } },
        on_sort: move |ev: SortEvent| apply_swap(&mut tiles.write(), ev),
    }
}
```

Dropping a tile on another either inserts (everything reflows, like a
photo gallery; pair with `apply_sort`) or swaps (tiles trade places, like
a dashboard; pair with `apply_swap`). `mode` itself changes no behavior:
it surfaces as `data-mode="insert"` or `"swap"` on the root so the two
feels can style differently, and your choice of `apply_sort` or
`apply_swap` decides the semantics.

The grid renders a `display: grid` wrapper with `cols` equal columns. A
forwarded `style` merges after that default, so
`style: "grid-template-columns: 2fr 1fr 1fr;"` customizes the tracks while
`display: grid` survives, and spacing is just `class: "gap-2"`. The
hovered tile is simply the one under the pointer - no midpoint hysteresis,
because tiles do not shift while you hover. `cell_of` and `index_of`
convert between flat indices and `(row, col)` for custom layouts and
keyboard grid navigation.

## Touch

Whole rows are the touch target by default, with `TouchSense::Auto`: a
vertical swipe keeps scrolling the list, a short hold or a sideways pull
picks the row up. Nothing to configure inside scrollable views. Two
alternatives:

- `touch: TouchSense::Immediate` makes any 8px travel drag, which stops
  finger-scrolling across the rows. Right for lists that never scroll.
- `touch_handle: true` confines pointer drags to a leading grip. A grip is
  an explicit statement of intent, so it is always immediate, while the
  rest of each row keeps scrolling by finger. The grip is exposed as
  `[data-sort-handle]`; replace its default glyph per row with the
  `handle` prop.

Grid tiles always carry `touch-action: none`: grids rarely need to scroll
by dragging across their own tiles.

## Styling

Both components render their own item wrappers, and those wrappers are
where `data-dragging` (the dragged item) and `data-drop-target` (the
hovered landing slot) live. Both attributes are present while active and
absent otherwise, so presence-based selectors work directly. For lists,
reach the wrappers from the root's forwarded `class` with direct child
selectors; the grid also takes `item_class` for its tile wrappers:

```rust,ignore
SortableList {
    len, render, on_sort,
    class: "[&>*]:rounded [&>*]:border [&>*]:bg-white [&>*]:p-2
            [&>[data-dragging]]:opacity-40
            [&>[data-drop-target]]:border-blue-500",
}
```

See [Styling](styling.md) for the crate-wide contract.

## Scrolling mid-drag

Wrap a scrolling list in `AutoScroll` and drags near its edges scroll it.
After every scroll it pings the rect-refresh channel, and the sortable
re-anchors its cached row slots against the wrapper's movement, so hover
highlighting and the drop land on what the user actually sees. The
sortables need no provider; `AutoScroll` anchors the channel for them. If
you move layout under a live drag some other way, call `refresh_all()`
from `use_rect_refresh()`. See [Auto-scroll](autoscroll.md).

## Reordering without a drag

Sortables ship no keyboard drag of their own. The accessible path is
`ReorderButtons`: real move-up and move-down buttons that emit the same
`SortEvent`, rendered inside your `render` content, so one `on_sort`
serves pointer drags and button presses alike. The
[Weekly focus](https://kindintelligence.github.io/dioxus-dnd/weekly-focus)
page demonstrates it. See [Accessibility](accessibility.md).

## Gotchas

- **The preview assumes uniform item sizes.** Neighbor shifts use the
  measured slot pitch, but the source's travel distance multiplies that
  one pitch, so wildly mixed row heights preview approximately. The
  committed reorder is exact regardless: hit-testing uses each row's own
  measured rect.
- **`mode` is a styling hint.** `ReorderMode::Swap` with `apply_sort`
  compiles fine and confuses users; keep the prop and the apply function
  paired.
- **A release outside the list or grid cancels.** Dropping a row
  "nowhere" commits no reorder and fires no event.
- **`touch_handle` restructures the row.** The wrapper becomes a flex row
  holding the grip plus a `[data-sort-content]` div around your content;
  style with that in mind.
- **Reordering across containers is not this pattern.** One sortable is
  one closed surface; reach for [Boards](boards.md) when items move
  between lists.

## Related

- [Drag and drop](drag-and-drop.md): the provider-based machinery, for
  when payloads and cross-zone drops enter the picture.
- [Boards](boards.md): reordering across columns.
- [Touch and input](touch-and-input.md): the full `TouchSense` story.
- [Auto-scroll](autoscroll.md): scrollable sortables.
- [Accessibility](accessibility.md): `ReorderButtons` and announcements.

# Sortable lists API reference

Self-contained reorder components: `SortableList` drags rows along one
axis, `SortableGrid` drags tiles in a `cols`-column grid, and both emit
index-based `SortEvent`s you apply with `apply_sort` or `apply_swap`. No
`DndProvider` is needed; drag state lives inside each component and the
payload transport is indices, so you keep ownership of the data.

Concept guide:
[docs/concepts/sortable-lists.md](../concepts/sortable-lists.md).
`SortableGrid`, `cell_of` and `index_of` live in the sibling `grid`
module; everything on this page is re-exported through the prelude except
`pointer_target` (reach it as `dioxus_dnd::sortable::pointer_target`).

```rust,ignore
let mut items = use_signal(|| vec!["a".to_string(), "b".into(), "c".into()]);
rsx! {
    SortableList {
        len: items.read().len(),
        render: move |ix: usize| rsx! { li { "{items.read()[ix]}" } },
        on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
    }
}
```

## `SortableList`

A list whose items drag to reorder. Data-agnostic: it renders one wrapper
`div` per index inside a root `div` (arbitrary attributes forward to the
root), measures the wrappers, runs the pointer gesture, and emits a
`SortEvent` on drop. Headless: the component ships behavior plus `data-*`
styling hooks; you compose the looks. Rows slide to preview the drop by
default; opt into a floating, caller-composed ghost with `overlay`.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `len` | `usize` | required | Number of items. |
| `render` | `Callback<usize, Element>` | required | Renders the item at the given index. |
| `on_sort` | `EventHandler<SortEvent>` | required | Fired when the user drops an item at a new position. |
| `axis` | `Axis` | `Vertical` | Which axis rows are laid out (and shifted) along. |
| `live_preview` | `bool` | `true` | Open a live gap where the drop would land by translating the rows in between. `false` keeps rows still; style the hovered slot via `data-drop-target`. |
| `transition_ms` | `u32` | `160` | Duration of the row-slide transition during live preview, in milliseconds. |
| `overlay` | `Option<Callback<usize, Element>>` | `None` | Opt-in floating ghost: renders `overlay(index)` pinned to the pointer, sized to the picked-up row's measured rect, and hides the in-flow row so its slot reads as the drop gap. Keep it lightweight; it is your content, not a clone of the row. |
| `touch_handle` | `bool` | `false` | Confine pointer drags to a leading grip instead of the whole row. The grip carries `touch-action: none`; the rest of the row keeps scrolling by finger. Style it via `[data-sort-handle]`. |
| `touch` | `TouchSense` | `Auto` | How a finger shares whole rows with native scrolling. `Auto` keeps vertical swipes scrolling and picks a row up on a short hold or a sideways pull; `Immediate` makes any 8px travel drag. Ignored under `touch_handle`, where the grip owns every touch. |
| `handle` | `Option<Callback<usize, Element>>` | `None` | Content for the `touch_handle` grip, keyed by index. Defaults to a braille-dots glyph. |

Data attributes, present while true and absent otherwise:

| Attribute | Where | Present while |
|---|---|---|
| `data-dragging` | item wrapper | this row is being dragged |
| `data-drop-target` | item wrapper | this row is the hovered landing slot (never the source row) |
| `data-dnd-motion` | item wrapper | always; marks the sliding element for the crate's `prefers-reduced-motion` override |
| `data-sort-handle` | grip `span` | always, under `touch_handle`; the styling hook for the grip |
| `data-sort-content` | content `div` | always, under `touch_handle`; wraps your `render` output |

Behavior:

- **Live preview.** Rows between the source and the hovered target shift
  by one slot pitch (the measured distance between consecutive row
  origins, so CSS margins and gaps count) to close the source's slot, and
  the source row translates to the target slot, so every slot stays
  filled by exactly one box. A row is adopted as the target only once the
  pointer crosses its midpoint in the travel direction; hovering the
  source row or leaving the list keeps the previous target.
- **Measurement.** Rows are measured at mount and re-measured at every
  drag start, so hit-testing runs against current slots. Mid-drag scrolls
  that ping the rect-refresh channel (an `AutoScroll` above, or
  `refresh_all()` from `use_rect_refresh()`) shift the cached slots by the
  wrapper's measured movement instead of re-measuring rows whose
  transforms are mid-transition.
- **Commit and cancel.** A release outside the bounding box of the
  measured rows cancels: no event fires. Inside it, the drop emits
  `SortEvent { from, to }` only when `from != to`. All internal drag
  state clears before `on_sort` runs, so the re-render your handler
  triggers never sees a stale preview.
- **Overlay.** The ghost renders only when the source row's rect was
  measured and both press and pointer positions are known; otherwise the
  row simply slides, so nothing ever disappears. While the ghost shows,
  the in-flow row is `opacity: 0` but keeps translating; its invisible box
  is the gap.
- **`touch_handle` layout.** The row wrapper becomes a flex row: the grip
  `span`, then a `data-sort-content` div (`flex: 1 1 auto`) around your
  content.
- **Robustness.** While a drag is in flight without native pointer
  capture (capture engages with the `web` feature), an invisible
  full-viewport layer keeps move events flowing to the list; a mouse
  released off-list is recovered when it returns with no button held.
  Android's long-press context menu is suppressed mid-gesture only.

## `SortableGrid`

A grid of tiles reordered (or swapped) by dragging: dashboards, tile
galleries, icon views. A grid is a flat `Vec` displayed in `cols`
columns; dropping a tile onto another either inserts (everything reflows,
like a photo gallery) or swaps (tiles trade places, like a dashboard)
depending on which apply function you pair with `ReorderMode`. It reuses
the sortable vocabulary: drops emit `SortEvent`s you apply with
`apply_sort` or `apply_swap`.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `len` | `usize` | required | Number of tiles. |
| `cols` | `usize` | required | Number of columns. |
| `render` | `Callback<usize, Element>` | required | Renders the tile at the given index. |
| `on_sort` | `EventHandler<SortEvent>` | required | Fired when the user drops a tile on another. |
| `mode` | `ReorderMode` | `Insert` | Insert-and-reflow (gallery) or swap (dashboard). Changes no behavior; it renders as `data-mode` so the two feels can style differently. Pair `Insert` with `apply_sort`, `Swap` with `apply_swap`. |
| `item_class` | `Option<String>` | `None` | Classes for each tile's wrapper div, the element that carries `data-dragging` / `data-drop-target`. |

The root renders `display: grid; grid-template-columns: repeat(cols, 1fr)`
and forwards arbitrary attributes. A forwarded `style` merges after that
default, so per-property overrides win (custom tracks via
`style: "grid-template-columns: 2fr 1fr 1fr;"`) while `display: grid`
stays; spacing needs no override at all (`class: "gap-2"`).

| Attribute | Where | Present while |
|---|---|---|
| `data-mode` | root | always; valued `"insert"` or `"swap"` |
| `data-dragging` | tile wrapper | this tile is being dragged |
| `data-drop-target` | tile wrapper | this tile is hovered as the target (never the source) |

Behavior:

- The hovered tile is simply the one whose rect contains the pointer - no
  midpoint hysteresis, since tiles do not shift while you hover in
  swap/insert grids. A move over a tile element itself also falls back to
  that tile's index when the pointer misses every measured rect.
- Tile wrappers carry `touch-action: none`; grids rarely need to scroll
  by dragging across their own tiles. Mouse, touch and pen use the same
  gesture machine as `Draggable`, so the browser never creates a native
  drag image and any 8px travel drags.
- Tiles never transform mid-drag, so a rect-refresh ping triggers a plain
  re-measure (lists re-anchor instead).
- A release outside the tiles' bounding box cancels, and drag state
  clears before `on_sort` runs. The same capture-substitute layer,
  lost-release recovery and context-menu suppression as `SortableList`
  apply.
- The grid anchors the reduced-motion stylesheet once for its whole
  subtree, so animated tiles (`FlipItem` and friends) inherit the
  override without wiring of their own.

## `SortEvent`

"Move the item at `from` so it ends up at index `to`."

| Field | Type | Meaning |
|---|---|---|
| `from` | `usize` | The index the item was picked up from. |
| `to` | `usize` | The index the item ends up at in the post-move collection. |

Non-exhaustive, so reorder context can be added without a major release.
Synthesize your own events (reorder buttons, tests) with
`SortEvent::new(from, to)`.

## `ReorderMode`

What a completed reorder gesture means. `Insert` (the default) removes
the item and inserts it at the target index, the list reorder; `Swap`
exchanges the two items' positions, the grid or tile swap. On
`SortableGrid` it only sets `data-mode`; the apply function you call
carries the semantics.

## `Axis`

Layout direction of a `SortableList`: `Vertical` (the default) or
`Horizontal`. Decides whether the midpoint test and the preview slide use
the Y axis or the X axis.

## Applying events

- `apply_sort(&mut Vec<T>, SortEvent)` removes the item at `from` and
  inserts it at `to`: the standard list reorder. No-op when
  `from == to` or either index is out of range.
- `apply_swap(&mut [T], SortEvent)` exchanges the two positions instead,
  for fixed-slot layouts. Same out-of-range guards; works on any mutable
  slice.

## Preview and hit-testing primitives

Both pure, public for custom surfaces and tests.

- `displacement(ix, from, over, step) -> f64` is the live-preview offset
  in CSS px along the list axis for the row at `ix` while row `from` is
  dragged over row `over`, with `step` the slot pitch. Rows between the
  two indices shift by `step` to close the source slot, and the source
  row itself translates to the target slot; the offsets sum to zero, so
  every slot stays filled. Assumes uniform row sizes for the source's
  travel distance.
- `pointer_target(rects, from, current, at, axis) -> Option<usize>`
  resolves which row should be the drop target while a drag from `from`
  hovers at `at`, given per-row rects measured at drag start (the stable,
  pre-displacement layout). A row is adopted only once the pointer
  crosses its center in the travel direction; over the source row or
  outside every rect it returns `current` unchanged. Not in the prelude;
  call `dioxus_dnd::sortable::pointer_target`.

## Grid coordinates

Provided for custom layouts and keyboard grid navigation:

- `cell_of(index, cols) -> (usize, usize)` gives the `(row, col)` of a
  flat index. `cols` is clamped to at least 1.
- `index_of(row, col, cols, len) -> Option<usize>` gives the flat index
  of `(row, col)`, or `None` when `col >= cols` or the index reaches past
  `len`.

## Where the rest lives

`ReorderButtons`, the no-gesture reorder fallback emitting the same
`SortEvent`: [docs/api/accessibility.md](accessibility.md). `TouchSense`
details:
[docs/concepts/touch-and-input.md](../concepts/touch-and-input.md).
`AutoScroll` and the rect-refresh channel:
[docs/api/autoscroll.md](autoscroll.md). Cross-container moves:
[docs/api/boards.md](boards.md).

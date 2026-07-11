# Boards API reference

Cross-container moves, the kanban pattern: `BoardItem` cards travel between
`BoardColumn`s, and optionally to an exact position within one via
`BoardSlot`, through the shared drag context.

The payload type flowing through the context is `BoardPayload<T>`, which
remembers where the item came from: wrap your app (or board) in
`DndProvider::<BoardPayload<Card>>`. Every completed move arrives as one
`MoveEvent`, and `apply_move` applies it to a
`HashMap<ContainerId, Vec<T>>` model.

Concept guide: [docs/concepts/boards.md](../concepts/boards.md). Live demo:
the [Sprint board](https://kindintelligence.github.io/dioxus-dnd/sprint-board)
gallery page.

```rust,ignore
rsx! {
    DndProvider::<BoardPayload<Card>> {
        for (col_id, cards) in columns {
            BoardColumn::<Card> {
                id: col_id,
                on_move: move |mv: MoveEvent<Card>| {
                    apply_move(&mut board.write(), mv);
                },
                for (ix, card) in cards.iter().enumerate() {
                    BoardItem::<Card> { item: card.clone(), column: col_id, index: ix,
                        CardView { card: card.clone() }
                    }
                }
            }
        }
    }
}
```

All three components are generic over the item type
`T: Clone + PartialEq + 'static` and forward arbitrary attributes
(`class`, `style`, `id`, ...) to their wrapper `div`.

## `BoardItem`

A draggable card living in a column. Thin wrapper over `Draggable` that
packs origin info into a `BoardPayload<T>` and declares the column as its
`zone`, so `DropOutcome::from` reports it.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `item` | `T` | required | The card value, delivered as `BoardPayload::item`. |
| `column` | `ContainerId` | required | Column the item currently lives in; becomes `BoardPayload::from`. |
| `index` | `usize` | required | Index within that column; becomes `BoardPayload::index`. |
| `label` | `Option<String>` | `None` | Human name for announcements ("Picked up {label}"). |

Carries `data-dragging` while its payload is in flight, and the full
`Draggable` keyboard path comes along: Space picks up, arrows navigate
zones, Space drops, Escape cancels. The underlying `Draggable` defaults
apply (8px threshold, `TouchSense::Auto`, `DropEffect::Move`); to tune
them, render `Draggable::<BoardPayload<T>>` directly with a hand-built
payload - that is all `BoardItem` does.

## `BoardColumn`

A column that receives `BoardItem`s. Wraps `DropZone`; a drop anywhere on
the column body emits a `MoveEvent` with `to.1 = None` (append). For
precise within-column positions, nest `BoardSlot`s between items.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `id` | `ContainerId` | required | The column's zone identity. Use explicit ids in `u32` range; see the id rule under `ContainerId`. |
| `label` | `Option<String>` | `None` | Human name for announcements ("Over {label}"). |
| `on_move` | `EventHandler<MoveEvent<T>>` | required | Receives every completed move that targets this column body. |
| `accepts` | `Option<Callback<BoardPayload<T>, bool>>` | accept all | Reject payloads (WIP limits). Sees the full payload, origin included. Inherited by every nested `BoardSlot` through context. |

Data attributes, from the underlying `DropZone`:

| Attribute | Present while |
|---|---|
| `data-active` | an acceptable drag is in flight anywhere |
| `data-over` | that drag hovers this column |

There is no `data-edge`: columns do not expose the `edge` prop, exact
positioning is `BoardSlot`'s job. The column provides its `accepts` filter
through context so nested slots inherit it with no extra wiring, and it
provides `ParentZone`, so slots register as its children and keyboard Right
descends from the column into them.

## `BoardSlot`

An insertion point between items in a column. Dropping on it produces a
`MoveEvent` targeting exactly `(column, Some(index))`. Render one slot
before each item and one after the last, so a column with N cards offers
indexes 0 through N.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `column` | `ContainerId` | required | The column this slot belongs to. |
| `index` | `usize` | required | The index an item dropped here is inserted at. |
| `label` | `Option<String>` | `None` | Announcement name. When `None`, the slot announces as "Insert at position {index}", kept in sync as `index` shifts. |
| `on_move` | `EventHandler<MoveEvent<T>>` | required | Receives the precise-insert move. |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-active` | a drag is in flight and its payload passes the inherited column filter |
| `data-over` | that drag hovers this slot (and passes the filter) |

Behavior:

- Registers its own zone under an auto-generated id (auto ids start at
  2^32), as a child of the enclosing column, so keyboard traversal reaches
  it.
- Inherits the column's `accepts` through context: the same closure gates
  its highlighting and is re-checked at delivery, so a precise insert
  honors the same WIP limit as an append. Without a column filter it
  accepts everything.
- `column`, `index` and `on_move` are positional and read live at drop
  time, not captured at registration, so a slot whose `index` shifts as
  items move above it stays correct. Pass the fresh values each render.
- Style the open state without reflow. Pointer hit-testing uses rects
  cached at drag start, so a slot that changes layout size mid-drag strands
  the highlight on stale geometry; scale in an indicator line on
  `data-over` instead, and widen the hit area with a fixed-size band
  overlapping the card edges. Hit-testing reads the measured rect, not DOM
  events, so `pointer-events: none` on the slot still receives drops.
- In a joined multi-window world, `data-over` follows the window-aware
  hover, so slots stay live during cross-window drags.

## `BoardPayload`

What travels through the context while a board item is dragged, and the
provider's type parameter: `DndProvider::<BoardPayload<Card>>`. Derives
`Debug`, `Clone`, `PartialEq`; all fields are public, so a custom drag
source can build one.

| Field | Type | Meaning |
|---|---|---|
| `item` | `T` | Your value. |
| `from` | `ContainerId` | Column the item was picked up from. |
| `index` | `usize` | Index within that column. |

## `MoveEvent`

A completed cross-container move, delivered to `on_move` by both zone
kinds.

| Field | Type | Meaning |
|---|---|---|
| `item` | `T` | The moved value. |
| `from` | `(ContainerId, usize)` | Column and index the item came from. |
| `to` | `(ContainerId, Option<usize>)` | Target column and index; `None` means append to the end. |

The struct is non-exhaustive so move context can be added without a major
release: keep struct patterns open with `..`, and synthesize your own
events (tests, undo stacks) via the constructor:

```rust,ignore
pub fn new(item: T, from: (ContainerId, usize), to: (ContainerId, Option<usize>)) -> Self
```

## `apply_move`

```rust,ignore
pub fn apply_move<T>(board: &mut HashMap<ContainerId, Vec<T>>, mv: MoveEvent<T>)
```

Applies a `MoveEvent` to the plain `HashMap` board model. Takes the event
by value; the item inside becomes the inserted element.

- Removes the item from the source column by index. If the model drifted
  (column missing, index out of range) the removal is skipped and the
  insert still happens, so the event's item is never lost.
- Adjusts same-column forward moves: when source and target columns match
  and the source index is below the target index, removal shifts the tail
  up one, so the target is decremented. Slot indexes computed against the
  pre-move list land correctly; backward and cross-column moves are used
  as-is.
- Inserts at the (adjusted) target index; `None` or an index past the end
  appends. A target column absent from the map is created.

## `ContainerId`

A type alias for `ZoneId`: columns are just zones. Construct explicit ids
as `ZoneId(9101)`. The safety rule: every `BoardSlot` takes an
auto-generated id, and auto ids start at 2^32, so explicit column ids in
`u32` range can never collide with a slot. Do not mint explicit ids at or
above 2^32.

## Where the rest lives

`Draggable`, `DropZone` and `DropOutcome`, the pieces `BoardItem` and
`BoardColumn` build on: [docs/api/drag-and-drop.md](drag-and-drop.md).
`ZoneId`, geometry and the zone registry: [docs/api/core.md](core.md).
Single-container reordering with a live preview:
[docs/api/sortable-lists.md](sortable-lists.md).

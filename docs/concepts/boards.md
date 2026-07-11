# Boards (kanban)

Cards move between columns, and optionally to an exact position inside one.
The board pattern packages the core drag machinery for cross-container
moves: items remember where they came from, and one event describes the
whole move, whichever input performed it.

API reference: [api/boards.md](../api/boards.md).
Live demo: the
[Sprint board](https://kindintelligence.github.io/dioxus-dnd/sprint-board)
page runs three columns with insertion slots and a live WIP limit.

## The mental model

The provider's payload type is `BoardPayload<T>`: your item plus the column
and index it was picked up from. Wrap the board in
`DndProvider::<BoardPayload<Card>>`, then compose three components:

- `BoardItem<T>` wraps `Draggable`. On pickup it packs the origin into the
  payload.
- `BoardColumn<T>` wraps `DropZone`. A drop anywhere on the column appends.
- `BoardSlot<T>` is an insertion point between cards. A drop on one inserts
  at exactly its index.

Both zone kinds emit the same `MoveEvent<T>`: the item, the
`(column, index)` it left, and the `(column, Option<index>)` it targets,
where `None` means append. `apply_move` applies either shape to a
`HashMap<ContainerId, Vec<T>>`, so the model layer is often one line. Your
`on_move` handler neither knows nor cares whether a drop was an append or a
precise insert.

## A complete example

```rust,ignore
use std::collections::HashMap;
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[derive(Clone, PartialEq)]
struct Task { id: u32, title: String }

const BACKLOG: ContainerId = ZoneId(1);
const DOING: ContainerId = ZoneId(2);
const WIP: usize = 3;

#[component]
fn Board() -> Element {
    let mut board = use_signal(|| {
        HashMap::from([
            (BACKLOG, vec![Task { id: 1, title: "Dark mode tokens".into() }]),
            (DOING, Vec::new()),
        ])
    });
    let on_move = move |mv: MoveEvent<Task>| apply_move(&mut board.write(), mv);
    let count = move |col: ContainerId| board.read().get(&col).map(|v| v.len()).unwrap_or(0);

    rsx! {
        DndProvider::<BoardPayload<Task>> {
            LiveRegion::<BoardPayload<Task>> {}
            for (name, col) in [("Backlog", BACKLOG), ("Doing", DOING)] {
                BoardColumn::<Task> {
                    id: col,
                    label: name,
                    on_move,
                    accepts: move |p: BoardPayload<Task>| {
                        col != DOING || p.from == DOING || count(DOING) < WIP
                    },
                    BoardSlot::<Task> { column: col, index: 0, on_move }
                    for (ix, task) in board.read().get(&col).cloned().unwrap_or_default().into_iter().enumerate() {
                        BoardItem::<Task> {
                            key: "{task.id}",
                            item: task.clone(),
                            column: col,
                            index: ix,
                            label: task.title.clone(),
                            TaskCard { task }
                        }
                        BoardSlot::<Task> { column: col, index: ix + 1, on_move }
                    }
                }
            }
        }
    }
}
```

The rhythm is slot, card, slot, card, slot: one insertion point before each
card and one after the last. Key each card by a stable id so its state
follows it across columns. Both zone kinds share the same `on_move`
handler, and `apply_move` does the rest.

## Columns append, slots insert

A drop on the column body emits a `MoveEvent` whose target index is `None`,
and `apply_move` pushes the item to the end. A drop on a slot emits
`Some(index)` and inserts at exactly that position. Slots are real zones:
pointer, touch and keyboard drops can all target them, and each announces
as "Insert at position N" unless you pass a `label`. They register as
children of their column, so a keyboard user presses Right on a column to
step into its insertion points and Left to step back out.

## The WIP limit idiom

`accepts` on a column sees the full payload, origin included:

```rust,ignore
accepts: move |p: BoardPayload<Task>| p.from == DOING || count(DOING) < WIP
```

A column's `accepts` inherits to every slot inside it through context, the
same mechanism `DndProvider` uses to reach every draggable, so a WIP limit
is one closure with no per-slot wiring. When it returns `false` the whole
column refuses: neither the column nor its slots highlight, keyboard
navigation skips them, and drops bounce, on pointer, touch and keyboard
alike. The `p.from == DOING` arm keeps reordering alive when the column is
full: a move within the column never changes the count.

## Index shifting within a column

Slot indexes are computed against the list as rendered, before the move.
Moving a card forward within its own column invalidates them: remove `a`
from index 0 of `[a, b, c, d]` and the slot that read index 3 now points
one past its target. `apply_move` compensates: when source and target
columns match and the source index is below the target, it decrements the
target after removal, so `(a, 0)` to `Some(3)` yields `[b, c, a, d]`.
Backward moves and cross-column moves need no adjustment and get none.

The two slots hugging the dragged card are no-ops, the card would land
where it already sits. Suppress their indicator by comparing the in-flight
payload against the slot:

```rust,ignore
let is_noop = move |col: ContainerId, ix: usize| {
    dnd.payload()
        .map(|p| p.from == col && (ix == p.index || ix == p.index + 1))
        .unwrap_or(false)
};
```

Reading `dnd.payload()` means calling `use_dnd` in a component that is a
child of the provider, not a sibling.

## Slots must not reflow

Pointer hit-testing uses zone rects cached at drag start. A slot that grows
in the layout mid-drag shifts every card below it and strands the highlight
on stale geometry. Show the open state with zero reflow: an indicator line
scaling in on `data-over` via `transform` and opacity. To make a thin slot
easy to hit, give its element a fixed-height band that overlaps the card
edges with negative margins. Hit-testing reads the measured rect, not DOM
events, so the slot can even be `pointer-events: none` and still receive
drops, while the invisible overlap never steals a pointerdown from the
cards. The Sprint board demo does exactly this.

## Gotchas

- **Explicit column ids belong below 2^32.** Every `BoardSlot` takes an
  auto-generated id, and auto ids start at 2^32, so explicit ids in `u32`
  range can never collide with them. `ZoneId(9101)` is safe forever; an
  explicit id at or above 2^32 is not.
- **The provider's type parameter is the wrapper.** It is
  `DndProvider::<BoardPayload<Card>>`, not `DndProvider::<Card>`. A bare
  `Draggable<Card>` next to a board lives in a different drag world and
  cannot land on its columns; see
  [Mixing payload types](mixing-payload-types.md) to bridge.
- **`MoveEvent` is non-exhaustive.** Construct one with `MoveEvent::new`
  (tests, undo stacks) and keep struct patterns open with `..`.
- **`apply_move` forgives drift but cannot hide it.** If the model changed
  under the event, removal by index is skipped and the insert still
  happens, so the item is never lost; keep `index` props fresh each render
  so it never comes to that.
- **One `LiveRegion` per provider.** Without it, keyboard moves work but
  announce nothing.

## Related

- [Drag and drop](drag-and-drop.md): the `Draggable` and `DropZone`
  underneath, and every prop they expose.
- [Sortable lists](sortable-lists.md): reordering one container with a live
  preview instead of insertion slots.
- [Auto-scroll](autoscroll.md): tall columns that scroll while a drag
  hovers their edge.
- [Accessibility](accessibility.md): what `LiveRegion` announces and why
  labels matter.

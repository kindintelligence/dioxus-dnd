# Multi-select API reference

Multi-select drag: select several items, drag them as one `Vec<K>` payload.
`use_selection` and `Selection` hold which keys are selected,
`SelectableDraggable` resolves its drag payload from that selection, and
`SelectionCount` badges the drag ghost.

Concept guide: [docs/concepts/multiselect.md](../concepts/multiselect.md).
The design leans on the core being generic: the payload type flowing
through the provider is simply `Vec<K>`, so the provider, zones and
overlay are the ordinary components from
[docs/api/drag-and-drop.md](drag-and-drop.md), parameterized with
`Vec<K>`, and every `DropZone::<Vec<K>>` receives the whole selection in
one `DropOutcome<Vec<K>>`.

```rust,ignore
let selection = use_selection::<FileId>();
rsx! {
    DndProvider::<Vec<FileId>> {
        for file in files {
            SelectableDraggable::<FileId> {
                key: "{file.id.0}",
                item: file.id,
                selection,
                FileRow { file }
            }
        }
        DropZone::<Vec<FileId>> {
            on_drop: move |o: DropOutcome<Vec<FileId>>| trash(o.payload),
            "Trash"
        }
        DragOverlay::<Vec<FileId>> { SelectionCount::<FileId> {} }
    }
}
```

## `SelectableDraggable`

A draggable list or grid item participating in a selection. Renders a
wrapper `div` (forwarded attributes, click handling, `data-selected`) with
a `Draggable::<Vec<K>>` inside it. Requires a `DndProvider::<Vec<K>>`
ancestor and panics at first render without one.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `item` | `K` | required | This item's key. `K` is any `Clone + PartialEq + 'static` type; ids are typical. |
| `selection` | `Selection<K>` | required | The shared selection state from `use_selection`. |
| `zone` | `Option<ZoneId>` | `None` | The zone this item lives in, reported in `DropOutcome::from`. |
| `effect` | `DropEffect` | `Move` | Base drop effect; modifier keys can override it at release. |
| `label` | `Option<String>` | `None` | Human name used in screen-reader announcements ("Picked up {label}"). |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-selected` | the item is selected; valued `"true"`, absent otherwise, so presence-based selectors (CSS `[data-selected]`, Tailwind `data-selected:ring-2`) work directly |

Behavior:

- **Click semantics.** A plain click selects only this item; a click with
  Ctrl or Cmd held toggles it. `SelectableDraggable` retains the 3.x
  `Selection::click` behavior. Custom rows can opt into Shift ranges with
  `Selection::click_in_order`.
- **Payload resolution.** Resolved from the current selection each render:
  a selected item drags `selection.items()`, the whole group in selection
  order; an unselected item drags `vec![item]`.
- **Trailing-click protection.** The browser fires a trailing `click` on
  the source after a completed pointer drag; letting it through would
  collapse a just-dragged multi-selection to this one item. Pointer drag
  start arms a flag and that click consumes it. Keyboard drags never arm the
  flag, cancellation clears it, and a new pointerdown retires any stale token
  before the next genuine click.
- **Wrapper structure.** Forwarded attributes and `data-selected` sit on
  the outer div. The inner `Draggable` renders its own wrapper carrying
  `data-dragging` and the keyboard behavior. During a stack drag every
  selected item's inner wrapper carries `data-dragging`, because they all
  resolve to the same `Vec<K>` payload.
- **Input.** Mouse, touch, pen and keyboard drags all work, with the inner
  `Draggable`'s defaults (`threshold` 8.0, `touch` `Auto`). `disabled`,
  `threshold`, `touch`, `on_drag_start` and `on_drag_end` are not
  forwarded (`on_drag_start` drives the trailing-click flag internally);
  compose the core `Draggable::<Vec<K>>` directly when you need them.

## `SelectionCount`

A "N items" badge for the drag ghost: a `span` whose text is the
`selection_count` string applied to the in-flight payload's length. Render
it inside `DragOverlay::<Vec<K>>`.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `phantom` | `PhantomData<K>` | `PhantomData` | Internal marker that carries `K`. Never set it; write `SelectionCount::<K> {}`. |

The default English text is `"{n} item(s)"`; provide `DndStrings` with a
`selection_count` closure for real plural rules, see
[docs/api/localization.md](localization.md). The overlay renders only
while a drag is in flight or settling, and the payload stays readable
through the drop-settle glide, so the badge keeps its count until the
ghost unmounts.

## `Selection`

Selection state for keys of type `K`: a cheap-to-copy handle to shared
signal state. Implements `Copy`, `Clone` and `PartialEq`; copies alias the
same selection, which is why every row can receive the same value and stay
in sync. Reads subscribe the calling component, so `data-selected` and
anything else derived from the selection updates reactively.

| Method | Signature | What it does |
|---|---|---|
| `from_signal` | `(Signal<Vec<K>>) -> Selection<K>` | Non-hook compatibility wrapper. Range operations derive their anchor from the first selected item. |
| `from_signals` | `(Signal<Vec<K>>, Signal<Option<K>>) -> Selection<K>` | Non-hook wrapper for externally owned item and anchor signals. |
| `is_selected` | `(&K) -> bool` | Membership test; drives `data-selected`. |
| `select_only` | `(K)` | Replace the selection with just this key. |
| `toggle` | `(K)` | Add or remove this key (the Ctrl/Cmd+click semantics). Moves the range anchor to this key either way, so a following Shift+click ranges from it. |
| `clear` | `()` | Empty the selection. |
| `items` | `() -> Vec<K>` | Snapshot of the selected keys, in selection order. This is the stack a selected item drags. |
| `len` | `() -> usize` | Number of selected keys. |
| `is_empty` | `() -> bool` | Is nothing selected? |
| `click` | `(K, Modifiers)` | The standard convention in one call: toggles when `Modifiers::CONTROL` or `Modifiers::META` is held, otherwise selects only this key. `SelectableDraggable` calls it for you; in custom rows pass `evt.modifiers()`. |
| `select_range` | `(&[K], &K, bool) -> bool` | Select the inclusive range from the current anchor to a key in stable visual order. `additive` preserves existing selection. Returns false if an endpoint is absent. |
| `click_in_order` | `(K, Modifiers, &[K])` | Plain and Ctrl/Cmd click behavior plus Shift-range and Ctrl/Cmd+Shift additive range selection. |
| `keyboard_range` | `(&[K], usize, isize, bool) -> Option<usize>` | Move a focus index, clamped to the collection, and either select the next item or extend the anchored range. |

Mutating methods take `&mut self`, which the `Copy` handle satisfies with
a `mut` binding, signal-style: `let mut selection = use_selection::<K>()`.

## `use_selection`

`use_selection::<K>() -> Selection<K>` creates selection state owned by
the calling component (a `use_signal` under the hood). Call it once in the
component that owns the list and pass the handle down. For selection that
outlives that component, build the `Signal<Vec<K>>` yourself and wrap it
with the non-hook `Selection::from_signal`; if both pieces of state are owned
elsewhere, use `Selection::from_signals`.

`use_selection_from_signal(items)` is the hook-style option when the item
signal is already owned elsewhere but this component should own the range
anchor.

## Where the rest lives

`DndProvider`, `DropZone`, `DragOverlay`, `Draggable` and `DropOutcome`:
[docs/api/drag-and-drop.md](drag-and-drop.md). `DropEffect` and the
modifier convention behind copy-versus-move drops:
[docs/api/drop-effects.md](drop-effects.md). `ZoneId`:
[docs/api/core.md](core.md). The `selection_count` string and the rest of
`DndStrings`: [docs/api/localization.md](localization.md).

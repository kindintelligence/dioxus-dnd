# Styling

The library ships no CSS and no theme. Every component forwards `class`
(and `style`, `id`, any other attribute) to the wrapper element it renders,
and drag state surfaces as data attributes on those wrappers. Your
stylesheet does all the looking; the crate only tells it what is happening.

Styling has no separate API file: the contract table below is the
reference, verified against source. Every page of the live
[gallery](https://kindintelligence.github.io/dioxus-dnd/) is styled through
nothing but this contract.

## The mental model

Two halves, both flowing through the wrapper `div`:

- **Attributes in.** `class`, `style` and friends forward to the wrapper
  untouched.
- **State out.** While a drag runs, the wrapper carries `data-*` attributes
  that are **present while the state holds and absent otherwise** - never
  `="false"`.

Because presence is the signal, plain CSS attribute selectors and
Tailwind's data-attribute variants both work with zero configuration:

```rust,ignore
// Tailwind: presence-based variant, no custom config.
Draggable::<Card> { payload: card, class: "data-dragging:opacity-40", "Drag me" }
```

```rust,ignore
/* Plain CSS, same contract. */
[data-dragging] { opacity: 0.4; }
[data-over]     { border-color: #3b82f6; }
```

Context-backed attributes follow mouse, touch, pen and keyboard drags
alike, because they read the shared drag context rather than DOM events.
The native boundary zones (`FileDropZone`, `ExternalDropZone`,
`TypedDropZone`) reflect browser drag events from outside the app instead -
same attribute, same styling.

## The contract

| Attribute | Found on | Present while |
|---|---|---|
| `data-dragging` | `Draggable`; `SortableList` / `SortableGrid` item wrappers | that element's payload is being dragged |
| `data-drop-target` | `SortableList` / `SortableGrid` item wrappers | hovered as the drop slot |
| `data-over` | `DropZone`, `BridgeDropZone`, `BoardSlot`, `FileDropZone`, `ExternalDropZone`, `TypedDropZone` | a (compatible) drag hovers the zone (highlight it) |
| `data-active` | `DropZone`, `BridgeDropZone`, `BoardSlot`, `CanvasDropZone` | a compatible drag is in flight anywhere (reveal your targets) |
| `data-edge` | `DropZone` with `edge` set | an acceptable pointer drag hovers; valued `"top" \| "right" \| "bottom" \| "left"` |
| `data-intent` | `TreeNodeTarget` | hovered; valued `"before" \| "after" \| "into"` |
| `data-selected` | `SelectableDraggable` | the item is selected |
| `data-disabled` | `Draggable` | `disabled` is set |
| `data-sort-handle` | the grip inside `SortableList` rows under `touch_handle: true` | always - it marks the grip element rather than tracking state |

Two more attributes are always present and carry a value rather than
signaling state: `SortableGrid` marks its root with
`data-mode="insert"` or `"swap"` so the two reorder styles can differ, and
elements the crate animates carry `data-dnd-motion`, the marker its
`prefers-reduced-motion` CSS targets. Leave the latter to the crate.

## A complete example

The Tailwind form of a full drag interaction, no extra state anywhere:

```rust,ignore
DndProvider::<Card> {
    Draggable::<Card> {
        payload: card,
        class: "rounded-lg border p-3 data-dragging:opacity-40 data-dragging:cursor-grabbing",
        "Drag me"
    }
    DropZone::<Card> {
        on_drop: handle_drop,
        class: "rounded-xl border-2 border-dashed border-transparent p-4
                data-active:border-gray-300 data-over:border-blue-500 data-over:bg-blue-50",
        "Drop here"
    }
}
```

While a compatible drag is in flight every zone shows its dashed border
(`data-active`); the hovered one turns blue (`data-over`). Keyboard drags
light the same selectors.

## Value selectors

`data-edge` and `data-intent` carry a value, so use value selectors.
Tree insertion indicators:

```rust,ignore
class: "data-[intent=before]:border-t-2 data-[intent=into]:bg-blue-50
        data-[intent=after]:border-b-2"
```

Edge indicators on a bare zone (opt in with `edge: EdgeSet::Vertical`):

```rust,ignore
class: "data-[edge=top]:shadow-[0_-2px_0_0_currentColor]
        data-[edge=bottom]:shadow-[0_2px_0_0_currentColor]"
```

Plain CSS spells these `[data-intent="into"]` and `[data-edge="top"]`.

## Lists and grids

`SortableList` and `SortableGrid` render their own item wrappers, and those
wrappers are where `data-dragging` / `data-drop-target` live. For lists,
style the wrappers from the list's forwarded root `class` with direct-child
selectors:

```rust,ignore
SortableList {
    len, render, on_sort,
    class: "[&>*]:rounded [&>*]:border [&>*]:bg-white [&>*]:p-2
            [&>[data-dragging]]:opacity-40
            [&>[data-drop-target]]:border-blue-500",
}
```

`SortableGrid` also takes an `item_class` prop that lands directly on its
tile wrappers. The `touch_handle` grip is a descendant, so reach it with
`[&_[data-sort-handle]]:w-6` from the root class, or `[data-sort-handle]`
in plain CSS.

## How styles merge

Some components need functional inline styles to work: `touch-action` on
`Draggable`, positioning and transforms on `DragOverlay`, and the
`display: grid` layout on `SortableGrid`. Every forwarded style fragment is
collected before the final inline style is emitted. Configurable defaults
such as grid tracks come first, so user declarations win; behavior invariants
such as touch ownership, drag transforms, and settle visibility come last, so
styling cannot disable the interaction. Grid spacing is just
`class: "gap-2"`, and custom column tracks remain
`style: "grid-template-columns: 2fr 1fr 1fr;"`.

Functional styles are inline, so classes do not override them. Properties
documented as configurable have an explicit user-last merge policy; invariant
properties should be composed through the component API rather than replaced.

## Styling children

`class` lands on the component's outer wrapper `div`, and your children sit
nested one level (or more) deeper. Layout utilities on `class` therefore do
not reach them: `flex` on a `Draggable` lays out the wrapper's children,
which is your single content root, not the things inside it. Put layout
classes on your own inner element.

To *react to drag state* from children, two techniques:

- Mark the state-carrying wrapper a group (`SortableGrid`'s
  `item_class: "group"`, or a list root selector like `class: "[&>*]:group"`)
  and use `group-data-dragging:opacity-40` on inner elements.
- With Tailwind v4, skip the wrapper class entirely: the `in-*` variant
  reads ancestors from inside, so `in-data-dragging:italic` on any element
  of your `render` content reacts to the row's drag state with zero wiring.

## Gotchas

- **Never test for `="false"`.** State attributes are absent when false;
  `[data-over="false"]` matches nothing, ever.
- **The wrapper is real.** `class` styles the wrapper `div`, not your
  content; layout like `flex` stops there. Use the group or `in-*`
  technique for state, and your own elements for layout.
- **Inline beats class.** Configurable inline defaults such as grid tracks can
  be overridden through the `style` prop. Interaction invariants such as
  `touch-action`, overlay positioning, and animation transforms are emitted
  last and cannot be replaced by forwarded styles; style your own child or an
  outer wrapper when you need an additional transform.
- **`data-active` is your reveal hook.** Styling only `data-over` makes
  targets invisible until the pointer finds them; light `data-active` too
  so users can see where drops are possible.

## Related

- [Drag and drop](drag-and-drop.md): the components these attributes
  live on.
- [Sortable lists](sortable-lists.md): item wrappers, live preview, grips.
- [Trees](trees.md): `data-intent` in context.
- [Touch and input](touch-and-input.md): where the functional
  `touch-action` styles come from.
- [Animation](animation.md): drop-settle and FLIP, styled the same way.

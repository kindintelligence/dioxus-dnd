# Canvas API reference

Free-position drops - node editors, whiteboards, floor planners. The drop
answers not just "what landed" but "*where* exactly": corrected for grab
offset, optionally snapped to a grid and clamped to bounds.

Concept guide: [docs/concepts/canvas.md](../concepts/canvas.md). The pan/zoom
viewport and the composable modifier chain documented below live in
`dioxus_dnd::core` (`src/core/viewport.rs`, `src/core/modifiers.rs`);
everything on this page is re-exported from the prelude.

```rust,ignore
CanvasDropZone::<Node> {
    snap: SnapGrid(16.0),
    bounds: Bounds { width: 640.0, height: 360.0 },
    on_drop: move |drop: CanvasDrop<Node>| {
        place_node(drop.payload.id, drop.position);
    },
    for node in nodes.read().clone() {
        Draggable::<Node> { payload: node, style: "position: absolute;", NodeView {} }
    }
}
```

## `CanvasDropZone`

A canvas that reports drop positions. It uses the shared `DndContext<T>`;
start drags with the core `Draggable` - its recorded grab offset is what
makes the drop position feel exact, the element lands where its ghost was,
not where the pointer tip was. Generic over `T: Clone + PartialEq +
'static`, renders a wrapper `div` and forwards arbitrary attributes
(`class`, `style`, `id`, ...) to it.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `id` | `Option<ZoneId>` | auto | Stable identity. Auto-generated ids start at 2^32; explicit ids in `u32` range never collide with them. |
| `snap` | `Option<SnapGrid>` | `None` | Snap the corrected position to a square grid. |
| `bounds` | `Option<Bounds>` | `None` | Clamp the corrected top-left position into these bounds. |
| `keyboard` | `CanvasKeyboardPlacement` | `Center` | Placement policy for keyboard-driven canvas drops. |
| `label` | `Option<String>` | `None` | Announced to screen readers when a keyboard drag targets the canvas. Kept in sync if the prop changes. |
| `on_drop` | `EventHandler<CanvasDrop<T>>` | required | Fired with the completed, corrected drop. |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-active` | any drag is in flight (valued `"true"`, absent otherwise) |

Behavior notes:

- The zone registers in the provider's registry, so pointer and keyboard
  drags can land on it, and measures itself the moment it mounts.
- It has no `accepts` filter: every payload of `T` can land, and
  `data-active` lights for every drag in the provider. Style the canvas as
  a target then, e.g. Tailwind `data-active:outline-dashed`.
- `snap`, `bounds` and `keyboard` are live props. They are mirrored into
  signals so the drop callback, registered once at first render, reads the
  current values; a change is observed by same-frame drops.
- On a keyboard drop the zone resolves the placement pointer through the
  `keyboard` policy before running the position pipeline. Keyboard drops
  carry a zero grab offset.
- The zone discovers an enclosing `ParentZone` and registers as its child,
  so keyboard traversal can descend into it, but it does not provide
  `ParentZone` itself: zones nested inside a canvas do not see it as their
  parent.

## `CanvasDrop`

A payload dropped at a position on the canvas, delivered to `on_drop`. Both
points are relative to the canvas.

| Field | Type | Meaning |
|---|---|---|
| `payload` | `T` | The value that was dragged. |
| `position` | `Point` | Top-left position for the dropped element: `pointer` minus the grab offset, then snap, then bounds. Write this into your model. |
| `pointer` | `Point` | The raw pointer position, untouched. For keyboard drops this is the policy-resolved placement point. |

Non-exhaustive: emitted by the zone, only ever consumed by callers, and
likely to grow context fields (modifiers, effect). Destructure with `..`.

## `SnapGrid`

`SnapGrid(pub f64)` snaps positions to a square grid. `snap(p)` rounds each
axis to the nearest multiple of the step; a step `<= 0.0` returns the point
unchanged, so `SnapGrid(0.0)` is a no-op. For independent per-axis steps
use `DragModifier::Snap` below.

## `Bounds`

`Bounds { width: f64, height: f64 }` clamps reported top-left positions
into `0..=width` by `0..=height`. Bounds constrain the drop position
returned in `CanvasDrop::position`; they do not account for the dropped
element's own width or height. Subtract that yourself, or use the item
methods, when the whole element must stay inside.

| Method | Returns | What it does |
|---|---|---|
| `clamp(p)` | `Point` | Clamp a top-left point into `0..=width` by `0..=height`. |
| `clamp_item(p, width, height)` | `Point` | Clamp a top-left so an item of `width` by `height` stays fully inside. An item larger than the bounds on an axis pins to zero on that axis. |
| `clamp_rect(rect)` | `Point` | Clamp a rectangle by moving its top-left so the whole rectangle stays inside. Returns the corrected top-left. |

Non-finite bounds never panic (std `f64::clamp` would): a NaN bound acts as
unconstrained on that side rather than snapping the item to the origin, and
negative coordinates still floor at zero. The oversized-item pin matches
`DragModifier::KeepInside`.

## `CanvasKeyboardPlacement`

Where a keyboard-driven canvas drop places its pointer. Pointer drops use
their event geometry; this policy applies only when the completed drop came
from keyboard interaction.

| Variant | Placement |
|---|---|
| `Center` (default) | The selected zone geometry supplied by core keyboard navigation: the canvas's measured center, or the origin if the canvas is unmeasured. |
| `Origin` | The canvas origin. |
| `Fixed(Point)` | A fixed canvas-local point. |

Snap and bounds still apply afterwards, so keyboard drops land on the grid
and inside the canvas like pointer drops do.

## Position helpers

Pure functions, usable for live previews and custom flows:

| Function | Returns | What it does |
|---|---|---|
| `canvas_position(pointer, grab, snap, bounds)` | `Point` | The exact pipeline the zone runs: `pointer - grab`, then optional `SnapGrid`, then optional `Bounds`, in that fixed order. |
| `canvas_keyboard_pointer(policy, element)` | `Point` | Resolve the canvas-local pointer for a keyboard drop; `element` is the zone-local point core navigation supplies. |
| `client_to_canvas(client, canvas_rect)` | `Point` | Convert a viewport/client point to canvas-local coordinates (`client - canvas_rect.origin()`). |
| `canvas_to_client(point, canvas_rect)` | `Point` | Convert a canvas-local point back to viewport/client coordinates. |

Because clamping runs after snapping, a snapped position outside the bounds
clamps back in and can sit off-grid at the edges.

## Pan and zoom: `CanvasViewport`

Pure pan/zoom geometry for canvas-like coordinate planes, in
`dioxus_dnd::core`. The module intentionally has no event handling or
component state: apps decide how zoom and pan are controlled, the helpers
only convert points and deltas between screen/local space and world space.

`CanvasViewport { pan: Point, zoom: f64 }` is the transform: `pan` is the
screen-space translation and `zoom` is the scale from world coordinates to
screen coordinates. `Default` is `pan: (0, 0)`, `zoom: 1.0`; construct with
`CanvasViewport::new(pan, zoom)`.

| Function | Returns | What it does |
|---|---|---|
| `screen_to_world(point, viewport)` | `Point` | `(point - pan) / zoom`: a canvas-local point (a drop's `position`) into world coordinates. |
| `world_to_screen(point, viewport)` | `Point` | `point * zoom + pan`: a world point into canvas-local coordinates for rendering. |
| `screen_delta_to_world(delta, viewport)` | `Point` | `delta / zoom`: a screen movement into world units. Deltas ignore `pan`. |
| `world_delta_to_screen(delta, viewport)` | `Point` | `delta * zoom`: a world movement into screen units. |
| `CanvasViewport::clamped_zoom(min, max)` | `CanvasViewport` | Copy of the viewport with `zoom` clamped into `min..=max`. An invalid `min` (non-finite or `<= 0`) becomes `0.0`; an invalid `max` leaves the upper side unclamped. |

Every helper treats a non-finite or non-positive `zoom` as `1.0`, so a zero
zoom converts at identity scale instead of dividing to infinity.

## Modifier chains: `apply_modifiers`

Composable drag constraints in `dioxus_dnd::core`, applied as a chain to a
proposed position. Where the canvas bakes snap-and-clamp into one
component, this generalizes the idea, in the spirit of dnd-kit's modifiers:
each `DragModifier` is a pure `Point -> Point` transform, and
`apply_modifiers(chain, p, ctx)` feeds each output into the next, in slice
order. A single modifier applies with `DragModifier::apply(p, &ctx)`.

```rust,ignore
let ctx = ModifierCtx {
    container: Some(Rect::new(0.0, 0.0, 400.0, 300.0)),
    element: Some(Rect::new(0.0, 0.0, 40.0, 40.0)),
};
let chain = [
    DragModifier::LockAxis { horizontal: false, vertical: true },
    DragModifier::Snap { x: 8.0, y: 8.0 },
    DragModifier::KeepInside,
];
let p = apply_modifiers(&chain, Point::new(123.0, 999.0), &ctx);
assert_eq!((p.x, p.y), (0.0, 260.0));
```

`ModifierCtx` carries the geometry a modifier may need; fields a modifier
does not need can stay `None`:

| Field | Type | Meaning |
|---|---|---|
| `container` | `Option<Rect>` | The container the element should stay inside, for `KeepInside`. |
| `element` | `Option<Rect>` | The dragged element's size, positioned at the proposed point. |

`DragModifier` variants:

| Variant | What it does |
|---|---|
| `LockAxis { horizontal, vertical }` | Zeroes the value on a locked axis; `horizontal: false` freezes X. Reads as "zero out movement", so run chains over movement deltas when locking, an absolute position collapses to `0` on the locked axis. |
| `Snap { x, y }` | Snap each axis to its own grid step; a step `<= 0` leaves that axis alone. |
| `KeepInside` | Clamp so the element (its `ModifierCtx::element` size) stays inside `ModifierCtx::container`. No-op when either rect is missing. An element larger than the container on an axis pins to the container's origin on that axis. |

Order matters: snap before `KeepInside` and the clamp wins at the edges;
snap after and the result always sits on the grid but may poke outside.

## Where the rest lives

`Point`, `Rect`, `ZoneId`, the context (`use_dnd`, `pointer()`, `grab()`)
and the registry (`cached_rect` for live previews):
[docs/api/core.md](core.md). `Draggable`, `DropZone` and the `DropOutcome`
the registry delivers: [docs/api/drag-and-drop.md](drag-and-drop.md).
Native file and external drags over a canvas:
[docs/api/file-drops.md](file-drops.md) and
[docs/api/external-content.md](external-content.md).

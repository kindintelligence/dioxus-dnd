# Canvas

Free-position drops for node editors, whiteboards and floor planners. On a
canvas a drop does not mean "slot 3", it means "exactly there", so
`CanvasDropZone` answers with a position: the corrected top-left where the
dropped element should land, ready to write into your model.

API reference: [api/canvas.md](../api/canvas.md).
Live demo: [Moodboard](https://kindintelligence.github.io/dioxus-dnd/moodboard).

## The mental model

`CanvasDropZone<T>` is a drop target over the shared drag context. Drags
start with the ordinary `Draggable<T>` from
[Drag and drop](drag-and-drop.md); the canvas only changes what a completed
drop delivers. Instead of a `DropOutcome`, your `on_drop` receives a
`CanvasDrop<T>` carrying two points, both relative to the canvas:

- `pointer` is the raw release position inside the canvas, untouched.
- `position` is `pointer - grab`, then optional snap, then optional bounds.

The grab offset is where inside the element the pointer picked it up, and
subtracting it is what makes placement feel exact: the element lands where
its ghost was, not jumping so its top-left corner meets the cursor tip.
`Draggable` records the offset for you at pickup.

The canvas does not own your layout. It reports where each drop should put
an element; you write that into your model and position the elements
yourself, which keeps pan/zoom layers and custom rendering entirely in your
hands.

## A complete example

A board of sticky notes whose positions live in your own state:

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[derive(Clone, PartialEq)]
struct Note { id: u32, label: String, x: f64, y: f64 }

#[component]
fn Board(notes: Signal<Vec<Note>>) -> Element {
    rsx! {
        DndProvider::<Note> {
            LiveRegion::<Note> {}
            CanvasDropZone::<Note> {
                snap: SnapGrid(16.0),
                bounds: Bounds { width: 640.0, height: 220.0 },
                on_drop: move |d: CanvasDrop<Note>| {
                    let mut ns = notes.write();
                    if let Some(n) = ns.iter_mut().find(|n| n.id == d.payload.id) {
                        n.x = d.position.x;
                        n.y = d.position.y;
                    }
                },
                class: "relative h-56 overflow-hidden data-active:outline-dashed",
                for note in notes.read().clone() {
                    Draggable::<Note> {
                        key: "{note.id}",
                        payload: note.clone(),
                        label: note.label.clone(),
                        style: "position: absolute; left: {note.x}px; top: {note.y}px;",
                        "{note.label}"
                    }
                }
            }
        }
    }
}
```

The zone renders a wrapper `div` and forwards attributes to it. Give it
`position: relative` so the absolutely positioned children resolve against
the canvas, matching the canvas-relative coordinates the drop reports. While
any drag is in flight the div carries `data-active="true"`; style the canvas
as a target then.

## Grab, then snap, then bounds

The correction pipeline runs in a fixed order so results are predictable:
subtract the grab offset, round to the `SnapGrid` if one is set, clamp into
`Bounds` if they are set. The same pipeline is public as `canvas_position`,
so a preview can compute exactly what a drop will decide. Two consequences
of the ordering:

- Snapping happens on the corrected top-left, not on the pointer, so a
  16px grid aligns element corners, which is what a layout grid means.
- Bounds win over the grid. A snapped position outside the canvas clamps
  back in, and the clamped result can sit off-grid at the edges.

## Bounds clamp a point, not the item

`Bounds { width, height }` clamps the reported top-left into
`0..=width` by `0..=height`. It does not know the dropped element's own
size, so an element released near the right edge can legally hang past it.
When the whole item must stay inside, use `Bounds::clamp_item(position,
w, h)` with the size you know in your app, or `Bounds::clamp_rect` if you
already hold a `Rect`. An item larger than the bounds pins to zero on that
axis rather than oscillating or panicking.

For richer mid-drag constraints, the composable modifier chain in core
generalizes the same idea: `apply_modifiers` runs a slice of
`DragModifier`s (axis locks, per-axis snapping, `KeepInside` with the real
element size) over a proposed position. See the
[API reference](../api/canvas.md) for the chain semantics.

## Keyboard placement

Keyboard drags carry no pointer, so the canvas needs a policy for where a
keyboard drop lands. The `keyboard` prop takes a
`CanvasKeyboardPlacement`:

- `Center` (the default) places at the canvas's measured center, the
  geometry core keyboard navigation supplies for the selected zone.
- `Origin` places at the canvas origin.
- `Fixed(point)` places at a fixed canvas-local point.

Keyboard drops have a zero grab offset, so `position` is the placed point
after snap and bounds, and `pointer` is the policy-resolved point. Snap and
bounds apply the same way for both input modes; a keyboard drop on a 16px
grid lands on the grid.

## Pan and zoom

`CanvasDropZone` works in canvas-local CSS pixels and stays deliberately
simple. For a zoomable plane, keep a `CanvasViewport { pan, zoom }` in your
state and convert at the boundary: `screen_to_world` turns the drop's
canvas-local `position` into world coordinates before writing it into your
model, `world_to_screen` turns model coordinates back into CSS positions
when rendering. The helpers are pure geometry; how wheel events or pinch
gestures drive `pan` and `zoom` is your call.

## Live preview during the drag

Because `canvas_position` is public, the in-flight element can ride the
pointer through the exact placement math its drop will use. Each render,
convert the live pointer into canvas coordinates and run the pipeline:

```rust,ignore
let dnd = use_dnd::<Note>();
let registry = use_zone_registry::<Note>();
if let Some(rect) = registry.cached_rect(BOARD) {
    let pointer = client_to_canvas(dnd.pointer(), rect);
    let live = canvas_position(pointer, dnd.grab(), None, Some(BOUNDS));
    // position the dragged element at `live`
}
```

The element travels across the board and stops where you let go, with no
overlay and nothing left behind. The
[Moodboard demo](https://kindintelligence.github.io/dioxus-dnd/moodboard)
runs exactly this.

## Gotchas

- **`Bounds` does not know the item's size.** It clamps the top-left point
  only. Use `Bounds::clamp_item` when the whole element must stay inside.
- **The canvas accepts every drag of `T`.** There is no `accepts` prop, so
  `data-active` lights for any in-flight drag in the provider. Filter in
  `on_drop` if some payloads should not land here.
- **Positioning context is yours.** The wrapper div is not positioned by
  default; without `position: relative` on it, `left`/`top` on children
  resolve against some ancestor and drops appear to land in the wrong
  place.
- **Snap and bounds props are live.** Changing them re-arms the zone
  immediately; the very next drop uses the new values, even mid-drag.
- **Native content needs its own zone.** File and external drags never
  enter the shared context; layer a `FileDropZone` or `ExternalDropZone`
  over the canvas and place its element point with the same
  `client_to_canvas` and `canvas_position` helpers.

## Related

- [Drag and drop](drag-and-drop.md): `Draggable`, the grab offset, and
  everything the canvas inherits from the core machinery.
- [Accessibility](accessibility.md): keyboard operation and the
  `LiveRegion` announcements canvas drops participate in.
- [File drops](file-drops.md) and
  [External content](external-content.md): native content landing on a
  canvas.
- [Styling](styling.md): the `data-active` contract.

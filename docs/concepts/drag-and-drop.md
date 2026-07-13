# Drag and drop

The core pattern: a `Draggable` you pick up, a `DropZone` that receives it,
and a typed payload that travels between them. Every other pattern in the
crate (sortable lists, boards, trees, canvas) is built on this machinery, so
the ideas here carry everywhere.

API reference: [api/drag-and-drop.md](../api/drag-and-drop.md).
Live demo: most gallery pages use these pieces; the
[Reading list](https://kindintelligence.github.io/dioxus-dnd/reading-list)
page is the plainest.

## The mental model

Three components, one shared value:

- `DndProvider<T>` creates a drag world for payload type `T` and provides it
  to a subtree through Dioxus context.
- `Draggable<T>` wraps anything you can pick up. On drag start it pushes its
  `payload` into the world.
- `DropZone<T>` wraps anything that can receive. On release over it, your
  `on_drop` handler gets the payload back, with everything known about the
  drop.

The payload is any `Clone + PartialEq` Rust value. It moves through a
`Store<DragState<T>>` in context, never through serialization: no JSON, no
string ids, no `DataTransfer`. The type parameter is the wiring: a
`Draggable<Card>` can only land on a `DropZone<Card>`, and the compiler
checks it. One provider means one payload type; when you need more shapes,
see [Mixing payload types](mixing-payload-types.md).

## A complete example

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[derive(Clone, PartialEq)]
struct Card { id: u32, title: String }

#[component]
fn App() -> Element {
    rsx! {
        DndProvider::<Card> {
            Draggable::<Card> {
                payload: Card { id: 1, title: "hello".into() },
                label: "hello",
                "Drag me"
            }
            DropZone::<Card> {
                label: "Archive",
                accepts: move |c: Card| c.id != 0,
                on_drop: move |o: DropOutcome<Card>| {
                    tracing::info!("card {} landed", o.payload.id);
                },
                "Drop here"
            }
        }
    }
}
```

Both components render a wrapper `div` and forward arbitrary attributes
(`class`, `style`, `id`, ...) to it, so styling is direct. The `label` props
are what screen readers announce; make them meaningful.

## What arrives on drop

`on_drop` receives a `DropOutcome<T>`. Beyond `payload`, the fields answer
the questions a handler actually asks:

- Where from, where to: `from` (the `zone` the `Draggable` declared, if any)
  and `to` (the receiving zone's id).
- Move or copy: `effect` resolves the modifier keys held at release. See
  [Drop effects](drop-effects.md).
- How: `mode` is `Pointer` or `Keyboard`. Both paths deliver the same
  outcome through the same handler.
- Where exactly: `client` (viewport coordinates), `element` (relative to the
  zone), and `grab` (where inside the item the pointer picked it up).
  `element - grab` is where the item's top-left should land, which is what
  free-position placement uses.
- Which edge: `edge`, when the zone opted in. See below.

## Acceptance

`accepts` is a payload filter: return `false` and the zone refuses the drag.
A refusing zone never highlights, keyboard navigation skips it, and a
release over it falls through to whatever acceptable zone is beneath. The
closure runs on hover and again at delivery, so keep it cheap and pure; it
sees the payload value, so acceptance is business logic, not string
matching:

```rust,ignore
DropZone::<Card> { accepts: move |c: Card| !c.archived, on_drop, "Active only" }
```

## Insertion edges

A bare zone can tell you which side of it a drop wants. Opt in with
`edge: EdgeSet::Vertical` (a vertical stack cares about top and bottom) and
two things happen: while a pointer drag hovers, the zone carries
`data-edge="top"` or `"bottom"` live for styling, and the delivered
`DropOutcome::edge` records the edge held at release. The handler maps `Top`
to "insert before" without re-deriving geometry. Keyboard drops carry
`None`; treat it as your neutral intent, usually append.

## The drag ghost

By default the picked-up element stays put and carries `data-dragging`;
style that however you like. For a floating ghost pinned to the pointer,
render a `DragOverlay<T>` inside the provider:

```rust,ignore
DragOverlay::<Card> { settle: true, class: "rotate-3 shadow-xl", GhostCard {} }
```

`settle: true` turns on the drop animation: on a successful pointer drop the
ghost glides into the receiving zone instead of vanishing. `match_source:
true` sizes the ghost to the picked-up element's measured rect, so it
appears exactly over what you grabbed. Keyboard drags carry no pointer
position, so skip rendering the ghost when `dnd.mode()` is `Keyboard` if
that matters to your design.

## What you get for free

Behavior you do not write, on every `Draggable` and `DropZone`:

- **Keyboard operation.** Space picks up, arrows choose a zone, Space drops,
  Escape cancels. Same context, same `on_drop`.
  See [Accessibility](accessibility.md).
- **Touch that does not fight scrolling.** Vertical swipes scroll, a short
  hold or sideways pull drags. See [Touch and input](touch-and-input.md).
- **Click protection.** Presses become drags only after 8px of travel
  (`threshold`), so clicks stay clicks.
- **Near-miss forgiveness.** A release just outside every zone snaps to the
  closest acceptable zone within 48px, and a miss re-measures zones and
  retries, so drops in gutters still land.
- **Nesting.** Zones inside zones discover their parents through context,
  which powers hierarchical keyboard traversal. No configuration.

## Gotchas

- **Explicit `ZoneId`s belong below 2^32.** Auto-generated ids start at
  2^32 precisely so hand-written ids in `u32` range can never collide with
  them. Pick small numbers and you are safe forever.
- **The wrapper div is real.** `class` styles the wrapper, not your
  children; a `flex` there does not reach content nested deeper. See
  [Styling](styling.md) for the group technique.
- **`zone` feeds `from`.** If your handler needs to know where an item came
  from, declare `zone: Some(SHELF)` on the `Draggable`; `DropOutcome::from`
  is `None` otherwise.
- **One `LiveRegion` per provider.** Announcements are one component away;
  without it, keyboard drags work but say nothing.
  See [Accessibility](accessibility.md).
- **Overlaps use registry order, not paint order.** Among overlapping
  acceptable zones, the later record receives the drop; rejecting records are
  skipped at release. CSS `z-index`, stacking contexts and portals are not
  inspected, so align registry and visual order or avoid overlapping targets.
  Replacing a same-id record retains its slot.

## Related

- [Architecture](architecture.md): what the provider actually creates, and
  the state machine and registry underneath.
- [Styling](styling.md): the full data-attribute contract.
- [Sortable lists](sortable-lists.md), [Boards](boards.md),
  [Trees](trees.md), [Canvas](canvas.md): the patterns built on these
  pieces.

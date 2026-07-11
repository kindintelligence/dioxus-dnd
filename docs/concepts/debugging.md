# Debugging

When a zone will not light up or a drop lands somewhere surprising, the
useful question is not "what does my DOM look like" but "what does the crate
see". `DndDebugOverlay` answers it by drawing the crate's own zone registry
over your page.

API reference: [api/debugging.md](../api/debugging.md). The overlay is dev
chrome rather than a pattern, so no gallery page demos it; drop it into any
page of your own app.

## The mental model

Pointer hit-testing, keyboard navigation and near-miss snapping all read one
data structure: the provider's zone registry, which holds each zone's id,
label, acceptance callback and cached rectangle. The overlay renders that
registry itself, not a second opinion derived from the DOM. If an outline is
missing or misplaced, hit-testing sees exactly the same wrong thing, which
is the point: the overlay cannot look right while the drag behaves wrong.

## Turning it on

Render one per provider, anywhere inside it. The type parameter picks the
payload world to inspect:

```rust,ignore
DndProvider::<Card> {
    if cfg!(debug_assertions) {
        DndDebugOverlay::<Card> {}
    }
    // ... your app ...
}
```

It is a development tool: unstyled chrome over your UI, intentionally not
localized. Gate it out of release builds yourself, as above.

## Reading the overlay

While idle, every measured zone draws as a tinted outline with a tag naming
its label and id. The tint derives from the zone id, so a zone keeps its
color across renders and you can follow one zone through a session. A status
chip in the corner reads, for example, `12 zones (0 unmeasured) - idle`.

While a drag is in flight, the overlay evaluates each zone's `accepts`
against the live payload:

- Rejecting zones dim, switch to a dashed border, and their tag appends
  `- rejects`.
- The hovered zone fills and its tag appends `- over`. This follows pointer
  and keyboard drags alike, because the overlay reads the shared drag
  context, not DOM events.
- The chip switches to `dragging - over zone 7`, or `dragging - over
  nothing` when no zone contains the pointer.

Zones the registry has not measured yet draw no outline; the chip's
unmeasured count makes that absence visible instead of silent.

## Symptoms to causes

| You see | The registry is telling you |
|---|---|
| No outline where a zone should be | The zone never registered in this world (wrong provider, wrong payload type) or it is still unmeasured; the chip's unmeasured count separates the two. |
| An outline in the wrong place | The cached rect is stale. Idle outlines refresh when zones mount or unmount; the next drag start re-measures everything, so drag and watch it correct itself, or not. |
| A zone dims and goes dashed mid-drag | Its `accepts` callback returns `false` for this payload. That zone never highlights, keyboard navigation skips it, and a release over it falls through. |
| A drop lands on an earlier overlapping record | The later record in registry order rejects the payload, so release selection continued in reverse registry order. This is hit-test order, not CSS paint order. |
| `dragging - over nothing` in a gap between zones | No zone contains the pointer. A release here snaps to the closest acceptable zone within 48px, or ends the drag with nothing consumed. |

## Mixing payload types and multiple windows

One overlay covers one payload world. With several providers of different
types, render one overlay inside each; a `DropZone<B>` never appears in the
`A` overlay, which is itself diagnostic when a zone seems to ignore a drag.
In a desktop window joined to a multi-window `DndWorld`, the overlay takes
its hover state from the world, so a drag arriving from another window
highlights zones correctly. See
[Mixing payload types](mixing-payload-types.md) and
[Multi-window desktop drags](multi-window.md).

## Gotchas

- **It cannot cause the bug it inspects.** The overlay is click-through
  (`pointer-events: none`) by design, so rendering it never changes
  hit-testing, focus or scrolling.
- **Idle outlines can lag layout.** Scroll or resize moves the real
  elements but not the cached rects; the picture is authoritative again the
  moment a drag starts, because drag start re-measures every zone.
- **It sits at `z-index: 9998`.** A modal layered above that covers the
  outlines; the zones underneath are still drawn.
- **Keep it out of release builds.** Nothing in the crate gates it for you;
  `cfg!(debug_assertions)` is the one-line answer.

## Related

- [Testing](testing.md): drive whole drags headlessly in CI once you can
  see what is wrong.
- [Architecture](architecture.md): the registry and state machine the
  overlay renders.
- [Drag and drop](drag-and-drop.md): `accepts`, fall-through and the 48px
  near-miss snap the symptoms table refers to.

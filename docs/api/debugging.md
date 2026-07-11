# Debugging API reference

**Dev-only** drag-and-drop inspector: `DndDebugOverlay` draws every zone
registered in a provider as a tinted, labeled outline pinned over the page.

Concept guide: [docs/concepts/debugging.md](../concepts/debugging.md).

Everything it shows *is* the registry - if an outline is missing or
misplaced, hit-testing sees exactly the same wrong thing, which is the
point. It is a development tool: it renders unstyled debug chrome over your
UI and its output is not localized. Gate it yourself and keep it out of
release builds:

```rust,ignore
DndProvider::<Card> {
    if cfg!(debug_assertions) {
        DndDebugOverlay::<Card> {}
    }
    // ... your app ...
}
```

## `DndDebugOverlay`

Draws every registered zone of one payload world as a tinted outline, with
the zone's label and id in a tag, live hover highlighting, and per-zone
acceptance state while a drag is in flight. Render one per provider,
anywhere inside it; the type parameter selects the world.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `phantom` | `PhantomData<T>` | `PhantomData` | Internal marker; never set it. Name the payload type with the turbofish instead: `DndDebugOverlay::<Card> {}`. |

Data attributes, on the debug chrome itself (handy for tests and tooling):

| Attribute | Where |
|---|---|
| `data-dnd-debug="true"` | the fixed full-viewport root |
| `data-debug-zone="{id}"` | each drawn zone outline |
| `data-over="true"` | the hovered zone's outline |
| `data-accepts="true" \| "false"` | every outline while a drag is in flight; absent when idle |
| `data-debug-status="true"` | the status chip |

What it draws:

- **Outlines.** Each measured zone gets a fixed-position outline whose tint
  derives from the zone id, so it is stable across renders, with neighboring
  ids scattered around the color wheel. A tag shows the zone's `label` (or
  `zone`) and its id.
- **Acceptance, live.** While a drag is in flight, each zone's `accepts` is
  evaluated against the live payload: rejecting zones dim, switch to a
  dashed border, and their tag appends `- rejects`.
- **Hover.** The hovered zone fills and its tag appends `- over`. This
  follows pointer and keyboard drags alike, because the overlay reads the
  shared context rather than DOM events. In a window joined to a
  multi-window `DndWorld`, hover comes from the world, so a drag arriving
  from another window highlights correctly
  (see [docs/api/multi-window.md](multi-window.md)).
- **The status chip.** Idle it reads `12 zones (0 unmeasured) - idle`;
  mid-drag, `dragging - over zone 7` or `dragging - over nothing`.

Zones the registry has not measured yet draw no outline - they are exactly
as invisible to the overlay as they are to hit-testing - and the chip counts
them so absence is visible too.

Measurement: the core only measures zone rects at drag start, but an
inspector wants outlines while idle, so the overlay re-measures whenever the
zone set changes or a zone's DOM handle arrives. It subscribes through a
registry revision that ignores rect writes, so the measuring it triggers
cannot loop. Idle outlines can lag a scroll or resize; the next drag start
re-measures everything.

Click-through by design (`pointer-events: none`), so it never changes the
interaction it inspects. The chrome sits at `z-index: 9998`.

## Where the rest lives

The registry it renders (`records`, `refresh_rects`, `ZoneRecord`):
[docs/api/core.md](core.md). Driving whole drags headlessly in CI:
[docs/api/testing.md](testing.md). One overlay covers one payload world; for
apps with several, see
[docs/api/mixing-payload-types.md](mixing-payload-types.md).

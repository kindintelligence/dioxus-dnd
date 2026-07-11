# Trees API reference

Hierarchical drops for file explorers, nested menus and outliners: a drop
on a node can mean three different things, so `DropIntent` captures the
trichotomy, `TreeNodeTarget` resolves it live from where inside the row the
pointer sits, `intent_from_offset` is the public band math behind it, and
`would_create_cycle` guards against dropping a node into its own subtree.

Concept guide: [docs/concepts/trees.md](../concepts/trees.md).

```rust,ignore
TreeNodeTarget::<u64> {
    node: NodeId(n.id),
    accepts: move |(dragged, intent): (u64, DropIntent)| {
        if intent == DropIntent::Into && !n.folder { return false; }
        !would_create_cycle(parent_of, NodeId(dragged), NodeId(target))
    },
    on_drop: move |ev: TreeDropEvent<u64>| reparent(ev.payload, ev.target, ev.intent),
    Draggable::<u64> { payload: n.id, RowFace {} }
}
```

## `TreeNodeTarget`

A single tree row acting as a drop target with intent detection. The row is
target-only; drags start from a core `Draggable` placed inside it, and the
payload travels through the shared `DndContext<T>`. Renders a wrapper `div`
and forwards arbitrary attributes (`class`, `style`, `id`, ...) to it.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `node` | `NodeId` | required | The node this row represents, handed back as `TreeDropEvent::target`. |
| `row_height` | `f64` | `28.0` | Height in px used for the before/into/after bands. Keep it close to the rendered height: keyboard drops resolve their intent from the measured row center against this value, so a large mismatch can bias a keyboard drop toward `After` or `Before` instead of `Into`. |
| `accepts` | `Option<Callback<(T, DropIntent), bool>>` | accept all | Receives payload and intent together; return `false` to refuse the combination (cycle prevention, leaves refusing `Into`). |
| `on_drop` | `EventHandler<TreeDropEvent<T>>` | required | Fired on an accepted drop with payload, target and intent. |
| `label` | `Option<String>` | `None` | Human name announced during keyboard navigation. Kept in sync if the prop changes. |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-intent` | an acceptable pointer drag hovers this row; valued `"before" \| "after" \| "into"` per the live band |

The attribute is absent when not hovered, so both value selectors
(Tailwind `data-[intent=into]:bg-blue-50`) and presence selectors
(`data-intent:outline`) work. The band derives from the shared pointer
position rather than DOM hover events, so mouse, touch and pen see the same
feedback; keyboard drags render no live band.

Behavior:

- Every target registers itself (id, parent, label, callbacks) in the
  shared zone registry, which is what makes it reachable by pointer
  hit-testing and keyboard navigation. It measures itself at mount,
  unregisters on unmount, and discovers a parent zone through `ParentZone`.
- At the registry level a target accepts a payload if `accepts` passes for
  *any* of the three intents, because hover cannot know the final band yet.
  The exact intent is re-checked at drop time; a release in a band whose
  exact combination is refused delivers nothing, so `on_drop` never sees a
  refused pair.
- Keyboard drops resolve from the measured row center against `row_height`,
  which lands in the `Into` band when the prop is honest.
- In a multi-window desktop drag world, hover and the pointer read through
  the joined window, so the live band tracks correctly in windows the drag
  did not start in.
- All props are re-synced when they change across renders, so rows rendered
  in loops stay current as the model reorders.

## `TreeDropEvent`

A completed tree drop, delivered to `on_drop`:

| Field | Type | Meaning |
|---|---|---|
| `payload` | `T` | The value that was dragged. |
| `target` | `NodeId` | The node whose row received the drop. |
| `intent` | `DropIntent` | Where, relative to `target`, the payload should land. |

Non-exhaustive, so drop context can be added without a major release:
destructure with `..`, and synthesize events for tests or programmatic
moves through `TreeDropEvent::new(payload, target, intent)`.

## `DropIntent`

Where, relative to the target node, the payload should land:

| Variant | Meaning |
|---|---|
| `Before` | Insert as the target's previous sibling. |
| `After` | Insert as the target's next sibling. |
| `Into` | Insert as the target's child. |

`Copy` and `Eq`, so intent rules are plain comparisons.

## `NodeId`

Identifies a tree node: a newtype over a public `u64` (`NodeId(pub u64)`)
with `From<u64>`. `Copy`, `Eq`, `Hash` and `Ord`, so it works as a map key
and sorts.

## `intent_from_offset`

```rust,ignore
pub fn intent_from_offset(y: f64, row_height: f64) -> DropIntent
```

Derives a `DropIntent` from the pointer's Y offset within a row of the
given height: the top 25% is `Before`, the bottom 25% is `After`, the
middle half is `Into`. The quarter boundaries themselves land `Into`.
Offsets outside the row clamp to the nearest end band, and heights are
floored at 1.0 so a degenerate row never divides by zero.

This is the exact math `TreeNodeTarget` runs, public for custom tree
interactions. If your rows cannot receive children (a flat outline), map
`Into` to whichever sibling intent you prefer.

## `would_create_cycle`

```rust,ignore
pub fn would_create_cycle(
    parent_of: impl Fn(NodeId) -> Option<NodeId>,
    dragged: NodeId,
    target: NodeId,
) -> bool
```

Would attaching `dragged` under `target` create a cycle? Walks `target`'s
ancestry through the `parent_of` lookup you provide, returning `true` when
`dragged` is `target` itself or any of its ancestors. The crate never holds
your tree, so `parent_of` is any closure over your own model.

The walk is bounded at 10,000 steps; a parent map that never terminates (a
cycle in your own data) reads as unsafe (`true`) rather than looping
forever.

## Where the rest lives

The `Draggable` that starts row drags, and `DropOutcome`:
[docs/api/drag-and-drop.md](drag-and-drop.md). Zone ids, the registry and
`ParentZone`: [docs/api/core.md](core.md). What keyboard drags announce:
[docs/api/accessibility.md](accessibility.md).

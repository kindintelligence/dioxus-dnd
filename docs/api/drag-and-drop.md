# Drag-and-drop API reference

Ready-made components over the shared drag context: `DndProvider`,
`Draggable`, `DropZone` and `DragOverlay`, plus `DragHandle`, `NoDrag`,
`SettleSlot` for seamless drop-settle handoffs, and `ParentZone`, the
context marker behind automatic nesting.

Concept guide: [docs/concepts/drag-and-drop.md](../concepts/drag-and-drop.md).
The two-world `BridgeDropZone` and the N-world `bridge_drop_zone!` macro
also live in this module; their reference is
[docs/api/mixing-payload-types.md](mixing-payload-types.md).

```rust,ignore
rsx! {
    DndProvider::<Card> {
        Draggable::<Card> { payload: card.clone(), "Drag me" }
        DropZone::<Card> {
            on_drop: move |outcome: DropOutcome<Card>| { /* ... */ },
            "Drop here"
        }
    }
}
```

All components are generic over the payload type `T: Clone + PartialEq +
'static`. Except for `DndProvider`, each renders a wrapper `div` and
forwards arbitrary attributes (`class`, `style`, `id`, ...) to it. Every
forwarded style fragment is merged: configurable defaults are user-overridable,
while behavior-critical declarations such as touch ownership and drag
transforms are emitted last. Attributes
that implement the component's contract—its event listeners, ARIA role,
focusability, mounted hook, and `data-*` state—remain component-owned and
cannot be replaced through the forwarded attribute list; use the component's
typed callbacks for supported user events.

## `DndProvider`

Provides a `DndContext<T>` (the drag world) to its children. Renders no DOM
of its own.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `dir` | `Direction` | `Ltr` | `Rtl` mirrors keyboard navigation and spatial zone ordering to follow a right-to-left layout. Live prop changes propagate reactively. |
| `release` | `ReleasePolicy<T>` | pointer-within, 48px recovery, not sticky | Collision detector, release recovery radius, and sticky-hover behavior for this provider. See [core.md](core.md#collision-and-release-policy). |

## `Draggable`

A focusable pointer and keyboard drag source. On drag start it pushes
`payload` into the shared context.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `payload` | `T` | required | The value delivered to whichever zone receives this drag. |
| `drag_id` | `Option<DragId>` | stable auto id | Stable source identity. Set it when a domain id should survive remounts; otherwise one is generated per mounted draggable. The ordinary HTML `id` remains available through forwarded attributes. |
| `zone` | `Option<ZoneId>` | `None` | The zone this item currently lives in, reported in `DropOutcome::from`. |
| `effect` | `DropEffect` | `Move` | Base drop effect; modifier keys can override it at release. |
| `disabled` | `bool` | `false` | Disable dragging without unmounting. Renders `data-disabled`. |
| `threshold` | `f64` | `8.0` | Movement in CSS px before a pointer press becomes a drag. |
| `activation` | `Option<ActivationPolicy>` | `None` | Composable surface, handle, delay, distance, either, or manual activation. When omitted, `threshold` preserves the 3.x behavior. |
| `touch` | `TouchSense` | `Auto` | How a finger shares the element with native scrolling. `Auto` keeps vertical swipes scrolling and picks up on a short hold or sideways pull; `Immediate` owns every touch from the first pixel. A mouse is identical under both; pens follow the finger rules. |
| `label` | `Option<String>` | `None` | Human name used in screen-reader announcements ("Picked up {label}"). |
| `on_drag_start` | `Option<EventHandler<()>>` | `None` | Fired when a drag begins. |
| `on_drag_end` | `Option<EventHandler<bool>>` | `None` | Fired when the drag ends; `true` if a zone consumed the payload, `false` if cancelled. |

Data attributes, present while true and absent otherwise:

| Attribute | Present while |
|---|---|
| `data-dragging` | this element's payload is in flight (also correct when a custom source started the drag) |
| `data-disabled` | `disabled` is set |

Keyboard, on the focused element: Space or Enter picks up and drops, Up and
Down cycle zones in spatial order, Right descends into nested zones and Left
ascends (mirrored under `Direction::Rtl`), Escape cancels. Keyboard drops
deliver the same `DropOutcome` with `mode: DragMode::Keyboard`.

### Activation policies and handles

`ActivationPolicy` combines an `Activator` (`Surface`, `Handle`, or
`Manual`) with an `ActivationConstraint`:

- `Distance(px)` promotes after pointer travel.
- `Delay { duration_ms, tolerance }` promotes after a hold while movement
  remains within the tolerance.
- `Either(vec![...])` promotes when any contained constraint succeeds.
- `Manual` leaves pickup to a custom `DndContext` source.

Use `DragHandle` for editable cards and other mixed-interaction surfaces.
It renders an accessible `button`, is keyboard-operable, and activates only
the nearest handle-configured draggable. Its functional style claims touch
on the handle while the rest of the card remains available for native
scrolling:

```rust,ignore
Draggable::<Card> {
    payload: card,
    activation: ActivationPolicy::handle(ActivationConstraint::Distance(6.0)),
    DragHandle { label: "Move card", "Move" }
    CardEditor {}
}
```

For a surface-configured draggable, wrap controls that must never start a
drag in `NoDrag`. It stops pointer and keyboard activation while preserving
the control's normal behavior and exposes `data-no-drag` for inspection.

## `DropZone`

A region that accepts drags carrying `T`. Registers itself (id, label,
callbacks, element handle) in the provider's zone registry, which powers
pointer hit-testing and keyboard navigation, and measures itself the moment
it mounts, so a zone appearing mid-drag (a virtualized row) is immediately
hit-testable.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `id` | `Option<ZoneId>` | auto | Stable identity. Auto-generated ids start at 2^32; explicit ids in `u32` range never collide with them. |
| `label` | `Option<String>` | `None` | Human name for announcements ("Over {label}"). Kept in sync if the prop changes. |
| `accepts` | `Option<Callback<T, bool>>` | accept all | Return `false` to reject a payload: the zone will not highlight, keyboard navigation skips it, and drops fall through it. Keep it cheap; registry queries snapshot candidates before invoking application callbacks. |
| `accepts_query` | `Option<Callback<DropQuery<T>, bool>>` | accept all | Rich acceptance using payload, source, proposed effect, input mode, pointer kind, and drag id. Evaluated after `accepts`. |
| `allowed_effects` | `DropEffects` | `DropEffects::ALL` | Effects this target supports. An unsupported proposal falls back deterministically to Move, Copy, then Link; an empty set rejects the drag. |
| `edge` | `Option<EdgeSet>` | `None` | Opt into the closest-edge signal: renders `data-edge` live while an acceptable pointer drag hovers, and fills `DropOutcome::edge` at release. |
| `on_drop` | `EventHandler<DropOutcome<T>>` | required | Fired on a successful drop. |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-active` | an acceptable drag is in flight anywhere (reveal your targets) |
| `data-over` | that drag hovers this zone (highlight it) |
| `data-edge` | hovered with `edge` set; valued `"top" \| "right" \| "bottom" \| "left"` |

All three follow pointer, touch and keyboard drags alike, because they read
the shared context rather than DOM events.

Nesting is automatic: a `DropZone` inside another discovers its parent
through `ParentZone` and provides itself to zones deeper down, which is
what hierarchical keyboard traversal walks.

Overlap precedence follows registry order, not browser paint order. Among
overlapping acceptable zones, the later record receives the drop; a rejecting
one is skipped at release. CSS `z-index`, stacking contexts and portals are
not inspected, so keep registry and visual order aligned when targets overlap,
or avoid the overlap. Replacing a same-id record retains its slot.

## Drag monitor

`use_dnd_monitor::<T>` subscribes to the nearest provider's complete
lifecycle without placing callbacks on every source or target:

```rust,ignore
use_dnd_monitor::<Card>(move |event| match event {
    DndEvent::Dropped(receipt) => save_undo(receipt),
    DndEvent::Cancelled { reason, .. } => log_cancel(reason),
    _ => {}
});
```

Events are `Started`, `Moved`, `TargetChanged`, `Dropped`, and `Cancelled`.
Each carries a `DragSnapshot<T>` with the stable `DragId`; dropped events
carry a parallel `DropReceipt<T>` containing both snapshot and the unchanged
3.x `DropOutcome<T>`. This adds metadata without extending the constructible,
exhaustive `DropOutcome` field set.

## `DragOverlay`

Renders its children pinned to the pointer while a drag is in flight: a
custom ghost that follows the cursor.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `settle` | `bool` | `false` | On a successful pointer drop, glide the ghost into the receiving zone instead of vanishing. Cancelled drags and keyboard drops never settle. |
| `duration` | `f64` | `200.0` | Settle transition duration in milliseconds. |
| `easing` | `String` | `"ease"` | CSS easing function for the settle glide. |
| `match_source` | `bool` | `false` | Size the ghost to the grabbed element's measured rect, so it appears exactly over what was picked up. Custom drag sources must call `set_source_rect` or the ghost stays hidden. |
| `on_settled` | `Option<EventHandler<()>>` | `None` | Fired when the drop-settle finishes, including the degenerate no-glide cases. Never fires for cancelled drags. |

During the glide the context is *settling*: `dragging()` is already false
(zones have unlit) but `payload()` stays readable, so the ghost keeps its
content. The glide honors `prefers-reduced-motion` by snapping near
instantly; cleanup still runs because `transitionend` still fires.

Keyboard drags carry no pointer position, so during one the ghost sits at
the viewport origin. Check `dnd.mode()` and skip rendering it if that
matters.

## `SettleSlot`

Wraps the element a drop just created so the drop-settle reads as one
object: while the ghost glides, the wrapper holds the element's space but
keeps it invisible (no second copy next to the ghost), re-aims the glide at
its own measured rect, and reveals the element the instant the ghost
unmounts.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `active` | `bool` | required | True on the just-landed element only, typically by remembering the dropped payload's id in `on_drop` and comparing. |

Inert while nothing is settling (keyboard drops, cancelled drags, overlays
without `settle`), so it is always safe to render:

```rust,ignore
on_drop: move |o: DropOutcome<Card>| { landed.set(Some(o.payload.id)); /* model */ },
// ...
SettleSlot::<Card> { active: landed() == Some(card.id),
    Draggable::<Card> { payload: card.clone(), CardFace { card } }
}
```

## `ParentZone`

The context marker a `DropZone` provides so zones nested inside it discover
their parent. Read it (`try_use_context::<ParentZone>()`) when building a
custom zone that should participate in hierarchical keyboard traversal;
provide it if custom zones can nest inside yours.

## `DropOutcome`

Everything a consumer learns from a completed drop, delivered to `on_drop`:

| Field | Type | Meaning |
|---|---|---|
| `payload` | `T` | The value that was dragged. |
| `from` | `Option<ZoneId>` | The zone the `Draggable` declared via its `zone` prop, if any. |
| `to` | `ZoneId` | The zone that received the drop. |
| `effect` | `DropEffect` | The resolved effect, modifier keys applied. See [docs/api/drop-effects.md](drop-effects.md). |
| `mode` | `DragMode` | `Pointer` or `Keyboard`. Non-exhaustive; match the variants you handle. |
| `client` | `Point` | Pointer position in viewport coordinates at drop time. |
| `element` | `Point` | Pointer position relative to the zone's element. |
| `grab` | `Point` | Where inside the dragged element the pointer grabbed it. `element - grab` is where the element's top-left should land. Zero for keyboard drops. |
| `edge` | `Option<Edge>` | The zone edge nearest the release point. `Some` only for pointer drops on zones that set `edge`. Treat `None` as your neutral intent. |

## Closest-edge primitives

`Edge` names one side of a zone (`Top`, `Right`, `Bottom`, `Left`), with
`as_str()` matching the `data-edge` attribute values. `EdgeSet` names which
edges compete, by stacking direction: `Vertical` (top/bottom), `Horizontal`
(left/right), `All`. `edge_of(point, rect, edges)` is the pure function
behind both: it clamps the point into the rect and returns the nearest
allowed edge, preferring `Top`, then `Bottom`, `Left`, `Right` on ties.
Call it directly for custom zones.

## Where the rest lives

Ids, geometry (`Point`, `Rect`), the context and registry:
[docs/api/core.md](core.md). `DropEffect` and the modifier convention:
[docs/api/drop-effects.md](drop-effects.md). `TouchSense` details:
[docs/concepts/touch-and-input.md](../concepts/touch-and-input.md).

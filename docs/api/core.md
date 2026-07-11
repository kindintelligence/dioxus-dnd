# Core API reference

Shared primitives every other module builds on: the drag context and its
state store, the provider and consumer hooks, the zone registry, the pointer
gesture state machine, and the id and geometry types.

Concept guide: [docs/concepts/architecture.md](../concepts/architecture.md).
The ready-made components re-exported from this module (`DndProvider`,
`Draggable`, `DropZone`, `DragOverlay`, `SettleSlot`, `ParentZone`) are
documented in [docs/api/drag-and-drop.md](drag-and-drop.md); the directory
at the end of this page maps every other submodule to its reference.

```rust,ignore
let dnd = use_dnd::<Card>();
let registry = use_zone_registry::<Card>();

// Each accessor subscribes to one field: this component reruns when the
// hovered zone changes, never per pointer move.
let over = dnd.over();
let target_rect = over.and_then(|id| registry.cached_rect(id));
```

## `DndContext<T>`

The handle to the shared drag state, provided by `use_dnd_provider` (or the
`DndProvider` component) and consumed with `use_dnd`. It is `Copy` and cheap
to pass around - just a store key. All state lives in a `Store<DragState<T>>`,
and every accessor reads through a per-field lens, so render-time reads
subscribe only to the field they touch.

Read accessors:

| Method | Returns | What it reads |
|---|---|---|
| `dragging()` | `bool` | A drag is in flight. False while a completed drop is still settling, even though `payload()` remains readable. |
| `payload()` | `Option<T>` | Clone of the payload in flight. |
| `over()` | `Option<ZoneId>` | Zone currently hovered. |
| `source()` | `Option<ZoneId>` | Zone the drag started from. |
| `pointer()` | `Point` | Last known pointer position, client coordinates. |
| `grab()` | `Point` | Where inside the dragged element the pointer grabbed it. |
| `source_rect()` | `Option<Rect>` | Dragged element's client rect measured at pickup; `None` until the async measurement lands or when a custom source never set it. |
| `effect()` | `DropEffect` | Effect the drag was started with. |
| `mode()` | `DragMode` | How the drag is driven, `Pointer` or `Keyboard`. |
| `pointer_kind()` | `PointerKind` | Device driving a pointer drag; meaningful only while `mode()` is `Pointer`, `Mouse` otherwise and by default. |
| `settling()` | `Option<Rect>` | Destination rect of a drop currently settling. |
| `announcement()` | `String` | Current screen-reader announcement text. |

Methods that drive a drag:

| Method | What it does |
|---|---|
| `start(payload, source, pointer, grab, effect, mode)` | Begins a drag, resetting every field. `pointer_kind` and `source_rect` get defaults; refine them right after with `set_pointer_kind` and `set_source_rect`, as `Draggable` does. |
| `update_pointer(point)` | Tracks the pointer (drives `DragOverlay`). Granular: only `pointer` subscribers rerun. An exact (0,0) is ignored as a bogus platform report. |
| `enter(zone)` | Marks `zone` hovered. Granular: only `over` subscribers rerun. |
| `leave(zone)` | Clears hover, but only if `zone` is still the hovered one, so enter/leave races between adjacent zones resolve correctly. |
| `take()` | Consumes the payload on a successful drop; returns `Some((payload, source))`. Resets all state; `dragging()` is false after. |
| `take_settling(to: Rect)` | Like `take`, but enters the settling phase: the stored payload stays readable, `over` clears, `dragging()` turns false, and `settle` records the destination rect so a settle-enabled `DragOverlay` can glide the ghost home. In a joined multi-window world, custom sources must claim the settle first; see [docs/api/multi-window.md](multi-window.md). |
| `retarget_settle(to: Rect)` | Re-aims an in-flight settle at a better rect (`SettleSlot` does this with the landed element's own). No-op unless settling, and equality-guarded so effects that subscribe to `settle` cannot loop. |
| `finish_settle()` | Ends the settling phase and resets all state. No-op unless settling, so a late `transitionend` can never clobber a new drag. |
| `cancel()` | Aborts the drag and resets all state. |

Helpers for custom sources and flows:

| Method | What it does |
|---|---|
| `set_pointer_kind(kind)` | Records which device drives the current drag. `Draggable` sets it from the initiating event's `pointerType`; call it from custom pointer sources so host-side glue can tell captured pointers from blind ones. Left alone, every drag reads as `Mouse`. |
| `set_source_rect(rect)` | Records the dragged element's client rect so size-matched ghosts (`DragOverlay { match_source: true }`) can dress themselves. |
| `request_refocus(payload)` | Marks `payload` as just landed via keyboard so its re-mounted element takes focus. `Draggable` calls it on its own keyboard drops. |
| `claim_refocus(&payload)` | Claims a pending focus restoration if it matches; returns whether the caller should focus itself. First matching claimant wins - the request is consumed. Requires `T: PartialEq`. |
| `announce(msg)` | Pushes a screen-reader announcement, rendered by `LiveRegion`. The built-in keyboard interaction calls it automatically; call it yourself for custom flows. |
| `from_parts(state, announcement)` | Wraps an existing store and announcement signal. Prefer `use_dnd_provider`. |

Pointer drags started by `Draggable` are additionally tracked as sessions
(see `DragSessionId` below) so the source's `on_drag_end` fires exactly
once, even when the drop handler unmounts the source mid-delivery. That
session machinery is internal; custom sources get the same guarantee by
going through `Draggable` or the drag world.

## `DragState<T>`

The snapshot of an in-flight drag, held in a `Store`. Deriving `Store`
generates per-field lenses, which is what makes `DndContext`'s accessors
granular. `Default` is the idle state (everything `None`, zero points).

| Field | Type | Meaning |
|---|---|---|
| `payload` | `Option<T>` | The payload being dragged, if any. |
| `source` | `Option<ZoneId>` | Zone the drag started from. |
| `over` | `Option<ZoneId>` | Zone the pointer is currently over. |
| `pointer` | `Point` | Last known pointer position (client coordinates). |
| `grab` | `Point` | Grab offset inside the dragged element. |
| `effect` | `DropEffect` | Effect requested by the draggable. |
| `mode` | `DragMode` | Pointer vs keyboard. |
| `pointer_kind` | `PointerKind` | Device driving a pointer drag; meaningful only while `mode` is `Pointer`. |
| `source_rect` | `Option<Rect>` | Dragged element's rect at pickup, for size-matched ghosts. |
| `refocus` | `Option<T>` | Payload of a just-completed keyboard drop awaiting focus restoration; the matching `Draggable` claims it on mount so keyboard users keep their place. |
| `settle` | `Option<Rect>` | Destination rect of a drop whose overlay is still gliding home. While set, `dragging()` is false but `payload` stays readable. |

## Hooks

| Hook | Returns | What it does |
|---|---|---|
| `use_dnd_provider::<T>()` | `DndContext<T>` | Creates the context, zone registry, and rect-refresh channel and provides all of them to the subtree. Call once, high up, or use `DndProvider`. When a `DndWorld<T>` is in context, the provider joins it instead of creating isolated state; see [docs/api/multi-window.md](multi-window.md). |
| `use_dnd::<T>()` | `DndContext<T>` | The nearest context. Panics if no ancestor provided one for this payload type. |
| `use_zone_registry::<T>()` | `ZoneRegistry<T>` | The provider's zone registry. Panics without an ancestor provider. |
| `use_zone_id()` | `ZoneId` | A stable, auto-generated zone id for this component instance. |
| `use_rect_refresh()` | `RectRefresh` | The provider tree's re-measure channel. Panics without an ancestor provider. |

Pure helpers for native `DragEvent`s (in-app drags never produce these;
they serve the boundary modules and custom native zones):

| Function | Returns | What it does |
|---|---|---|
| `client_point(&DragEvent)` | `Point` | Client (viewport) coordinates of a native drag event. |
| `element_point(&DragEvent)` | `Point` | Element-relative coordinates of a native drag event. |

`use_bridge_world`, `BridgeGeometry` and `BridgeWorld` also live in this
module; they belong to the cross-type bridge and are documented in
[docs/api/mixing-payload-types.md](mixing-payload-types.md).

## The zone registry

Every mounted `DropZone` records itself here; pointer hit-testing and
keyboard navigation are queries against it. The registry always mirrors
what is mounted, which is why virtualized lists work unmodified: see
[docs/concepts/virtualized-lists.md](../concepts/virtualized-lists.md).

### `ZoneRecord<T>`

One registered drop zone. Constructible as a plain struct literal, so
custom zones can register themselves.

| Field | Type | Meaning |
|---|---|---|
| `id` | `ZoneId` | The zone's identity. |
| `parent` | `Option<ZoneId>` | The enclosing zone when nested inside another `DropZone` (discovered via context). |
| `label` | `Option<String>` | Human label used in screen-reader announcements. |
| `on_drop` | `Callback<DropOutcome<T>>` | Delivers a completed drop to the zone's owner. |
| `accepts` | `Option<Callback<T, bool>>` | Acceptance filter, if any. |
| `mounted` | `Option<Rc<MountedData>>` | The zone's mounted element, once available; update through `ZoneRegistry::set_mounted`. |
| `rect` | `Option<Rect>` | Cached client rect; update through `ZoneRegistry::set_rect_if_present`. |

Methods: `accepts_payload(&payload)` runs the filter (true when there is
none), `cached_rect()` and `mounted_handle()` read this snapshot's values.

### `ZoneRegistry<T>`

A `Copy` handle over provider-owned storage of `ZoneRecord`s in mount
order.

Registration:

| Method | What it does |
|---|---|
| `register(record)` | Adds a zone, or replaces the existing record with the same id. Returns a `ZoneRegistration` token. |
| `unregister(id)` | Removes a zone; call when its component unmounts. |
| `sync_label(id, label)` | Updates a zone's label in place; no-op if unchanged or unknown. Safe to call every render. |
| `set_mounted(registration, mounted)` | Attaches the mounted element to this exact registration; a stale token is ignored. |
| `set_rect_if_present(registration, rect)` | Stores a rect only while the registration is still current. Never inserts a missing zone, so an async measurement cannot resurrect a zone that unmounted mid-flight. |
| `set_rect(id, rect)` | Synchronous, manual counterpart to `set_rect_if_present` for the current registration of `id`; used by custom layout adapters and the headless test driver. |
| `from_signal(zones)` | Wraps an existing signal. Prefer `use_dnd_provider`. |

Lookups. All of these peek (read without subscribing) except `records`:

| Method | Returns | What it does |
|---|---|---|
| `get(id)` | `Option<ZoneRecord<T>>` | Look up one zone. |
| `cached_rect(id)` | `Option<Rect>` | The cached client rect; `None` when unmeasured, unknown, or the provider is gone. |
| `mounted_handle(id)` | `Option<Rc<MountedData>>` | The mounted element; `None` before mount, for an unknown zone, or after teardown. |
| `contains(id)` | `bool` | Is this id registered here? The parent-zone context is shared across payload types, so a record's `parent` can name a zone living in another type's registry - check before navigating to one. |
| `acceptable(&payload)` | `Vec<ZoneRecord<T>>` | All zones accepting the payload, in registration order. |
| `records()` | `Vec<ZoneRecord<T>>` | Every zone, in registration order. A subscribing read - a component rendering from it reruns when zones mount or unmount - because its consumers (the debug overlay, your devtools) are renderers. |

Keyboard navigation. Order is spatial - top-to-bottom, then reading order,
which is left-to-right under `Direction::Ltr` and right-to-left under
`Rtl`; zones without a measured rect come last in registration order:

| Method | What it does |
|---|---|
| `direction()` / `set_direction(dir)` | The layout direction spatial ordering follows. Setting is a no-op if unchanged; `DndProvider`'s `dir` prop calls it for you. |
| `step_zone(current, &payload, step)` | Next (`+1`) or previous (`-1`) acceptable zone, cyclic, in spatial order. Call `refresh_rects` first, as the built-in keyboard interaction does on pickup. |
| `parent_of(id)` | The parent of a nested zone. |
| `ascend(current)` | The zone to enter when ascending: the parent, but only when this registry can resolve it. |
| `children_of(parent, &payload)` | Acceptable zones directly inside `parent` (`None` is the root level), spatially ordered. |
| `first_child(id, &payload)` | The first (spatially) acceptable zone nested inside `id`. |
| `step_sibling(current, &payload, step)` | Next/previous zone among `current`'s siblings, cyclic. With no `current`, cycles the root level. |

Hit-testing, against cached rects:

| Method | What it does |
|---|---|
| `hit_test(point)` | Topmost zone containing the point; later-mounted zones win, approximating DOM paint order. |
| `hit_test_closest(point, &payload, max_distance)` | Acceptance-aware: the topmost zone that contains the point and accepts the payload, so a drop can land on an accepting zone under a rejecting or decorative one. When nothing contains the point, falls back to the acceptable zone whose rect edge is nearest, within `max_distance` CSS px - the built-in drop passes 48.0 - which forgives releases in the gutter between zones. |

Measurement:

| Method | What it does |
|---|---|
| `refresh_rects()` | Re-measures every mounted zone's client rect. Async, fire-and-forget. |
| `measure_all()` | Like `refresh_rects` but `async` and awaits the measurements, for a hit-test that must see fresh geometry (retrying a missed touch drop after a layout change). |

### `ZoneRegistration`

The token `register` returns, identifying one particular registration of a
`ZoneId`. A zone id can be replaced in place; async measurements carry this
token so a result started for the old registration cannot land in its
same-id replacement. `set_mounted` and `set_rect_if_present` quietly drop
writes carrying a stale token.

### `RectRefresh`

A payload-type-erased "re-measure your zones" channel shared by every
registry under one provider tree. Cached rects go stale the moment layout
moves under a live drag; things that move layout should not need to know
any payload type to say so. Each provider registers a thunk that re-measures
its own registry only while it has a drag in flight, so pinging from every
scroll event is free while idle. `AutoScroll` pings it automatically; grab
the channel with `use_rect_refresh` to wire up custom layout mutators.

| Method | What it does |
|---|---|
| `refresh_all()` | Asks every provider in the tree to re-measure its zones. Safe to call from high-frequency sources like scroll events. |
| `len()` / `is_empty()` | Number of registered providers; diagnostics and tests. |
| `from_signal(thunks)` | Wraps an existing signal. Prefer `use_dnd_provider`, which creates one per provider tree - nested providers inherit and re-provide the outermost channel. |

## The gesture machine

The pointer-drag lifecycle - press, threshold promotion, tracking, release
or abort - as a pure transition function over explicit states and events.
Every edge (stray pointer ids, release before the threshold, cancellation
mid-drag) is an exhaustive match arm with a test. `Draggable` drives it;
drive it yourself for custom pointer interactions:

```rust,ignore
use dioxus_dnd::core::{transition, GestureEffect, GesturePhase, GestureEvent, Point};

let mut phase = GesturePhase::Idle;
let (next, fx) = transition(phase, GestureEvent::Down { at: Point::new(10.0, 10.0), pointer_id: 1 }, 8.0);
phase = next;
assert_eq!(fx, GestureEffect::None); // pressed, not yet a drag

let (next, fx) = transition(phase, GestureEvent::Move { at: Point::new(30.0, 10.0), pointer_id: 1 }, 8.0);
assert!(matches!(fx, GestureEffect::Begin { .. })); // crossed the threshold
```

| Function | What it does |
|---|---|
| `transition(phase, event, threshold)` | Advances the machine under the default `Promotion::Distance` policy. Pure. `threshold` is the travel distance in CSS px that promotes a press to a drag; releases inside it resolve as `Tap`. |
| `transition_with(phase, event, threshold, promotion)` | The same, under an explicit `Promotion` policy. The policy only shapes how a press becomes a drag; everything after `Begin` is policy-independent. |

`transition` is in the prelude; import `transition_with` and `Promotion`
from `dioxus_dnd::core`.

`GesturePhase` - where the gesture stands:

| Variant | Meaning |
|---|---|
| `Idle` | No interaction in progress. |
| `Pressed { origin, pointer_id }` | Pointer is down but has not traveled past the threshold - could still resolve as a tap. |
| `Dragging { origin, pointer_id }` | An active drag. |

`GestureEvent` - inputs. Non-exhaustive, because the gesture vocabulary
provably grows (`Hold` arrived in 2.5): feed events in freely, but never
match this enum exhaustively - treat unknown inputs as inert, like the
machine does.

| Variant | Meaning |
|---|---|
| `Down { at, pointer_id }` | Pointer pressed. A second pointer pressing mid-gesture does not steal it. |
| `Move { at, pointer_id }` | Pointer moved. Events from foreign pointer ids are ignored. |
| `Up { at, pointer_id }` | Pointer released. |
| `Hold { pointer_id }` | The press's hold timer elapsed with the pointer still: a long-press. Promotes a matching `Pressed` straight to a drag at the press origin, regardless of policy; inert in every other phase, so a stale timer is harmless. |
| `Cancel` | The platform cancelled the gesture (`pointercancel`). Aborts a drag; silent while merely pressed or idle. |

`Promotion` - how a press becomes a drag, the policy half of the touch
auto-sensor:

| Variant | Meaning |
|---|---|
| `Distance` (default) | Travel in any direction past the threshold begins the drag. Right for mouse and pen, and for touch surfaces that own every gesture (`touch-action: none`). |
| `HoldOrSideways` | For touch sharing the viewport with native vertical scrolling (`touch-action: pan-y`): a `Hold` or a sideways-dominant pull past the threshold begins the drag, while a vertical-dominant pull (an exact diagonal included) resolves the press as scroll intent - the machine returns to `Idle` and the browser's pan takes the gesture. |

`GestureEffect` - what the caller should do after a transition:

| Variant | Meaning |
|---|---|
| `None` | Nothing - including events from foreign pointer ids, which the machine deliberately ignores. |
| `Begin { origin, at }` | The threshold was crossed: begin the drag. `origin` is where the press started (use it for the grab offset), `at` the current position. |
| `Track { at }` | An active drag moved: track the pointer and update hover. |
| `Drop { at }` | An active drag released: attempt the drop at `at`. |
| `Tap` | The press resolved as a tap (released before the threshold). |
| `Abort` | An active drag was aborted: clean up drag state. |

## Shared types

| Type | What it is |
|---|---|
| `ZoneId(pub u64)` | Identifies a drop zone. `ZoneId::auto()` generates a process-unique id; call it inside `use_hook` (or use `use_zone_id`) so it sticks across renders. Auto ids start at 2^32; explicit ids in `u32` range can never collide with them. The registry replaces records by id, so a collision would silently knock a zone out - the reservation makes it impossible. `From<u64>` is implemented. |
| `DragId(pub u64)` | Identifies a draggable item. `DragId::auto()` draws from the same 2^32-and-up sequence. `From<u64>` is implemented. |
| `DragSessionId(pub u64)` | Identifies one pointer-drag gesture from pickup through its exactly-once completion. Unlike `DragId`, which applications may use as item identity, this id is generated afresh for every gesture; the crate creates and consumes them internally. |
| `Point` | A 2D point in CSS pixels, `{ x, y }`. Implements `Add` and `Sub`; `Point::new(x, y)`. |
| `Rect` | An axis-aligned rectangle in client (viewport) coordinates, `{ x, y, width, height }`. `contains(point)` is edge-inclusive; `center()` and `origin()` return the middle and top-left. |
| `Direction` | Horizontal layout direction, `Ltr` (default) or `Rtl`. Under `Rtl`, keyboard navigation mirrors (ArrowRight ascends, the WAI-ARIA tree convention) and spatial ordering runs right-to-left within a row. Set via `DndProvider`'s `dir` prop or `ZoneRegistry::set_direction`. |
| `DragMode` | How the current drag is driven: `Pointer` (default) or `Keyboard`. Non-exhaustive: input paths accrete (gamepad and switch-access drags are plausible futures), so compare against the variants you handle rather than matching exhaustively. |
| `PointerKind` | Which device drives a pointer drag: `Mouse` (default), `Touch`, `Pen`. Recorded at pickup so host-side glue can decide which input layers need bridging: a touch contact is implicitly captured by the browser, while mouse and pen go blind at the viewport edge when native capture is unavailable. Non-exhaustive: pointer taxonomies grow with input hardware, so glue must decide through `implicitly_captured()`, which encodes the safe default (bridge) for kinds it has never heard of. `from_pointer_type(str)` maps a DOM `pointerType` string, falling back to `Mouse` for anything unrecognized - the safe default, since an unbridged blind pointer loses drops while a double-driven captured one merely jitters. Only `Touch` reports `implicitly_captured() == true`. |
| `TouchSense` | How a draggable shares touch with native gestures. `Auto` (default): the element carries `touch-action: pan-y`, vertical swipes keep scrolling, and a short hold (250ms, finger still) or a sideways-dominant pull picks the item up. `Immediate`: the element owns every touch from the first pixel (`touch-action: none`). A mouse is unaffected by either; pens follow the finger rules. |

Also defined in `core::types` but documented with their consumers: `Edge`,
`EdgeSet`, `edge_of` and `DropOutcome` in
[docs/api/drag-and-drop.md](drag-and-drop.md); `DropEffect` and
`effective_effect` in [docs/api/drop-effects.md](drop-effects.md).

## Where the rest lives

| Submodule | Documented in |
|---|---|
| `core::state` (`DndContext`, `DragState`) | this file |
| `core::hooks` | this file; the bridge pieces (`use_bridge_world`, `BridgeGeometry`, `BridgeWorld`) in [mixing-payload-types.md](mixing-payload-types.md) |
| `core::registry` | this file |
| `core::machine` | this file |
| `core::types` | this file; edge primitives and `DropOutcome` in [drag-and-drop.md](drag-and-drop.md), `DropEffect` in [drop-effects.md](drop-effects.md) |
| `core::components` | [drag-and-drop.md](drag-and-drop.md); `BridgeDropZone` in [mixing-payload-types.md](mixing-payload-types.md) |
| `core::model` | [drop-effects.md](drop-effects.md) |
| `core::modifiers` | [canvas.md](canvas.md) |
| `core::viewport` | [canvas.md](canvas.md) |
| `core::strings` | [localization.md](localization.md) |
| `core::world` | [multi-window.md](multi-window.md) |

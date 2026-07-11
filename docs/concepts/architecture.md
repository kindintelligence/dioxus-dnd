# Architecture

`DndProvider` looks like one component, but it creates three cooperating
layers: a reactive state store, a zone registry, and a pure gesture state
machine. `Draggable` and `DropZone` are ordinary consumers of those layers,
and every layer is public, so anything they do, your own components can do.

API reference: [api/core.md](../api/core.md).
Live demo: every gallery page runs on these layers; the
[Archive](https://kindintelligence.github.io/dioxus-dnd/archive) page renders
the registry's live census over ten thousand virtualized rows.

## The mental model

One provider, three layers:

- A `Store<DragState<T>>` in Dioxus context holds the drag in flight: the
  payload, the source and hovered zones, the pointer, the grab offset, the
  effect and the input mode. `DndContext<T>` is the cheap `Copy` handle over
  it.
- A `ZoneRegistry<T>` alongside it records every mounted drop zone: id,
  label, drop callback, acceptance filter, DOM handle, cached client rect.
  Pointer hit-testing and keyboard navigation are queries against this
  registry, not DOM walks.
- A gesture state machine (`core::machine`) decides what a pointer press
  means: tap, scroll, or drag. It is a pure transition function, so every
  edge case is an exhaustive match arm with a test.

Native drag events appear nowhere in this picture. They are reserved for
the true app boundary - OS file drops, external content in, dragging out -
where the `DataTransfer` protocol is the only option. In-app drags use
pointer events plus keyboard, which keeps the payload a Rust value and the
visuals under your control.

## The state store

The payload travels through the store as a plain `Clone` value: no JSON, no
string ids, no serialization. Stores are Dioxus 0.7's fine-grained
reactivity primitive - each field of `DragState` gets its own lazy
subscription, and every `DndContext` accessor reads through a per-field
lens. A component that calls `dnd.over()` in its render reruns only when the
hovered zone changes, not on every pointer move:

```rust,ignore
let dnd = use_dnd::<Card>();
let lit = dnd.over() == Some(MY_ZONE);   // reruns on hover change only
let ghost_at = dnd.pointer();            // reruns per move - overlays only
```

Writes are granular too: `update_pointer` notifies only `pointer`
subscribers, `enter` and `leave` only `over` subscribers. This is why a
board with hundreds of zones stays smooth mid-drag - per-move work is
confined to the components that actually track the pointer.

## The zone registry

Every mounted `DropZone` registers itself and unregisters on unmount, so
the registry always mirrors what is on screen - a virtualized list with ten
thousand rows keeps only the mounted few dozen registered. Two query
families power the built-in interactions:

- **Pointer hit-testing.** `hit_test_closest` finds the topmost zone that
  contains the point and accepts the payload, using cached rects; when the
  release lands in a gutter, it falls back to the acceptable zone whose rect
  edge is nearest (the built-in drop passes 48px).
- **Keyboard navigation.** `step_zone` and its sibling/child variants walk
  acceptable zones in spatial order (top-to-bottom, then reading order,
  mirrored under `Direction::Rtl`), which is what the arrow keys traverse.
  Zone labels feed the screen-reader announcements.

Rects are cached, measured fresh at pickup. When layout moves under a live
drag - scrolling, a collapsing panel - the `RectRefresh` channel
(`use_rect_refresh`) pings every provider in the tree to re-measure. Idle
providers ignore the ping, so wiring it to raw scroll events costs nothing;
`AutoScroll` pings it for you.

## The gesture machine

A press is not yet a drag. The lifecycle - press, promotion, tracking,
release or abort - is a formal state machine over three phases (`Idle`,
`Pressed`, `Dragging`) with two promotion policies:

- `Promotion::Distance`: travel past the threshold in any direction begins
  the drag. Right for mouse, pen, and touch surfaces that own every gesture.
- `Promotion::HoldOrSideways`: a 250ms hold or a sideways-dominant pull
  begins the drag, while a vertical-dominant pull yields the gesture to
  native scrolling. This is `TouchSense::Auto`, the reason draggables in a
  scrollable list do not fight the finger.

`transition` and `transition_with` are pure functions: same inputs, same
outputs, no side effects. Stray inputs - foreign pointer ids, a second
finger pressing, a hold timer firing after the gesture resolved - are
deliberately inert, each one a tested match arm rather than an ad-hoc `if`.
`Draggable` drives this machine; you can drive it yourself.

## Build your own

Because the layers are public, a custom drop target is just a component
that registers itself and reads the context. This is a working bare zone:

```rust,ignore
#[component]
fn TrashZone(on_trash: EventHandler<DropOutcome<Card>>) -> Element {
    let dnd = use_dnd::<Card>();
    let mut registry = use_zone_registry::<Card>();
    let id = use_zone_id();

    let registration = use_hook(|| registry.register(ZoneRecord {
        id,
        parent: None,
        label: Some("Trash".into()),
        on_drop: Callback::new(move |o| on_trash.call(o)),
        accepts: None,
        mounted: None,
        rect: None,
    }));
    use_drop(move || registry.unregister(id));

    let armed = dnd.dragging();
    let over = dnd.over() == Some(id);

    rsx! {
        div {
            onmounted: move |evt| {
                let m = evt.data();
                registry.set_mounted(registration, m.clone());
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        registry.set_rect_if_present(registration,
                            Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height));
                    }
                });
            },
            class: if over { "trash hot" } else if armed { "trash armed" } else { "trash" },
            "Trash"
        }
    }
}
```

Registration is what buys the behavior: pointer drops land here, keyboard
navigation reaches it and announces "Trash", near-miss releases snap to it.
The [Standup](https://kindintelligence.github.io/dioxus-dnd/standup) gallery
page pushes this recipe further, registering one element in two payload
worlds at once.

## Gotchas

- **`records()` subscribes, the id lookups peek.** Rendering from
  `registry.records()` reruns on every mount and unmount - right for
  devtools, wrong inside a hot zone. `get`, `cached_rect` and
  `mounted_handle` never subscribe.
- **`dragging()` is false while a drop settles.** During the overlay's
  settle glide, `payload()` stays readable so the ghost keeps its content,
  but zones have already unlit. Check `settling()` when the distinction
  matters.
- **Do not match the growing enums exhaustively.** `DragMode`,
  `PointerKind` and `GestureEvent` are `non_exhaustive`; new input paths
  arrive as new variants. Match what you handle and let the rest fall
  through.
- **Explicit `ZoneId`s belong below 2^32.** The registry replaces records
  by id, so a collision silently knocks a zone out. Auto ids start at 2^32
  precisely so hand-written `u32`-range ids can never collide.
- **An exact (0,0) pointer is dropped.** `update_pointer` treats it as a
  bogus platform report, so a custom source feeding synthetic moves never
  sees the overlay jump to the corner.

## Related

- [Drag and drop](drag-and-drop.md): the components built on these layers.
- [Virtualized lists](virtualized-lists.md): the registry's
  mount-and-measure model doing its best work.
- [Multi-window desktop drags](multi-window.md): several windows joining
  one shared world built from the same context.
- [Testing](testing.md): `DragSim` drives the production delivery path
  through these same layers, no browser.
- [Debugging](debugging.md): the overlay renders the registry live.

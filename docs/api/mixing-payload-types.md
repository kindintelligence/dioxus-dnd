# Mixing payload types API reference

Drop targets shared between coexisting payload worlds: the two-world
`BridgeDropZone<A, B>` component, the N-world `bridge_drop_zone!` macro, and
the `use_bridge_world` hook with its `BridgeGeometry` and `BridgeWorld`
helpers.

Concept guide:
[docs/concepts/mixing-payload-types.md](../concepts/mixing-payload-types.md).
All items are exported from `dioxus_dnd::prelude`; the macro also lives at
the crate root as `dioxus_dnd::bridge_drop_zone!`.

```rust,ignore
BridgeDropZone::<Ticket, Person> {
    label: "Standup agenda",
    on_drop_a: move |o: DropOutcome<Ticket>| discuss(o.payload),
    on_drop_b: move |o: DropOutcome<Person>| update_from(o.payload),
    "Drop a ticket or a teammate"
}
```

## `BridgeDropZone`

A drop target registered in two payload worlds at once, bridging two
coexisting providers (`DndProvider<A>` and `DndProvider<B>`). Zone ids are
process-global while registries are per-type, so one element holds the same
`ZoneId` in both registries; each world's hit-testing, acceptance filtering
and keyboard navigation then finds the zone independently, and each drop
arrives through its own typed callback. Renders a wrapper `div` and forwards
arbitrary attributes to it.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `id` | `Option<ZoneId>` | auto | Stable identity, valid in both worlds. Auto-generated if omitted. |
| `label` | `Option<String>` | `None` | Human name for screen-reader announcements, used by both worlds. Kept in sync if the prop changes. |
| `accepts_a` | `Option<Callback<A, bool>>` | accept all | Return `false` to reject a payload from the first world. |
| `accepts_b` | `Option<Callback<B, bool>>` | accept all | Return `false` to reject a payload from the second world. |
| `on_drop_a` | `EventHandler<DropOutcome<A>>` | required | Fired when a drag from the first world drops here. |
| `on_drop_b` | `EventHandler<DropOutcome<B>>` | required | Fired when a drag from the second world drops here. |

Data attributes, present while true and absent otherwise:

| Attribute | Present while |
|---|---|
| `data-active` | an acceptable drag from either world is in flight anywhere |
| `data-over` | that drag hovers this zone |

Behavior notes:

- Requires an ancestor `DndProvider<A>` and `DndProvider<B>`; panics
  without both.
- Measures at mount, so a bridge appearing mid-drag is immediately
  hit-testable in both worlds. One DOM read fans into both provider-owned
  registries.
- Provides `ParentZone` once with the shared id, which resolves in both
  registries, so a `DropZone` of either type nested inside the bridge
  ascends correctly.
- Keyboard drags from both worlds reach it: each world's spatial navigation
  lists the shared rectangle among its own zones.
- No `edge` prop: delivered outcomes carry `edge: None` and the div never
  renders `data-edge`.
- No downcasts, no shared erased channel: an `A` drag can only reach
  `on_drop_a`, a `B` drag only `on_drop_b`.

## `bridge_drop_zone!`

Generates a bridge drop-zone component for any number of coexisting payload
worlds: `BridgeDropZone`'s recipe, packaged for N > 2 without `dyn Any`.
Rust has no variadic generics, so the component is generated per concrete
type list rather than parameterized over one, which is also why
`BridgeDropZone` stops at two.

```rust,ignore
dioxus_dnd::bridge_drop_zone!(
    /// Doc comments and other attributes carry through.
    pub StandupZone {
        (Ticket, accepts_ticket, on_drop_ticket),
        (Person, accepts_person, on_drop_person),
        (Alert, accepts_alert, on_drop_alert),
    }
);
```

The row syntax is `(Type, accepts_prop, on_drop_prop)`, one row per world,
trailing comma allowed. Attributes and a visibility go before the component
name, as on a normal item. Each row contributes two props to the generated
component, named by the identifiers you wrote:

| Prop | Type | Default | What it does |
|---|---|---|---|
| `id` | `Option<ZoneId>` | auto | Stable identity, valid in every world. |
| `label` | `Option<String>` | `None` | Announcement label, used by every world. |
| *accepts_prop* (per row) | `Option<Callback<Type, bool>>` | accept all | Per-world acceptance filter. |
| *on_drop_prop* (per row) | `EventHandler<DropOutcome<Type>>` | required | That world's typed drop callback. |

The generated component renders a wrapper `div`, forwards extra attributes
to it, provides `ParentZone`, measures at mount with one DOM read fanned
into every registry, and carries the same `data-active` / `data-over`
attributes, lit by whichever world's drag qualifies. Like the two-world
component it has no `edge` prop.

Requirements: `use dioxus::prelude::*;` in scope at the invocation site, and
an ancestor `DndProvider` for every listed type.

## How the attributes light across worlds

Each world computes its own pair: *active* means an acceptable drag of that
world's payload is in flight, *over* means that drag currently hovers this
zone, and both already include the world's `accepts` verdict. The bridge ORs
the pairs, so `data-active` appears when any world qualifies and `data-over`
when any world's drag hovers. Only one drag exists per world at a time, and
the attributes read the shared contexts rather than DOM events, so they
follow pointer, touch and keyboard drags alike. Presence-based selectors
(CSS `[data-over]`, Tailwind `data-over:ring-2`) work exactly as on
`DropZone`; there is no per-world variant of the attributes, so style
per-world states with `use_bridge_world` directly if you need them.

## `use_bridge_world`

The building block behind both bridges: register `zone_id` as a drop target
in `T`'s payload world and report that world's live state this render. Call
it once per coexisting provider type with the same id and `BridgeGeometry`.

```rust,ignore
pub fn use_bridge_world<T: Clone + PartialEq + 'static>(
    zone_id: ZoneId,
    parent: Option<ZoneId>,
    label: Option<String>,
    accepts: Option<Callback<T, bool>>,
    on_drop: EventHandler<DropOutcome<T>>,
    geometry: BridgeGeometry,
) -> BridgeWorld
```

| Argument | What it does |
|---|---|
| `zone_id` | The shared zone identity. Pass the same value to every world's call; `use_zone_id()` gives a stable auto id. |
| `parent` | The enclosing zone, if any, for hierarchical keyboard traversal. Typically `try_use_context::<ParentZone>().map(\|p\| p.0)`. |
| `label` | Announcement label, registered in `T`'s world and kept in sync across renders. |
| `accepts` | Per-world acceptance filter; `None` accepts all. |
| `on_drop` | This world's typed drop callback. The outcome passes through unenriched, so `edge` stays `None`. |
| `geometry` | The shared fan-out. The hook joins it once, so later `set_mounted` and `set_rect_if_present` calls reach this world's registration. |

It is a hook: it registers a `ZoneRecord` in `T`'s registry once per mount,
unregisters on unmount, and panics if no ancestor provided a
`DndProvider<T>`. The registration is provider-owned plain data, no signals
or callbacks created in the child scope. Hover tracking follows joined
multi-window drags the same way `DropZone`'s does.

A custom bridge is the hook plus the fan-out. This skeleton is
`BridgeDropZone`'s body, minus the `id` prop and attribute forwarding:

```rust,ignore
let zone_id = use_zone_id();
let parent = try_use_context::<ParentZone>().map(|p| p.0);
use_context_provider(|| ParentZone(zone_id));
let geometry = use_hook(BridgeGeometry::default);
let a = use_bridge_world::<Ticket>(zone_id, parent, label.clone(),
    accepts_a, on_drop_a, geometry.clone());
let b = use_bridge_world::<Person>(zone_id, parent, label,
    accepts_b, on_drop_b, geometry.clone());

rsx! {
    div {
        "data-active": if a.active || b.active { "true" },
        "data-over": if a.over || b.over { "true" },
        onmounted: move |evt| {
            let m = evt.data();
            geometry.set_mounted(&m);
            let geometry = geometry.clone();
            spawn(async move {
                if let Ok(r) = m.get_client_rect().await {
                    geometry.set_rect_if_present(Rect::new(
                        r.origin.x, r.origin.y, r.size.width, r.size.height,
                    ));
                }
            });
        },
        {children}
    }
}
```

## `BridgeGeometry`

A plain, component-owned fan-out for one bridge element's geometry. Create
one with `Default` inside `use_hook` and pass a clone to every
`use_bridge_world` call for the element; its methods copy one DOM
observation into every joined provider-owned registry.

| Method | What it does |
|---|---|
| `set_mounted(&self, mounted: &Rc<MountedData>)` | Copy the bridge element's mounted handle into every joined registry. |
| `set_rect_if_present(&self, rect: Rect)` | Copy a completed measurement into every registration that is still current. |

Clones share the same fan-out (the handle is reference-counted), which is
what lets the `onmounted` closure and every world's registration see one
list of writers. `PartialEq` is identity: two `BridgeGeometry`s are equal
only if they are clones of the same fan-out. `Debug` reports how many
worlds have joined.

## `BridgeWorld`

The live, type-erased view of one payload world at a bridge zone, returned
by `use_bridge_world` each render, so callers can OR any number of worlds
together without naming their `T`s again. `Copy`, so combining is free.

| Field | Type | Meaning |
|---|---|---|
| `active` | `bool` | An acceptable drag of this world's payload is in flight. |
| `over` | `bool` | That drag currently hovers this zone. Acceptance included: an unacceptable payload never reads as over. |

## Where the rest lives

`DropZone`, `DropOutcome` and `ParentZone`:
[docs/api/drag-and-drop.md](drag-and-drop.md). `ZoneId`, `ZoneRecord`,
`use_zone_id`, `use_zone_registry` and the registry a bridge registers
into: [docs/api/core.md](core.md). This file is the standalone reference
for the bridge surface; its items span the components and hooks modules, so
it is not included as any single module's rustdoc.

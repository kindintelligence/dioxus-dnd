# Mixing payload types

A provider is monomorphic on purpose: a `Ticket` drag can only land on a
`DropZone<Ticket>`, and the compiler checks it. When a design seems to need
"polymorphic" drops, it is really one of two different problems, and they
have different answers. This guide teaches the decision first, then each
tool.

API reference: [api/mixing-payload-types.md](../api/mixing-payload-types.md).
Live demo: the [Standup](https://kindintelligence.github.io/dioxus-dnd/standup)
page bridges tickets and teammates into one agenda tray.

## The mental model

Ask one question: how many drag worlds actually exist?

- **One world whose payload comes in several shapes** (files and folders,
  cards and separators): make the payload an enum. One provider, plain
  `DropZone`, compile-time guarantee intact.
- **Two genuinely independent worlds sharing one target** (tickets and
  teammates as separate features, and only the agenda hears both):
  `BridgeDropZone<A, B>`.
- **Three or more worlds**: generate a component for your exact type list
  with `bridge_drop_zone!`.
- **Something none of these fit**: `use_bridge_world` is the public floor
  the bridges are built on.

The enum is the default. Reach for a bridge only when the types belong to
separate features with separate providers. A bridge used to erase types
inside one interaction is more machinery for less clarity, and it splits
what should be one registry's work across two.

## One world, several shapes: the enum

If everything belongs to one interaction, the payload is one type with
variants. The zone's `accepts` filters variants, the handler matches on
them, and the machinery never knows the difference:

```rust,ignore
#[derive(Clone, PartialEq)]
enum Node { File(u64), Folder(u64) }

DropZone::<Node> {
    accepts: move |n: Node| matches!(n, Node::Folder(_)),
    on_drop: move |o: DropOutcome<Node>| match o.payload {
        Node::File(id) => open(id),
        Node::Folder(id) => reveal(id),
    },
    "Folders only"
}
```

Everything from [Drag and drop](drag-and-drop.md) applies unchanged:
rejecting zones never highlight, keyboard navigation skips them, and a
`Draggable<Node>` still cannot land on some other provider's `DropZone<Card>`.

## Two worlds, one target: `BridgeDropZone`

Sometimes two providers genuinely coexist. In the Standup demo, tickets
drag in `DndProvider<Ticket>` and teammates in `DndProvider<Person>`; the
two worlds cannot see each other, by design, and only the agenda tray
should accept both. That is `BridgeDropZone<A, B>`:

```rust,ignore
BridgeDropZone::<Ticket, Person> {
    label: "Standup agenda",
    accepts_a: move |t: Ticket| !t.done,
    on_drop_a: move |o: DropOutcome<Ticket>| discuss(o.payload),
    on_drop_b: move |o: DropOutcome<Person>| update_from(o.payload),
    "Drop a ticket or a teammate"
}
```

The trick underneath: zone ids are process-global while registries are
per-type, so one element can hold the *same* `ZoneId` in both worlds'
registries. Each provider owns a plain geometry copy; the element fans one
mount and one measurement into both. From there each world's machinery -
hit-testing, `accepts` filtering, keyboard navigation - finds the zone
entirely on its own. Acceptance is per-world (`accepts_a` / `accepts_b`),
and every drop arrives through its own typed callback: an `A` drag can only
reach `on_drop_a`, a `B` drag only `on_drop_b`. There is no downcast and no
shared erased channel; dispatch happened at the type level, before the app
ever ran.

Styling matches `DropZone`: `data-active` while an acceptable drag from
either world is in flight, `data-over` while one hovers the bridge.

## Three or more worlds: `bridge_drop_zone!`

Rust has no variadic generics, so no component can be parameterized over
"any number of payload types". The `bridge_drop_zone!` macro generates the
component for your exact type list instead, which is also why
`BridgeDropZone` stops at two. Each `(Type, accepts_prop, on_drop_prop)`
row becomes one world:

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

dioxus_dnd::bridge_drop_zone!(pub StandupZone {
    (Ticket, accepts_ticket, on_drop_ticket),
    (Person, accepts_person, on_drop_person),
    (Alert, accepts_alert, on_drop_alert),
});

rsx! {
    StandupZone {
        label: "agenda",
        accepts_ticket: move |t: Ticket| !t.done,
        on_drop_ticket: move |o: DropOutcome<Ticket>| { /* ... */ },
        on_drop_person: move |o: DropOutcome<Person>| { /* ... */ },
        on_drop_alert: move |o: DropOutcome<Alert>| { /* ... */ },
        "standup agenda"
    }
}
```

The generated component takes the shared `id` and `label` props, forwards
extra attributes to its div, and carries the same `data-active` /
`data-over` hooks, lit by whichever world's drag qualifies. Before reaching
for three worlds, ask the enum question again.

## The floor: `use_bridge_world`

Both bridges are built from one public hook. `use_bridge_world::<T>`
registers a zone id in `T`'s world and returns that world's live state;
`BridgeGeometry` is the fan-out that copies one mount and one measurement
into every joined registry. Call the hook once per world with the same id
and geometry, OR the returned states for styling, and you have a custom
bridge - a bridge that is also a `SettleSlot`, a bridge with per-world
styling, whatever the component API does not cover. The
[API reference](../api/mixing-payload-types.md) shows the full skeleton.

## Gotchas

- **A bridge is not a union type.** If one drag world merely carries
  several shapes, the enum is simpler, keeps one registry doing the work,
  and reads better in every handler.
- **Every listed type needs an ancestor provider.** `use_bridge_world`
  (and so both bridges) panics without a `DndProvider<T>` above it.
- **No `edge` on bridges.** `BridgeDropZone` and the generated components
  have no `edge` prop; a delivered `DropOutcome::edge` is always `None`.
  Need insertion edges? That is a sign the interaction is one world, and an
  enum with a plain `DropZone` gets you `edge` back.
- **Announcements are per world.** One `LiveRegion` per provider, so a
  bridged page renders one for each payload type or keyboard drags from
  the silent world say nothing.
- **The macro needs the Dioxus prelude in scope.** Invoke
  `bridge_drop_zone!` where `use dioxus::prelude::*;` is visible.

## Related

- [Drag and drop](drag-and-drop.md): the single-world machinery every
  bridge leg reuses.
- [Architecture](architecture.md): the per-type registry a bridge registers
  into twice.
- [Accessibility](accessibility.md): `LiveRegion` and the keyboard model
  each world brings to the bridge.

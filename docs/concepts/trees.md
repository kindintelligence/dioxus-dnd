# Trees

The classic tree problem: a drop on a node can mean three different things.
`TreeNodeTarget` splits each row into three bands - the top quarter inserts
before, the bottom quarter after, the middle half nests inside - and
delivers the resolved `DropIntent` with the payload, so one handler covers
reordering and reparenting.

API reference: [api/trees.md](../api/trees.md).
Live demo: the
[Project files](https://kindintelligence.github.io/dioxus-dnd/project-files)
page, a reparenting file tree with the cycle guard running live.

## The mental model

There is no tree component holding your nodes. You render rows in display
order (typically a depth-first flatten of your model), wrap each row in a
`TreeNodeTarget`, and put a core `Draggable` inside it. Each row is both a
target and a source. The payload is just the node's id, so the tree
structure lives in exactly one place, your model, and a drop handler is a
plain model edit.

Three small pieces carry the pattern:

- `NodeId` names a node: a `u64` newtype that works as a map key.
- `DropIntent` is the trichotomy: `Before`, `After`, or `Into`.
- `TreeDropEvent<T>` is what `on_drop` receives: `payload`, `target`, and
  `intent`. Nothing else is needed to perform the move.

## A worked example

A parent-pointer model, flattened for display, with folders that accept
children and files that refuse them:

```rust,ignore
#[derive(Clone, Copy, PartialEq)]
struct FsNode { id: u64, parent: Option<u64>, name: &'static str, folder: bool }

// nodes: Signal<Vec<FsNode>>; flat: Vec<(usize, FsNode)> in depth-first order
rsx! {
    DndProvider::<u64> {
        LiveRegion::<u64> {}
        for (depth, n) in flat {
            TreeNodeTarget::<u64> {
                key: "{n.id}",
                node: NodeId(n.id),
                row_height: 38.0,
                label: n.name,
                accepts: {
                    let (target, folder) = (n.id, n.folder);
                    move |(dragged, intent): (u64, DropIntent)| {
                        // Only folders can contain things.
                        if intent == DropIntent::Into && !folder { return false; }
                        // And nothing may land inside its own subtree.
                        let ns = nodes.read();
                        !would_create_cycle(
                            |id: NodeId| ns.iter()
                                .find(|x| x.id == id.0)
                                .and_then(|x| x.parent).map(NodeId),
                            NodeId(dragged),
                            NodeId(target),
                        )
                    }
                },
                on_drop: move |ev: TreeDropEvent<u64>| reparent(&mut nodes, ev),
                class: "data-[intent=before]:shadow-[inset_0_2px_0_0_currentColor]
                        data-[intent=after]:shadow-[inset_0_-2px_0_0_currentColor]
                        data-[intent=into]:bg-blue-50",
                Draggable::<u64> { payload: n.id, label: n.name, RowFace { node: n, depth } }
            }
        }
    }
}
```

The `accepts` closure captures per-row data by wrapping the `move` closure
in a block that first copies what it needs. That block-then-closure shape
is the standard Rust answer whenever each row of a loop needs its own
captured values.

The reparent itself is small because children keep pointing at the dragged
node, so a whole subtree moves with one field write:

```rust,ignore
let (new_parent, at) = match ev.intent {
    DropIntent::Into => (Some(target_id), ns.len()),
    DropIntent::Before => (ns[t].parent, t),
    DropIntent::After => (ns[t].parent, t + 1),
};
dragged.parent = new_parent;
ns.insert(at, dragged);
```

## The three bands

The pointer's vertical offset within the row resolves the intent: the top
25% is `Before`, the bottom 25% is `After`, the middle half is `Into`. The
math is the public function `intent_from_offset(y, row_height)`, so custom
tree interactions can reuse it exactly.

`row_height` (default 28.0) should stay close to the actual rendered
height. Pointer bands scale with it, and keyboard drops resolve their
intent from the measured row center against this value, so a large mismatch
can bias a keyboard drop toward `After` or `Before` instead of `Into`.

If your rows cannot receive children (a flat outline), do not fight the
middle band: accept `Into` and map it to whichever sibling intent you
prefer in your handler.

## Styling with `data-intent`

While an acceptable pointer drag hovers a row, the wrapper carries
`data-intent="before" | "after" | "into"` live, and drops the attribute the
moment the pointer leaves. Value selectors draw the three indicators with
no extra state: `data-[intent=before]:border-t-2`,
`data-[intent=into]:bg-blue-50`, `data-[intent=after]:border-b-2`. The
attribute follows mouse, touch, and pen alike, because the band is derived
from the shared pointer position, not from DOM hover events.

The demo's chevron trick is free: a folder chevron inside the row swings
open on `in-data-[intent=into]:rotate-90`, signalling "this will go inside"
with pure CSS.

## Acceptance sees payload and intent together

`accepts` receives the pair `(payload, intent)`, because tree rules are
about the combination, not the payload alone. "Files refuse Into" is one
comparison. "Nothing lands in its own subtree" only matters for the drop
that would reattach a node, so it belongs next to the intent check.

Hover cannot know the final band yet, so at the registry level a row
accepts a payload if your `accepts` passes for any of the three intents;
the exact intent is re-checked at release. A release in a band whose exact
combination is refused delivers nothing: the drag ends and `on_drop` is not
called, so the model stays untouched.

## The cycle guard

`would_create_cycle(parent_of, dragged, target)` answers one question:
would attaching `dragged` under `target` make a node its own ancestor? The
crate never holds your tree, so it walks `target`'s ancestry through the
`parent_of` lookup you provide - any closure from `NodeId` to
`Option<NodeId>` over your own model. It returns `true` for a drop onto
itself or onto any descendant.

It is defensive about broken data: the walk is bounded, and a cycle in your
own parent map reads as unsafe (`true`) rather than looping forever.

## Keyboard

Every row registers in the shared zone registry, so keyboard drags reach it
like any zone: Space picks up the row's `Draggable`, arrows move between
rows, Space drops. Give each row a `label`; that is what the `LiveRegion`
announces. A keyboard drop resolves from the measured row center, which is
the `Into` band, and nesting into the focused row is what a keyboard drop
should mean.

## Gotchas

- **The live band does not consult `accepts` per band.** A row whose `Into`
  is refused still shows `data-intent="into"` over its middle, and a
  release there delivers nothing. Either style the refused band as inert,
  or accept `Into` and remap it to a sibling intent in your handler.
- **Refusing `Into` on leaves makes them dead keyboard targets.** Keyboard
  drops always resolve the center band, so a row that refuses `Into`
  silently swallows keyboard drops. Mapping `Into` to `After` in the
  handler keeps keyboard users placing items everywhere.
- **Keep `row_height` honest.** Keyboard intent resolution compares the
  measured row center against it; wrapped or custom content much taller
  than the prop biases keyboard drops away from `Into`.
- **`TreeDropEvent` is non-exhaustive.** Destructure it with `..` and
  construct synthetic events (tests, programmatic moves) through
  `TreeDropEvent::new`.
- **Keep the payload an id.** The handler looks the node up in the model,
  so structure lives in one place and a stale payload cannot smuggle in an
  outdated subtree.

## Related

- [Drag and drop](drag-and-drop.md): the `Draggable` inside each row, and
  everything a `DropOutcome` carries.
- [Sortable lists](sortable-lists.md): flat reordering, when you do not
  need the `Into` band.
- [Styling](styling.md): the full data-attribute contract.
- [Accessibility](accessibility.md): the `LiveRegion` and what keyboard
  drags announce.

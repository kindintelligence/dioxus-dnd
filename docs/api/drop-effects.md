# Drop effects API reference

The move/copy/link vocabulary of a drop, its modifier-key resolution, and
the model helpers that apply a completed drop to app-owned state.

Concept guide: [docs/concepts/drop-effects.md](../concepts/drop-effects.md).

The crate never touches your data: drops arrive as `DropOutcome` values and
you decide what they mean. The two helpers here cover the most common
meaning, the remove-from-source, append-to-target dance, without imposing
bounds on your item type: no `Clone` (the payload arrives owned), no
`PartialEq` (matching is by the key you extract). `DropEffect` and
`effective_effect` are defined in the core types and re-exported from the
prelude alongside the helpers; this file is the reference for all four.

```rust,ignore
DropZone::<Card> {
    on_drop: move |o: DropOutcome<Card>| {
        apply_clone_or_move(
            &mut zones.write(),
            o,
            |c| c.id,                       // identity function
            |mut c| { c.id = fresh_id(); c },  // clone hook, runs only on Copy
        );
    },
    "Drop here"
}
```

## `DropEffect`

The visual and semantic effect of a drop, mirroring the HTML5
`dropEffect`/`effectAllowed` vocabulary.

| Variant | Meaning |
|---|---|
| `Move` | The item leaves its source and lands in the target. The default, and what most drags mean. |
| `Copy` | The target receives a duplicate; the source keeps the original. Forced by Ctrl or Cmd held at release. |
| `Link` | A reference-style drop, forced by Alt. Rare, but the vocabulary matches the platform convention. |
| `None` | Advertises that the drag carries no data effect. Modifier keys never override it; the crate still delivers the outcome, so interpretation stays with your handler. |

`as_str` returns the string the native `DataTransfer` API expects:
`"move"`, `"copy"`, `"link"`, `"none"`. The drag-out sources pass it to
`effectAllowed` when advertising a drag to other applications
([drag-out.md](drag-out.md)).

Where it appears:

- The `Draggable` `effect` prop (default `Move`) sets the drag's base
  effect ([drag-and-drop.md](drag-and-drop.md)).
- `DropOutcome::effect` carries the resolved value to `on_drop`.

## `effective_effect`

```rust,ignore
pub fn effective_effect(base: DropEffect, modifiers: dioxus::prelude::Modifiers) -> DropEffect
```

Resolves the effect a drag should use given the currently held modifier
keys, the file-manager convention:

| Held at release | Result |
|---|---|
| Ctrl or Cmd (Meta) | `Copy` |
| Alt | `Link` |
| Ctrl and Alt together | `Copy` (Ctrl wins) |
| Neither | `base` |
| Anything, when `base` is `DropEffect::None` | `None` (a base of `None` is never overridden) |

The library applies this for you. Pointer drags sample the held modifiers
on every pointer event and resolve them against the `Draggable`'s base
effect at release, just before delivery, so the state held at release wins,
not the state at pickup. Host-side multi-window deliveries resolve the same
function against the drag world's modifier snapshot, so a release in
another window behaves identically. Keyboard drops skip resolution and
deliver the base effect unchanged. The function is public for custom drag
sources and handlers that need the same answer.

## `apply_clone_or_move`

```rust,ignore
pub fn apply_clone_or_move<T, K>(
    zones: &mut HashMap<ZoneId, Vec<T>>,
    outcome: DropOutcome<T>,
    key: impl Fn(&T) -> K,
    clone_item: impl FnMut(T) -> T,
) where
    K: PartialEq,
```

Applies a drop to a `HashMap<ZoneId, Vec<T>>` model: one `Vec` per zone,
keyed by the ids your `DropZone`s declare.

| Argument | Type | What it does |
|---|---|---|
| `zones` | `&mut HashMap<ZoneId, Vec<T>>` | Your model. An unknown `to` zone is created on the fly rather than dropping the item on the floor. |
| `outcome` | `DropOutcome<T>` | The drop as delivered; `from`, `to` and `effect` steer what happens. |
| `key` | `impl Fn(&T) -> K` | The identity function. Extracts each item's key (typically an id field) so a move can find and remove the original in the source `Vec`. Matching is by this key, never `PartialEq` on the item. |
| `clone_item` | `impl FnMut(T) -> T` | The clone hook. Runs only on `Copy`, receiving the owned payload and returning the item to append. Assign the fresh id here so the copy gets its own identity. |

Semantics:

- `Move` removes the matching item from `outcome.from`, then appends the
  payload to `outcome.to`. Every effect other than `Copy` takes this
  branch, `Link` and `None` included.
- `Copy` leaves the source alone and appends `clone_item(payload)` to the
  target.
- Removal matches **every** item in the source whose key equals the
  payload's key. Keys are expected to be unique within a zone; if they are
  not, a single move prunes all of them.
- A move where `from == Some(to)` removes and re-appends, so dropping an
  item back onto its own zone sends it to the **end of that list**.
- A move with `from: None` (payload from outside any zone, e.g. a palette)
  skips removal and just appends. `from` is filled by the `Draggable`'s
  `zone` prop; declare it or removal never runs.

## `apply_list_clone_or_move`

```rust,ignore
pub fn apply_list_clone_or_move<T, K>(
    source: Option<&mut Vec<T>>,
    target: &mut Vec<T>,
    outcome: DropOutcome<T>,
    key: impl Fn(&T) -> K,
    clone_item: impl FnMut(T) -> T,
) where
    K: PartialEq,
```

The two-list version: applies a drop between two plain `Vec<T>`s with the
same move/copy semantics. You choose which lists to pass, so the outcome's
`from` and `to` fields are **ignored** here; only `payload` and `effect`
are consulted.

| Argument | Type | What it does |
|---|---|---|
| `source` | `Option<&mut Vec<T>>` | The list a move removes from. Pass `None` when the payload came from outside any list; removal is skipped. |
| `target` | `&mut Vec<T>` | The list the item is appended to. |
| `outcome` | `DropOutcome<T>` | Only `payload` and `effect` are read. |
| `key` | `impl Fn(&T) -> K` | Identity function, as above. A move removes every source item whose key matches the payload's. |
| `clone_item` | `impl FnMut(T) -> T` | Clone hook, as above. Runs only on `Copy`. |

## Where the rest lives

The full `DropOutcome` field reference and the `Draggable` and `DropZone`
props: [drag-and-drop.md](drag-and-drop.md). `ZoneId` and the registry:
[core.md](core.md). Advertising effects to other applications:
[drag-out.md](drag-out.md). Cross-window modifier behavior:
[multi-window.md](multi-window.md).

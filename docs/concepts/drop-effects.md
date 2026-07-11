# Drop effects

A drop can mean three things: move the item, copy it, or link to it. Users
already choose between them with modifier keys in their file manager, and
dioxus-dnd resolves that convention for you, out of the box, nothing to
configure. Your handler receives the answer; what it means for your data
stays yours to decide.

API reference: [api/drop-effects.md](../api/drop-effects.md).
Live demo: the
[Newsletter builder](https://kindintelligence.github.io/dioxus-dnd/newsletter-builder)
page runs `apply_clone_or_move`; the
[Mailbox](https://kindintelligence.github.io/dioxus-dnd/mailbox) page
branches on the effect by hand.

## The mental model

Every drag carries a base `DropEffect`, set by the `Draggable`'s `effect`
prop and defaulting to `Move`. While the drag is in flight, the held
modifier keys are sampled on every pointer event, so the state held at the
moment of release is what wins, not what was held at pickup:

- **Ctrl or Cmd** forces `Copy`.
- **Alt** forces `Link`.
- Neither held: the base effect stands.
- A base of `None` is never overridden. When Ctrl and Alt are both held,
  Ctrl wins.

The resolved value arrives in `DropOutcome::effect`. At that point nothing
has happened to your data: the library reports what the user asked for and
leaves the interpretation to you. The resolution itself is the pure
function `effective_effect`, public for custom drag sources and handlers
that need the same answer.

## Branching on the effect

The direct route is a branch in `on_drop`. The mailbox demo files receipts
by copy (Ctrl or Cmd held keeps the originals in the inbox) and moves
otherwise:

```rust,ignore
DropZone::<Vec<u32>> {
    label: "Receipts",
    on_drop: move |o: DropOutcome<Vec<u32>>| {
        if o.effect == DropEffect::Copy {
            labeled.write().extend(o.payload);   // originals stay
        } else {
            inbox.write().retain(|m| !o.payload.contains(&m.id));
        }
    },
}
```

The `effect` prop sets the base the modifiers override. Leave it at `Move`
for ordinary drags, set `Copy` on sources whose drags always duplicate (a
stamp palette), and `None` to advertise that a drag carries no data effect;
`None` is the one base modifiers never touch.

## Worked example: zones in a HashMap

For models shaped as one `Vec` per zone, `apply_clone_or_move` applies the
whole convention in one call. Give it an identity function so a move can
find and remove the source item, and a clone hook that assigns a fresh id
on copy:

```rust,ignore
use std::collections::HashMap;

const PALETTE: ZoneId = ZoneId(1);
const STAGE: ZoneId = ZoneId(2);

#[derive(Clone, PartialEq)]
struct Block { id: u32, name: String }

#[component]
fn Builder() -> Element {
    let mut zones = use_signal(|| HashMap::from([
        (PALETTE, vec![
            Block { id: 1, name: "Heading".into() },
            Block { id: 2, name: "Image".into() },
        ]),
        (STAGE, Vec::new()),
    ]));
    let mut next_id = use_signal(|| 100u32);
    let on_drop = move |o: DropOutcome<Block>| {
        apply_clone_or_move(
            &mut zones.write(),
            o,
            |b| b.id,          // identity: a Move finds and removes this key
            move |mut b| {     // clone hook: runs only on Copy
                b.id = next_id();
                next_id += 1;
                b
            },
        );
    };
    rsx! {
        DndProvider::<Block> {
            for (name, zone) in [("Blocks", PALETTE), ("Your email", STAGE)] {
                DropZone::<Block> {
                    id: zone,
                    label: name,
                    on_drop,
                    for block in zones.read().get(&zone).cloned().unwrap_or_default() {
                        Draggable::<Block> {
                            key: "{block.id}",
                            payload: block.clone(),
                            zone,          // fills DropOutcome::from
                            label: block.name.clone(),
                            "{block.name}"
                        }
                    }
                }
            }
        }
    }
}
```

One handler covers both gestures. A plain drag moves a block between the
two `Vec`s; the same drag with Ctrl or Cmd held leaves the palette alone
and appends a copy with its own id. The `zone` prop matters: it fills
`DropOutcome::from`, which is how the helper knows which `Vec` to prune.

## Worked example: two plain lists

When the model is just two `Vec`s, `apply_list_clone_or_move` takes them
directly. It ignores the outcome's `from` and `to`, you choose the lists,
so no `zone` declaration is needed:

```rust,ignore
#[derive(Clone, PartialEq)]
struct Task { id: u32, title: String }

let mut backlog = use_signal(seed_tasks);
let mut today = use_signal(Vec::<Task>::new);
let mut next_id = use_signal(|| 100u32);

rsx! {
    DndProvider::<Task> {
        for task in backlog() {
            Draggable::<Task> { key: "{task.id}", payload: task.clone(), "{task.title}" }
        }
        DropZone::<Task> {
            label: "Today",
            on_drop: move |o: DropOutcome<Task>| {
                apply_list_clone_or_move(
                    Some(&mut backlog.write()),
                    &mut today.write(),
                    o,
                    |t| t.id,
                    move |mut t| { t.id = next_id(); next_id += 1; t },
                );
            },
            "Plan for today"
        }
    }
}
```

Pass `None` for the source when the payload came from outside any list (a
palette, an external drop); the move then just appends.

## Multi-window drags

On desktop multi-window setups the convention does not change. Modifier
state travels with the shared drag world, so a drag picked up in one window
and released in another applies the modifiers held at release exactly as a
local drop does, including host-side deliveries where the release happens
outside the origin webview. Every window's `on_drop` branches on
`DropOutcome::effect` the same way. See
[Multi-window desktop drags](multi-window.md).

## Gotchas

- **Keyboard drops carry the base effect.** Modifier resolution is pointer
  vocabulary; the keyboard path spends its keys on pickup, navigation and
  drop, so `DropOutcome::effect` is the `effect` prop unchanged. Your
  branch needs no special case, the field is always filled.
- **The helpers only special-case `Copy`.** `Link` and `None` take the move
  branch. If `Link` means something in your app, branch on
  `DropOutcome::effect` before, or instead of, calling the helper.
- **A move prunes only a declared source.** `apply_clone_or_move` finds the
  source through `DropOutcome::from`, which the `Draggable`'s `zone` prop
  fills. Without it, a move just appends and the original stays.
- **Copies need fresh identity.** The clone hook is where the new id comes
  from. Return the item unchanged and the copy shares the original's key;
  removal matches every item with the payload's key, so the next move
  prunes both.
- **A self-drop reorders to the end.** A move with `from == to` removes and
  re-appends. When in-list order matters, reach for
  [Sortable lists](sortable-lists.md) or [Boards](boards.md) instead.

## Related

- [Drag and drop](drag-and-drop.md): `DropOutcome` in full and the
  components these props live on.
- [Multi-window desktop drags](multi-window.md): how a drag and its
  modifier state travel across windows.
- [Dragging out](drag-out.md): the same `DropEffect` vocabulary advertised
  to other applications through `effectAllowed`.

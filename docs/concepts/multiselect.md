# Multi-select

The mail-client pattern: click selects one item, Ctrl/Cmd+click builds a
stack, and dragging any selected item moves the whole stack as one payload.
There is no special multi-drag machinery underneath. The core is generic
over the payload type, and multi-select is what falls out when that type
is `Vec<K>`.

API reference: [api/multiselect.md](../api/multiselect.md).
Live demo: the
[Mailbox](https://kindintelligence.github.io/dioxus-dnd/mailbox) page,
multi-select triage with a modifier-keyed copy.

## The mental model

Everything rides the ordinary components, parameterized with `Vec<K>`:

- `DndProvider::<Vec<K>>` is the drag world. `K` is your item key type, any
  `Clone + PartialEq` value; ids are typical. Zones are plain
  `DropZone::<Vec<K>>`s and receive the whole selection in one
  `DropOutcome<Vec<K>>`.
- `use_selection::<K>()` returns a `Selection<K>`, shared selection state
  and the single source of truth for what is selected. Like a Dioxus
  signal handle it is `Copy`: passing it into rows hands out keys to the
  same state, so every row stays in sync.
- `SelectableDraggable<K>` wraps each item. It wires the click conventions
  into the selection and resolves its drag payload from it: a selected
  item drags the group, an unselected one drags itself.

Keys keep the payload tiny and your model authoritative: the provider's
type is `Vec<u32>`, not `Vec<Email>`. A drop handler maps keys back to
rows it already owns.

## A complete example

```rust,ignore
let mut selection = use_selection::<u32>();
rsx! {
    DndProvider::<Vec<u32>> {
        for mail in inbox() {
            SelectableDraggable::<u32> {
                key: "{mail.id}",
                item: mail.id,
                selection,
                label: mail.subject,
                MailRow { mail }
            }
        }
        DropZone::<Vec<u32>> {
            label: "Archive",
            on_drop: move |o: DropOutcome<Vec<u32>>| {
                inbox.write().retain(|m| !o.payload.contains(&m.id));
                selection.clear();
            },
            "Archive"
        }
        DragOverlay::<Vec<u32>> { SelectionCount::<u32> {} }
    }
}
```

Note what is absent: no selection bookkeeping in your model, no per-row
event handlers, no multi-drag mode. The selection handle and the `Vec`
payload carry all of it.

## Click, toggle, drag

`SelectableDraggable` implements the platform conventions:

- A plain click selects only that item.
- Ctrl+click (Cmd on macOS) toggles it in or out of the stack.
- Dragging a selected item picks up the whole selection, in selection
  order. Dragging an unselected item picks up just that item, a
  one-element `Vec<K>`.
- The browser fires a trailing `click` on the source after a completed
  pointer drag; letting it through would collapse the just-dragged stack
  to that one item. The component swallows exactly that one click.

The conventions come from `Selection::click`; the rest of the `Selection`
API (`select_only`, `toggle`, `clear`, `items`) is public for custom
interactions, a select-all checkbox or shift ranges, which stay in sync
with the rows automatically because the state is shared.

## The ghost counts, it does not clone

Render a count in the `DragOverlay` instead of cloning rows: dragging
forty items costs the same as dragging one. `SelectionCount` is the
ready-made badge; it reads the in-flight payload and renders "3 item(s)"
(the text is a `DndStrings` closure, see
[Localization](localization.md)). For a custom ghost, read the payload
yourself:

```rust,ignore
#[component]
fn StackGhost() -> Element {
    let dnd = use_dnd::<Vec<u32>>();
    let n = dnd.payload().map(|p| p.len()).unwrap_or(0);
    rsx! { "{n} selected" }
}
```

## Styling selection

Each selected item's wrapper carries `data-selected="true"`, absent when
unselected, so presence-based selectors work directly:

```rust,ignore
SelectableDraggable::<u32> {
    item: mail.id,
    selection,
    class: "rounded data-selected:bg-emerald-100",
    MailRow { mail }
}
```

The attribute follows the shared `Selection`, not component-local state,
so a clear-all button anywhere updates every row. During a stack drag,
every selected item also carries `data-dragging` (on the inner `Draggable`
wrapper), because they all resolve to the same `Vec<K>` payload; dim them
all and the stack visibly leaves.

## Copy versus move

The drop is an ordinary `DropOutcome<Vec<K>>`, so everything from
[Drag and drop](drag-and-drop.md) composes: `accepts` can veto a stack,
`edge` reads insertion sides, and `effect` resolves the modifier keys held
at release. The Mailbox demo files a copy when Ctrl or Cmd is held and
moves otherwise:

```rust,ignore
on_drop: move |o: DropOutcome<Vec<u32>>| {
    if o.effect == DropEffect::Copy {
        labeled.write().extend(o.payload);      // originals stay
    } else {
        inbox.write().retain(|m| !o.payload.contains(&m.id));
    }
    selection.clear();
},
```

## Gotchas

- **Clear the selection in your drop handler.** The library does not guess
  whether a completed drop should keep the stack; call `selection.clear()`
  when it should not.
- **Selection order, not document order.** The payload is the selection
  snapshot: keys arrive in the order they were toggled in. Sort against
  your model if order matters at the destination.
- **Selection building is a click interaction.** Keyboard pickup (Space or
  Enter on a focused row) starts a drag; it does not change the selection.
  Keyboard and switch users can drag whatever is already selected, so give
  them a non-pointer way in, such as a per-row checkbox calling
  `selection.toggle`.
- **Your `class` lands on the outer wrapper.** `data-selected` lives
  there, but `data-dragging` sits one div deeper, on the inner
  `Draggable`. Style drag state with a descendant or `:has()` selector.
  See [Styling](styling.md).
- **Need `disabled`, `touch` or `threshold`?** `SelectableDraggable` does
  not forward them. Compose the core `Draggable::<Vec<K>>` yourself and
  resolve the payload from `selection` the same way.

## Related

- [Drag and drop](drag-and-drop.md): the components this pattern
  parameterizes with `Vec<K>`.
- [Drop effects](drop-effects.md): the modifier convention behind
  copy-versus-move triage.
- [Localization](localization.md): `selection_count` and the rest of the
  announcement strings.
- [Styling](styling.md): the full data-attribute contract.

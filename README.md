# dioxus-dnd

[![Crates.io](https://img.shields.io/crates/v/dioxus-dnd.svg)](https://crates.io/crates/dioxus-dnd)
[![Documentation](https://docs.rs/dioxus-dnd/badge.svg)](https://docs.rs/dioxus-dnd)
[![Downloads](https://img.shields.io/crates/d/dioxus-dnd.svg)](https://crates.io/crates/dioxus-dnd)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![Dioxus 0.8](https://img.shields.io/badge/dioxus-0.8-0E6B63)](https://dioxuslabs.com)
[![MSRV 1.85](https://img.shields.io/badge/rustc-1.85%2B-orange.svg)](https://releases.rs/docs/1.85.0/)
[![CI](https://github.com/kindintelligence/dioxus-dnd/actions/workflows/ci.yml/badge.svg)](https://github.com/kindintelligence/dioxus-dnd/actions)

Modular, accessible drag-and-drop for Dioxus. One small core, one module per
drop pattern: use only what you need. Keyboard accessible by default, with
touch support, live drop previews, and auto-scroll built in.

## Compatibility

| dioxus-dnd | Dioxus | Rust |
|---|---|---|
| 0.5 | **0.8** (verified against `0.8.0-alpha.0`; also compiles unchanged on `0.7.9`) | 1.85+ |

The crate depends on `dioxus` with `default-features = false, features =
["minimal"]`, so it adds no renderer, no JavaScript, and no extra
dependencies of its own.

## Modules

| Module | Pattern | Payload transport |
|---|---|---|
| `core` | `Draggable` to `DropZone` with any `Clone` payload | Rust `Store` context |
| `files` | OS file drops (`FileDropZone`, `FileFilter`) | native event (`evt.files()`) |
| `sortable` | reorder within one list, with live preview (`SortableList`) | self-contained (indices) |
| `board` | kanban and cross-container moves (`BoardColumn`, `BoardItem`, `BoardSlot`) | context (`BoardPayload<T>`) |
| `tree` | nested drops with before/after/into intent (`TreeNodeTarget`) | context |
| `canvas` | free-position drops with snap and bounds (`CanvasDropZone`) | context |
| `grid` | 2D tile reorder or swap (`SortableGrid`) | self-contained (indices) |
| `multiselect` | drag N selected items as one (`SelectableDraggable`) | context (`Vec<K>`) |
| `external` | text, URLs and HTML dropped in from other apps | native `DataTransfer` |
| `dragout` | drag text, links and HTML out to other apps (`ExternalDragSource`) | native `DataTransfer` |
| `pointer` | touch and pen drags (`PointerDraggable`) | context + pointer events |
| `autoscroll` | edge-scrolling containers (`AutoScroll`) | n/a |
| `a11y` | screen-reader announcements (`LiveRegion`), no-drag reordering (`ReorderButtons`) | n/a |
| `animate` | FLIP reorder transitions (`FlipItem`, experimental) | n/a |

## Design

In-app payloads travel through a shared `Store<DragState<T>>` in Dioxus
context: any `Clone` type, zero serialization, no `DataTransfer` string
round-trip. Stores (Dioxus 0.8's fine-grained reactivity) give each state
field its own lazy subscription, so a component that reads `dnd.over()` in
render to highlight a zone reruns only when the hovered zone changes, not
on every pointer move.

The native events are used for what only they can do: gestures,
coordinates, OS files, and content crossing the app boundary. Firefox's
requirement that something is set on `DataTransfer` for a drag to start is
handled for you.

The provider also carries a zone registry. Every mounted `DropZone`
records its id, label, drop callback, acceptance filter and DOM handle
there; that registry powers keyboard navigation and touch hit-testing, and
it is public (`use_zone_registry`) if you want to build your own
interaction on top. The pointer gesture lifecycle itself is a formal,
exhaustively tested state machine (`core::machine`), also public.

## Quick start

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[derive(Clone, PartialEq)]
struct Card { id: u32, title: String }

#[component]
fn App() -> Element {
    rsx! {
        DndProvider::<Card> {
            Draggable::<Card> {
                payload: Card { id: 1, title: "hello".into() },
                class: "card",
                "Drag me"
            }
            DropZone::<Card> {
                on_drop: move |o: DropOutcome<Card>| {
                    tracing::info!("got card {} at {:?}", o.payload.id, o.element);
                },
                accepts: move |c: Card| c.id != 0,
                class: "bin",
                "Drop here"
            }
        }
    }
}
```

Both components forward arbitrary attributes (`class`, `style`, and so on)
to their wrapper `div`.

## Accessibility (built in, not opt-in)

Every core `Draggable` (and `PointerDraggable`) is focusable and keyboard
operable:

- **Space / Enter** picks the item up
- **Up / Down** cycles drop zones at the current level (spatial order)
- **Right / Left** descends into a zone's nested zones or ascends to the
  parent (nesting is detected automatically when `DropZone`s contain
  `DropZone`s; in flat apps these fall back to next/previous)
- **Space / Enter** drops into the selected zone
- **Escape** cancels

Render one `LiveRegion::<T>` per provider to voice announcements to screen
readers, and give `Draggable` and `DropZone` a `label`:

```rust,ignore
DndProvider::<Card> {
    LiveRegion::<Card> {}
    Draggable::<Card> { payload: card, label: "Ship it", /* ... */ }
    DropZone::<Card>  { label: "Done", on_drop, /* ... */ }
}
// "Picked up Ship it. Use arrow keys to choose a drop target, ..."
// "Over Done." then "Dropped in Done."
```

For a no-drag fallback, `a11y::ReorderButtons` renders headless move-up and
move-down buttons that emit the same `SortEvent` as dragging, so one
`on_sort` serves both inputs. Custom flows can push their own messages
with `dnd.announce(...)`.

## Touch

Every pattern in this crate works with touch and pen instantly, in every
browser. You do not need to do anything to get it.

Native HTML5 drag from a touch long-press exists in some browsers (Safari
on iOS/iPadOS 15+; Android support is inconsistent), so relying on it means
your app works on some phones after a hold and not at all on others. This
crate instead runs a pointer-event gesture path alongside the native one:
mouse input takes the native HTML5 path, while touch and pen input is
recognized by a small movement threshold (no hold delay) and hit-tested
against measured client rects. Both paths feed the same state and drop
callbacks, so your code cannot tell which one fired.

- `PointerDraggable` is the touch-capable drag source for `DropZone`
  targets. `BoardItem`, `SelectableDraggable` and `TreeNodeTarget` already
  build on this machinery, so boards, multi-select and trees are
  touch-ready as-is. Missed finger drops re-measure the zones and retry
  with a closest-center fallback, so drops in the gutter between zones
  still land.
- `SortableList` and `SortableGrid` carry their own built-in pointer path.
  Rows and tiles respond to touch with the same live displacement preview
  as mouse drags.

```rust,ignore
PointerDraggable::<Card> { payload: card, label: "Ship it", "Ship it" }
```

The one tradeoff to know about: a touch drag surface must set
`touch-action: none`, which stops the browser from scrolling when a finger
moves on it. For a `SortableList` inside a scrollable container, set
`touch_handle: true` so only a leading grip (style it via
`[data-sort-handle]`) claims the finger and the rows themselves keep
scrolling:

```rust,ignore
SortableList { len, render, on_sort, touch_handle: true }
```

There is deliberately no long-press activation option; a movement
threshold plus an explicit handle is more predictable than a timer, and
works the same for pens.

## Sortable lists with live preview

```rust,ignore
let mut items = use_signal(|| vec!["alpha".to_string(), "beta".into(), "gamma".into()]);
rsx! {
    SortableList {
        len: items.read().len(),
        render: move |ix: usize| rsx! { "{items.read()[ix]}" },
        on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
    }
}
```

While you drag, the row slides toward its landing slot and its neighbors
translate out of the way: a live preview of the final order (disable with
`live_preview: false`). Style the hover target via
`[data-drop-target="true"]` and the dragged row via
`[data-dragging="true"]`.

## Auto-scroll

Wrap any scrollable container in `AutoScroll` and drags hovering within
`threshold` px of an edge (default 48) scroll it by up to `speed` px per
event (default 24), ramped by proximity. Works for native mouse drags and
`PointerDraggable` touch drags. Pure `MountedData`, no JavaScript eval.

```rust,ignore
AutoScroll { style: "max-height: 300px; overflow-y: auto;",
    for row in rows { /* ... */ }
}
```

## Modifier keys

The file-manager convention works out of the box: holding **Ctrl/Cmd**
during a drag forces a Copy effect, **Alt** forces Link. Both are
reflected in the browser's cursor feedback and in `DropOutcome::effect`,
so your `on_drop` can branch on move-vs-copy. `effective_effect` is public
if you need the same resolution in custom handlers. For positional
constraints there is a composable modifier chain (`core::modifiers`):
`LockAxis`, `Snap` and `KeepInside`, applied with `apply_modifiers`.

For simple zone models, `apply_clone_or_move` applies that convention to a
`HashMap<ZoneId, Vec<T>>`. Give it an identity function so moves can remove
the source item, and a clone hook for assigning a fresh id on copy:

```rust,ignore
DropZone::<Card> {
    on_drop: move |outcome: DropOutcome<Card>| {
        apply_clone_or_move(
            &mut cards_by_zone.write(),
            outcome,
            |card| card.id,
            |mut card| {
                card.id = next_id();
                next_id += 1;
                card
            },
        );
    },
    "Drop here"
}
```

For two plain lists, use `apply_list_clone_or_move` and pass the source list
directly:

```rust,ignore
DropZone::<Card> {
    id: DONE,
    on_drop: move |outcome: DropOutcome<Card>| {
        let mut todo_items = todo.write();
        let mut done_items = done.write();

        apply_list_clone_or_move(
            Some(&mut todo_items),
            &mut done_items,
            outcome,
            |card| card.id,
            |mut card| {
                card.id = next_id();
                next_id += 1;
                card
            },
        );
    },
    "Drop here"
}
```

## Multi-select

```rust,ignore
let selection = use_selection::<FileId>();
rsx! {
    DndProvider::<Vec<FileId>> {
        for f in files { SelectableDraggable::<FileId> { item: f.id, selection, FileRow { f } } }
        DropZone::<Vec<FileId>> { on_drop: move |o| trash(o.payload), "Trash" }
        DragOverlay::<Vec<FileId>> { SelectionCount::<FileId> {} }
    }
}
```

Click selects one, Ctrl/Cmd+click toggles. Dragging a selected item
carries the whole selection; dragging an unselected one carries just
itself.

## File drops

```rust,ignore
FileDropZone {
    filter: FileFilter::new()
        .extensions(["png", "jpg"])
        .content_types(["image/*"])
        .max_size(5_000_000)
        .max_files(10),
    on_files: move |drop: FileDrop| async move {
        for f in drop.files {
            let bytes = f.read_bytes().await?; // web
            // or f.path()                     // desktop
        }
    },
    on_rejected: move |bad: Vec<(FileData, FileRejection)>| { /* toast */ },
    "Drop images"
}
```

## Dragging out

```rust,ignore
ExternalDragSource {
    content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
    "Drag this link to another tab"
}
```

`OutboundContent` covers text, links (written as `text/uri-list` plus
`text/plain` plus `text/html`), rich HTML with a plain-text fallback, and
raw custom `(format, data)` pairs.

## Nesting

Sortables inside sortables, boards inside boards: inner drag scopes stop
propagation on drag start and drop, so each level owns its own gestures.
Nested `DropZone`s discover their parents automatically through context,
which is what powers hierarchical keyboard traversal. No configuration
needed.

## Live showcase

`examples/showcase.rs` is a full landing page whose centerpiece is a live
playground: one interactive demo per pattern, with an "outcome tape" that
prints every `DropOutcome` the library delivers. It is designed to deploy
as-is as the project website:

```sh
dx serve --example showcase --platform web
```

There is also a focused board example:

```sh
dx serve --example kanban --platform web
```

## Feature flags

- `serde`: enables `external::typed::{store, retrieve}`, JSON-typed
  payloads over the native `DataTransfer` (wire-compatible with
  dioxus-html's own `store`/`retrieve`) for drags that must cross app or
  window boundaries.

## Platform notes

- **Firefox**: handled. Drags set `text/plain` data automatically so the
  gesture starts.
- **`DragOverlay`**: pointer tracking uses the `drag` event, whose
  coordinates some webviews report as `(0, 0)`. Those samples are ignored,
  so treat the overlay as progressive enhancement on desktop.
- **Windows desktop file drops** have a history of platform quirks in
  wry-based webviews. Test on your target and consider a file input
  fallback.
- **`animate::FlipItem`** is experimental: it is the one module whose
  behavior depends on browser paint timing rather than pure logic.

## Prior art

The Dioxus ecosystem has several dnd crates with different philosophies:
`dioxus-dnd-kit` (mouse-synthesized, layout-stability focused), `taino-dnd`
(framework-agnostic core, pointer-events) and `dioxus-nox-dnd` (headless
sortable primitives). This crate's live-preview displacement, collision
fallback, modifier chain and gesture state machine were informed by
reading them, and by dnd-kit and react-beautiful-dnd before that. What it
does that the others do not: the native HTML5 path (OS file drops,
drag-out to other apps, copy/move cursor effects) alongside touch and
keyboard, across twelve patterns.

## License

Licensed under the [MIT license](LICENSE-MIT).

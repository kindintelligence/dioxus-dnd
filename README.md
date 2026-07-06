Refector in progress due to bug. version v0.2.0 will be stable release. 

# dioxus-dnd

[![Crates.io](https://img.shields.io/crates/v/dioxus-dnd.svg)]([https://crates.io/crates/dioxus-dnd](https://docs.rs/dioxus-dnd/1.0.0/dioxus_dnd/))
[![Documentation](https://docs.rs/dioxus-dnd/badge.svg)](https://docs.rs/dioxus-dnd)
[![Downloads](https://img.shields.io/crates/d/dioxus-dnd.svg)](https://crates.io/crates/dioxus-dnd)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![Dioxus 0.8](https://img.shields.io/badge/dioxus-0.8-0E6B63)](https://dioxuslabs.com)
[![MSRV 1.85](https://img.shields.io/badge/rustc-1.85%2B-orange.svg)](https://releases.rs/docs/1.85.0/)

Modular, accessible drag-and-drop for Dioxus. One small core, one module per
drop pattern: use only what you need. Keyboard accessible by default, with
touch support, live drop previews, and auto-scroll built in.

## Compatibility

| dioxus-dnd | Dioxus | Rust |
|---|---|---|
| 1.0 | **0.8** (verified against `0.8.0-alpha.0`; also compiles unchanged on `0.7.9`) | 1.85+ |

The crate depends on `dioxus` with `default-features = false, features =
["minimal"]`, so by default it adds no renderer, no JavaScript, and no extra
dependencies of its own. The optional `web` feature is the only exception: it
pulls in `web-sys` for native pointer capture (see [Feature flags](#feature-flags)).

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
| `pointer` | configurable pointer/native drag source (`PointerDraggable`) | context + pointer events |
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

Native events are used for what only they can do: OS files, `DataTransfer`,
and content crossing the app boundary. In-app drag sources can instead use
pointer events, which avoids the browser's native drag image and keeps the
visual state under your control. Firefox's requirement that something is set
on `DataTransfer` for a native drag to start is handled for you.

The provider also carries a zone registry. Every mounted `DropZone`
records its id, label, drop callback, acceptance filter and DOM handle
there; that registry powers keyboard navigation and touch hit-testing, and
it is public (`use_zone_registry`) if you want to build your own
interaction on top. The pointer gesture lifecycle itself is a formal,
exhaustively tested state machine (`core::machine`), also public.

## Quick start

```text
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

## Styling (Tailwind-ready)

The library ships no CSS and no theme. Every component forwards `class` to
its wrapper, and drag state is exposed as data attributes that are
**present while active and absent otherwise** - never `="false"` - so both
plain CSS (`[data-dragging] { … }`) and Tailwind's presence-based variants
(`data-dragging:opacity-50`) work directly:

| Attribute | Found on | Present while |
|---|---|---|
| `data-dragging` | `Draggable`, `PointerDraggable`, `SortableList` / `SortableGrid` items | that element's payload is being dragged |
| `data-drop-target` | `SortableList` / `SortableGrid` items | hovered as the drop slot |
| `data-over` | `DropZone`, `FileDropZone`, `ExternalDropZone` | a (compatible) drag hovers the zone |
| `data-active` | `DropZone`, `BoardSlot`, `CanvasDropZone` | a compatible drag is in flight anywhere - reveal your targets |
| `data-intent` | `TreeNodeTarget` | hovered; valued `"before" \| "after" \| "into"` |
| `data-selected` | `SelectableDraggable` | the item is selected |
| `data-disabled` | `Draggable`, `PointerDraggable` | dragging is disabled |

Context-backed attributes (`Draggable`, `PointerDraggable`, `DropZone`,
`CanvasDropZone`, `BoardSlot`, `TreeNodeTarget`) follow mouse, touch, pen
and keyboard drags. Native boundary components (`FileDropZone`,
`ExternalDropZone`) reflect browser drag events from outside the app. With
Tailwind that composes into complete drag styling with no extra state:

```text
DndProvider::<Card> {
    Draggable::<Card> {
        payload: card,
        class: "rounded-lg border p-3 data-dragging:opacity-40 data-dragging:cursor-grabbing",
        "Drag me"
    }
    DropZone::<Card> {
        on_drop: handle_drop,
        class: "rounded-xl border-2 border-dashed border-transparent p-4
                data-active:border-gray-300 data-over:border-blue-500 data-over:bg-blue-50",
        "Drop here"
    }
}
```

`SortableList` and `SortableGrid` render their own item wrappers; those
wrappers are where `data-dragging` / `data-drop-target` live. For lists,
style those wrappers from the list's forwarded root `class` with direct
child selectors. `SortableGrid` also has an `item_class` prop for its tile
wrappers:

```text
SortableList {
    len, render, on_sort,
    class: "[&>*]:rounded [&>*]:border [&>*]:bg-white [&>*]:p-2
            [&>[data-dragging]]:opacity-40
            [&>[data-drop-target]]:border-blue-500",
}
```

Value selectors work too, e.g. tree insertion indicators:
`data-[intent=before]:border-t-2 data-[intent=into]:bg-blue-50
data-[intent=after]:border-b-2`.

The drag ghost styles the same way - `DragOverlay` forwards `class` to its
wrapper (positioning stays functional):

```text
DragOverlay::<Card> { class: "rotate-3 scale-105 shadow-xl", GhostCard {} }
```

To style *children* of a state-carrying wrapper, either mark the wrapper a
group (`SortableGrid`'s `item_class: "group"`, or a list root selector such
as `class: "[&>*]:group"`) and use `group-data-dragging:opacity-40` on
inner elements, or - Tailwind v4 - use the `in-*` variant from inside with
no wrapper class at all: `in-data-dragging:italic` inside your `render`
content reacts to the row's drag state with zero wiring.

One mechanic worth knowing: a forwarded `style` is *merged after* any
functional inline style (`touch-action` on `PointerDraggable`, positioning
on `DragOverlay`, the `display: grid` layout on `SortableGrid`) rather
than replacing it - your declarations win per property, the functional
ones survive. So grid spacing is just `class: "gap-2"`, and custom column
tracks are `style: "grid-template-columns: 2fr 1fr 1fr;"`.

See `examples/tailwind.rs` for all of this in one place (`dx serve
--example tailwind --platform web --features web`).

Not using Tailwind? The same contract serves plain CSS: `[data-over]`,
`[data-intent="into"]`, `[data-sort-handle]`, and so on.

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

```text
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

Every in-app drag pattern in this crate defaults to pointer events for mouse,
touch and pen. You do not need to do anything to get consistent styled drags
inside the app.

Native HTML5 drag from a touch long-press exists in some browsers (Safari
on iOS/iPadOS 15+; Android support is inconsistent), so relying on it means
your app works on some phones after a hold and not at all on others. This
crate uses the pointer-event path for ordinary app UI. Set
`input: DragInputMode::Native` or `Hybrid` only when you need the browser's
HTML5 drag behavior.

- `PointerDraggable` is the configurable pointer-capable drag source for
  `DropZone` targets. `BoardItem`, `SelectableDraggable` and
  `TreeNodeTarget` already build on this machinery, so boards, multi-select
  and trees are touch-ready as-is. Missed pointer drops re-measure the zones
  and retry with a closest-center fallback, so drops in the gutter between
  zones still land.
- `SortableList` and `SortableGrid` carry their own built-in pointer path.
  They default to pointer events for mouse, touch and pen, which avoids the
  browser's native drag image during reorders. Use
  `input: DragInputMode::Native` or `Hybrid` to opt back into HTML5 drag.
- Native components stay native: `FileDropZone`, `ExternalDropZone`,
  `ExternalDragSource`, `external::typed` and plain `Draggable` use
  `DataTransfer` for file drops, external drops, drag-out and cross-window
  interop.

```text
PointerDraggable::<Card> {
    payload: card,
    label: "Ship it",
    "Ship it"
}
```

The one tradeoff to know about: a touch drag surface must set
`touch-action: none`, which stops the browser from scrolling when a finger
moves on it. For a `SortableList` inside a scrollable container, set
`touch_handle: true` so only a leading grip claims the finger and the rows
themselves keep scrolling. The default grip is exposed as
`[data-sort-handle]`, so style it from the list root class or plain CSS:

```text
SortableList { len, render, on_sort, touch_handle: true,
    class: "[&_[data-sort-handle]]:w-6 [&_[data-sort-handle]]:cursor-grab",
}
```

There is deliberately no long-press activation option; a movement
threshold plus an explicit handle is more predictable than a timer, and
works the same for pens.

## Sortable lists with live preview

```text
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
`live_preview: false`). Native HTML5 drag is off by default for sortables;
set `input: DragInputMode::Native` or `Hybrid` only when you need browser
drag behavior. Style the hover target via `[data-drop-target]` and the
dragged row via `[data-dragging]` (or Tailwind presence selectors on the
list root, such as `[&>[data-drop-target]]:border-blue-500`).

## Canvas drops

`CanvasDropZone` is the free-position primitive for node editors,
whiteboards and floor planners. Start in-app moves with `PointerDraggable`;
the completed `CanvasDrop` gives you both the raw canvas-relative pointer
and the corrected top-left position:

- `pointer`: where the pointer landed inside the canvas.
- `position`: `pointer - grab`, then optional snap and bounds.
- `Bounds`: clamps the returned top-left point. It does not know the dropped
  element's own width or height.

```text
CanvasDropZone::<Node> {
    snap: SnapGrid(16.0),
    bounds: Bounds { width: 640.0, height: 360.0 },
    on_drop: move |drop: CanvasDrop<Node>| {
        place_node(drop.payload.id, drop.position);
    },
    for node in nodes.read().clone() {
        PointerDraggable::<Node> {
            payload: node,
            style: "position: absolute;",
            NodeView {}
        }
    }
}
```

For richer constraints such as "keep the whole node inside the canvas", use
the existing modifier chain (`apply_modifiers`, `DragModifier::KeepInside`,
`ModifierCtx`) with the element size you know in your app. Keep native
`DataTransfer` components for browser and OS boundary drags, such as files
or external text dropped onto a canvas.

## Auto-scroll

Wrap any scrollable container in `AutoScroll` and drags hovering within
`threshold` px of an edge (default 48) scroll it by up to `speed` px per
event (default 24), ramped by proximity. Works for native drags and
`PointerDraggable` pointer drags. Pure `MountedData`, no JavaScript eval.

```text
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

```text
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

```text
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

```text
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

```text
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

```text
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
dx serve --example showcase --platform web --features web
```

There is also a focused board example:

```sh
dx serve --example kanban --platform web --features web
```

And a focused canvas example that keeps the core headless while using
`core::modifiers` for snap and item-aware bounds:

```sh
dx serve --example canvas --platform web --features web
```

## Browser regression checks

The Rust tests cover pure state, SSR output and geometry helpers. Pointer
capture is browser behavior, so the web path also has Playwright
regressions. The Playwright config follows Dioxus' web testing setup: it
starts `dx serve` as the test web server and reuses an existing local server
outside CI.

```sh
npm install
npm run test:web
```

The spec checks that the sortable overlay matches the source row and
cleans up after drop, and that pointer-driven canvas drops move the node
using the recorded grab offset.

## Feature flags

- `serde`: enables `external::typed::{store, retrieve}`, JSON-typed
  payloads over the native `DataTransfer` (wire-compatible with
  dioxus-html's own `store`/`retrieve`) for drags that must cross app or
  window boundaries.
- `web`: enables native **pointer capture** (via `web-sys`, pinned to the
  version `dioxus-web` uses) so mouse pointer-drags stay glued to the drag
  source even when the cursor leaves it - capture-solid dragging that keeps
  your styled `DragOverlay`. Off by default: the core stays dependency-free,
  and mouse pointer-drag falls back to a best-effort reconciliation (see
  [Platform notes](#platform-notes)). Enable it for web builds:
  `dx serve --example tailwind --platform web --features web`, or
  `features = ["web"]` in your `Cargo.toml`. Touch and pen never need it.

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
- **Mouse pointer drags** (`input: DragInputMode::Pointer` on a mouse).
  Dioxus 0.8 exposes no pointer-capture API, so the behavior depends on the
  `web` feature:
  - **With `web`** (recommended for web): the crate grabs real pointer
    capture on press, so the drag stays glued to the source no matter where
    the cursor goes - release anywhere commits the drop. This is the robust
    path and keeps your custom `DragOverlay`.
  - **Without it** (dependency-free default): a capture-free fallback -
    straying off the drag surface no longer cancels the drag, and a mouse
    released outside is reconciled when the cursor returns (via the
    held-button state). Best-effort; a release that never returns won't
    commit. Use `DragInputMode::Hybrid`/`Native` when mouse should take the
    browser-managed native HTML5 path instead.

  Touch and pen are unaffected either way - the browser implicitly captures
  them.

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

# dioxus-dnd API Audit

This audit reflects the simplified API direction: native browser drag is used
only for app-boundary `DataTransfer` work, while in-app drag/drop uses typed
Rust payloads through Dioxus context, pointer events, and keyboard controls.

## Core Rule

There are two drag/drop worlds:

- **Inside the app:** use `DndProvider<T>`, `Draggable<T>`, `DropZone<T>`, and
  the pattern helpers. Payloads are typed Rust values in context. No
  `DataTransfer`, no native/hybrid mode choice.
- **Across the browser/OS boundary:** use `FileDropZone`, `ExternalDropZone`,
  `ExternalDragSource`, and optional `external::typed`. These APIs use native
  drag events because files, outside text/links/HTML, drag-out, and
  cross-window transport require `DataTransfer`.

That gives users one simple decision: "am I dragging inside my app, or across
the app boundary?"

## Package and Features

- Crate: `dioxus-dnd`
- Version: `1.0.0`
- Rust: `1.85+`
- Dioxus: `0.8.0-alpha.0`
- Default features: none
- `web`: enables real pointer capture through `web-sys::Element` for robust
  mouse pointer drags on web.
- `serde`: enables `external::typed::{store, retrieve}` for JSON
  `DataTransfer` payloads between windows/apps.

## Public Modules

| Module | Public role |
| --- | --- |
| `core` | Provider, typed drag state, app `Draggable`, app `DropZone`, overlay, registry, hooks, geometry, modifiers, viewport helpers, gesture machine. |
| `sortable` | Self-contained sortable list. Emits `SortEvent`; no provider required. |
| `grid` | Self-contained sortable grid. Emits `SortEvent`; no provider required. |
| `board` | Kanban/cross-container moves with `BoardItem`, `BoardColumn`, and `BoardSlot`. |
| `tree` | Hierarchical node drops with before/after/into intent. |
| `canvas` | Free-position typed drops with grab offset, snapping, and bounds. |
| `multiselect` | Selection state and dragging `Vec<K>` as one typed payload. |
| `files` | Native OS file drops. |
| `external` | Native external text/link/HTML/file drops plus typed `DataTransfer` helpers. |
| `dragout` | Native drag-out sources. |
| `autoscroll` | Scroll containers near edges during app or boundary drags. |
| `a11y` | Live announcements and no-drag reorder buttons. |
| `animate` | Experimental FLIP animation wrapper. |

## Prelude

`dioxus_dnd::prelude::*` re-exports the intended public surface:

- Core: `DndProvider`, `Draggable`, `DropZone`, `DragOverlay`, `DndContext`,
  `DragState`, `ZoneRegistry`, `ZoneRecord`, `ZoneId`, `DragId`, `Point`,
  `Rect`, `DropEffect`, `DragMode`, `DropOutcome`, hooks and helpers.
- Patterns: `SortableList`, `SortableGrid`, `BoardItem`, `BoardColumn`,
  `BoardSlot`, `TreeNodeTarget`, `CanvasDropZone`, `SelectableDraggable`.
- Boundary APIs: `FileDropZone`, `ExternalDropZone`, `ExternalDragSource`.
- Utilities: model helpers, modifiers, viewport helpers, gesture machine,
  auto-scroll, a11y, FLIP animation.

## Core In-App API

### `DndProvider<T>`

Provides `DndContext<T>` and `ZoneRegistry<T>` to children.

```rust
DndProvider::<Card> {
    Draggable::<Card> { payload: card, "Drag me" }
    DropZone::<Card> { on_drop: move |outcome| { /* update state */ } }
}
```

Use one provider per payload type/scope.

### `Draggable<T>`

The normal app drag source. It is pointer-first and keyboard-capable.

Props:

- `payload: T`
- `zone: Option<ZoneId> = None`
- `effect: DropEffect = Move`
- `disabled: bool = false`
- `threshold: f64 = 8.0`
- `label: Option<String> = None`
- `on_drag_start: Option<EventHandler<()>>`
- `on_drag_end: Option<EventHandler<bool>>`
- forwarded `div` attributes
- children

Behavior:

- Starts pointer drags after `threshold` pixels.
- Uses `touch-action: none` on the wrapper and merges caller `style` after it.
- Uses real pointer capture when the `web` feature is enabled.
- Re-measures zones on drag start and retries close misses with
  `ZoneRegistry::hit_test_closest`.
- Supports keyboard pickup/drop with Space/Enter, arrow-key zone navigation,
  and Escape cancel.
- Emits `data-dragging` while this payload is in flight.
- Emits `data-disabled` when disabled.
- Does **not** render native HTML `draggable` attributes.

### `DropZone<T>`

Typed in-app drop target.

Props:

- `id: Option<ZoneId> = None`
- `label: Option<String> = None`
- `accepts: Option<Callback<T, bool>> = None`
- `on_drop: EventHandler<DropOutcome<T>>`
- forwarded `div` attributes
- children

Behavior:

- Registers with the zone registry.
- Supports nested zone parent discovery.
- Emits `data-active` while a compatible payload is in flight.
- Emits `data-over` while the compatible payload is over this zone.
- Does not handle native browser drops; use `ExternalDropZone`/`FileDropZone`
  for boundary data.

### `DropOutcome<T>`

Fields:

- `payload: T`
- `from: Option<ZoneId>`
- `to: ZoneId`
- `effect: DropEffect`
- `mode: DragMode`
- `client: Point`
- `element: Point`
- `grab: Point`

Use `effect` for copy/move/link semantics and `grab` for exact placement on
canvas-like surfaces.

### Core Types

- `ZoneId(pub u64)`: drop target identity. `ZoneId::auto()` starts above
  `2^32` so explicit low ids do not collide.
- `DragId(pub u64)`: utility draggable id.
- `Point { x, y }`: CSS-pixel point with `Add`/`Sub`.
- `Rect { x, y, width, height }`: client rect with `contains`, `center`,
  `origin`.
- `DropEffect`: `Move`, `Copy`, `Link`, `None`.
- `DragMode`: `Pointer` or `Keyboard`.

`effective_effect(base, modifiers)` applies the standard convention:
Ctrl/Cmd means copy, Alt means link, and `None` stays disabled.

## Registry and Custom Integrations

`ZoneRegistry<T>` is public for custom interactions:

- `register(record)`
- `unregister(id)`
- `sync_label(id, label)`
- `get(id)`
- `acceptable(payload)`
- `step_zone(current, payload, step)`
- `parent_of(id)`
- `children_of(parent, payload)`
- `step_sibling(current, payload, step)`
- `first_child(id, payload)`
- `hit_test(point)`
- `hit_test_closest(point, payload, max_distance)`
- `measure_all().await`
- `refresh_rects()`

Registry callbacks are registered once, so components that register callbacks
must read current props through signals or re-register. Current examples:

- `TreeNodeTarget` mirrors `label`, `accepts`, `row_height`, `on_drop`, `node`.
- `BoardSlot` mirrors `column`, `index`, `on_move`.
- `CanvasDropZone` mirrors `snap`, `bounds`, `keyboard`.

## Sortable APIs

### `SortableList`

Self-contained list reordering. No provider required.

Props:

- `len`
- `render`
- `on_sort`
- `axis: Axis = Vertical`
- `live_preview: bool = true`
- `transition_ms: u32 = 160`
- `overlay: Option<Callback<usize, Element>>`
- `touch_handle: bool = false`
- `handle: Option<Callback<usize, Element>>`
- forwarded `div` attributes

Emits `SortEvent { from, to }`. Use `apply_sort(&mut items, ev)` for standard
remove/insert behavior.

Styling attributes:

- `data-dragging`
- `data-drop-target`
- `[data-sort-handle]` when `touch_handle` is enabled

### `SortableGrid`

Self-contained grid reorder/swap. No provider required.

Props:

- `len`
- `cols`
- `render`
- `on_sort`
- `mode: ReorderMode = Insert`
- `item_class: Option<String>`
- forwarded `div` attributes

Use `apply_sort` for insert/reflow grids and `apply_swap` for dashboard-style
swaps. The root emits `data-mode="insert" | "swap"`.

## Board APIs

Use `DndProvider::<BoardPayload<T>>`.

- `BoardPayload<T> { item, from, index }`
- `MoveEvent<T> { item, from: (ContainerId, usize), to: (ContainerId, Option<usize>) }`
- `ContainerId = ZoneId`
- `apply_move(board, mv)`

Components:

- `BoardItem<T> { item, column, index, label?, ... }`
- `BoardColumn<T> { id, label?, accepts?, on_move, ... }`
- `BoardSlot<T> { column, index, label?, on_move, ... }`

`BoardColumn` appends. `BoardSlot` inserts at an exact index and inherits the
column acceptance filter.

## Tree APIs

- `NodeId(pub u64)`
- `DropIntent::{Before, After, Into}`
- `TreeDropEvent<T> { payload, target, intent }`
- `intent_from_offset(y, row_height)`
- `would_create_cycle(parent_of, dragged, target)`

`TreeNodeTarget<T>` props:

- `node`
- `row_height: f64 = 28.0`
- `accepts: Option<Callback<(T, DropIntent), bool>>`
- `on_drop`
- `label`
- forwarded attributes

It emits `data-intent="before" | "after" | "into"` while hovered.

## Canvas APIs

- `CanvasDrop<T> { payload, position, pointer }`
- `SnapGrid(pub f64)`
- `Bounds { width, height }`
- `CanvasKeyboardPlacement::{Center, Origin, Fixed(Point)}`
- `client_to_canvas`
- `canvas_to_client`
- `canvas_position`
- `canvas_keyboard_pointer`

`CanvasDropZone<T>` props:

- `id`
- `snap`
- `bounds`
- `keyboard`
- `label`
- `on_drop`
- forwarded attributes
- children

`position` is the corrected top-left: `pointer - grab`, then optional snap and
bounds. Boundary file/text/link drops onto a canvas should be modeled with
`FileDropZone` or `ExternalDropZone` separately.

## Multi-select APIs

Use `DndProvider::<Vec<K>>`.

`Selection<K>` methods:

- `from_signal`
- `is_selected`
- `select_only`
- `toggle`
- `clear`
- `items`
- `len`
- `is_empty`
- `click`

Components:

- `SelectableDraggable<K> { item, selection, zone?, effect?, label?, ... }`
- `SelectionCount<K>` for `DragOverlay::<Vec<K>>`

Dragging a selected item carries the whole selection. Dragging an unselected
item carries only that item.

## Boundary APIs

### `FileDropZone`

Native OS file drops. No provider required.

- `FileDrop { files, client, element }`
- `FileFilter::new().extensions(...).content_types(...).max_size(...).max_files(...)`
- `FileRejection::{Extension, ContentType, TooLarge, TooMany}`

Security: file filters are UX hints, not trust boundaries. Validate actual
bytes before trusting uploads.

### `ExternalDropZone`

Native outside-in drops. No provider required.

- `ExternalPayload::{Url, Html, Text}`
- `ExternalDrop { payloads, files, client, element }`
- `classify(evt)`

Security: external HTML and URLs are untrusted. Sanitize HTML and validate URL
schemes before rendering or navigating.

### `ExternalDragSource`

Native drag-out source. No provider required.

- `OutboundContent::Text`
- `OutboundContent::Url`
- `OutboundContent::Html`
- `OutboundContent::Custom`
- `OutboundContent::url(url, title)`
- `OutboundContent::entries()`

URL HTML entries escape attribute/text content and omit `href` for active
schemes such as `javascript:`, `data:`, and `vbscript:`.

### `external::typed`

Feature-gated by `serde`:

- `store(evt, value)`
- `retrieve(evt)`

Use only when the browser must carry typed data between windows/apps.

## Auto-scroll

`AutoScroll` props:

- `threshold: f64 = 48.0`
- `speed: f64 = 24.0`
- `axis: ScrollAxis = Y`
- `active: Option<bool>`
- forwarded attributes

`edge_delta(pos, rect, threshold, speed, axis)` is pure and testable.

Works with in-app pointer drags and native boundary drags.

## Accessibility

- `Draggable` is focusable and keyboard operable.
- `LiveRegion<T>` voices announcements from `DndContext<T>`.
- `ReorderButtons` emits the same `SortEvent` as sortable dragging for a
  no-drag reorder fallback.
- Use `label` props on draggables and zones for useful screen-reader output.

Keyboard model:

- Space/Enter picks up.
- Arrow keys navigate registered zones.
- Space/Enter drops.
- Escape cancels.

## Animation

`FlipItem` is experimental. It uses FLIP measurements when `epoch` changes.
Validate it in target browsers because it depends on paint timing.

## Website Page Mapping

| Gallery page | API family |
| --- | --- |
| `reading-list` | `DndProvider`, `Draggable`, `DropZone`, `DragOverlay`, `DropOutcome` |
| `newsletter-builder` | copy/move effects, `effective_effect`, `apply_clone_or_move` |
| `mailbox` | `use_selection`, `SelectableDraggable`, `SelectionCount` |
| `playlist` | `SortableList`, `SortEvent`, `apply_sort` |
| `weekly-focus` | `SortableList`, `ReorderButtons` |
| `photo-album` | `SortableGrid`, `ReorderMode::Insert`, `apply_sort` |
| `podcast-queue` | `AutoScroll`, `touch_handle` |
| `sprint-board` | `BoardItem`, `BoardColumn`, `BoardSlot`, `MoveEvent`, `apply_move` |
| `project-files` | `TreeNodeTarget`, `DropIntent`, `would_create_cycle` |
| `moodboard` | `CanvasDropZone`, `CanvasDrop`, `SnapGrid`, `Bounds`, `Draggable` |
| `shuffle` | `FlipItem` on reorder |
| `menu` | `FlipItem` on filter/layout changes |
| `upload` | `FileDropZone`, `FileFilter`, `FileRejection` |
| `share` | `ExternalDropZone`, `ExternalPayload`, `ExternalDragSource`, `OutboundContent` |

## Sharp Edges

- `Draggable` sets `touch-action: none`; use sortable touch handles for
  scrollable rows.
- `apply_clone_or_move` removes every source item with the same key.
- `SortableGrid::mode` is semantic/styling only; caller chooses `apply_sort`
  or `apply_swap`.
- `Bounds` clamps canvas top-left positions, not full item rectangles unless
  you use `clamp_item`/`clamp_rect`.
- `TreeNodeTarget::row_height` should match rendered row height.
- Enable `web` for the strongest mouse pointer capture behavior on web.


# Changelog

## 2.0.0 - 2026-07-08

The pointer-first release. In-app drag and drop now runs entirely on pointer
events and keyboard input with typed Rust payloads; native browser drag is
reserved for the app boundary, where `DataTransfer` is the only transport.
Styling is Tailwind-ready through presence-based data attributes, and the
project website is a new multi-page gallery that teaches every pattern.

### Breaking: one input model

- In-app components are pointer + keyboard only. Removed `PointerDraggable`,
  `DragInputMode`, `DragMode::Native`, `Draggable::native`, the `input`
  props on sortable/grid/board/multiselect components, and every in-app
  native `ondrag*` branch (`DropZone`, `SortableList`, `SortableGrid`,
  `BoardSlot`, `TreeNodeTarget`, `CanvasDropZone`). `Draggable` is now the
  one in-app drag source, and there is no mode to choose.
- Native `DataTransfer` remains exactly where the app boundary requires it:
  `FileDropZone`, `ExternalDropZone`, `ExternalDragSource`,
  `external::typed`, and `AutoScroll`'s dragover listener.
- `DropOutcome` gained two fields: `mode: DragMode` (`Pointer` or
  `Keyboard`) and `grab: Point` (the pointer offset inside the element at
  pickup; `element - grab` is the landing top-left). Manual `DropOutcome`
  literals must add both.
- `ZoneId::auto` and `DragId::auto` now start at 2^32 so auto-generated ids
  can never collide with explicit `u32`-range ids and silently replace a
  neighboring zone in the registry (previously dependent on mount order).

### Styling: presence-based data attributes

- Drag state is exposed as attributes that are present while active and
  absent otherwise, so Tailwind variants like `data-dragging:opacity-50`
  and CSS `[data-over]` selectors match only active elements: `data-over` +
  `data-active` on `DropZone`, `FileDropZone` and `ExternalDropZone`,
  `data-dragging` + `data-disabled` on `Draggable`, `data-dragging` +
  `data-drop-target` on sortable/grid items, `data-active` + `data-over` on
  `BoardSlot`, `data-selected` on `SelectableDraggable`, `data-intent` on
  `TreeNodeTarget`, `data-active` on `CanvasDropZone`, and
  `data-sort-handle` on touch grips.
- Forwarded `style` props merge after functional inline styles instead of
  replacing them, so `touch-action`, overlay positioning and the grid's
  `display: grid` survive while your declarations win per property.
- New `item_class` prop on `SortableGrid` for the library-rendered tile
  wrappers; `DragOverlay` forwards `class` and friends to the ghost.

### Pointer path

- New `web` feature: real pointer capture via `web-sys`, so a mouse drag
  stays glued to its source when the cursor leaves it. Off by default; the
  core stays dependency-free with a held-button recovery fallback, and a
  formal gesture state machine (`transition`, `GesturePhase`,
  `GestureEvent`, `GestureEffect`) drives every drag.
- New `SortableList::overlay` prop: a fixed-position ghost sized from the
  measured source row while the in-flow row becomes the live gap.
- Live preview translates rows by the measured slot pitch (margins and gaps
  included) so spacing never squashes mid-drag; hover hit-tests client
  coordinates against rects measured at drag start.
- A release outside a list or grid commits no reorder, and a drop over a
  rejecting zone falls through to an accepting zone stacked beneath it.
  Missed drops re-measure zones and retry within 48px for touch.
- `AutoScroll` follows default mouse pointer drags via held-button state,
  never scrolls on passive hover, stops when the pointer leaves the
  container, and takes an optional `active` gate.
- `Ctrl`/`Cmd` at release resolves pointer drops to `Copy` and `Alt` to
  `Link` (`effective_effect` is public); keyboard drops report the selected
  zone's center in `DropOutcome::element` and honor
  `CanvasKeyboardPlacement` (`Center`, `Origin`, `Fixed`).
- `BoardSlot` registers as a real zone (pointer, touch and keyboard can
  target it) and `apply_move` adjusts same-column forward insertions.
- `TreeNodeTarget`, `BoardSlot` and `CanvasDropZone` mirror changing props
  through signals so their one-time registry callbacks always read current
  values.
- `FileFilter::content_types` gained a real MIME matcher (exact types,
  `type/*`, `*/*`, structured `*+json` suffixes, parameters and case
  ignored, malformed patterns rejected), and `extensions` normalizes dots,
  case and whitespace.

### The gallery website

- `examples/gallery/` replaces the showcase, tailwind and canvas examples:
  a multi-page site with one page per pattern (fourteen in all), each
  pairing a live demo with a numbered walkthrough, an annotated snippet, a
  "New to Dioxus?" callout and a real API reference. Deployed to GitHub
  Pages by CI.
- The design follows the KI-U paper system: paper and ink scales, one
  forest-green accent, earth tones used sparingly, Poppins for UI text and
  Geist Mono for code, no glow shadows. Code panels are inverse-ink with a
  homegrown Rust/rsx highlighter (no JavaScript highlighter).
- The landing page hero is itself a demo: draggable paper scraps on a
  `CanvasDropZone`.
- Removed `examples/showcase.rs`, `examples/tailwind.rs` and
  `examples/canvas.rs`. The Playwright suite now drives headless fixtures
  in `examples/regressions.rs` on a single dev server, covering overlay
  geometry and cleanup, outside-release cancellation for lists and grids,
  autoscroll edge behavior, canvas grab-offset placement, drop
  fall-through, the Ctrl-drag copy convention, reorder buttons inside
  sortable rows, and the native boundary paths.

## 1.0.0

Stable release. Carries the full feature set, the model helper APIs,
the docs.rs example cleanup, and the public repository metadata.



- Universal touch. Every interaction pattern now works with fingers and pens in every browser, not just the ones whose native HTML5 drag happens to support touch.
- `BoardItem` and `SelectableDraggable` wrap `PointerDraggable`, so board cards and multi-select boxes respond to touch, pen, mouse and keyboard out of the box.
- `SortableList` and `SortableGrid` run a pointer-event gesture path alongside the native one, driven by the same gesture state machine as `PointerDraggable` and hit-tested against per-row rects measured at drag start (stable pre-displacement slots, so the preview never oscillates).
- New `SortableList` prop `touch_handle`: confines touch drags to a leading grip (`[data-sort-handle]`) so rows inside scrollable lists keep finger-scrolling. Default remains whole-row, which sets `touch-action: none` on rows.
- New pure function `sortable::pointer_target` (the hit-test plus midpoint hysteresis logic), unit tested.
- New model helpers `core::apply_clone_or_move` and `core::apply_list_clone_or_move`: apply move-vs-copy drops to `HashMap<ZoneId, Vec<T>>` zone models or two plain `Vec<T>` lists, with key-based source removal and a clone hook for assigning fresh ids on copy.
- `TreeNodeTarget` registers itself in the zone registry: tree rows are now reachable by touch hit-testing and keyboard navigation. Keyboard drops land with `Into` intent. New `label` prop feeds screen-reader announcements. Touch hovers show the same live `data-intent` bands as mouse hovers.
- Pointer paths listen to `lostpointercapture` as a cancel signal, so a drag aborts cleanly if the browser revokes capture mid-gesture.
- **Fixed `sortable::displacement` leaving a phantom gap**: the source row
  now translates toward its landing slot while neighbors make room -
  previously the shifted neighbors overlapped the source (which still
  occupies its slot during a native drag), leaving a gap at the target.
  Offsets now conserve slot occupancy (they sum to zero).
- Showcase rework: drops are visibly stateful everywhere (crates land in
  bays and can be dragged onward, keyboard moves render, the tree really
  restructures, received links and loaded boxes stay visible), the
  FLIP-plus-displacement double animation is gone, and the layout gains a
  white workbench surface so the taupe reads as the floor around it.
- **Formal gesture state machine** (`core::machine`): the pointer drag
  lifecycle (Idle → Pressed → Dragging, threshold promotion, tap vs drop,
  foreign-pointer rejection, cancellation) is now a pure, exhaustively
  tested `transition` function. `PointerDraggable` drives it; you can too,
  for custom pointer interactions.
- **Nested keyboard traversal**: `DropZone`s nested inside other
  `DropZone`s discover their parents automatically (via context - zero
  configuration). Keyboard drags follow the WAI-ARIA tree convention:
  Up/Down cycle siblings at the current level, Right descends into a
  zone's children, Left ascends - falling back to plain next/previous in
  flat apps. Announcements gain nesting context ("Over Column 2, inside
  Board A."). Registry API: `ZoneRecord::parent`, `parent_of`,
  `children_of`, `step_sibling`, `first_child`.
- **Live drop preview** in `SortableList`: rows translate out of the way so
  a gap opens where the drop would land, with midpoint hysteresis to keep
  the gap from oscillating. On by default; `live_preview: false` restores
  the highlight-only behavior. Pure math exposed as `sortable::displacement`.
- **Forgiving touch drops**: `PointerDraggable` now re-measures zone rects
  on a missed drop (stale after mid-drag scroll/resize) and retries with a
  closest-center fallback (`ZoneRegistry::hit_test_closest`,
  `ZoneRegistry::measure_all`).
- **Composable drag modifiers** (`core::modifiers`): `LockAxis`, `Snap`,
  `KeepInside`, chained with `apply_modifiers` - the generalized form of
  canvas snapping, usable in custom interactions.
- **`a11y::ReorderButtons`**: headless move-up/move-down buttons emitting
  the same `SortEvent` as dragging - reordering with no drag gesture at
  all, the most robust accessibility fallback.
- `dragout` - `ExternalDragSource` / `OutboundContent`: drag text, links and
  HTML *out* of your app into other tabs and applications.
- `grid` - `SortableGrid`: 2D tile reorder (insert-and-reflow) or swap
  (dashboard) over CSS grid, plus `cell_of`/`index_of` coordinate helpers.
- `multiselect` - `use_selection`, `SelectableDraggable`, `SelectionCount`:
  select several items (click / Ctrl+click) and drag them as one
  `Vec<K>` payload.
- `animate` *(experimental)* - `FlipItem`: FLIP glide transitions on
  reorder, driven by an epoch counter.
- Modifier-key drop effects: Ctrl/Cmd forces Copy, Alt forces Link
  (file-manager convention), reflected in `dropEffect` feedback and
  `DropOutcome::effect`. Pure helper: `effective_effect`.
- Keyboard zone navigation is now **spatial** (top-to-bottom,
  left-to-right by measured rects) instead of registration order.
- Nested drag scopes: `Draggable`, `SortableList` and `SortableGrid`
  stop propagation on drag start/drop, so sortables inside sortables
  (and boards inside boards) each own their gestures.
- `DropZone` labels re-sync when the prop changes across renders.
- `sortable`: `ReorderMode` (`Insert`/`Swap`) and `apply_swap`.
- Ported to Dioxus `0.8.0-alpha.0` (also compiles on 0.7.9).
- State moved from `Signal<DragState<T>>` to `Store<DragState<T>>` for
  per-field reactivity: reading `dnd.over()` in render no longer reruns on
  pointer moves; `enter`/`leave`/`update_pointer` write through field lenses.
- Zone registry (`use_zone_registry`): every `DropZone` registers id, label,
  drop callback, acceptance filter and mounted element.
- Keyboard accessibility built into `Draggable` (Space/Enter pick up and
  drop, arrows choose a zone, Escape cancels) with `a11y::LiveRegion`
  screen-reader announcements.
- Touch/pen support: `pointer::PointerDraggable` (native HTML5 path for
  mouse, pointer-event hit-testing for touch).
- `autoscroll::AutoScroll`: edge-scrolling containers during drags, pure
  `MountedData` (no JS eval).
- `Core` (`DndProvider`, `Draggable`,
  `DropZone`, `DragOverlay`, store-context payload transport), `files`,
  `sortable`, `board`, `tree`, `canvas`, `external` modules; optional
  `serde` feature for typed `DataTransfer` interop.

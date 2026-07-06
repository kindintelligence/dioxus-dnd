# Changelog

## Unreleased

Tailwind-ready styling pass. The library stays headless; drag state is now
uniformly exposed as **presence-based data attributes** - present while
active, *absent* otherwise (previously boolean attributes rendered as
`="false"` when inactive) - so Tailwind variants like
`data-dragging:opacity-50` and CSS `[data-dragging]` match only active
elements. Existing `[data-…="true"]` selectors keep working.

- New state attributes: `data-over` + `data-active` on `DropZone`
  (highlight the hovered zone / reveal targets while a compatible drag is
  in flight - for pointer, touch and keyboard drags alike), `data-over` on
  `FileDropZone` and `ExternalDropZone` (no more `on_hover` wiring for the
  classic highlight), `data-dragging` + `data-disabled` on `Draggable` and
  `PointerDraggable`, `data-active` on `CanvasDropZone`.
- New input policy: `DragInputMode::{Pointer, Native, Hybrid}`. Core
  `Draggable` now has `native: false` for keyboard-only/native-off wrappers,
  and `PointerDraggable` has an `input` prop for choosing native, pointer, or
  compatibility behavior.
- `SortableList` and `SortableGrid` now default to pointer events for mouse,
  touch and pen, avoiding the browser's native drag image during reorders.
  Set `input: DragInputMode::Native` or `Hybrid` to opt back into HTML5 drag.
- Now presence-based: `data-dragging` / `data-drop-target` on
  `SortableList` and `SortableGrid` items, `data-active` on `BoardSlot`,
  `data-selected` on `SelectableDraggable`, `data-intent` on
  `TreeNodeTarget` (absent instead of `=""` when not hovered).
- New `item_class` prop on `SortableGrid`: classes for the
  library-rendered tile wrappers - the elements that carry `data-dragging`
  / `data-drop-target`. `SortableList` row wrappers are styled from the
  list root with child selectors such as `[&>[data-drop-target]]:...`.
- `DragOverlay` now forwards attributes (`class`, …) to its wrapper, so
  the drag ghost styles directly (`class: "rotate-3 scale-105 shadow-xl"`).
- Forwarded `style` props are now **merged after** functional inline
  styles instead of replacing them: `touch-action: none` on
  `PointerDraggable` (previously any user `style` silently broke touch
  dragging), positioning on `DragOverlay`, and the `display: grid` layout
  on `SortableGrid` (per-property overrides like custom
  `grid-template-columns` win; previously SSR emitted two conflicting
  `style` attributes).
- **Fixed native-drag hover misfiring when rows/rows' content have child
  elements**: `SortableList`'s midpoint test and `TreeNodeTarget`'s intent
  bands used drag-event element offsets, which browsers report relative to
  the *child under the cursor* (a grip icon, a text div) - so a neighbor
  row could light up the moment the pointer grazed its edge, and tree
  drops could land with a different intent than the hovered band showed.
  Both now hit-test client coordinates against rects measured at drag
  start, matching the pointer path (the sortable also re-measures at native
  drag start, so it's no longer stale after scrolling).
- **Fixed the live preview squashing rows together when styling adds
  margins/gaps between them**: displaced rows translated by the bare row
  size, overlapping by the margin mid-drag. The step is now the measured
  slot pitch (distance between consecutive row origins, which includes
  margin/gap).
- New `examples/tailwind.rs`: cards, sortable list with a touch grip, tree
  intents and file drop, styled entirely with utility classes.
- README: new "Styling (Tailwind-ready)" section with the full attribute
  table, ghost styling, and `group-data-*` / `in-data-*` recipes for
  styling children of state-carrying wrappers.
- New **`web` feature**: native pointer capture via `web-sys` (pinned to the
  version `dioxus-web` uses) for `SortableList`, `SortableGrid` and
  `PointerDraggable`. On press the drag source grabs the pointer, so a mouse
  reorder/drag stays glued to it even when the cursor leaves - release
  anywhere commits. Off by default; the core stays dependency-free. All
  `web-sys` is isolated to one feature-gated `core::platform` module.
- **Fixed mouse pointer-drags aborting when the cursor left the drag
  surface.** `SortableList` moved its move/up handling to the container and
  cancelled on `pointerleave`; without pointer capture (which Dioxus 0.8
  does not expose) a mouse straying off the list silently dropped the drag.
  The eager `pointerleave` cancel is gone, and when the `web` feature is off
  a capture-free fallback reconciles a mouse released outside via its
  held-button state. `PointerDraggable` shares the same recovery.
- Sortable example fix: the touch grip now carries the `cursor-grab`
  affordance instead of the whole row advertising as draggable.
- The pointer/`web` drag path now reaches every in-app component:
  `BoardItem` and `SelectableDraggable` gained an `input: DragInputMode`
  prop (forwarded to their inner `PointerDraggable`), and `CanvasDropZone`
  now registers as a drop zone so `PointerDraggable` (touch, pen, and mouse
  under `web`) can drop onto it - previously canvas accepted native HTML5
  drags only. Native and pointer drops place the element identically.
- `DropOutcome` gained a `grab` field (pointer offset within the dragged
  element at pickup); `element - grab` is the element's landing top-left.
  `PointerDraggable` now records the real grab offset (from the press point
  within the element) instead of the bare threshold travel, so the
  `DragOverlay` ghost is held at the grab point - matching the native path -
  and canvas drops land exactly.
- New `SortableList::overlay` prop: callers can render a lightweight,
  fixed-position ghost that follows the pointer while the in-flow source row
  becomes the live gap. The overlay wrapper is sized from the measured source
  row, so a headless overlay can look exactly like the original row. Native
  HTML5 drags keep the browser drag image unchanged.
- **Fixed a `SortableGrid` mouse-drag hang** in the dependency-free build: a
  mouse released outside the grid left the drag stuck forever (the grid
  lacked the held-button recovery `SortableList` already had). The grid now
  recovers on pointer re-entry, and its container gates pointer input by
  type like the list does.
- The sortable touch grip no longer hard-codes `cursor`/width inline, so
  `[data-sort-handle]` classes (cursor, size, colour) actually apply; only
  functional styles (`touch-action`, `user-select`, centring) stay inline.
- `CanvasDropZone` now reads `snap`/`bounds` through signals, so runtime
  changes to them apply to pointer and keyboard drops, not just native
  mouse drops.

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

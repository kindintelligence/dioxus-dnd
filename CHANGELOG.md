# Changelog

## 0.6.0

Universal touch. Every interaction pattern now works with fingers and pens in every browser, not just the ones whose native HTML5 drag happens to support touch.

- `BoardItem` and `SelectableDraggable` wrap `PointerDraggable`, so board cards and multi-select boxes respond to touch, pen, mouse and keyboard out of the box.
- `SortableList` and `SortableGrid` run a pointer-event gesture path alongside the native one, driven by the same gesture state machine as `PointerDraggable` and hit-tested against per-row rects measured at drag start (stable pre-displacement slots, so the preview never oscillates).
- New `SortableList` prop `touch_handle`: confines touch drags to a leading grip (`[data-sort-handle]`) so rows inside scrollable lists keep finger-scrolling. Default remains whole-row, which sets `touch-action: none` on rows.
- New pure function `sortable::pointer_target` (the hit-test plus midpoint hysteresis logic), unit tested.
- New model helpers `core::apply_clone_or_move` and `core::apply_list_clone_or_move`: apply move-vs-copy drops to `HashMap<ZoneId, Vec<T>>` zone models or two plain `Vec<T>` lists, with key-based source removal and a clone hook for assigning fresh ids on copy.
- `TreeNodeTarget` registers itself in the zone registry: tree rows are now reachable by touch hit-testing and keyboard navigation. Keyboard drops land with `Into` intent. New `label` prop feeds screen-reader announcements. Touch hovers show the same live `data-intent` bands as mouse hovers.
- Pointer paths listen to `lostpointercapture` as a cancel signal, so a drag aborts cleanly if the browser revokes capture mid-gesture.

No breaking changes: all new props default to prior behavior, and native HTML5 drag paths are untouched.

## 0.5.1

Fixes from runtime testing of the showcase:

- **Fixed `sortable::displacement` leaving a phantom gap**: the source row
  now translates toward its landing slot while neighbors make room —
  previously the shifted neighbors overlapped the source (which still
  occupies its slot during a native drag), leaving a gap at the target.
  Offsets now conserve slot occupancy (they sum to zero).
- Showcase rework: drops are visibly stateful everywhere (crates land in
  bays and can be dragged onward, keyboard moves render, the tree really
  restructures, received links and loaded boxes stay visible), the
  FLIP-plus-displacement double animation is gone, and the layout gains a
  white workbench surface so the taupe reads as the floor around it.

## 0.5.0

The rigor release — the last two roadmap items from the ecosystem
comparison:

- **Formal gesture state machine** (`core::machine`): the pointer drag
  lifecycle (Idle → Pressed → Dragging, threshold promotion, tap vs drop,
  foreign-pointer rejection, cancellation) is now a pure, exhaustively
  tested `transition` function. `PointerDraggable` drives it; you can too,
  for custom pointer interactions.
- **Nested keyboard traversal**: `DropZone`s nested inside other
  `DropZone`s discover their parents automatically (via context — zero
  configuration). Keyboard drags follow the WAI-ARIA tree convention:
  Up/Down cycle siblings at the current level, Right descends into a
  zone's children, Left ascends — falling back to plain next/previous in
  flat apps. Announcements gain nesting context ("Over Column 2, inside
  Board A."). Registry API: `ZoneRecord::parent`, `parent_of`,
  `children_of`, `step_sibling`, `first_child`.


## 0.4.0

The "feel" release, informed by a source read of the other Dioxus dnd
crates (dioxus-dnd-kit, taino-dnd, dioxus-nox-dnd):

- **Live drop preview** in `SortableList`: rows translate out of the way so
  a gap opens where the drop would land, with midpoint hysteresis to keep
  the gap from oscillating. On by default; `live_preview: false` restores
  the highlight-only behavior. Pure math exposed as `sortable::displacement`.
- **Forgiving touch drops**: `PointerDraggable` now re-measures zone rects
  on a missed drop (stale after mid-drag scroll/resize) and retries with a
  closest-center fallback (`ZoneRegistry::hit_test_closest`,
  `ZoneRegistry::measure_all`).
- **Composable drag modifiers** (`core::modifiers`): `LockAxis`, `Snap`,
  `KeepInside`, chained with `apply_modifiers` — the generalized form of
  canvas snapping, usable in custom interactions.
- **`a11y::ReorderButtons`**: headless move-up/move-down buttons emitting
  the same `SortEvent` as dragging — reordering with no drag gesture at
  all, the most robust accessibility fallback.


## 0.3.0

New drop patterns:
- `dragout` — `ExternalDragSource` / `OutboundContent`: drag text, links and
  HTML *out* of your app into other tabs and applications.
- `grid` — `SortableGrid`: 2D tile reorder (insert-and-reflow) or swap
  (dashboard) over CSS grid, plus `cell_of`/`index_of` coordinate helpers.
- `multiselect` — `use_selection`, `SelectableDraggable`, `SelectionCount`:
  select several items (click / Ctrl+click) and drag them as one
  `Vec<K>` payload.
- `animate` *(experimental)* — `FlipItem`: FLIP glide transitions on
  reorder, driven by an epoch counter.

Core improvements:
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

## 0.2.0

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

## 0.1.0

- Initial release against Dioxus 0.7: `core` (`DndProvider`, `Draggable`,
  `DropZone`, `DragOverlay`, store-context payload transport), `files`,
  `sortable`, `board`, `tree`, `canvas`, `external` modules; optional
  `serde` feature for typed `DataTransfer` interop.

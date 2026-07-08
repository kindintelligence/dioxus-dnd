# Repository Audit

Date: 2026-07-07
Branch: `tailwind-support`

Scope: whole repository audit after the canvas keyboard placement work. This
file records findings and proposed fixes only. It is not a commit plan.

## Verification Run During Audit

- `cargo check --examples --features web`: passed
- Agent verification: `cargo test --all-features`: passed
- Agent verification: `cargo test --no-default-features`: passed
- Working tree was clean before this audit file was written.

## Fix Order

1. Done and pushed: fix `BoardSlot` so pointer and keyboard board insertion
   work with the new default input model.
2. Done and pushed: fix `apply_move` same-column forward insertion.
3. Done and pushed: fix `AutoScroll` for default mouse pointer drags.
4. Done and pushed: sync dynamic `TreeNodeTarget` registration props.
5. Tighten low-level docs and MIME wildcard behavior.
6. Partly done in working tree: update runtime and browser coverage.
7. Clean up root and canvas roadmaps.
8. Add or explicitly defer agent support docs.

## Findings

### High: `BoardSlot` Is Native-Only While `BoardItem` Defaults To Pointer

Files:

- `src/board.rs`
- `README.md`

Status:

- Done and pushed in `2048b07 Fix board slot drops`.
- `BoardSlot` now registers as a zone with `use_zone_id`.
- Runtime coverage proves board slots are registered under their parent column
  and receive pointer and keyboard `DropOutcome` callbacks.
- Not done: browser coverage for dragging a default pointer board card onto an
  exact insertion slot.

Current state:

- `BoardItem` wraps `PointerDraggable` and defaults to `DragInputMode::Pointer`.
- `BoardSlot` only handles native HTML drag events with `ondragover` and
  `ondrop`.
- `BoardSlot` never registers as a `ZoneRecord`, so the shared pointer and
  keyboard drop paths cannot target precise insertion slots.

Impact:

- Default board card drags can append to `BoardColumn`, but cannot reliably
  drop on precise `BoardSlot` insertion points.
- Keyboard users cannot target board slots.
- README says context-backed state attributes include `BoardSlot`, which
  implies it participates in pointer and keyboard drags.
- This is visible in shipped examples that render `BoardSlot`, including
  `examples/kanban.rs` and the board flow in `examples/showcase.rs`.

Proposed fix:

- Make `BoardSlot` register as a zone, like `DropZone` and `CanvasDropZone`.
- Give each slot a stable zone id via `use_zone_id` unless an explicit id is
  added later.
- Register an `on_drop` callback that turns `DropOutcome<BoardPayload<T>>`
  into `MoveEvent { to: (column, Some(index)) }`.
- Keep the native `ondrop` handler for `Draggable` / native HTML paths.
- Mirror dynamic props through signals if the registered callback captures
  `column`, `index`, or `on_move`.
- Add runtime coverage proving `BoardSlot` registers and receives synthetic
  `DropOutcome` callbacks.
- Add browser coverage or extend the kanban/gallery flow to verify a default
  pointer board drag can hit an insertion slot.

### Medium: `apply_move` Same-Column Forward Moves Insert Too Far

File:

- `src/board.rs`

Status:

- Done and pushed in `2048b07 Fix board slot drops`.
- Unit coverage now covers same-column forward move, backward move and append.
- Not done: out-of-range source fallback test from the original proposed list.

Current state:

- `apply_move` removes `from_ix` before inserting at `to_ix`.
- When `from_col == to_col` and `from_ix < to_ix`, the target index should be
  adjusted after removal.

Impact:

- Moving item `0` to slot `2` in the same column lands after the intended
  slot.
- Existing tests cover cross-column movement but not same-column forward
  insertion.

Proposed fix:

- If `from_col == to_col`, `from_ix < to_ix`, and the source item was removed,
  insert at `to_ix - 1`.
- Add unit tests for:
  - same-column forward move
  - same-column backward move
  - append into same column
  - out-of-range source fallback behavior

### Medium: `AutoScroll` Ignores Default Mouse Pointer Drags

Files:

- `src/autoscroll.rs`
- `README.md`

Status:

- Done and pushed in `e52b360 Fix autoscroll pointer drags`.
- `AutoScroll` now treats mouse pointer moves with held buttons as active
  pointer drags.
- Passive mouse hover near an edge remains inert.
- `active: Option<bool>` lets callers force or suppress pointer-move scrolling
  when they track drag state themselves.
- Unit and Playwright coverage added. Browser coverage serves `showcase` and
  verifies the sortable autoscroll path.

Current state:

- `AutoScroll` handles native `dragover`.
- It handles pointer moves only when `evt.pointer_type() != "mouse"` and
  `pressure() > 0.0`.
- In-app mouse drags now default to pointer events, so the native `dragover`
  path does not run for the default mouse path.

Impact:

- `AutoScroll` does not work for default mouse pointer drags, despite README
  saying it works for `PointerDraggable` pointer drags.
- Because it does not check `DndContext`, touch or pen contact near an edge can
  also call `scroll_for` even when no crate drag is active.

Proposed fix:

- Read `use_dnd` or accept an explicit active-drag signal so pointer moves
  only scroll while a drag is actually in flight.
- Use `evt.held_buttons()` or equivalent Dioxus pointer state to treat mouse
  pointer moves with a pressed button as active drags.
- Keep touch and pen pressure/contact handling.
- Avoid scrolling on passive mouse hover with no button held.
- Add unit coverage for any extracted predicate.
- Add a browser regression later if practical, because scrolling is browser
  behavior.

### Medium: `TreeNodeTarget` Does Not Sync Dynamic Registration Props

File:

- `src/tree.rs`

Status:

- Done and pushed in `1f32c82 Sync tree target registry props`.
- `TreeNodeTarget` now mirrors `node`, `label`, `accepts`, `row_height` and
  `on_drop` through signals for the one-time registry callback.
- `registry.sync_label` keeps keyboard labels current.
- Runtime coverage proves rerendered registry callbacks use the latest node,
  label, accepts, row height and drop handler.
- Extra tree tests are in the working tree but not committed yet.

Current state:

- `TreeNodeTarget` registers once through `use_hook`.
- The registered callback and filter capture `label`, `accepts`,
  `row_height`, and `on_drop` from first render.
- `DropZone` and `CanvasDropZone` already have patterns for syncing dynamic
  registration state.

Impact:

- Updated labels become stale in keyboard announcements.
- Updated `row_height` can produce stale intent bands.
- Updated `accepts` or `on_drop` can be ignored by the registered callback.

Proposed fix:

- Mirror dynamic props into signals:
  - `label_now`
  - `accepts_now`
  - `row_height_now`
  - `on_drop_now`
- Have the registered callback read current values from those signals.
- Call `registry.sync_label(zone_id, label.clone())` during render.
- If `accepts` needs registry-level filtering, add a `sync_accepts` API or
  re-register safely when it changes.
- Add runtime tests that rerender a tree target and assert the registered
  callback reads current `row_height` / label behavior.

### Medium: Runtime Coverage Does Not Fully Lock Input Defaults

Files:

- `tests/runtime.rs`
- `src/sortable.rs`
- `src/grid.rs`
- `ROADMAP.md`

Status:

- Partly done in working tree.
- Added tree helper unit tests in `src/tree.rs`.
- Added runtime coverage for exact-intent tree acceptance after the registry
  any-intent filter.
- Still open: extend sortable native drag coverage to `Hybrid`.
- Still open: add `SortableGrid` default/native/hybrid runtime assertions.

Current state:

- `PointerDraggable` has default / pointer / native / hybrid native-attribute
  coverage.
- `BoardItem` and `SelectableDraggable` have some wrapper coverage.
- `SortableList` covers default and native, but not hybrid.
- `SortableGrid` has no runtime assertion proving default/native/hybrid
  `draggable` behavior.

Impact:

- The roadmap says tests should lock these defaults down, but coverage is
  incomplete.

Proposed fix:

- Extend `sortable_native_drag_is_opt_in` to include `Hybrid`.
- Add `sortable_grid_native_drag_is_opt_in` covering:
  - default grid: `draggable=false`
  - native grid: `draggable=true`
  - hybrid grid: `draggable=true`
- Keep assertions precise enough that wrapper `draggable` counts do not become
  brittle if examples are added to the same test component.

### Medium: Gallery Omits `LiveRegion` For In-App Providers

File:

- `examples/gallery.rs`

Current state:

- Several `DndProvider`s in the gallery do not render `LiveRegion`.
- README tells users to render one `LiveRegion` per provider for announcements.

Impact:

- Keyboard drag/drop can work silently for screen-reader users in the primary
  gallery example.

Proposed fix:

- Add `LiveRegion::<T> {}` inside each in-app `DndProvider`.
- Keep native-only examples unchanged where no provider exists.
- Add a simple SSR/runtime assertion if there is an example render test later.

### Medium: Browser Smoke Coverage Misses Shipped Examples

Files:

- `playwright.config.js`
- `tests/browser/web-pointer-regressions.spec.js`
- `examples/showcase.rs`
- `examples/kanban.rs`
- `examples/tailwind.rs`

Current state:

- Playwright serves `gallery` and `canvas`.
- It does not smoke `showcase`, `kanban`, or `tailwind`.
- README positions `showcase` as deployable and `kanban` / `canvas` as
  focused examples.

Impact:

- Visual/layout regressions in the website and focused examples can compile
  but not be caught in browser checks.

Proposed fix:

- Add lightweight browser smoke tests for:
  - `showcase`: heading visible and one interactive section rendered
  - `kanban`: board heading and at least one column/card visible
  - `tailwind`: heading and one styled drag surface visible
- Either add web servers for those examples in Playwright config or create a
  separate smoke config to keep the main regression run fast.

### Medium: Canvas Connection Handles Are Too Small

File:

- `examples/canvas.rs`

Current state:

- Connection handles are visible `h-4 w-4` buttons.
- Playwright clicks them by accessible label, so it does not prove practical
  pointer usability.

Impact:

- The handles are hard to hit in real use, especially on touch or high DPI
  screens.

Proposed fix:

- Keep the visible dot small if desired, but make the actual button hit area
  at least `32x32` or `40x40`.
- Use an inner visual dot inside a larger transparent button.
- Add browser assertion for handle bounding box size or use a real coordinate
  click in addition to label click.

### Low: File MIME Wildcard Matching Is Too Loose

File:

- `src/files.rs`

Current state:

- `FileFilter` handles `image/*` by stripping to `image` and checking
  `starts_with`.

Impact:

- Invalid MIME-like strings such as `imageevil/png` match `image/*`.

Proposed fix:

- For wildcard types, match the slash-delimited prefix, for example
  `ct.starts_with("image/")`.
- Add a unit test proving `image/png` passes and `imageevil/png` fails.

### Low: Public External Module Docs Mention Dioxus 0.7

File:

- `src/external.rs`

Current state:

- Module docs mention “Dioxus 0.7's `DataTransfer` bridge”.
- The crate is positioned around Dioxus 0.8.

Impact:

- Public docs look stale.

Proposed fix:

- Reword to “Dioxus HTML's `DataTransfer` bridge” or similar version-neutral
  wording.

### Low: Showcase File Drop Uses Custom Hover State

File:

- `examples/showcase.rs`

Current state:

- Showcase file-drop demo wires `on_hover` into custom `data-hover`.
- Changelog and styling docs say `FileDropZone` now exposes `data-over` for
  this directly.

Impact:

- The public website does not model the recommended contract.

Proposed fix:

- Remove the custom hover signal if it is only styling the drop zone.
- Style via `data-over` instead.
- Keep `on_hover` only if the example needs app state beyond styling.

### Low: Showcase Test Count Copy Is Stale

File:

- `examples/showcase.rs`

Current state:

- Public UI claims “50 tests”.
- Current repo has substantially more Rust tests plus browser specs.

Impact:

- Deployable site copy under-sells and looks stale.

Proposed fix:

- Replace hard-coded count with less brittle wording, such as “tested core”
  or “runtime and browser regressions”.
- If a number is desired, update it during release prep only.

### Low: Canvas Keyboard Placement Browser Test Still Leans On Internals

File:

- `tests/browser/web-pointer-regressions.spec.js`

Current state:

- The test checks marker `left/top` and created nodes' `data-world-*`.
- It does not assert the marker and created nodes are visibly unobscured.

Impact:

- The test can pass even if styling makes the marker hard to see.

Proposed fix:

- Add bounding-box visibility assertions for the preview marker and latest
  created node.
- Consider screenshot or pixel-level checks only if this keeps failing in real
  use.

## Roadmap And Plan Findings

### `.codex/plan/keyboard-placement-policy.md` Is Stale

Current state:

- It is written as an implementation plan for work that is now complete.

Impact:

- It reads like actionable unfinished work even though the acceptance criteria
  are met.

Proposed fix:

- Convert it to a completion record with:
  - implemented API
  - tests run
  - remaining follow-up items
- Or archive/delete it if `.codex/plan` is meant to contain active plans only.

### `.codex-plans/canvas-roadmap.md` Under-Reports Verification

Current state:

- It says browser regression covers selected canvas keyboard geometry.
- Current browser coverage checks `Center`, `Origin`, and `Fixed`.

Proposed fix:

- Update the verification bullet to say browser coverage checks all public
  keyboard placement policies through the focused canvas UI.

### `ROADMAP.md` Mixes Current Baseline And Future Work

Current state:

- Tests, docs, and web-feature sections describe several already-completed
  items as “should”.
- A native interop demo is listed under Later even though gallery/canvas now
  cover file drops, external drops, and drag-out paths.

Proposed fix:

- Split `ROADMAP.md` into:
  - “Current baseline”
  - “Open gaps”
  - “Later”
- Move completed test/docs/web-feature bullets into current baseline.
- Remove or rewrite the native interop demo item as a docs/showcase polish
  item if the remaining work is presentation rather than capability.

### Agent Support Section Is Unimplemented

Current state:

- No `llms.txt`, `llms-full.txt`, prompts folder, or agent-skill docs exist.

Proposed fix:

- Either mark the section as pending explicitly or add a minimal `llms.txt`.
- If adding it, keep it short and point to:
  - `README.md`
  - `ROADMAP.md`
  - `CHANGELOG.md`
  - `examples/gallery.rs`
  - `examples/canvas.rs`
  - `src/core/components.rs`
  - `src/pointer.rs`
  - `src/sortable.rs`
  - `src/board.rs`

## Suggested Work Chunks

### Chunk 1: Board Correctness

- Fix `apply_move` same-column index adjustment.
- Make `BoardSlot` participate in pointer and keyboard drops.
- Add unit/runtime tests.
- Add browser coverage if a board slot is present in an example.

### Chunk 2: Input Defaults Coverage

- Add `SortableList` hybrid runtime coverage.
- Add `SortableGrid` default/native/hybrid runtime coverage.
- Update roadmap test section after coverage lands.

### Chunk 3: AutoScroll

- Support default mouse pointer drags.
- Add predicate/unit coverage.
- Consider browser coverage later.

### Chunk 4: Tree Dynamic Registration

- Sync label and callback-related dynamic props.
- Add runtime rerender tests.

### Chunk 5: Examples And Accessibility

- Add `LiveRegion` to gallery providers.
- Enlarge canvas connection hit targets.
- Update showcase file-drop state styling and stale test count copy.
- Add browser smoke tests for showcase, kanban, and tailwind.

### Chunk 6: Roadmap Cleanup

- Convert stale implementation plan into a completion note or archive it.
- Update canvas roadmap verification bullets.
- Split root roadmap into current baseline and open gaps.

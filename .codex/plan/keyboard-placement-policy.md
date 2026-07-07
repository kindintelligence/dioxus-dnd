# Keyboard Placement Policy Plan

## Goal

Make canvas keyboard placement explicit without weakening the existing
pointer and native drag contracts.

The current keyboard behavior is useful but implicit:

- Core keyboard drops use the selected zone's measured center.
- If the selected zone has no measured rect, core reports origin.
- `CanvasDropZone` receives that as `DropOutcome::element` and treats it like
  any other canvas-relative pointer.

The policy should let apps choose the keyboard placement point for
`CanvasDropZone` while keeping pointer and native drops exact.

## Non-Goals

- Do not add graph, node, edge, port, layout or editor behavior to
  `CanvasDropZone`.
- Do not make `CanvasDropZone` parse `DataTransfer`.
- Do not add pan or zoom state to `CanvasDropZone`.
- Do not add `LastPointer` in the first pass.
- Do not add callback-based placement in the first pass.
- Do not infer keyboard drops from `grab == Point::default()`.

## Current Code Shape

Core keyboard path:

- `Draggable` owns keyboard pickup, arrow navigation, drop and Escape cancel
  in `src/core/components.rs`.
- `keyboard_drop_points(rect)` returns:
  - selected zone center as `client`
  - `center - rect.origin()` as `element`
  - origin for both when there is no rect
- Keyboard drops construct `DropOutcome` with zero `grab`.

Pointer path:

- `PointerDraggable` records the press offset as `grab`.
- Pointer delivery subtracts the target zone origin from the client point and
  writes that as `DropOutcome::element`.

Native path:

- Core `DropZone` uses native event `client` and `element` coordinates.
- `CanvasDropZone` has its own native drop handler for crate-managed native
  drags and uses `element_point(evt)` plus `dnd.grab()`.

Canvas path:

- `CanvasDropZone` registers as a zone and receives `DropOutcome`.
- It calls `place(o.payload, o.element, o.grab)`.
- `place` calls `canvas_position(pointer, grab, snap, bounds)`.
- `CanvasDrop` exposes `pointer` and corrected `position`.

Problem:

- `DropOutcome` does not include `DragMode`.
- `CanvasDropZone` cannot reliably tell keyboard drops from pointer/native
  drops.
- Guessing from zero `grab` is incorrect because pointer/native drops can
  also have zero grab.

## API Design

### Add Mode To `DropOutcome`

Add a field:

```rust
pub struct DropOutcome<T> {
    pub payload: T,
    pub from: Option<ZoneId>,
    pub to: ZoneId,
    pub effect: DropEffect,
    pub mode: DragMode,
    pub client: Point,
    pub element: Point,
    pub grab: Point,
}
```

Rationale:

- The drag mode is already tracked in `DndContext`.
- Consumers other than canvas may also need to distinguish keyboard, pointer
  and native drops.
- It avoids fragile inference from geometry.

Construction rules:

- Keyboard drop: `mode: DragMode::Keyboard`
- Pointer drop: `mode: DragMode::Pointer`
- Native drop through core `DropZone`: `mode: dnd.mode()`
- Manual tests and helper literals must set the field explicitly.

### Add Canvas Keyboard Placement Type

Add to `src/canvas.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CanvasKeyboardPlacement {
    #[default]
    Center,
    Origin,
    Fixed(Point),
}
```

Default:

- `Center`
- This preserves current measured-zone behavior.

Semantics:

- `Center`: use `DropOutcome::element`, which core computes from the selected
  target's measured center.
- `Origin`: use `Point::default()`.
- `Fixed(point)`: use the given canvas-local point.
- Snap and bounds still apply after the keyboard point is selected.
- `grab` remains zero for keyboard drops.

### Add Prop To `CanvasDropZone`

Add:

```rust
#[props(default)]
keyboard: CanvasKeyboardPlacement,
```

Apply only when `o.mode == DragMode::Keyboard`.

Pointer and native `DropOutcome` delivery must ignore this prop.

The native `ondrop` handler inside `CanvasDropZone` must also ignore this
prop because that path is pointer-driven native HTML5 drag, not keyboard.

## Implementation Steps

1. Add `mode: DragMode` to `DropOutcome`.

   File: `src/core/types.rs`

   Update the field docs to explain that the mode says which input path
   produced the completed drop.

2. Update all `DropOutcome` construction sites.

   Files:

   - `src/core/components.rs`
   - `src/pointer.rs`
   - `src/core/model.rs` tests/helpers if they construct literals
   - `tests/runtime.rs`

   Rules:

   - Keyboard path sets `DragMode::Keyboard`.
   - Pointer path sets `DragMode::Pointer`.
   - Core native path reads `dnd.mode()`.
   - Test-only literals choose the mode that matches the scenario.

3. Add `CanvasKeyboardPlacement`.

   File: `src/canvas.rs`

   Add a pure helper:

   ```rust
   pub fn canvas_keyboard_pointer(
       policy: CanvasKeyboardPlacement,
       outcome_element: Point,
   ) -> Point
   ```

   Expected behavior:

   - `Center` returns `outcome_element`
   - `Origin` returns `Point::default()`
   - `Fixed(p)` returns `p`

   This keeps policy behavior testable without mounting Dioxus.

4. Add `keyboard` prop to `CanvasDropZone`.

   File: `src/canvas.rs`

   Mirror `keyboard` through a signal like `snap` and `bounds`, because the
   registered drop callback is created once. The callback must read the
   current policy.

   Pattern:

   - `let mut keyboard_now = use_signal(|| keyboard);`
   - update it during render when the prop changes
   - in `registered_drop`, choose pointer:
     - keyboard mode: `canvas_keyboard_pointer(*keyboard_now.peek(), o.element)`
     - other modes: `o.element`

5. Export the policy.

   Files:

   - `src/lib.rs`
   - possibly `src/canvas.rs` module docs

   Add `CanvasKeyboardPlacement` and the helper to the prelude if the helper is
   public.

6. Update the focused canvas example.

   Add compact toolbar controls for `Center`, `Origin` and `Fixed` keyboard
   placement so the policy can be tested through the UI.

   Keep the controls example-owned and avoid explaining the API through
   visible in-app instructional copy.

7. Update docs.

   Files:

   - `README.md`
   - `CHANGELOG.md`
   - `ROADMAP.md`

   README should state:

   - Default keyboard canvas placement is the selected target center.
   - `CanvasKeyboardPlacement::Origin` and `Fixed(Point)` are available for
     apps that want explicit keyboard placement.
   - Pointer and native placements still use pointer geometry and grab offset.

   Changelog should mention:

   - `DropOutcome::mode`
   - `CanvasKeyboardPlacement`
   - default behavior remains center for measured keyboard targets

## Tests

### Unit Tests

File: `src/canvas.rs`

- `canvas_keyboard_pointer_uses_center_element_by_default`
- `canvas_keyboard_pointer_can_use_origin`
- `canvas_keyboard_pointer_can_use_fixed_point`
- Existing `canvas_position` tests stay unchanged.

File: `src/core/components.rs`

- Existing `keyboard_drop_points` tests stay unchanged.
- They continue to prove core coherent geometry, not canvas policy.

### Runtime Tests

File: `tests/runtime.rs`

Add a dynamic canvas test that calls the registered `on_drop` callback with
different `DropOutcome::mode` values.

Cases:

- Default policy with `DragMode::Keyboard` uses `o.element`.
- `CanvasKeyboardPlacement::Origin` with `DragMode::Keyboard` ignores
  `o.element`.
- `CanvasKeyboardPlacement::Fixed(Point::new(...))` with
  `DragMode::Keyboard` ignores `o.element`.
- `CanvasKeyboardPlacement::Origin` with `DragMode::Pointer` still uses
  `o.element`.

These tests should not require browser mounting.

### Browser Tests

Update the focused canvas keyboard test:

- It confirms the default policy still lands at the selected canvas geometry.
- It clicks the example policy controls and verifies `Origin` and `Fixed`
  placement through real keyboard drops.

### Full Verification

Run:

```sh
cargo fmt -- --check
cargo test
npm run test:web -- tests/browser/web-pointer-regressions.spec.js
```

## Compatibility Notes

Adding `DropOutcome::mode` changes the public struct literal shape.

This is acceptable if the branch is still in unreleased work, but it must be
called out in the changelog.

Alternatives considered:

- Infer keyboard drops from `grab == Point::default()`: rejected because
  pointer/native drops can have zero grab.
- Add keyboard policy in core `Draggable`: rejected because placement policy
  is canvas-specific. Core should only provide coherent `DropOutcome`
  geometry and mode metadata.
- Add callback placement first: rejected because it is heavier than the
  current evidence requires.
- Add `LastPointer`: rejected for first pass because it needs state and has
  unclear behavior under pan/zoom.

## Acceptance Criteria

- `DropOutcome` tells consumers whether the completed drop was pointer,
  native or keyboard.
- `CanvasDropZone` supports `Center`, `Origin` and `Fixed(Point)` keyboard
  placement.
- Default canvas keyboard behavior remains selected target center.
- Pointer and native canvas placement are unchanged.
- Snap and bounds apply after keyboard policy resolution.
- Runtime tests cover policy behavior without browser setup.
- Browser regression still passes for the default focused canvas keyboard
  path.
- README and changelog document the new API and the unchanged default.

# Roadmap

This is the working path after 1.0.

The library stays headless. The default path now matches what people expect in
a modern web app: styled, accessible, pointer driven drag inside the app, with
native browser drag preserved for the places only the browser can reach.

## Input model

Pointer is the default for in-app drag.

- `PointerDraggable`, `BoardItem`, `SelectableDraggable`, `SortableList` and
  `SortableGrid` default to `DragInputMode::Pointer`.
- `Draggable` should stay native by default. It is the low-level HTML drag
  source and the compatibility escape hatch.
- `DragInputMode::Native` and `DragInputMode::Hybrid` should stay public and
  documented.
- Native should be an explicit choice when the browser needs to carry the drag,
  not the default for ordinary app UI.

## Native boundary

Native drag is still required for the browser and OS boundary.

- `FileDropZone` stays native. OS file drops arrive through `DataTransfer`.
- `ExternalDropZone` stays native. Text, links, HTML and files from outside the
  app arrive through `DataTransfer`.
- `ExternalDragSource` stays native. Dragging links, text or HTML out of the
  app needs the browser's drag data store.
- `external::typed` stays native. Cross-window or cross-app payloads need
  serialized `DataTransfer` data.
- Plain `Draggable` remains the way to opt into native app-internal drags.

## Web feature

The `web` feature should be the recommended web setup.

- It only pulls in `web-sys` for pointer capture.
- Pointer capture keeps mouse drags attached when the cursor leaves the source
  element.
- The dependency-free build still works with the fallback recovery path.
- Examples and docs should show `--features web` for browser demos.
- The feature should stay isolated in `core::platform`.

## Docs

The docs need to make the split obvious.

- Pointer is for app-internal dragging.
- Native is for `DataTransfer`, file drops, external drops, drag-out and
  cross-window interop.
- Hybrid is compatibility mode, not the preferred default.
- Scrollable touch lists should use handles because pointer drags need
  `touch-action: none` on the drag surface.
- The README should say this early, before the examples.

## Agent support

Agents should be able to use the crate without guessing.

- Add a small `llms.txt` for the crate. It should point agents at the README,
  roadmap, changelog, examples and the important source modules.
- Add an `llms-full.txt` later if the docs grow enough to justify a single
  combined context file.
- Keep the agent docs short and mechanical: what to import, which component to
  use, when to choose pointer, when to choose native, and which examples match
  each use case.
- Add a prompts or agent-skills folder with copyable instructions for common
  tasks: sortable list, board, file drop, drag-out, tree, canvas and Tailwind
  styling.
- Include a Dioxus setup note for agents: web demos should use
  `dx serve --platform web --features web`, and browser regressions should use
  Playwright through `npm run test:web`.
- Link to Dioxus' own `llms.txt` docs so agents can fetch current Dioxus
  framework context instead of relying on stale training data.
- Keep the agent material repo-native and plain text. No generated site is
  needed until the public docs move out of the README.

## Tests

The test suite should lock the new defaults down.

- Runtime tests should prove pointer wrappers render with native drag disabled
  by default.
- Runtime tests should prove `DragInputMode::Native` and `Hybrid` still opt
  back into native `draggable`.
- Browser tests should keep covering the web pointer path through `dx serve`.
- Browser tests should stay small and behavior-focused: sortable overlay,
  canvas drop position, and any future pointer capture regressions.

## Later

These are useful, but not first.

- A small native interop demo that shows file drop, external drop and drag-out
  side by side.
- Canvas polish: sync changing `CanvasDropZone` labels into the zone registry,
  and design a better keyboard placement policy than the default origin drop.
- More keyboard examples for sortable and board flows.
- A migration note for users who depended on native mouse as the default.
- A tighter public docs page for choosing between pointer, native and hybrid.

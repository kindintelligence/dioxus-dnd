# Native HTML Drag Audit

This document records the post-refactor native drag boundary.

## Policy

Native HTML drag is not a general in-app drag mode.

- In-app drag/drop uses typed Rust payloads through `DndContext<T>`, pointer
  events, and keyboard controls.
- Native browser drag remains only where the browser/OS boundary requires
  `DataTransfer`.

This removes the old user choice between pointer/native/hybrid app dragging.
Users now choose only between:

- "inside my app" -> `Draggable`, `DropZone`, and pattern components
- "crossing the app boundary" -> file/external/drag-out APIs

## Native Code That Intentionally Remains

| File | API/code | Reason |
| --- | --- | --- |
| `src/files.rs` | `FileDropZone` native `ondrag*`/`ondrop` | OS files arrive through native drop events. |
| `src/external.rs` | `ExternalDropZone`, `classify`, `ExternalDrop`, `ExternalPayload` | Outside text, links, HTML, and files arrive through `DataTransfer`. |
| `src/external.rs` | `external::typed::{store, retrieve}` | Optional typed JSON payloads across windows/apps. |
| `src/dragout.rs` | `ExternalDragSource`, `OutboundContent` | Dragging content out requires writing native `DataTransfer` formats. |
| `src/autoscroll.rs` | `ondragover` listener | Lets scroll containers respond while native boundary drags hover over them. |

## Native Code Removed From In-App APIs

Removed from the public app model:

- `DragInputMode`
- `DragMode::Native`
- `PointerDraggable`
- `pointer` module export
- `Draggable::native`
- in-app native `draggable` attributes
- native `ondragstart`/`ondragover`/`ondrop` branches in `Draggable`,
  `DropZone`, `SortableList`, `SortableGrid`, `BoardSlot`, `TreeNodeTarget`,
  and `CanvasDropZone`
- `input` props from sortable/grid/board/multiselect components

## Current In-App API

`Draggable<T>` is now the normal app drag source:

- pointer events for mouse, touch, and pen
- keyboard pickup/drop
- typed payloads through context
- `DragOverlay` for custom ghosts
- no native HTML drag attributes

`DropZone<T>` is now a typed registry target:

- registers `id`, `label`, `accepts`, mounted handle, rect, and `on_drop`
- exposes `data-active` and `data-over`
- does not accept browser `DataTransfer` drops

## Boundary API Guidance

Use these when the browser/OS boundary is involved:

- Files from desktop: `FileDropZone`
- Text/link/HTML/files from another app/tab: `ExternalDropZone`
- Drag text/link/HTML/custom formats out: `ExternalDragSource`
- Cross-window typed JSON: `external::typed` with `serde`

Do not use `CanvasDropZone<T>` or `DropZone<T>` for native browser payloads.
Layer `ExternalDropZone` or `FileDropZone` where boundary drops are needed.

## Verification

The source scan after the refactor leaves native handlers only in:

- `files`
- `external`
- `dragout`
- `autoscroll`

Rust verification:

- `cargo check --all-targets --features web,serde`
- `cargo test --features web,serde`


# TODO — code review follow-ups

Findings from the 2026-07-08 review. **All items below are fixed** (each with a
regression test where the logic is pure). Kept as a record of what changed.

## Cross-cutting theme (addressed)

Most bugs were the same shape: each pattern had a **native HTML5** drop path and
a **registered** (pointer/touch/keyboard) drop path, and the two had drifted.
`TreeNodeTarget` already mirrored props through signals into `*_now`; that
pattern is now applied to `BoardSlot`, and the canvas native path now derives
canvas-relative coordinates the same way the pointer path does.

## Bugs fixed (ranked)

- [x] **1. `dragout.rs` — unescaped HTML in the link builder.**
  Added `escape_html_attr`/`escape_html_text` and `is_safe_href`; the anchor now
  escapes both fields and drops the `href` for `javascript:`/`data:`/`vbscript:`
  schemes. Tests: escaping + dangerous-scheme.
- [x] **2. `canvas.rs` — native drop used element-relative coords.**
  Native `ondrop` now uses `client_point(&evt) - rect.origin()` when measured,
  matching the pointer/keyboard path; falls back to `element_point` pre-measure.
- [x] **3. `board.rs` — `BoardSlot` captured a stale `index`.**
  `column`/`index`/`on_move` mirrored through signals; the registered drop reads
  the current values.
- [x] **4. `board.rs` — `BoardSlot` bypassed the column's `accepts`/WIP limit.**
  `BoardColumn` now shares its filter via a `ColumnAccepts` context; the slot
  registers with it, gates `data-active`/`data-over`/`ondragover`, and checks it
  in both drop paths.
- [x] **5. `pointer.rs` — drop rejected when a non-accepting zone was topmost.**
  `hit_test_closest` is now acceptance-aware (topmost containing **and**
  accepting zone), and `finish_drop` falls through to the retry instead of
  cancelling. Test: registry acceptance-aware hit-test.
- [x] **6. `sortable.rs` / `grid.rs` — pointer release outside the list still reordered.**
  Added `list_bounds`; a release outside the list/grid bounds commits no reorder,
  matching the native cancel. Test: `list_bounds`.
- [x] **7. `autoscroll.rs` — full-speed scroll while the pointer was outside.**
  `edge_delta` now returns `(0,0)` when the pointer is outside the container, and
  scrolls toward the nearer edge (fixes narrow containers). Tests: outside-gate +
  narrow-container.

## Minor / nits fixed

- [x] `components.rs` — collapsed the embedded whitespace in the pickup announcement.
- [x] `canvas.rs` — `Bounds::clamp`/`clamp_axis` are NaN-safe now (test added).
- [x] `grid.rs` — Drop arm clears drag state before `on_sort` (matches sortable).
- [x] `grid.rs` — per-tile `onpointermove` now gates on `input.uses_pointer`.
- [x] `tree.rs` — documented that keyboard-drop intent depends on `row_height`.
- [x] `files.rs` — filename now ASCII-lowercased to match extension casing.
- [x] `files.rs` / `external.rs` — documented that filters are advisory and that
  inbound `ExternalPayload::Html`/`Url` are untrusted.
- [x] `multiselect.rs` — reviewed; the docs are accurate (no false claim), no change.

## Deferred (low value / needs runtime signal)

- [ ] Registry rects aren't re-measured on scroll during a native drag, so tree
  intent bands / canvas native positions can drift after scrolling. Shared
  registry limitation; needs a scroll listener or on-drag re-measure.
- [ ] `autoscroll.rs` `busy` flag can wedge if `get_client_rect`/`get_scroll_offset`
  never resolves. Low likelihood; would need a timeout/reset.

## Notes

- Build is clean: `cargo clippy` passes on default / `web` / `--all-features`;
  111 tests pass (77 lib + 32 runtime + 2 browser). Examples build with `web`.

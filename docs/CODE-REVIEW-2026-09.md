# Code review: 2026-09

- Date: 2026-09-02 (line references at commit `324fd80`, branch `dev`)
- Scope: the whole crate, read end to end. Findings only; nothing here has
  been fixed.
- Purpose: record what a full read turned up so each item can be triaged,
  fixed, or explicitly declined, instead of being rediscovered later.

**Honesty rule**: every finding below names the file and line it was read
at. "Verified" means the behavior was traced in source; where a finding
rests on a platform the container cannot build, it says so. Do not promote
a finding to fixed without a test that fails before the change.

## What was verified

| Check | Command | Result |
|---|---|---|
| Tests | `cargo test --features serde` | 275 pass, 0 fail |
| Lints | `cargo clippy --features "serde web" --all-targets -- -D warnings` | clean |
| Formatting | `cargo fmt --all -- --check` | clean |
| Desktop feature | `cargo check --all-features` | not buildable in the review container (`gdk-3.0` absent) |

Test breakdown: 151 lib, 82 `tests/runtime.rs`, 33 `tests/multiwindow.rs`,
2 `tests/multiwindow_seam.rs`, 2 `tests/typed_transport.rs`, 5
`tests/compatibility.rs`, plus 5 doctests.

`src/desktop/**` and the `x11rb` dead-space leg were reviewed by reading
only. Nothing in this document claims to have exercised them.

There is no `unsafe` in the crate. Runtime panics on library paths are
limited to two provably unreachable `expect` calls (`src/core/types.rs:215`
and the row-band `unwrap` calls in `spatial_sort`), plus the counter
exhaustion panic in `SortableGroupId::auto`.

## Summary table

| # | Finding | Area | Severity | Kind |
|---|---|---|---|---|
| 1 | Registry exposes two acceptance paths; the public one ignores policy | `core::registry` | high | correctness drift |
| 2 | `update_pointer` discards any sample at exactly (0, 0) | `core::state` | medium | correctness |
| 3 | `apply_move` removes by index with no identity check | `board` | medium | correctness |
| 4 | `AutoScroll` samples the DOM on every `pointermove` | `autoscroll` | medium | performance |
| 5 | `Selection::toggle` leaves the range anchor stale | `multiselect` | low | behavior vs docs |
| 6 | `FileDropZone` hover can stick when `disabled` flips mid-drag | `files` | low | correctness |
| 7 | Custom collision ranking is quadratic | `core::collision` | low | performance |
| 8 | CI never runs on pushes to `dev` | workflows | low | infrastructure |
| 9 | `docs/README.md` pins a different rumdl version than CI | docs | low | drift |

Smaller notes that did not earn a number are collected at the end.

## 1. Two acceptance paths, and the public one is now the wrong one

Files: `src/core/registry.rs:671`, `:714`, `:747`, `:802`, `:824`, `:834`

`acceptable`, `step_zone`, `step_sibling`, `children_of`, `first_child`,
`hit_test` and `hit_test_closest` take a bare `&T` and consult only
`ZoneRecord::accepts`. Every built-in component has migrated to the
`_query` variants and to `resolve` / `resolve_hover`.

Verified by grep: outside `registry.rs` itself, the payload-only family has
no internal callers. `hit_test_closest` survives only in
`tests/runtime.rs`. The three remaining internal uses are the payload-only
functions calling each other (`step_zone` to `acceptable`, `step_sibling`
and `first_child` to `children_of`).

Consequence: a custom drag source written against the documented registry
API silently ignores `accepts_query`, `allowed_effects`, and the provider's
`ReleasePolicy` / `CollisionDetector`. It targets differently than
`Draggable` does, and only when someone configures a collision strategy or
an effect filter. `docs/api/core.md` documents `hit_test_closest`, so this
is supported surface, not an internal leftover.

Suggested fix: re-implement each payload-only function in terms of its
`_query` counterpart with `DropQuery::new(payload.clone())`, so both share
one policy pipeline, then deprecate the payload-only names. Point
`docs/api/core.md` at `resolve` / `resolve_hover` as the targeting entry
point.

## 2. `update_pointer` discards any sample at exactly (0, 0)

File: `src/core/state.rs:405`

```rust
if pointer.x == 0.0 && pointer.y == 0.0 {
    return;
}
```

A magic coordinate standing in for "this event is synthetic". A drag whose
pointer genuinely lands on the viewport's top-left corner stops updating:
the overlay freezes and hover stops tracking. `Draggable`'s `finish_drop`
also routes through here, so a release at exact (0, 0) delivers against a
stale pointer. The world path feeds the same function.

Narrow, because float client coordinates rarely land on exact zero, but it
is a real hole and no test asserts the intended-bogus case.

Suggested fix: reject only when the previous sample was far away - a jump
to the origin is the bogus signature, the origin itself is not - or tag
synthetic samples at the reporting path instead of inferring intent from a
value.

## 3. `apply_move` removes by index with no identity check

File: `src/board.rs:69`

```rust
if let Some(src) = board.get_mut(&from_col) {
    if from_ix < src.len() {
        src.remove(from_ix);
        removed = true;
    }
}
```

`docs/api/boards.md` covers the out-of-range case: "the removal is skipped
and the insert still happens, so the event's item is never lost". It does
not cover the in-range-but-wrong-item case. If the source column changed
between pickup and drop, this removes whichever card now sits at that
index and inserts the dragged one, corrupting two cards instead of moving
one.

Every sibling helper matches on a key: `apply_clone_or_move`,
`apply_list_clone_or_move`, `apply_reorder`. `apply_move` is the outlier.

Suggested fix: add a key-taking `try_apply_move` mirroring the `try_apply_*`
pattern introduced in 3.1.0, and document the drift hazard on the existing
helper.

## 4. `AutoScroll` samples the DOM on every `pointermove`

File: `src/autoscroll.rs:506` (definition at `:394`)

```rust
onpointermove: move |evt: PointerEvent| {
    if pointer_move_should_scroll(...) {
        start_clock(Point::new(c.x, c.y), ClockOwner::Pointer);
    }
    sample();
},
```

`sample()` spawns `get_scroll_offset().await` with no gate. The
`last_offset` dedup runs after the read resolves, so the round-trip always
happens. On dioxus-desktop that is a JS eval per `pointermove` - roughly
120 per second on a hovering trackpad with no drag in flight.

The correctness argument in the comment is sound (it trues up the window
after scrollbar drags and programmatic scrolls). The cost is what is
unbounded.

Suggested fix: sample only when the pointer position changed since the last
sample, or coalesce to at most one sample per animation frame. Both keep
the stated property.

## 5. `Selection::toggle` leaves the range anchor stale

File: `src/multiselect.rs:69`

`select_only` and `clear` update `anchor`; `toggle` does not. So a
Ctrl+click followed by a Shift+click ranges from whatever was selected
before the Ctrl+click, not from the Ctrl+clicked item. Explorer, Finder and
the common list-widget convention all move the anchor on Ctrl+click.

This is a documented-behavior mismatch rather than an open design choice:
`toggle` is documented as "Ctrl/Cmd+click semantics" and `click_in_order` as
"Standard click behavior plus Shift-range selection".

Suggested fix: set the anchor to `key` inside `toggle`, and add a test
covering Ctrl+click then Shift+click.

## 6. `FileDropZone` hover can stick when `disabled` flips mid-drag

File: `src/files.rs:395` (and `:377`)

```rust
ondragleave: move |_| {
    if disabled {
        depth.set(0);
        return;
    }
    ...
}
```

If `disabled` becomes true while a drag is hovering, after `on_hover(true)`
already fired, the consumer never receives the matching `false`.

Separately, `ondragover` at `:377` calls `prevent_default()` without
checking `disabled`, so a disabled zone still claims the OS drop and then
swallows it at `ondrop`. Swallowing is probably the wanted behavior (the
browser default is to navigate to the dropped file), but it is undocumented
and inconsistent with how `disabled` gates every other handler in the
component.

Suggested fix: fire `on_hover(false)` on the disabled leave path, and
document the deliberate dragover behavior in `docs/api/file-drops.md`.

## 7. Custom collision ranking is quadratic

File: `src/core/collision.rs:172`

```rust
let order = |zone| {
    orders
        .iter()
        .find(|(candidate, _)| *candidate == zone)
        .map(|(_, order)| *order)
        .unwrap_or_default()
};
```

The closure scans `orders` linearly and is called twice per comparison
inside `sort_by`, making the custom path O(n^2 log n). Candidate counts are
small in practice, but this runs per pointer move for anyone using
`CollisionDetector::Custom`.

Suggested fix: build a `HashMap<ZoneId, usize>` once before the sort.

## 8. CI never runs on pushes to `dev`

File: `.github/workflows/ci.yml:5`

```yaml
on:
  push:
    branches: [development, main]
```

The repository has exactly two branches, `dev` and `main`. `development`
does not exist, so pushes to `dev` get no CI at all; only the
`pull_request` trigger covers that branch. This looks like leftover state
from a branch rename.

Suggested fix: change the push filter to `[dev, main]`.

## 9. `docs/README.md` pins a different rumdl version than CI

Files: `docs/README.md` (Linting the documentation), `.github/workflows/ci.yml:36`

The contributor instructions say:

```console
cargo install rumdl --locked --version 0.2.30
```

CI runs `rvben/rumdl` at v0.2.52 with `version: "0.2.52"`. A contributor
following the documented command lints against a different ruleset than the
one that gates the pull request.

Suggested fix: bump the documented version to match CI, or reference the
workflow as the single source of truth.

## Smaller notes

- `DndContext`'s `PartialEq` (`src/core/state.rs`) compares only
  `announcement`. Deliberate and explained inline for 3.x handle identity,
  but it means two contexts sharing an announcement signal memoize as
  equal. Worth promoting the inline comment to a doc caveat on the impl.
- `SortableGrid`'s `captured` signal (`src/grid.rs`) is never reset on
  drop, cancel or tap. Harmless today because the render gate also requires
  `drag_from().is_some()` and every press re-sets it, but it is the one
  piece of gesture state the other surfaces do reset.
- `use_rect_refresh_thunk` (`src/core/hooks.rs`) keys its registration with
  `DragId::auto().0`. Unique, because the counter is shared with `ZoneId`,
  but a `DragId` used as a refresh-bus key does not document itself. A
  dedicated counter would.
- `DropEffects` (`src/core/effects.rs`) has `BitOr` and `BitOrAssign` but
  no `BitAnd`, `Not` or `FromIterator`. A small ergonomic gap for an
  otherwise flag-set-shaped type.
- The README is 52 KB and is included as crate docs via
  `#![doc = include_str!]`, alongside roughly 249 KB of `docs/api/*.md`.
  The arrangement keeps the references honest and is worth its cost, but it
  is a meaningful chunk of every downstream `cargo doc`.

## What was checked and found sound

Recorded so a later review does not re-derive it:

- The gesture machine (`src/core/machine.rs`) is pure and exhaustively
  matched, including foreign pointer ids, stale `Hold` timers, and the
  `HoldOrSideways` scroll-yield arm.
- Generation tokens are applied consistently across every async seam:
  `ZoneRegistration` for registry measurements, `DragSessionId` for source
  completion, the world generation plus session pair for desktop bridge
  legs, `measured_generation` and `measuring` for the settle glide, and
  `measurement_is_current` for sortable row rects.
- Source completion is exactly-once by construction
  (`src/core/session.rs`), including the commit-before-user-code ordering
  that survives a receiver unmounting the source mid-delivery.
- `spatial_sort` (`src/core/registry.rs:1180`) correctly keeps the
  non-transitive row-band tolerance out of the comparator by sorting
  vertically first, then banding.
- Coordinate conversions (`src/core/world/geometry.rs`) guard against zero
  and negative scale factors, and `canvas::clamp_axis` guards the NaN and
  inverted-range cases that `f64::clamp` panics on.
- Attribute protection (`protect_attributes`,
  `merge_style_invariant_last`) covers both textual `style` attributes and
  the namespaced per-property form, which is the case a spread would
  otherwise win.

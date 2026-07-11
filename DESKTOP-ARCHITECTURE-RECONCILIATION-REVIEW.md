# Desktop architecture reconciliation review

- Date: 2026-07-10
- Canonical base: Windows checkout, `development` at `0320047`
- Donor implementation: WSL checkout at `12ae7ad` plus its uncommitted hardening patch
- Dioxus baseline: 0.7.9
- Purpose: decide, by file and line range, what to preserve, move, replace, or discard before implementation
- Status: review and implementation specification; this document does not claim the reconciliation is already implemented

## Executive verdict

The concern about core bloat is valid. The donor patch grows the largest files substantially:

| File | Windows base | WSL donor | Change in physical size |
|---|---:|---:|---:|
| `src/core/components.rs` | 1,577 lines | 1,822 lines | +245 |
| `src/core/world.rs` | 800 lines | 1,158 lines | +358 |
| `src/core/state.rs` | 351 lines | 588 lines | +237 |
| desktop example `main.rs` | 257 lines | 546 lines | +289 |

The correction is not to move all of the new behavior into `desktop`. Exactly-once source completion, window-qualified zone identity, settle ownership, receiver-local pointer conversion, and host-drop idempotence are platform-neutral invariants of a multi-window drag world. If they live in the Tao adapter, custom hosts and headless tests can bypass them and recreate the same bugs.

The correct boundary is:

- **Core owns meaning and invariants.** It knows drag sessions, source completion, windows as abstract identities, geometry values supplied by a host, qualified zones, modifiers, pointer coordinates, settle ownership, and host-neutral `track_global` / `drop_at_global` operations.
- **Desktop owns observation and platform mechanics.** It knows Tao/Wry events, WebView2, `WM_INPUT`, `DeviceEventFilter`, X11, Wayland, AppKit/WKWebView, cursor polling, visibility/minimization, and backend capability detection.
- **Examples own application state and window construction.** They demonstrate the public adapter; they do not carry a second private desktop adapter.

The donor semantics should be kept, but the monoliths should be split before or while porting them.

### Source-control facts behind this review

- Commit ancestry is linear: `12ae7ad -> 05d0da1 -> ecfba5d -> 0320047`.
- The Windows checkout and live `origin/development` are both at `0320047`.
- The Windows worktree has no textual edits; its apparent modifications are NTFS/WSL mode changes (`100644 -> 100755`). Do not stage them.
- The WSL donor has 22 modified tracked files (`+2,084/-286`) plus its CI and earlier review files.
- Eleven files contain overlapping edits that do not apply cleanly in either direction. This must be a manual reconciliation, not patch application.

## Decision labels

- **PRESERVE**: carry the Windows implementation and public behavior forward.
- **PORT**: bring the donor behavior onto the Windows base, adapting it to the final API.
- **MOVE**: retain the behavior but place it in the named module.
- **REPLACE**: remove the current mechanism and implement the stated replacement.
- **DISCARD**: do not carry the code or claim into the reconciled tree.
- **DEFER**: legitimate follow-up, deliberately outside the first reconciliation slice.

Line references use:

- **W**: canonical Windows checkout at `0320047`.
- **D**: current WSL donor working tree. Donor line numbers will cease to be authoritative once the patch is ported.

## Target module layout

```text
src/
├── desktop/
│   ├── mod.rs
│   ├── bridge.rs
│   ├── geometry.rs
│   └── platform/
│       ├── mod.rs
│       ├── windows.rs
│       ├── linux.rs
│       ├── macos.rs
│       └── unsupported.rs
└── core/
    ├── components/
    │   ├── mod.rs
    │   ├── provider.rs
    │   ├── pointer.rs
    │   ├── delivery.rs
    │   ├── draggable.rs
    │   ├── drop_zone.rs
    │   └── overlay.rs
    ├── session.rs
    └── world/
        ├── mod.rs
        ├── geometry.rs
        ├── state.rs
        ├── session.rs
        ├── settle.rs
        ├── host.rs
        ├── joined.rs
        └── presentation.rs
```

This layout preserves the existing public paths through re-exports. Converting `src/desktop.rs` to `src/desktop/mod.rs`, `src/core/components.rs` to `src/core/components/mod.rs`, and `src/core/world.rs` to `src/core/world/mod.rs` is API-neutral if the existing names remain re-exported.

Do **not** add `desktop/platform/wsl.rs`. WSLg is a Linux environment that may run the Wayland backend or X11 via `GDK_BACKEND=x11`. Runtime behavior must select the actual backend. WSL-specific launch instructions and smoke-test notes belong in documentation, not runtime dispatch.

## Non-negotiable preservation list

These are release-critical and must survive the reconciliation:

1. The Windows checkout's optional `desktop` Cargo feature and public `dioxus_dnd::desktop::{use_window_geometry_feed, DragBridge}` API.
2. The verified WebView2 raw-input path using `DeviceEvent::{MouseMotion, Button}` and `DeviceEventFilter::Never`.
3. Windows' degrading `try_*` access around dead zone/window signal handles until the underlying ownership design is replaced.
4. Windows' public `DndContext::{set_pointer_kind, pointer_kind}` contract and `PointerKind::implicitly_captured` behavior.
5. Mouse/pen host bridging with synthesized-touch suppression; do not silently reduce it to mouse-only.
6. The Windows example's real N-way trays: unique `ZoneId::auto()`, independent card lists, numbered windows, and card return on tray closure.
7. The Windows README/CHANGELOG record of the WebView2 investigation, raw-input requirement, and observed fatal callback race.
8. The donor's exactly-once source lifecycle, settle ownership/generation, qualified zone identity, receiver-local pointer, live modifiers, eligibility, and creator-independent model owner.

## Non-negotiable rejection list

Do not carry these forward:

1. The donor's example-local `DragBridge` as the final adapter. It has no Windows raw `DeviceEvent` leg.
2. The donor's single `TRAY` constant and shared tray list. Multiple windows become mirrors instead of independent destinations.
3. The donor's deletion of the root `desktop` feature, `src/desktop.rs`, native lockfile graph, and registry dead-signal guards.
4. The donor's removal of the public pointer-kind API or its mouse-only desktop gating.
5. The Windows adapter's assumption that Wayland is detected by `inner_position()` or `cursor_position()` returning an error.
6. The Windows adapter's uncancelled `use_effect` poller.
7. Board-owned model signals passed to longer-lived tray `VirtualDom`s, including `ScopeId::ROOT` allocations owned by the board runtime.
8. Child-owned `mounted` and `rect` signals stored in a provider-owned zone registry. Fallible reads reduce crash severity but do not fix Dioxus ownership.
9. The WSL CI workflow unchanged: its `--all-features` Wasm lane would enable native desktop dependencies.
10. Documentation that describes Windows as pending or unverified.

## Desktop adapter ledger

### `src/desktop.rs` on the Windows base

Split this file; preserve its public surface.

| W lines | Decision | Destination and required treatment |
|---|---|---|
| 1-9 | PRESERVE + tighten | Move to `desktop/mod.rs`. Keep dependency boundary and verification status. Correct the contradiction between “macOS unverified” and later wording that implies AppKit was confirmed. |
| 11-26 | PRESERVE | Move the public usage example to `desktop/mod.rs`. Keep `DragBridge::<T>` and `use_window_geometry_feed` source-compatible. |
| 28-44 | MOVE + qualify | Put X11 mechanics in `platform/linux.rs`; put macOS strategy in `platform/macos.rs` and label it unverified until exercised. |
| 46-66 | PRESERVE + MOVE | Move the WebView2/`WM_INPUT` rationale to `platform/windows.rs`. This evidence explains why the raw leg exists and must not be summarized away. |
| 68-73 | PRESERVE concept | Put pointer-kind gating in `desktop/bridge.rs`. Preserve touch suppression and the public pen behavior unless fresh evidence changes it. |
| 75-78 | MOVE | Put the Wayland capability explanation in `platform/linux.rs`; detection must be explicit. |
| 80-85 | REPLACE | Each new module imports only its own Tao/Dioxus types. Platform-specific imports must not leak into shared bridge code unnecessarily. |
| 87-93 | PRESERVE | Public `use_window_geometry_feed` documentation remains at the re-export or implementation. |
| 94-129 | REPLACE | Implement in `desktop/geometry.rs`: explicit Linux backend detection, initial inert state until known, visibility/minimization eligibility, focus stamp, move/resize/scale refresh, and close/destroy clearing. |
| 131-140 | PRESERVE API | Move `DragBridge<T>` to `desktop/bridge.rs`; keep the public component signature. |
| 142-150 | PRESERVE concept + refactor | Shared joined-window and pointer-kind/session gates belong in `bridge.rs`. |
| 151-189 | PRESERVE + MOVE + harden | Move to `platform/windows.rs`. Keep raw motion/release, global cursor lookup, and origin bounds check. Add session-generation validation and install `DeviceEventFilter::Never` once, Windows-only, rather than from every window on every OS. |
| 191-221 | REPLACE implementation | Keep cursor polling as a fallback policy but use the donor's generation-bound, cancellable resource. A sleeper from drag N must never attach to drag N+1. |
| 223-253 | MOVE + harden | Put Linux/X11 fallback in `platform/linux.rs` and macOS fallback in `platform/macos.rs`. Require live geometry and current session; restrict completion to the relevant primary release/fallback signal. |
| 255-256 | PRESERVE | The component continues to render no DOM. |

### Donor desktop logic currently embedded in the example

| D lines | Decision | Treatment |
|---|---|---|
| 83-97 | PORT, then MOVE | `GlobalGeometry` capability logic belongs in `desktop/platform/linux.rs` or a small shared capability type, not in the example. |
| 99-114 | PORT, then MOVE | Tao modifier mapping belongs in `desktop/bridge.rs` with pure unit tests. |
| 140-215 | PORT selectively, then DELETE from example | Carry explicit Wayland detection and eligibility into `desktop/geometry.rs`; do not keep a duplicate hook in the example. |
| 288-345 | PORT selectively, then DELETE from example | Carry generation-bound polling into `desktop/bridge.rs`. |
| 347-406 | PORT selectively, then DELETE from example | Carry modifiers, active-drag rect refresh, and `CursorEntered` fallback. Do not use this block as a replacement for Windows raw input. |
| 524-545 | PORT tests | Move modifier and backend-capability tests beside the desktop modules. |

### Proposed desktop responsibilities

#### `desktop/mod.rs`

- Public docs and platform truth table.
- `pub use bridge::DragBridge;`
- `pub use geometry::use_window_geometry_feed;`
- No event loop logic.
- No public platform-specific types unless a proven consumer needs them.

#### `desktop/bridge.rs`

- Generic `DragBridge<T>` component.
- Current drag/session gate.
- Pointer-kind and modifier routing.
- Rect refresh on resize/scale changes.
- Generation-bound cancellable cursor poller.
- Calls sealed platform hooks; does not contain `cfg(target_os)` branches throughout the component.

#### `desktop/geometry.rs`

- Creates/provides `WindowGeometry` above `DndProvider`.
- Samples position, size, scale, visibility, minimization, focus, and destruction.
- Delegates backend capability detection to `platform`.
- Leaves geometry inert until capability is known.

#### `desktop/platform/windows.rs`

- WebView2 raw `DeviceEvent` motion and primary release.
- One-time `DeviceEventFilter::Never` setup.
- Raw-input/session idempotence.
- No X11/Wayland/AppKit commentary or fallback code.

#### `desktop/platform/linux.rs`

- Runtime `target.is_wayland()` detection.
- Wayland: global geometry and global cursor capability are unavailable; local dragging remains enabled.
- X11: host polling and foreign-release fallback.
- WSLg is covered by whichever backend is actually active.

#### `desktop/platform/macos.rs`

- AppKit/WKWebView fallback isolated from Windows/Linux code.
- Mark runtime status unverified until a real pass is completed.

## Core state and type ledger

### `src/core/types.rs`

| Lines | Decision | Treatment |
|---|---|---|
| D 15, 55-65 | PORT | Keep `DragSessionId` in core; it identifies one gesture independently of item identity. Generation is a semantic invariant, not a desktop quirk. |
| W 226-268 | PRESERVE | Keep `PointerKind`, `from_pointer_type`, and `implicitly_captured`. Preserve the safe Mouse default and Pen behavior. |
| D pointer-kind replacement around 239-262 | DISCARD as a wholesale replacement | Do not remove the existing public API or change unknown/custom pointers to a non-bridged state without a compatibility plan. |
| D 393-404 | PORT tests selectively | Keep session/id behavior tests; retain Windows pointer-kind tests as the compatibility baseline. |

`DragSessionId` may remain public for custom host integrations, but it should not automatically enter the general prelude unless the final public host API requires users to name it. Review `DragSessionId`, `ZoneLocation`, and `WindowRecord` prelude exposure rather than expanding it by accident.

### `src/core/state.rs`

| Lines | Decision | Treatment |
|---|---|---|
| W 25-77 | PRESERVE shape | Retain `DragState::pointer_kind` and its public meaning. Merge session completion around it rather than deleting it. |
| W 111-149, 334-338 | PRESERVE API | Keep `start`, `set_pointer_kind`, and `pointer_kind` source compatibility. `start_tracked` can wrap or extend this path. |
| D 19-23, 86-221 | PORT + MOVE | Move `SourceCompletion` and its two-phase commit/finalize machinery to `core/session.rs`; keep it crate-private. |
| D 117-221 | PRESERVE ordering | Success must be committed before receiver user code. Final notification then runs exactly once even if the receiver removes the source or starts a replacement drag. |
| D 224-448 | PRESERVE public facade | Keep existing `DndContext` drag, settle, pointer, focus, and announcement APIs. Merge, do not replace, the Windows pointer-kind API. |
| D 453-588 | PORT tests | Move exactly-once, settle-payload, and source-cleanup tests with the session implementation. |
| W/D line 9 | REPLACE stale docs | Refer to Dioxus 0.7, not 0.8. |

Target outcome: `state.rs` returns to being the public context/state facade; `session.rs` contains the internal completion state machine and its tests.

## Core world ledger

The donor's `world.rs` contains valuable invariants but should not remain an 1,100-line file.

| D lines | Decision | Destination and treatment |
|---|---|---|
| 94-103 | PRESERVE | `WindowKey` remains core identity in `world/geometry.rs` or `world/mod.rs`. |
| 105-114 | PORT | `ZoneLocation { window, zone }` belongs in core. It prevents duplicate explicit zone IDs from corrupting hover and delivery identity. |
| 117-121 | PORT, private | `ActiveDrag` is internal world-session state; keep it out of the public surface. |
| 123-143 | PORT | Pure coordinate conversions belong in `world/geometry.rs` and stay independently unit-tested. |
| 145-284 | MERGE, do not copy wholesale | Port `eligible` and reactive `live`; retain Windows' `try_read`/`try_peek`/`try_write` degradation for closed runtimes. Put the result in `world/geometry.rs`. |
| 288-311 | PRESERVE + MOVE | `WindowRecord` and its host-neutral fields move to `world/state.rs` or `world/joined.rs`. Do not add Tao handles. |
| 315-331 | PORT + consolidate | Qualified source/hover, global pointer, settle owner/generation, modifiers, and session state are valid. Group related fields in an internal world-drag state where reactivity permits; avoid an unstructured list of signals. |
| 346-405 | PRESERVE + adapt | World construction and join stay core. Keep degrading access for teardown. |
| 406-465 | PORT | Close-order cleanup and exactly-once session finalization stay core. A platform adapter must not decide semantic cancellation. |
| 466-510 | PRESERVE | Lookup, window ordering, global resolution, and refresh remain host-neutral. |
| 511-563 | PORT + reconcile | Preserve `begin_from` compatibility and add tracked begin semantics without duplicating pointer-kind authority unnecessarily. Prefer the existing `DndContext` pointer-kind API as the source of truth or provide compatible forwarding. |
| 564-670 | PORT | Global pointer, settle ownership/generation, source/over locations, modifiers, session queries, and origin window belong in core. |
| 672-790 | PORT + keep private where possible | Clearing, qualified hover, commit/finalize, cancellation, and refresh are world invariants. Minimize public exposure. |
| 792-906 | PORT + MOVE | Put host-neutral drive operations in `world/host.rs`. Preserve public `track_global`, `drop_at_global`, and `cancel_drag` for non-Tao hosts. No Tao types or OS branches may enter this module. |
| 915-976 | PRESERVE + MOVE | Public hook and membership types belong in `world/mod.rs` / `world/joined.rs`. |
| 977-1114 | PORT + MOVE | Joined-window qualified hover, receiver-local pointer, hit-testing, and overlay presentation belong in `world/joined.rs`. |
| 1116-1158 | PORT tests | Keep scale conversion, containment, and key uniqueness tests beside `geometry.rs`. |

Do not carry the donor's second `pointer_kind` signal in `DndWorld` as a parallel authority. The canonical `DndContext` already exposes pointer kind. `DndWorld::pointer_kind()` may forward to that context, while world-owned modifiers and host-pointer state remain internal host/session data.

### World ownership that should not be cemented

Both trees retain the append-only `WORLD_OWNERS` storage near the top of `world.rs`. Do not spread this pattern into new modules. Replacing it with reference-counted `WorldInner` ownership is valid, but it changes `DndWorld` from a `Copy` handle and deserves a separately reviewed compatibility slice. Mark it **DEFER**, not “fixed” by the reconciliation.

## Core component ledger

The donor's component changes should be ported by responsibility, not pasted into the existing monolith.

| Lines | Decision | Destination and treatment |
|---|---|---|
| W/D 138-158 | PRESERVE + MOVE | `DndProvider` moves to `components/provider.rs`. |
| W/D 160-187 | PRESERVE + MOVE | `primary_press` and lost-release debounce move to `components/pointer.rs`. They are DOM pointer normalization used by web and desktop, not Tao adapter code. Generalize comments; keep WSLg as one test case, not the architecture. |
| D 189-328 | PORT + MOVE | `DropCompletion`, `SettleRoute`, and `deliver_drop` move to `components/delivery.rs`. Preserve commit-before-callback ordering and retain the Windows `cached_rect()` access rather than donor raw child-signal peeks. |
| D 330-1010 | PORT + MOVE | `Draggable` moves to `components/draggable.rs`. Preserve shared source completion, unmount cleanup, session checks, authoritative release position, and immediate same-source restart. |
| W 514-526 equivalent stale-gesture reset | REPLACE as primary mechanism | Exactly-once completion fixes the source lifecycle. A defensive stale-phase assertion/reset may remain, but it must not substitute for `on_drag_end` and session finalization. |
| D 1011-1377 | PORT + MOVE | Drop zones and bridge zones move to `components/drop_zone.rs`; use qualified hover and receiver-local pointer behavior. Integrate the registry ownership redesign below. |
| D 1378-1774 | PORT + MOVE | Overlay and `SettleSlot` move to `components/overlay.rs`. Preserve elected presenter and settle-generation gates for measure, retarget, finish, cleanup, and callback. |
| D 1775-1822 | PORT tests | Keep pure input/style tests with their modules. |

Core component code may mention “host-completed” input, but it must not mention Tao event variants, WebView2 handles, `WM_INPUT`, X11, Wayland dispatch, or Tokio polling.

## Zone registry and Dioxus ownership ledger

This is a P1 reconciliation item. The Windows native app currently emits Dioxus 0.7 ownership warnings because a `DropZone` creates `mounted` and `rect` signals in the child scope and stores them in the provider-owned registry.

### What to preserve now

| W lines | Decision | Treatment |
|---|---|---|
| `registry.rs` 46-82 | PRESERVE intent | Keep degrading access while the ownership redesign lands; it prevents a dead-signal read from becoming a fatal callback. |
| `registry.rs` 104-340 | PRESERVE behavior | Keep registration order, filters, hit testing, closest snap, and async measurement semantics. |
| `registry.rs` 418-434 | PRESERVE | Keep spatial ordering through the safe access path. |
| `components.rs` 869-900 | REPLACE storage | Do not keep child-owned `mounted` / `rect` signals in `ZoneRecord`. |

### Required replacement design

1. `ZoneRegistry` owns plain per-zone `Option<Rc<MountedData>>` and `Option<Rect>` fields inside its provider-owned storage.
2. Zones call registry mutation methods such as `set_mounted(id, handle)`, `set_rect_if_present(id, rect)`, and `unregister(id)`.
3. Async measurement captures zone identity/generation, then updates only if that registration still exists.
4. Components read their rect through the provider-owned registry. Parent-owned state used by descendants follows Dioxus' ownership direction.
5. Apply the same pattern to `DropZone`, `BoardSlot`, `TreeNodeTarget`, and bridge-zone registrations; fixing only `DropZone` leaves the warning class elsewhere.
6. Retain fallible access at the registry/world boundary for close races even after the child-signal handles are removed.

Do not normalize the live warning as harmless. The Windows `try_*` change mitigates an observed fatal teardown race, but the ownership warning proves the scope direction is still wrong.

## Other core and reusable-module ledger

| File / donor lines | Decision | Treatment |
|---|---|---|
| `src/core/hooks.rs` D 201, 223-226 | PORT behavior | Use qualified hover in bridge worlds. Prefer `use_joined_window` or a narrow helper instead of importing private `WorldMembership` into unrelated modules. |
| `src/board.rs` D 185, 249-257 | PORT behavior | Use window-qualified hover, then migrate its zone geometry to registry-owned storage. |
| `src/tree.rs` D 124, 198-208 | PORT behavior | Use qualified hover and receiver-local pointer for intent; migrate zone geometry ownership. |
| `src/autoscroll.rs` D 138-146, 231-237 | PORT + generalize | Keep optional externally supplied client pointer with an explicit active gate. Document it as a generic host-driven pointer feed, not desktop-only policy. |
| `src/autoscroll.rs` D 286-302 | PORT test | Keep the external-pointer render seam and add a behavior test where feasible. |
| `src/core/platform.rs` W/D | PRESERVE + rename if useful | This is web pointer-capture capability, not desktop platform dispatch. Correct “Dioxus 0.8” to 0.7; consider `pointer_capture.rs` to avoid naming confusion. |
| `src/test.rs` donor changes | PORT | Keep completion logs, session-aware simulation, and qualified window helpers so headless tests exercise the real delivery path. |
| `src/grid.rs`, `src/sortable.rs` | DISCARD diff | The observed donor differences are rustfmt wrapping only. |
| `src/external.rs` | Optional cleanup | The donor's rustdoc link correction is harmless but unrelated. |
| `tests/typed_transport.rs` | DISCARD diff | Formatting only; let final `cargo fmt` decide. |

## Desktop example ledger

Use the Windows example as the base. It demonstrates the supported adapter and real N-way behavior.

| W lines | Decision | Treatment |
|---|---|---|
| 1-18 | PRESERVE + update | Keep N-way intent. Remove stale TODO language; move WSLg launch advice to platform verification docs. |
| 20-32 | PRESERVE | Keep adapter imports, board identity, and payload. |
| 34-43 | PRESERVE | Keep unique zone identity and independent per-tray shape. |
| 45-55 | REPLACE | Current model ownership claims are false. Use a dedicated `Owner<UnsyncStorage>` held by `Rc<ModelOwner>`. |
| 57-77 | PRESERVE behavior + adapt storage | Keep remove-then-insert and board fallback across N trays. |
| 79-89 | PRESERVE | Keep launcher and native window config. |
| 91-103 | PRESERVE adapter use; REPLACE model construction | World and geometry remain. Model allocation moves out of the board runtime's lifetime. |
| 104-129 | PRESERVE N-way construction | Keep numbering, `ZoneId::auto`, per-tray list, root context, and new window. Replace `Signal::new_in_scope(..., ScopeId::ROOT)` with owner-backed allocation. |
| 131-145 | PRESERVE | Board status and UI. |
| 147-182 | PRESERVE contract + adapt | Closing a tray returns cards and retires its zone. Make cleanup idempotent against app-owner teardown. |
| 184-202 | PRESERVE | This remains the concise demonstration of `DragBridge` inside `DndProvider`. |
| 204-257 | PRESERVE | Ghost, column, and styling. |

### Donor model-owner contribution

| D lines | Decision | Treatment |
|---|---|---|
| 37-80 | PORT pattern | Adapt `Rc<ModelOwner>` to hold `board`, `trays`, and each independently owned tray list. Do not port the single-tray shape. |
| 116-125 | DISCARD implementation | Single board/single tray routing regresses N-way behavior. |
| 218-266 | PORT lifetime handoff only | Keep `Rc` root-context ownership; retain Windows numbering, zone IDs, and cleanup. |
| 492-522 | PORT test | Adapt creator-close survivor mutation/rerender/final disposal coverage to N-way state. |

## Probe ledger

| W lines in `src/bin/probe.rs` | Decision | Treatment |
|---|---|---|
| 1-5 | REPLACE wording | Say “webview engine,” not WebKit on Windows. Document diagnostic-only status and explicit binary command. |
| 7-63 | PRESERVE | Keep launcher, engine pane, and Dioxus-decoded pane. |
| 65-111 | EXTEND | Keep Tao window events and add Windows raw `DeviceEvent` motion/button plus filter status. |
| 113-121 | EXTEND | Show detected backend and global-geometry capability. |
| 123-179 | PRESERVE + extend | Keep second-window and presentation; add backend/raw-input sections. |

The probe is identical in both trees. It currently fails to observe the very raw-input path the Windows adapter depends on, so it is not yet a complete diagnostic.

## Manifest and export ledger

### Root `Cargo.toml`

| W lines | Decision | Treatment |
|---|---|---|
| 1-25 | PRESERVE | Package metadata and portable dependencies. |
| 26-31 | PRESERVE dependency boundary | Keep optional `dioxus-desktop` and Tokio. Consider target-specific native dependency placement if Wasm/all-features behavior requires it. |
| 33-45 | PRESERVE API | Keep the opt-in `desktop` feature name and semantics. |
| 47-52 | PRESERVE | Keep the `regressions` example's `required-features = ["serde"]`. |
| 54-73 | PRESERVE | Unrelated development dependencies. |

Regenerate `Cargo.lock` from the Windows base after manifest changes. Never replace it with the donor lockfile; that would delete the native desktop dependency graph.

### `src/lib.rs`

- PRESERVE W 12-13: the public `desktop` module remains feature-gated.
- PRESERVE explicit desktop imports; do not put the heavy adapter surface into the general prelude.
- PORT only deliberate core types after reviewing whether users must name them. Avoid accidental prelude expansion.

### `src/core/mod.rs`

- PORT donor `ZoneLocation` re-export if its public query APIs remain.
- Keep `DragSessionId` available from `core` only if custom host consumers must name it; do not add it to the broad prelude solely for internal adapter/tests.
- Preserve all existing Windows public names and make the new directory modules implementation details behind the same facade.

### Desktop example manifest

| W lines | Decision | Treatment |
|---|---|---|
| 1-4, 6-19 | PRESERVE | Standalone workspace rationale and package structure. |
| 5 | REPLACE | `cargo run` is ambiguous because the package has two binaries. Add `default-run = "desktop-multiwindow"` or document `cargo run --bin desktop-multiwindow`. |
| 20-22 | PRESERVE desktop, remove serde | Use `dioxus-dnd = { path = "../..", features = ["desktop"] }`. |
| 23 | DELETE | Direct serde dependency is unused by both desktop binaries. |

Regenerate the nested lockfile after this cleanup.

## Test reconciliation ledger

Preserve the intent of all 13 Windows multi-window tests. Port the donor additions after the final API shape is settled; do not paste tests against temporary donor-only APIs.

| D range in `tests/multiwindow.rs` | Requirement to preserve |
|---|---|
| 469-498 | Receiver settle emits exactly one successful source completion. |
| 500-567 | Only the elected presenter may finish settle; non-presenter closure is inert. |
| 569-585 | Receiver-owned settle survives origin-window closure. |
| 587-611 | Custom settle claim/finish keeps world metadata coherent. |
| 650-708 | Host success completes once and the same source can immediately drag again. |
| 709-737 | Receiver observes host pointer in receiver-local coordinates. |
| 738-764 | Host drop resolves live Copy/Link modifiers. |
| 765-778 | Source success is committed before receiver user code. |
| 779-796 | Receiver-started replacement drag does not inherit/finalize the old session. |
| 797-819 | Ineligible windows retain geometry but cannot win hit testing. |
| 821-833 | Live/inert geometry status rerenders reactively. |
| 834-893 | Duplicate `ZoneId`s are internally window-qualified. |
| 894-910 | Outside cancellation completes once; repeated cancellation is inert. |

Also preserve:

- `tests/multiwindow_seam.rs`: unchanged cross-`VirtualDom` scheduler seam.
- `tests/runtime.rs`: unchanged 60-test runtime suite.
- Browser specifications: unchanged.
- Donor `state.rs` completion tests.
- Donor example tests, relocated to model/desktop modules.
- Windows dead-signal/close-order regressions and pointer-kind coverage.

Add new platform-policy tests for Windows raw-release gating, one-time filter setup, touch suppression, poller generation cancellation, backend capability selection, visibility eligibility, and repeated host/local completion idempotence.

## CI ledger

The donor `.github/workflows/ci.yml` is useful scaffolding but unsafe unchanged because it predates the root `desktop` feature.

| D lines | Decision | Treatment |
|---|---|---|
| 1-29 | PORT | Workflow triggers, permissions, concurrency, checkout, toolchain, and cache. |
| 30-45 | REPLACE commands | Do not use `--all-features` indiscriminately. Run default plus explicit portable `web,serde` gates; Wasm must use only portable features. |
| 47-63 | PORT | Native desktop OS matrix and cache layout. |
| 64-72 | PORT | Linux WebKitGTK/GTK/libsoup/libxdo dependencies for jobs that enable desktop. |
| 74-81 | PORT + extend | Check/test/build the standalone example and root `--features desktop`. Add Clippy and formatting. |

Use three lanes:

1. **Core/web Ubuntu:** default and `--features web,serde`; test, Clippy, rustdoc, and Wasm.
2. **Native adapter matrix:** Ubuntu, Windows, macOS with `--features desktop`; install Linux native packages.
3. **Standalone example matrix:** check, test, Clippy, and build both binaries.

Add `cargo fmt --check`, browser Playwright coverage, and a packaging assertion that `src/desktop/**` ships. Optional X11 and Wayland smoke jobs are useful; WSLg remains a smoke rig rather than platform authority.

Preserve `.github/workflows/pages.yml`; it is identical across the two trees and already pins Dioxus CLI 0.7.9.

## Documentation ledger

### README

Use Windows as the base.

- PRESERVE W 630-652: supported desktop feature and adapter explanation.
- PRESERVE W 878-911: Windows WebView2 evidence and current platform status, subject to fresh verification after the merge.
- REPLACE W 51-54 if it says `web` is the only dependency-adding feature.
- ADD an explicit `desktop` feature bullet.
- CLARIFY: `DndWorld` and payload sharing are core; the supported Tao/Wry host adapter requires `desktop`.
- PORT donor Wayland detection, app-lifetime model ownership, qualified identity, and settle ownership only after the code lands.
- DISCARD donor claims that the bridge is merely example-local or Windows is unverified.

### CHANGELOG

- PRESERVE W 43-203 in substance: feature promotion, raw bridge, pointer-kind behavior, fatal callback investigation, same-source recovery, and N-way trays.
- APPEND the reconciled lifecycle/settle/modifier/eligibility/identity/model hardening after implementation.
- UPDATE stale-gesture recovery as defense-in-depth once exactly-once completion fixes the root cause.
- UPDATE the global-unique-zone statement if qualified identity lands.
- Do not replace the detailed Windows history with the donor's short summary.

### Existing review and local notes

- `DESKTOP-DIOXUS-0.7-REVIEW.md` reviewed obsolete `12ae7ad`; salvage requirements, not its current verdict or line links.
- Preserve Windows `UPSTREAM-multiwindow.md`; it contains the WebView2 and dead-signal evidence absent from the donor copy.
- Treat WSL `TODO.md` Windows-pending statements as historical and stale.
- Convert `WINDOWS-TEST.md` from disposable scratchpad into a durable verification checklist or committed platform document.
- Preserve Windows `.gitignore` lines 8-13; the donor's apparent deletions must not be carried.

## Recommended implementation sequence

1. **Anchor on Windows `0320047`.** Create the implementation branch from the canonical checkout. Do not stage NTFS executable-bit noise.
2. **Split the adapter without changing behavior.** Convert `src/desktop.rs` into `src/desktop/` and prove Windows build/API parity.
3. **Fix zone ownership.** Move mounted/rect data into provider-owned registry storage and require a clean native launch without Dioxus cross-scope warnings.
4. **Split core monoliths without semantic change.** Establish `components/`, `session.rs`, and `world/` facades with current exports.
5. **Port tracked source completion.** Preserve Windows pointer APIs and dead-signal safety while adding commit/finalize/session behavior.
6. **Port world invariants.** Qualified identity, receiver-local pointer, modifiers, eligibility, and settle owner/generation.
7. **Reconcile desktop platform implementations.** Add explicit Linux backend detection and generation-aware polling while retaining Windows raw input.
8. **Reconcile the N-way example.** Apply `Rc<ModelOwner>` to the Windows model, not the donor's single-tray model.
9. **Port/adapt tests.** Land regression coverage with each behavior slice.
10. **Repair CI and docs.** Preserve Windows evidence, add current limitations, and use explicit feature lanes.
11. **Run native interactive verification.** Windows is the authoritative WebView2 pass; real X11 and Wayland sessions verify Linux. macOS remains unverified until exercised.

## Acceptance criteria

### Architecture

- No `dioxus_desktop`, Tao, Tokio, WebView2, `WM_INPUT`, X11, Wayland, AppKit, or OS `cfg` imports in core modules.
- `desktop/mod.rs` is a thin public facade.
- Platform modules are sealed/private.
- The example contains no private geometry feed or drag bridge.
- Existing public adapter and core imports remain source-compatible unless an explicit breaking change is approved.

### Dioxus ownership

- Native launch produces no unexplained “Copy Value created in child scope, used in parent scope” warning.
- Closing any window does not read/write a disposed zone or model signal.
- Closing the board first leaves trays able to read, mutate, and rerender.
- Final owner disposal releases model storage.

### Drag lifecycle

- Local completion, host completion, host cancellation, pointer cancellation, and source unmount converge on one exactly-once source result.
- `on_drag_end` fires exactly once with the committed result.
- Receiver code may remove the source or start a new drag without stale cleanup touching the replacement.
- A source can begin a new drag immediately after host success or cancellation.

### Multi-window behavior

- Open at least three independent trays.
- Board -> tray 1 -> tray 2 -> tray 3 -> board works.
- Duplicate explicit `ZoneId`s in different windows do not mirror hover or misroute delivery.
- Ctrl/Cmd/Alt effects match local delivery.
- Receiver edge/tree/autoscroll logic uses receiver-local pointer coordinates.
- Hidden/minimized windows cannot win hit testing.
- Receiver settle survives origin closure; non-presenter closure cannot finish it.
- Dead-space release cancels once and the next drag works.

### Platform verification

- Windows 11/WebView2: raw motion/release path verified with Computer Use, including close-order and same-source restart.
- Linux/X11: global dragging on.
- Linux/Wayland: global dragging explicitly off while local dragging remains correct.
- WSLg: smoke both actual Wayland and forced X11; do not treat WSLg as the only Linux authority.
- macOS: compile in CI and remain documented as unverified until a runtime pass.
- Mixed-DPI movement preserves pointer/ghost scale.

### Build and packaging

- Core default and `web,serde` checks/tests/Clippy/rustdoc pass.
- Wasm checks use portable features only.
- Root `desktop` feature passes on the native OS matrix.
- Standalone desktop example checks, tests, Clippy, and both binaries build.
- Browser suite remains green.
- `cargo package --list` includes every `src/desktop/**` module.
- Root and nested lockfiles are regenerated, not manually merged.

## Final preserve / do-not-preserve summary

### Preserve from Windows

- Canonical Git base and pushed history.
- Public desktop feature and adapter path.
- WebView2 raw-input implementation and its rationale.
- Pointer-kind public API and touch suppression.
- Dead-signal fallibility until registry ownership is corrected.
- N-way trays and close cleanup.
- Detailed Windows verification documentation.
- Native dependency graph and `required-features` manifest entry.

### Port from the WSL donor

- `DragSessionId` and two-phase source completion.
- Commit-before-receiver-callback ordering.
- Window-qualified `ZoneLocation`.
- Receiver-local/global pointer tracking.
- Live modifiers for host drops.
- Window eligibility and explicit Wayland capability.
- Settle owner and generation.
- Cancellable generation-bound host polling.
- App-owned `Rc<ModelOwner>` lifetime.
- Expanded simulation, multi-window, model, modifier, and capability tests.
- CI scaffolding after feature-matrix correction.

### Do not preserve

- Donor example-local adapter, missing Windows raw input.
- Donor one-tray data model.
- Donor removal of desktop feature, registry hardening, and Windows evidence.
- Windows error-based Wayland detection.
- Windows uncancelled poller.
- Board-owned shared model signals.
- Child-owned registry geometry signals.
- Core monolith growth as the final file organization.
- WSL-specific runtime branching.
- Unqualified claims that macOS is verified.
- `--all-features` on Wasm.

This reconciliation should be implemented as a manual port onto `0320047`, not as a directory copy, patch application, or merge of the WSL working tree.

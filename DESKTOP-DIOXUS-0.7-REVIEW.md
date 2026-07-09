# Desktop and Dioxus 0.7+ Review

- Date: 2026-07-10
- Scope: desktop-specific implementation, especially multi-window drag-and-drop
- Reviewed revision: 12ae7ad on development
- Dioxus baseline: 0.7.9
- Review mode: read-only; no implementation changes were made

## Verdict

The desktop architecture is thoughtful and already targets the latest stable
Dioxus release, 0.7.9. The standalone native crate keeps Wry and Tao out of
normal core builds, the coordinate-space model is unusually clear, and the
cross-VirtualDom seam has meaningful tests.

The multi-window work should not be treated as release-ready yet. Four P1
issues should be fixed before shipping it in the 2.5 release train. This is
lifecycle and platform correctness work; moving the mainline to the Dioxus 0.8
prerelease would not solve it.

- **P1:** fix before release
- **P2:** correctness hardening immediately after the P1 work
- **P3:** maintenance or ergonomic improvement

## Findings

### P1 — Wayland degradation relies on error paths Tao does not take

[use_window_geometry_feed](examples/desktop-multiwindow/src/main.rs#L70)
assumes inner_position() and cursor_position() fail on Wayland. The resolved
Tao 0.34.8 backend instead returns successful-looking placeholders:

- Wayland cursor position is explicitly Ok((0, 0)) in
  [Tao's Linux backend](https://github.com/tauri-apps/tao/blob/tao-v0.34.8/src/platform_impl/linux/util.rs#L18-L21).
- Linux inner_position() returns stored GTK coordinates as Ok(...) in
  [Tao's window implementation](https://github.com/tauri-apps/tao/blob/tao-v0.34.8/src/platform_impl/linux/window.rs#L457-L462).

This can mark Wayland windows as geometrically live at bogus or overlapping
origins rather than cleanly degrading to per-window dragging. The cursor
poller can also continue with a fake global (0, 0) coordinate.

Recommended correction:

1. Import EventLoopWindowTargetExtUnix on Linux.
2. Use target.is_wayland() through the event-loop target currently discarded
   at [main.rs:92](examples/desktop-multiwindow/src/main.rs#L92) and
   [main.rs:240](examples/desktop-multiwindow/src/main.rs#L240).
3. Keep geometry inert until the backend capability is known.
4. On Wayland, clear geometry and never start the global cursor poller.
5. Do not use (0, 0) itself as a sentinel because it is valid on X11.
6. Make WindowGeometry::live() reactive. It currently uses peek(), so a
   live-to-inert transition need not rerender the displayed status.

Add an extracted capability test plus a Wayland smoke test that asserts the
window table never becomes globally hittable.

### P1 — Host-completed drops bypass the source Draggable lifecycle

The bridge calls DndWorld::drop_at_global directly at
[main.rs:249](examples/desktop-multiwindow/src/main.rs#L249). That method
consumes or cancels shared drag state at
[world.rs:531](src/core/world.rs#L531), but the source gesture phase is local
to Draggable at [components.rs:308](src/core/components.rs#L308). Only the
normal source-side finish path resets the state machine and invokes
on_drag_end at [components.rs:391](src/core/components.rs#L391).

Observable consequences:

- on_drag_end is skipped for a host-completed drop or cancellation.
- The source can remain in GesturePhase::Dragging after a successful drop.
- The next press is ignored because the state machine deliberately preserves a
  dragging phase on a second Down at [machine.rs:141](src/core/machine.rs#L141).
- A later pointer event can report a false cancellation after the payload was
  already delivered successfully.

The bridge also guards its native event match with dragging_foreign. A raw
release delivered to the origin is ignored, while a later foreign CursorMoved
can become the release. A stationary release may remain pending, and later
movement can change the selected target.

Recommended correction:

1. Give each drag a generation or completion token.
2. Register an origin-runtime completion callback when the source starts.
   Dioxus Callback already carries its originating runtime.
3. Funnel local drop, host drop, host cancel, pointer cancel, and close-order
   cancellation through one exactly-once completion operation.
4. Reset the source gesture and pointer-capture state and call
   on_drag_end(result) exactly once.
5. Handle origin-side raw release idempotently and keep first-foreign-event
   inference only as a fallback.

Add regression coverage for host completion followed immediately by another
drag from the same source, plus callback-count assertions for success and
cancellation.

### P1 — Closing the board invalidates the surviving tray model

The model signals are created under the board VirtualDom at
[main.rs:112](examples/desktop-multiwindow/src/main.rs#L112), then copied into
the tray root context at
[main.rs:122](examples/desktop-multiwindow/src/main.rs#L122).

Dioxus signals are disposed when their owning component unmounts. Closing the
board therefore leaves a surviving tray holding dead signal handles, even
though DndWorld itself was deliberately made process-lived to support any
window close order. A later tray read, write, or rerender can panic.

Recommended correction:

- Put shared application state under a reference-counted app-state owner cloned
  into every window, or retain a non-closable controller root.
- Prefer the reference-counted owner so the state outlives the opener but is
  released when the final window closes.
- Do not replace it with a plain GlobalSignal; the repo's upstream
  investigation records globals as runtime-local across independent windows.

Extend the creator-close test so the tray reads, mutates, and rerenders the
shared model after the board closes.

### P1 when settle is enabled — A non-presenter can terminate another window's settle

Every settle-enabled overlay reacts to the shared settle state at
[components.rs:1287](src/core/components.rs#L1287), while presenter election
happens later at [components.rs:1371](src/core/components.rs#L1371).

If every window uses the natural shared shell with settle enabled, a
non-presenting overlay without a mounted ghost can call finish_settle.
Unmounting any settle-enabled overlay also finishes the global settle through
[components.rs:1243](src/core/components.rs#L1243), even when another window
owns the glide. Its on_settled callback can likewise come from the wrong
window.

Recommended correction:

- Make settle ownership an explicit world-level value.
- Gate measurement, transition completion, cleanup, and on_settled on the
  elected presenter.
- Test three windows with settle enabled, including closing a non-presenter
  during the receiver's glide.

### P2 — Host delivery is not feature-equivalent to local delivery

Normal delivery resolves Ctrl/Cmd/Alt at
[components.rs:391](src/core/components.rs#L391). Host delivery uses only the
drag-start base effect at [world.rs:544](src/core/world.rs#L544), so
cross-window Copy or Link behavior differs from an in-window drop. Track Tao
ModifiersChanged and carry the resolved effect into host completion.

There is also a coordinate-space mismatch. track_global stores the pointer in
origin-window client pixels at [world.rs:498](src/core/world.rs#L498), while
receiver components compare it with receiver-local rectangles:

- [DropZone live edge calculation](src/core/components.rs#L898)
- [TreeNodeTarget live intent calculation](src/tree.rs#L193)

Final delivered coordinates are converted correctly, but live data-edge and
tree intent can be wrong, particularly across translated or mixed-DPI
windows. Receiver-side autoscroll also has no host-driven pointer feed while
the foreign webview is event-blind.

Store the global pointer in the world, expose a joined-window-local conversion,
and use it for receiver intent and autoscroll.

### P2 — Polling tasks can overlap across rapid drag restarts

[DragBridge](examples/desktop-multiwindow/src/main.rs#L201) starts a task from
use_effect but discards the returned Task. Dioxus cancels a spawned task when
its component unmounts, not when an effect reruns.

Normally the old loop exits after observing an idle state at its next 30 ms
tick. If a new drag begins before that sleeper wakes, it sees an active drag
again and survives while the rerun effect starts another poller.

Retain and cancel the prior Task on every effect run, or use a reactive
use_resource. Also capture a drag generation and re-check the origin window
inside the loop.

### P2 — Zone identity is not window-qualified

Shared state stores only a ZoneId for source and hover at
[state.rs:26](src/core/state.rs#L26). Reusing an explicit ID in two windows can
light both zones, prevent hover cleanup, and make cross-window
DropOutcome.from/to ambiguous.

Prefer an internal (WindowKey, ZoneId) identity and expose optional source and
target window metadata. If that is too disruptive for 2.5, document and
debug-enforce world-global explicit IDs.

### P2 — Window eligibility and pointer kind need representation

- Hidden, minimized, or otherwise ineligible windows retain geometry and can
  win [window_under](src/core/world.rs#L413).
- The bridge polls the OS mouse cursor for every pointer drag at
  [main.rs:210](examples/desktop-multiwindow/src/main.rs#L210), including touch
  and pen drags whose location may be unrelated to the mouse cursor.

Add an active/visible capability to WindowGeometry, record pointer kind in drag
state, and document multi-window dragging as mouse-only until touch and pen
have a reliable native bridge.

## Dioxus 0.7+ opportunities

### Promote the desktop glue into a supported adapter

Once the correctness issues are fixed, move the geometry feed and bridge into
either an optional desktop adapter module or a small dioxus-dnd-desktop
companion crate.

This gives consumers one supported hook/component rather than roughly 200
lines of subtle copied host code while keeping Wry and Tao out of default core
builds. Keep use_wry_event_handler: it is the correct Dioxus 0.7.9 API and
already restores the originating runtime and unregisters during cleanup.

### Replace process-lived world leaks with reference-counted ownership

WORLD_OWNERS is append-only at [world.rs:80](src/core/world.rs#L80). The leak
is bounded only when callers create one world forever. Root remounts, hot
reload, long-running tests, or multiple independent worlds can accumulate
state.

Store the signal owners in a reference-counted WorldInner. Every window can
hold a clone, so the opener may close first while the final window dropping
still releases the world normally.

### Complete the accessibility story across windows

The world is currently pointer-aware while keyboard traversal remains local.
The next product-level opportunity is world-aware acceptable-zone ordering
plus a host focus callback for moving keyboard focus between windows. The same
receiver-local pointer work should add cross-window autoscroll.

### Add native CI and package-facing documentation

The standalone desktop crate is intentionally outside the root workspace, so
ordinary root checks cannot catch its API drift. The current workflow only
builds the web gallery on main.

Add pull-request jobs for:

- root tests and Clippy;
- Wasm checking;
- standalone desktop check/build on Linux, Windows, and macOS;
- extracted backend-capability tests;
- optional display-session X11 and Wayland smoke tests.

The README directs readers to examples/desktop-multiwindow, but
cargo package --list omits that nested package. Link the example directly to
GitHub from the crates.io-facing README or restructure it into a distributable
example.

### Small cleanups

- Remove the unused direct serde dependency and unnecessary library serde
  feature from examples/desktop-multiwindow/Cargo.toml.
- Correct the stale Dioxus 0.8 comments at
  [state.rs:9](src/core/state.rs#L9) and
  [platform.rs:8](src/core/platform.rs#L8).
- Refresh cached zone rectangles on window resize and scale-factor changes
  during an active drag.

## Strengths to preserve

- The standalone desktop package keeps Wry, Tao, WebKit, and their system
  dependencies out of normal library builds.
- Client CSS pixels versus global physical pixels are documented and isolated
  at a clear boundary.
- Mixed-DPI conversion and per-window overlay scaling are cleanly separated.
- Provider join/leave behavior handles hovered-window and origin-window closure
  thoughtfully.
- Multi-window, mixed-DPI, host-drive, close-order, and cross-VirtualDom seams
  have targeted headless coverage.
- File-drop filtering is correctly documented as advisory rather than a
  security boundary.

## Recommended implementation order

1. Correct Wayland capability detection and tracked geometry status.
2. Introduce exactly-once world/source completion and fix raw release handling.
3. Move the example model onto app-lifetime, reference-counted ownership.
4. Make settle ownership explicit and presenter-gated.
5. Carry global pointer, receiver-local pointer, modifiers, pointer kind, and
   window eligibility through the world.
6. Cancel or generation-gate polling tasks.
7. Qualify zone identity by window.
8. Add the missing regression tests and native CI matrix.
9. Promote the proven glue into a supported desktop adapter.
10. Address packaging, rustdoc, formatting, and dependency cleanups.

## Verification record

Passed during this review:

~~~text
cargo check --all-targets --all-features
cargo test --all-features                         # 174 passed, 21 ignored
cargo clippy --all-targets --all-features -- -D warnings
cargo check --locked --target wasm32-unknown-unknown --features web

# examples/desktop-multiwindow
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
dx check --locked
dx build --desktop --locked
dx build --desktop --locked --bin probe

# dependency audit
cargo audit
~~~

The root audit reported no vulnerability or warning output across 336
dependencies. The standalone desktop stack reported no vulnerability, but did
report allowed transitive maintenance and unsoundness warnings in the current
Dioxus/Wry GTK dependency graph. Those require upstream tracking rather than a
local code fix.

Existing gates that were not clean:

- RUSTDOCFLAGS=-D warnings cargo doc --locked --all-features --no-deps fails on
  the private WindowRecord::refresh link at
  [world.rs:475](src/core/world.rs#L475) and an ambiguous store link at
  [external.rs:181](src/external.rs#L181).
- cargo fmt --all -- --check reports existing formatting differences,
  including the desktop example.

No interactive GUI/display-session run was performed during this review, and
Windows and macOS runtime behavior remain unverified. The worktree was clean
before this review document was added.

# Changelog

## 3.0.0 - 2026-07-11

### Added

- **`docs/`: a full documentation tree, paired by concept.** One
  plain-language guide (`docs/concepts/`, 22 files) and one developer
  reference (`docs/api/`, 19 files) per concept, indexed from
  `docs/README.md`. The API references are also the rustdoc: each module's
  `//!` block is replaced by `#![doc = include_str!(...)]` pointing at its
  `docs/api/` file, so one file serves GitHub and docs.rs and cannot
  silently drift from the crate. Writing them surfaced and fixed a handful
  of stale claims: pens follow the finger promotion rules (not the mouse
  rules) under `TouchSense::Auto`, `data-intent` renders for pointer drags
  only, `DropOutcome::effect` is the modifier-resolved effect, dragging an
  unselected `SelectableDraggable` leaves the selection unchanged,
  `BoardSlot` styling should reveal without reflow, sortables ship no
  keyboard drag of their own (`ReorderButtons` is the path), and the typed
  `DataTransfer` transport is for separate apps rather than one app's
  windows.

- **`#[non_exhaustive]` on the event types that provably grow**
  (technically breaking if you exhaustively matched or literally
  constructed them - batched deliberately into the same release whose
  `GestureEvent::Hold` already broke exhaustive matchers):
  `GestureEvent`, `FileRejection`, `DragMode`, `PointerKind`,
  `CanvasDrop`, and - with new `::new()` constructors replacing literal
  construction - `SortEvent`, `MoveEvent`, `TreeDropEvent`. Deliberately
  NOT marked: `DropEffect` and `DropIntent` (closed vocabularies -
  HTML5's `dropEffect` set and Before/After/Into geometry - that
  consumers rightly match exhaustively), and `DropOutcome` (custom
  sources and downstream tests legitimately construct it; its field set
  IS the drop contract, and a nine-field builder would cost more API
  than the marker saves). CI grew a non-blocking `cargo-semver-checks`
  job against the latest published release, plus a tripwire asserting no
  tao/wry type ever leaks into the public `desktop` API surface.
- **`docs/RENDERER-CONTRACT.md`: the renderer contract, written down.**
  With Blitz/dioxus-native shipping as a webview-less sibling render
  target, the crate's browser/DOM dependencies are now enumerated
  explicitly - thirteen behaviors, each with the modules that need it,
  what breaks without it (only one, `transitionend`, can wedge state
  rather than degrade), file:line citations, and an honest
  unknown-means-unknown Blitz status. A draft tracking-issue body
  (`docs/BLITZ-TRACKING-ISSUE.md`, not filed) maps every module to
  works-as-is / needs-native-impl / N-A-under-Blitz and names the
  three blocking questions a first probe should answer.
- **Windows platform-model tripwire**: the raw-input bridge exists
  because tao never delivers `CursorMoved`/`MouseInput` on
  Windows/WebView2 (the child HWND consumes them). If a WebView2 or tao
  update ever changes that routing, the bridge now emits one
  `tracing::warn!` per drag naming the contradicted assumption - and
  deliberately does not act on the events, so the raw-input leg keeps
  sole ownership instead of double-driving the drag. The host
  `track_global` docs now also record why overlapping legs are safe by
  construction (same-tick idempotence, single-thread serialization, and
  per-leg generation re-validation) rather than by leg exclusivity.
- **Runtime bridging kill switch + per-leg diagnostics**:
  `DndWorld::set_bridging(false)` (read back with `bridging_enabled()`)
  stands down every host-side bridge leg AND the world's own
  `track_global`/`drop_at_global` entry points - the lever for the day a
  webview or OS update ships a cross-window regression that a rebuild
  cannot wait for. Drags degrade to per-window, the already-modeled
  Wayland behavior; local drags, settle and delivery are untouched, and
  `cancel_drag` stays live as an escape hatch. End users can set
  `DIOXUS_DND_NO_BRIDGE=1` before launch (read once at world creation)
  to flip the same switch without a rebuild. With `tracing` at `debug`
  (`desktop` feature), each leg now logs when it engages a drag
  (`cursor-poller` / `release` / `x11-deadspace` / `raw-input`), so a
  post-update bug report arrives pre-triaged; tracing was chosen over
  surfacing legs in `DndDebugOverlay` because the overlay is core and
  must not learn desktop leg names.
- **Scheduled upstream canary CI**
  (`.github/workflows/upstream-canary.yml`): the multi-window design rides
  two cross-VirtualDom contracts that Dioxus does not document as public
  API (`tests/multiwindow_seam.rs`), so a weekly workflow now runs the
  full suite against the latest published Dioxus 0.7.x patch and the two
  contract test files against Dioxus git main - upstream breakage
  surfaces as a named red job instead of a user's bug report after
  `cargo update`.

- **Typed transport components** (`serde` feature):
  `TypedDragSource<T>` serializes its payload to JSON under
  `application/json` at drag start - always alongside a `text/plain`
  fallback (defaults to the JSON; override with `fallback_text`) so
  non-typed targets still receive something legible - and
  `TypedDropZone<T>` decodes drops back to `T` as a `TypedDrop<T>`
  (payload + client/element points), silently ignores untyped drags, and
  reports undecodable JSON through `on_invalid`. `external::typed` grew
  the underlying seam (`store_in`/`retrieve_from` over a `DataTransfer`,
  plus the `MIME` const) and its first test coverage: headless
  round-trips through a mutable `DataTransfer` double and Playwright
  specs driving real `DragEvent`s. Fixed along the way: an untyped drop
  no longer reads as a decode error on web, where the DOM's `getData`
  returns `""` for absent formats rather than null.
- **Multi-window desktop drags** (`core::world`): drag between windows of
  one desktop app with the payload as a live Rust value - no
  serialization, no `DataTransfer`. `use_dnd_world::<T>()` creates a
  process-lived `DndWorld<T>` (windows may close in any order); pass it to
  sibling windows via `VirtualDom::with_root_context` and each window's
  `DndProvider` joins automatically. Zones light up and deliver across
  windows through the shared context; `DragOverlay` elects exactly one
  presenting window per frame (scale-aware, so mixed-DPI ghosts keep their
  physical size), and drop-settle glides in the receiving window. Feed
  each window's `WindowGeometry` from your windowing layer (recipe plus a
  working two-window example in `examples/desktop-multiwindow/`, probe
  binary included); without geometry - Wayland - drags gracefully stay
  per-window. Host-drive API (`track_global`, `drop_at_global`,
  `cancel_drag`, `use_joined_window`) lets desktop glue bridge what
  webviews cannot see: pointer events stop at the viewport edge, and
  non-origin windows are event-blind while a button is held, so the
  origin's glue polls the global cursor and a blind window's first event
  completes the drop. The headless `DragSim` speaks world too
  (`place_in`, `window_key`), and two new test suites
  (`tests/multiwindow.rs`, `tests/multiwindow_seam.rs`) pin the
  cross-VirtualDom contracts this rides on.

- **`desktop` cargo feature: the multi-window windowing glue is now
  library API** (`dioxus_dnd::desktop`), promoted from the
  desktop-multiwindow example once the per-platform behavior was probed
  and hand-verified - there is no first-class multi-window story in
  dioxus 0.7 to defer to, so this crate carries it. Two exports, the
  exact pair every window of a multi-window app needs:
  `use_window_geometry_feed()` (call ABOVE the `DndProvider`; feeds the
  window's position/size/scale into a `WindowGeometry` from tao
  move/resize/focus events, leaving geometry cleared on Wayland where
  positions are unknowable) and `DragBridge::<T>` (render INSIDE the
  provider; the three-legged host-side bridge for pointer drags that
  leave the origin window - global-cursor poller, foreign-window
  release detection, and the Windows raw-input leg with
  `DeviceEventFilter::Never`, all gated per the drag's `PointerKind` so
  touch is never double-driven). The module docs carry the full
  per-platform truth table, including the process-global
  `set_device_event_filter` caveat. Off by default: the feature pulls
  dioxus-desktop (wry/tao) plus a `time`-only tokio, and the core stays
  dependency-free. The example now consumes the feature and shrank to
  model + UI; behavior verified unchanged by re-running the full
  scripted matrix (mouse and injected-touch cross-window drags,
  dead-space cancel and recovery, tray close/reopen) against the
  promoted glue. The crate-layout ladder (example code, then feature,
  then a `dioxus-dnd-desktop` companion crate only if the glue grows
  again or needs its own release cadence against tao/wry churn) was
  recorded in the 3.5 plan; this lands the middle rung.
- **`PointerKind` (`Mouse`/`Touch`/`Pen`) recorded per drag**: the
  shared drag state now remembers which pointer device initiated a
  pointer drag. `Draggable` records it at pickup from the initiating
  event's `pointerType` (`ctx.set_pointer_kind`, exposed as
  `ctx.pointer_kind()`; custom sources that never set it get the safe
  `Mouse` default, and keyboard drags read as `Mouse` with
  `mode() == Keyboard`). The point of the API is host-side glue: a
  touch contact is implicitly captured by the browser, so the origin
  webview itself streams the entire gesture (out-of-viewport moves and
  the release included) to the source element and needs NO bridging,
  while mouse and pen go blind at the viewport edge whenever native
  capture is unavailable and need all of it.
  `PointerKind::implicitly_captured()` encodes exactly that decision.

### Fixed

- **Desktop bridge policy and ownership are now explicit per platform.**
  Linux asks Tao's live event-loop target whether it selected Wayland or
  X11 instead of treating a cursor/geometry API error as backend
  detection. Wayland deliberately leaves global geometry, cursor polling
  and foreign-release bridging inert while local drags continue normally;
  X11 keeps those legs and additionally observes the root pointer-button
  mask so a release over desktop dead space cancels on the next sample.
  Pollers are bound to the originating world/session generation;
  superseded runs cannot mutate a replacement drag, and a transient X11
  cursor-query miss
  skips one tick instead of permanently killing the run. Tao modifier
  changes now update the shared world outside the origin viewport, and a
  joined window refreshes active rects after resize or scale changes. The
  Windows raw-input leg now revalidates the same composite generation
  immediately before tracking or dropping, while its
  `DeviceEventFilter::Never` setup is Windows-only and claimed once for
  the process. Pure policy tests cover every ownership gate, touch
  suppression, raw-release/filter decisions and completion idempotence.
  Verified under WSLg with Tao reporting actual Wayland (global legs off,
  local drags intact) and forced X11 (cross-window drop, dead-space cancel
  and immediate re-drag). The authoritative Windows/WebView2 runtime pass
  then confirmed the surgical Windows changes end to end (Win 11 Home
  ARM64 build 26200, 1920x1200 at 1.5x, 2026-07-10, real `SendInput` and
  `InjectTouchInput`): the full four-window matrix - generation-bound raw
  tracking, dead-space cancel with immediate same-source restart,
  mid-drag target resize rect refresh, hovered-window and origin-window
  closes mid-drag, minimized-window exclusion, close/reopen churn,
  touch/mouse interleave with zero ghost-trajectory reversals, and the
  board-first close-order regression - with clean logs (no ownership
  warnings, panics or fatal callbacks) and clean exits. Multi-DPI remains
  unexercised there (single-monitor rig).
- **A press, cancel, or lost capture racing a source's completion no
  longer aborts the process.** `Draggable`'s stale-session retire (on
  pointerdown), pointer-cancel, and lost-capture paths all guarded on
  `if let Some(id) = *session.peek()` and called
  `finish_pointer_source` inside the body. Edition-2021 scrutinee
  temporaries keep that read guard alive through the body, and the
  finish synchronously runs the source-completion callback, whose
  `session.set(None)` then hits `AlreadyBorrowed` - a panic that lands
  in an unwind-proof Win32 callback and kills the process
  (`0xc0000409`). Observed live on Windows 11 under scripted rapid
  input while filming the showcase; all three sites now copy the
  session id out of the peek before finishing. Verified by a
  drop-chased-by-rapid-press hammer across windows (six rounds, clean
  log); the equivalent `SortableList`/`SortableGrid` handlers already
  kept their state changes outside the guarded body and are unaffected.
- **Windows drops now honor modifiers changed outside the origin
  viewport** (Ctrl=Copy, Alt=Link). Nothing fed the shared world once
  the pointer left the origin: tao never fires `ModifiersChanged` there
  because the WebView2 child HWND owns keyboard focus, the origin's
  streamed held-button events carry the correct `ctrlKey` but target
  `<html>` where no component handler hears them, and the foreign window
  is blind while the origin holds OS mouse capture - so a raw-leg drop
  resolved with the world's modifier snapshot still empty (probed live
  with DOM event recorders). The Windows raw-input leg now also consumes
  the raw keyboard stream its `DeviceEventFilter::Never` registration
  already delivers (`DeviceEvent::Key`), tracking each physical modifier
  side in a hook-local mask (releasing one Ctrl while the other is held
  cannot clear the state) and feeding the world only from the origin
  window during a live bridged generation. Found and verified by the
  Windows runtime matrix; pure policy tests pin the mask transitions.
- **Strict clippy passes on Windows targets again.** The reconciled tree
  left the shared portable legs, their bridge/world generation helpers
  and `GlobalCapability::Unavailable` reachable only from the cfg-gated
  Linux/macOS policy modules, so `cargo clippy -D warnings` failed with
  dead_code on every other desktop target. The shared module stays
  compiled everywhere per the sealed-platform layout policy (implement
  once, type-check on all toolchains), with `allow(dead_code)` scoped
  precisely to the targets whose policy never installs those legs.
- **Touch drags no longer glitch in multi-window use.** After the
  Windows raw-input bridge landed, touch drags jittered and could end
  early: Windows synthesizes MOUSE input from touch (the cursor trails
  the finger, and synthesized button transitions fire mid-gesture), so
  the bridge legs fed the drag from a second, laggier source alongside
  the touch pointer stream the webview was already delivering via
  implicit capture, and a synthesized left-button-up could complete the
  drop mid-drag. The desktop-multiwindow example now gates all three
  bridge legs (origin cursor poller, raw-input release/motion,
  foreign-window release detection) on
  `!ctx.pointer_kind().implicitly_captured()`: mouse and pen are
  bridged, touch is left entirely to the webview's implicit capture.
  Verified with real injected touch (`InjectTouchInput`) driving
  cross-window drags in both directions, interleaved with mouse drags:
  every drop lands, exactly one window highlights at a time, and the
  sampled ghost trajectory shows zero direction reversals (the
  double-driven jitter signature) in either window.
- **Dead-signal reads hardened across windows** (the observed
  `0xc000041d` process-kill, STATUS_FATAL_USER_CALLBACK_EXCEPTION, the
  DioxusLabs/dioxus#4466 failure class; hit once on Windows 11 after
  multi-window open/close cycles, with dioxus's read-after-scope-drop
  warning fingering the zone `rect`/`mounted` signals registered into
  the provider-lived registry). The mechanism: a `DropZone`'s
  `mounted`/`rect` signals and a window's `WindowGeometry` signals die
  with their owning scope, but copies of their records legitimately
  outlive it for a moment - in a cloned `registry.get` lookup, in an
  in-flight measurement (`refresh_rects`/`measure_all` WRITE the rect
  signal after an await, and the zone can unmount during that await),
  in a sibling window's cross-world hit-test racing that window's
  teardown, or in a windowing-layer callback that fires one event
  late. Any such touch panicked; on Windows the panic lands inside a
  Win32 callback that cannot unwind, which kills the process. All
  access now goes through degrading accessors: new
  `ZoneRecord::cached_rect()` / `mounted_handle()` (used by hit-tests,
  spatial sort, drop delivery and keyboard drops) and an internal
  `store_rect` that quietly drops a measurement whose zone died
  mid-flight, plus try-based reads AND writes throughout
  `WindowGeometry` (`set`/`clear`/`mark_focused` included). A dead
  zone now reads as unmeasured and a dead geometry as unknown, both
  states every consumer already models (unmeasured zones sort last and
  never hit; unknown geometry is the documented Wayland degradation),
  so this is honest degradation rather than error masking. The full
  suite plus a live four-window close-mid-drag soak pass with the
  change.
- **Pointer sources now complete exactly once across local and host-side
  endings.** Every pointer gesture carries a fresh `DragSessionId` and an
  origin-runtime completion callback. Local delivery, cross-window host
  delivery, host cancellation, pointer cancellation, lost capture, and
  source unmount all converge on that callback, so `on_drag_end` receives
  the committed result once even when the origin never sees pointerup.
  Successful delivery commits before receiver user code, then finalizes
  without touching a replacement drag the receiver started. This fixes
  the eaten next press after a cross-window drop or dead-space release at
  its source. The pointerdown mismatch reset remains as defense in depth
  for custom integrations that bypass tracked completion; it is no longer
  the mechanism that notices a host-ended drag. `SortableList` and
  `SortableGrid` remain unaffected because they do not hand their gesture
  machines to the shared world.
- **Multi-window world invariants: qualified identity, receiver-local
  pointers, live modifiers, window eligibility, and receiver-owned
  settles.** Five behaviors that used to lean on origin-window state now
  live in `core::world` proper:
  - Hover and source identity are window-qualified
    (`ZoneLocation { window, zone }`, with `source_location()` /
    `over_location()` on `DndWorld`): duplicate explicit `ZoneId`s in
    different windows no longer mirror each other's highlight or misroute
    delivery. Single-window `ZoneId` APIs are unchanged.
  - Receivers read the shared pointer in their own client coordinates
    (`JoinedWindow::local_pointer`), so `DropZone` edge readouts, tree
    drop intent and `AutoScroll`'s new host-fed `drag_pointer` prop stop
    reasoning in origin-window coordinates during cross-window hovers.
  - Host-side drops apply the modifiers held at release
    (`DndWorld::update_modifiers`, fed by `Draggable` from its DOM
    events), so Ctrl/Alt resolve to the same Copy/Link effects as local
    delivery, and each new drag starts from a clean modifier set.
  - `WindowGeometry` gained an `eligible` gate (fed by
    `use_window_geometry_feed` from visibility/minimize state): a hidden
    or minimized window keeps its last placement for restore but cannot
    win global hit-testing, and the live/inert status reads reactively.
    All eligibility access keeps the dead-signal degradation above.
  - Settles are owned. Delivery elects the receiving window with a
    generation-stamped claim before the shared context enters settling;
    only the elected presenter's overlay measures, retargets and
    finishes the glide, a stale generation can never finish its
    successor, non-presenter window closure is inert, and a
    receiver-owned settle survives its origin window closing (the
    release anchor and origin scale are snapshotted into world state).
    Custom world delivery uses the new `DndWorld::claim_settle` /
    `finish_settle_from`; the claim is required, not advisory - a
    claimless `take_settling` in a world presents nowhere and is only
    cleaned up at origin close or the next drag. Claim tokens are also
    intersected with the context's actual settle state, so custom code
    resetting the shared context mid-settle cannot leave a `SettleSlot`
    hidden on a settle that no longer exists.
- **`examples/regressions.rs` gained `required-features = ["serde"]`**
  in `Cargo.toml`, so a plain feature-less `cargo test` no longer fails
  compiling it. The fixture app uses the serde-gated typed transport
  (`TypedDragSource`/`TypedDropZone`) and has always documented itself
  as `dx serve ... --features web,serde`; Cargo just was never told.
- **The desktop-multiwindow example is properly N-way**: every tray
  previously rendered the same `model.tray` list AND registered the
  same `ZoneId(2)` in the shared world, so a second tray was a mirror
  of the first (same cards in both), hovering any tray highlighted all
  of them, and drops routed into the one shared list. Each "Open tray
  window" now mints a `Tray` record with its own `ZoneId::auto()` and
  its own card list owned by an application-lifetime `Rc<ModelOwner>`,
  independent of every window's `VirtualDom`; each open tray has a
  separately reclaimable signal owner, and every window holds the shared
  owner while it can touch the model. The board may therefore close
  before either tray without dropping the signals its survivors use;
  `move_card` routes by zone across the board and every open tray,
  falling back to the board for a zone that closed in the race between
  hit-test and delivery so a card can never vanish. Closing a tray
  atomically returns its cards to the board (in-flight drags included),
  retires it from the model, then reclaims that tray's storage; a borrow
  guard for every signal is acquired before mutation under the serialized
  VDOM-teardown owner condition, so cleanup cannot be partially applied,
  and repeated teardown is inert.
  A focused cross-`VirtualDom` regression reproduces board close, tray 1
  close, then a tray 2 mutation/rerender, and that exact sequence was
  also verified live. The wider N-way behavior was verified with four
  windows: a board->tray1->tray2->tray3->board drag chain, exactly one
  window highlighting at each hop, drags across an intervening tray,
  close-with-cards, and closing the drag's ORIGIN window mid-drag
  (world aborts the drag, the app survives, the card comes home).
  The lesson recorded then ("zone ids must be unique across the whole
  world") has since been superseded by window-qualified identity (see
  the world-invariants entry above): duplicate explicit ids are now safe
  for hover and delivery, and the example keeps unique ids only because
  its shared model keys each tray's card list by bare `ZoneId`.
- **Multi-window drags verified end to end on Windows (WebView2)**, and
  the example bridge grew the third leg that platform needs (no library
  change, no JavaScript). What the platform actually does, established
  by recording DOM pointer events and tao events in both windows during
  scripted real-input drags: (1) the origin webview keeps receiving the
  ENTIRE held-button mouse stream, including moves and the release far
  outside its own viewport, but those events target `<html>` (nothing
  retargets without pointer capture, and `capture_pointer` is a no-op
  off web), so no component handler ever hears them; (2) tao never
  fires `CursorMoved`/`MouseInput` while the cursor is over a webview,
  because the WebView2 child HWND consumes the mouse messages before
  the tao window sees them, leaving foreign-side release detection
  dead. Net effect: cross-window mouse releases were heard by nobody,
  drops never landed, and the drag wedged (touch always worked: implicit
  pointer capture retargets everything to the element). The example's
  `DragBridge` now also listens to Windows raw input, which no HWND can
  swallow: `DeviceEvent::MouseMotion`/`Button` via
  `use_wry_event_handler` (dioxus-desktop fans DeviceEvents out to
  every handler; only WindowEvents are per-window filtered), with
  `set_device_event_filter(DeviceEventFilter::Never)` because the
  default `Unfocused` filter registers without `RIDEV_INPUTSINK` and
  the foreground input owner is the WebView2 process's HWND, so raw
  input never arrives otherwise. On a raw button-up outside the origin
  viewport the bridge completes the drop through the existing
  `drop_at_global` at tao's `cursor_position()` (the same
  global-physical-px source the poller uses); raw motion feeds
  `track_global` at event rate, out-pacing the 30ms poller. Releases
  inside the origin viewport still go through the Draggable's own
  pointerup, keeping single-window semantics (snap, modifiers)
  untouched, and dead-space releases become clean cancels instead of
  parked drags. Full WINDOWS-TEST.md checklist passed on Win 11 ARM64
  at 1.5x scale; the per-leg platform story lives in the example source
  and the README platform notes.
- **Pointer drags on renderers without native capture** (desktop
  webviews; web without the `web` feature) no longer freeze when the
  cursor leaves the dragged element or container: while native capture
  is not engaged, `Draggable`, `SortableList` and `SortableGrid` render
  a full-viewport capture substitute that keeps the move stream flowing
  (with capture engaged the substitute never exists, so web behavior and
  DOM are unchanged).
- **Press detection and lost-release recovery hardened against corrupt
  button state** (observed on WSLg, where the display server's move-event
  button masks can be stale): mice now begin drags on the reliable
  trigger button instead of `is_primary`, and the "no buttons held"
  recovery in `Draggable`, `SortableList` and `SortableGrid` requires
  three consecutive empty moves instead of trusting one.

- **Size-matched ghosts**: `DragOverlay { match_source: true }` dresses the
  ghost in the grabbed element's measured client rect (recorded in the new
  `DragState::source_rect` / `dnd.source_rect()`, set by `Draggable` at
  pickup or via `set_source_rect` from custom sources). With sizes equal,
  the `pointer - grab` anchor is exact by construction: the ghost appears
  precisely over what you picked up, whatever rsx it renders - no shrink,
  no jump, no hand-tuned ghost widths.
- **`on_settled` on `DragOverlay`** - fires once when the drop-settle glide
  lands (including the degenerate no-glide cases), so completion effects
  (arrival flashes, sounds) can sequence off the ghost instead of racing
  it. Never fires for cancelled drags.
- **`SettleSlot` + `retarget_settle`** - the missing half of drop-settle.
  Wrap the element a drop just created and mark it `active`: it holds its
  space invisibly while the ghost glides (no second copy beside the
  ghost), re-aims the glide at its own measured rect via the new
  `DndContext::retarget_settle` (the ghost lands exactly where the element
  is, not at the zone's center - the overlay retargets smoothly even
  mid-glide), and reveals the element the instant the ghost unmounts. One
  object from pickup to landing.
- Gallery: ghosts stay where they teach - the reading list (the
  `DragOverlay` page) and the mailbox (multi-select count pill); the other
  demos keep their clean dim-in-place drags. On the reading list the drag
  is one object end to end: the original hides outright while in flight
  (the size-matched ghost lifts off from its exact rect), the flash waits
  for the glide via `on_settled`, and the drag-fade is a consistent
  `opacity-40` everywhere else.

### Fixed

- `DragOverlay` no longer renders during keyboard drags - the pointer never
  moves from the origin in that mode, so the ghost used to sit pinned at
  the viewport corner. Zones highlight and `LiveRegion` narrates instead.
- **A mouse press no longer focuses the `Draggable`.** `tabindex="0"`
  (there for the keyboard path) made the div mouse-focusable as a browser
  side effect; that stray focus outlived drops and - in lists whose nodes
  get reused across re-renders - could surface a focus ring on an
  unrelated item. `pointerdown` now runs `prevent_default()` (exactly what
  `SortableList` rows always did): no press focus, no selection noise,
  clicks on inner controls still fire, Tab-focus untouched.
- **Keyboard drops walk focus to the moved item.** The drop re-mounts the
  item elsewhere and the browser dumped focus on `<body>`, stranding
  keyboard users. The drop now records a refocus request
  (`request_refocus` / `claim_refocus` on `DndContext`, for custom sources
  too) and the landing `Draggable` claims it on mount and focuses itself.
- Gallery: the card loops are now keyed by id (reading list, newsletter,
  sprint board), so DOM nodes track cards instead of positions across
  drops.
- **No more ghost pop-in at pickup.** `Draggable` measures its rect at
  press time, so a `match_source` ghost is dressed synchronously the frame
  the drag begins - previously the measurement ran after promotion and the
  ghost appeared several frames late.
- **Native-behavior audit fixes** (the principle: browser side effects are
  suppressed on drag surfaces; only intended HTML semantics remain):
  - `SortableGrid` tiles now `prevent_default()` on pointerdown like every
    other drag surface - previously an `<img>`/`<a>` inside a tile could
    hijack the gesture with a native browser drag, and press could focus
    or start a text selection.
  - `TouchSense::Immediate` now pins `user-select`/`-webkit-touch-callout`
    like `Auto` (it only differed in `touch-action` by design).
  - `TouchSense::Auto` allows `pinch-zoom` again (`touch-action: pan-y
    pinch-zoom`) - two fingers were never a drag, and zoom is an
    accessibility floor.
  - Context menus are suppressed only while a gesture is in flight
    (Android's touch long-press menu tore the 250ms hold); idle
    right-clicks/long-presses keep the menu.
  - `SelectableDraggable` swallows the browser's trailing `click` after a
    completed drag - it used to collapse the just-dragged multi-selection
    back to a single item.
- Gallery: the arrival flash now animates `outline` instead of
  `box-shadow` - the cards' entire resting look (inset border, elevation)
  lives in box-shadow, so the old keyframes flattened the landed card for
  600ms and snapped its look back at the end.

- **Touch auto-sensing** (`TouchSense`, default `Auto`). `Draggable` and
  whole-row `SortableList` now carry `touch-action: pan-y` instead of
  `none`: a vertical finger swipe keeps scrolling the page, a short hold
  (250ms, finger still) or a sideways-dominant pull picks the item up,
  and a promoted drag then owns the touch (its `touchmove`s are
  cancelled, so the page can't pan mid-drag - dioxus-web's delegated
  listener on `#main` is non-passive, pinned by a browser spec). The
  scroll-trap that `touch_handle` existed to work around is gone by
  default; the grip stays available as an explicit affordance. Mouse
  drags are unchanged. `TouchSense::Immediate` restores the 2.4
  behavior for surfaces that never scroll. Under the hood the gesture
  machine gained `GestureEvent::Hold`, a `Promotion` policy enum and
  `transition_with` (the existing `transition` is unchanged, delegating
  with `Promotion::Distance`); the hold clock is a zero-size CSS
  animation whose `animationend` is the alarm, so the crate still has no
  timer dependency and unmounting cancels it by construction.

- **`bridge_drop_zone!` macro** - the `BridgeDropZone` recipe generated
  for *any* number of coexisting payload worlds, since Rust's lack of
  variadic generics is what caps the component form at `<A, B>`. Each
  `(Type, accepts_prop, on_drop_prop)` row becomes one world with typed
  callbacks; no `dyn Any` anywhere. Built on the new public
  `use_bridge_world` hook (register one zone id in `T`'s world on shared
  `mounted`/`rect` signals, get back erased `{active, over}` state);
  `BridgeDropZone` itself is now two calls to it.

### Changed

- **The X11 dead-space release leg no longer rides tao's xlib FFI
  re-export** (`desktop` feature, Linux only): the root pointer + button
  mask query now goes through a first-party `x11rb` connection (pure
  Rust, no extensions, no `unsafe`). tao's `platform::unix::x11::ffi`
  re-export is not part of its semver contract, so a tao minor could have
  stranded the leg mid-2.x. Behavior is unchanged: a failed sample is a
  transient miss, and the backend verdict stays with tao's
  `is_wayland()` - an XWayland session that would accept an X connection
  still never engages the leg.
- **`PLATFORMS.md`: the platform verification log moved out of the
  README.** The README's Platform notes had grown a full verification
  report (rigs, commits, per-session evidence, bridge mechanics); that
  detail now lives in `PLATFORMS.md` at the repo root, where regression
  reports can be compared against a concrete baseline, and the README
  keeps a per-platform status table, the wry#1639 hidden-window trap, and
  the macOS call for testers
  ([#20](https://github.com/kindintelligence/dioxus-dnd/issues/20)).
  Every fact moved; none were dropped.
- **Leaner published tarball**: internal working documents (the desktop
  reconciliation review, `docs/`) and the Playwright browser suite
  (`package.json`, `package-lock.json`, `playwright.config.js`,
  `tests/browser/`) no longer ship in the crate - a `cargo test` consumer
  can never run JS specs. `PLATFORMS.md` deliberately stays in: the
  README's platform table links to it.
- **`FlipItem` is no longer paint-timing dependent on web.** With the
  `web` feature, the reorder glide is armed synchronously on the real DOM
  element - inverted transform, forced style flush, release with the
  transition armed - so the browser is guaranteed to start the glide from
  the old position; the animation itself remains a compositor-driven CSS
  transition, entirely off the VDOM cycle. The render-twice path remains
  as the fallback without `web`, and keeps its experimental caveat.
- **Touch behavior change (deliberate):** under the new `Auto` default a
  quick vertical touch-pull on a `Draggable` scrolls instead of dragging;
  hold briefly or pull sideways to drag. Set
  `touch: TouchSense::Immediate` where the old reflex matters more than
  scroll-through. (`GestureEvent` also gained the `Hold` variant - a
  technically breaking addition if you exhaustively matched it.)

## 2.4.0 - 2026-07-09

### Added

- **Virtualized-list support, proven at 10,000 rows.** New gallery page
  **Archive** (Scale group): every row of a windowed 10k-row list is a
  `DropZone`, zones register/unregister as the list recycles, and drops
  land correctly even on rows that scrolled into existence mid-drag -
  pointer and keyboard alike. Two changes make it work. `DropZone` and
  `BridgeDropZone` now *measure themselves on mount* instead of waiting
  for the drag-start measurement or a scroll ping, so a zone mounting
  mid-drag is hit-testable the moment it exists (this also gives idle
  apps initial rects). And `AutoScroll` gained an `on_scroll` prop
  reporting the container's offset (sampled through `MountedData` after
  events it can observe - its own edge-scrolling above all) for driving a
  window. For user scrolls, drive the window from `onvisible` on the
  rendered rows - see the README's "Virtualized lists" section for the
  pattern, and the found-bug note below for why.

- **Headless test driver.** `dioxus_dnd::test` runs whole drag
  interactions inside a `VirtualDom` - in CI, no browser. Mount a
  `DragSimProbe<T>` in the provider under test, grab the captured
  `DragSim<T>`, `place` the zone rects (the headless stand-in for layout,
  which makes tests deterministic instead of flaky), then `pick_up` /
  `move_to` / `release`, asserting `over()`, the rendered
  `data-active`/`data-over` markup (via `rerender` + SSR), and your own
  model. `simulate_drag` wraps the common arc in one call;
  `release_as(DropEffect::Copy)` simulates the Ctrl-held copy drop.
  Releases mirror the pointer gesture - exact hit, else the 48px snap to
  the closest acceptable zone, else cancel - and
  delivery is the *same code path* as `Draggable`'s (extracted, not
  reimplemented), so acceptance filters, closest-edge enrichment and
  settle routing all behave exactly as in production. Not simulated:
  pointer capture, auto-scroll, and pre-snap re-measurement.

- **Debug overlay (dev-only).** `DndDebugOverlay<T>` draws every zone
  registered in a provider as a tinted outline pinned over the page -
  stable per-id colors, the zone's label and id in a tag, live `data-over`
  highlighting for pointer and keyboard drags alike, and per-zone
  acceptance while a drag is in flight (rejecting zones dim and go
  dashed). A status chip reports the census, including zones the registry
  hasn't measured (which draw no outline - if the inspector can't see a
  zone, neither can hit-testing, and now that's visible). Click-through
  by design, so it never changes the interaction it inspects; it also
  re-measures rects while idle so outlines don't wait for a drag.
  Supporting API: `ZoneRegistry::records()`, a subscribing read of every
  registered zone. Clearly marked dev-only: gate it behind
  `cfg!(debug_assertions)` or your own flag.

- **Localizable announcements.** Every phrase the crate voices now reads a
  `DndStrings` from context - keyboard announcements ("Picked up {name}…",
  "Dropped in {name}.", "Drag cancelled."), `ReorderButtons` aria-labels,
  the unlabeled-item/zone fallbacks, and `SelectionCount`'s badge (whose
  "{n} item(s)" also becomes properly pluralizable). Each field owns its
  whole sentence as an `Rc<dyn Fn(..) -> String>` with English defaults,
  so translations reorder and inflect freely; build one with struct-update
  syntax over `Default::default()` and provide it anywhere above the drag
  UI. The crate stays dependency-free - wire dioxus-i18n's `t!` or a plain
  match on your locale signal into the closures, which are called per
  phrase so a live language switch takes effect on the very next
  announcement. `use_dnd_strings()` is public for custom components. New
  gallery page **Packing list** (Voice group) shows the full dioxus-i18n
  wiring with a live English/Spanish toggle and a visible mirror of the
  screen-reader channel.

- **Closest-edge primitive.** `edge_of(point, rect, edges) -> Edge`: the
  generic "which edge am I nearest" signal for insertion indicators,
  public and pure (clamps the point into the rect; ties resolve top, then
  left). `EdgeSet` names the competing edges by stacking direction, like
  sortable's `Axis`: `Vertical` tracks top/bottom, `Horizontal`
  left/right, `All` every side. `DropZone` exposes it as an opt-in `edge`
  prop: while an acceptable pointer drag hovers, the zone carries
  `data-edge="top" | "right" | "bottom" | "left"` (live on every move,
  same contract as the tree's `data-intent`), and the delivered outcome
  records the edge held at release in the new `DropOutcome::edge` field -
  `None` for keyboard drops and non-opted zones, so handlers treat it as
  their neutral intent. Edges are physical and don't mirror under RTL.
  New gallery page **Itinerary** builds a drop-above/drop-below list from
  the attribute alone. (`DropOutcome` gained a field: construct it
  through the components as usual, or add `edge: None` in literals.)

- **Drop-settle animation.** `DragOverlay { settle: true }`: on a
  successful pointer drop the ghost no longer vanishes - it glides from
  the release point until its center meets the receiving zone's center,
  then unmounts on `transitionend` (tune with `duration`/`easing`,
  defaults 200ms `ease`). During the glide the context is *settling*:
  `dragging()` is already false (zones unlight and the drop handler ran at
  release), but `payload()` stays readable so the ghost keeps its content.
  Honors `prefers-reduced-motion` via `data-dnd-motion` (2.3.0's
  near-zero-not-zero duration means cleanup still runs). Cancelled drags
  and keyboard drops never settle. Works through `BoardItem` and
  `SelectableDraggable` too - anything delivering via the core
  `Draggable`. Hook users: `DndContext::{take_settling, finish_settle,
  settling}`; `DragState` gained a `settle` field (construct via
  `..Default::default()` if you build it literally). The gallery Reading
  list demos it.

- **`BridgeDropZone<A, B>`** promotes the documented cross-type bridge
  (README "Mixing payload types", the gallery's Standup page) from a
  user-land recipe to a crate component. One element holds the same
  `ZoneId` in two providers' registries, sharing its `mounted`/`rect`
  signals, so each world's hit-testing, keyboard navigation and
  announcements find it independently. Acceptance is per-world
  (`accepts_a`/`accepts_b`) and every drop arrives through its own typed
  callback (`on_drop_a`/`on_drop_b`) - no downcasts, no erased channel.
  Styling hooks match `DropZone` (`data-active`/`data-over`, gated on
  acceptance), it provides `ParentZone` so nested zones of either type
  ascend correctly, and it lives in the prelude. The gallery Standup page
  and the browser fixture now use it; the manual double-registration
  recipe stays documented on that page for three-plus worlds.

### Fixed

- **Zones mounting mid-drag were invisible to hit-testing.** Rects were
  only measured at drag start and on rect-refresh pings, both of which
  run before a newly recycled virtual-list row renders - so a drop on a
  row that appeared mid-drag missed it. Zones now measure on mount
  (browser regression pins the recycled-row drop).
- **dioxus-web 0.7 never delivers element scroll events**, found while
  building the virtual-list demo: `onscroll` handlers on scrollable
  elements simply don't fire (bisected against a plain div), and the eval
  channel drops JS→Rust messages that resolve after the receiver parked,
  so a listener-bridge can't carry the signal either. This crate's 2.3.0
  claim that `onscroll` covered wheel scrolling mid-drag was therefore
  never true on web; the auto-scroll path (which pings explicitly) always
  worked. Wheel and scrollbar coverage now comes from the `onvisible`
  row-sentinel pattern (see the Archive page) and `on_scroll` sampling;
  the dead `onscroll` handler is gone.
- **The 48px near-miss snap measured to the zone's center, not its
  edge.** A pointer or touch drop released just outside a zone falls
  back to the closest acceptable zone within 48px - but the distance ran
  to the zone's *center*, so any zone larger than ~96px never caught a
  near miss: release 5px beside a full-width tray and the drag
  cancelled. `hit_test_closest` now measures to the rect's nearest
  point. Surfaced by dogfooding the new headless driver.

### Tests

- Browser: a drop lands on a virtual row recycled in mid-drag (the whole
  visible window replaced after pickup - red without measure-on-mount),
  and a keyboard drag walks the mounted window and drops on a row.
- Runtime (dogfooding the new driver): a full pointer arc asserting the
  mid-flight `data-active`/`data-over` markup and the delivered outcome;
  releases respecting acceptance (rejecting zone cancels) and the 48px
  snap; `simulate_drag` landing with the receiving zone's closest-edge
  enrichment and a forced copy effect - proof the driver ends in the
  production drop path.
- Runtime: the debug overlay draws one outline per measured zone with its
  label, marks the hovered zone and per-zone acceptance during a drag,
  skips unmeasured zones while counting them in the status chip, and
  renders no drag markers while idle.
- Unit: the built-in English phrases are pinned. Runtime: a provided
  `DndStrings` reading a locale localizes `ReorderButtons` aria-labels and
  the `SelectionCount` badge in SSR output for both languages, while the
  no-context default stays English. Browser: a keyboard drag announces
  pickup, hover and drop through the provided strings, and flipping the
  locale mid-session changes the very next announcement (Spanish pickup,
  Spanish cancel) with no remount.
- Unit: `edge_of` nearest-edge selection, edge-set restriction, clamping,
  tie-breaking and the attribute string contract. Runtime: an edge-opted
  zone enriches pointer outcomes against its registered rect (keyboard
  drops and non-opted zones stay `None`); no `data-edge` renders idle.
  Browser: `data-edge` follows the pointer live within the hovered zone
  (top in the upper half even at the far left, flipping to bottom across
  the midline), the drop delivers the edge held at release, and the
  attribute leaves with the drag.
- Runtime: the settle state machine (payload readable while settling,
  hover cleared, guarded `finish_settle` that can't clobber a newer drag,
  `start` interrupting a glide), the mid-settle SSR markup (armed
  transition, release-point hold, motion marker + override sheet), and
  the non-settle overlay still vanishing on drop. Browser: a real drop
  keeps the ghost alive with `data-dnd-motion` while the zone unlights,
  then unmounts it on `transitionend`; a cancelled drag vanishes without
  settling.
- Runtime: `BridgeDropZone` registers in both registries with the synced
  label, per-world `accepts` filters payloads (and keyboard `step_zone`
  honors it), each drop lands through its own typed callback, and the
  idle SSR output carries neither styling hook. The existing browser test
  now drives the crate component instead of a fixture-local copy.

## 2.3.1 - 2026-07-08

### Fixed

- **The reduced-motion stylesheet could render as visible text.** 2.3.0's
  `<style>` override relied on the UA stylesheet's `style { display: none }`,
  which has zero specificity - an app rule like
  `.list > * { display: flex }` overrode it and painted the CSS source as
  visible text at the top of sortable lists (seen in the gallery). The
  element now carries an inline `display: none`, which outranks any
  selector; an SSR test pins the inline-hidden form.

## 2.3.0 - 2026-07-08

### Added

- **RTL keyboard navigation.** `Direction::Rtl` on `DndProvider` (or
  `ZoneRegistry::set_direction` for hook users) mirrors the keyboard
  experience: spatial zone ordering runs right-to-left within a row, and
  the descend/ascend arrows swap so "into" is always the arrow pointing
  along reading order - the WAI-ARIA tree convention. LTR behavior is
  unchanged and remains the default.
- **`prefers-reduced-motion` support.** `SortableList`'s live preview and
  `FlipItem`'s glide mark their moving elements with `data-dnd-motion` and
  ship a media-query override (one `<style>` per subtree; `SortableGrid`
  anchors it for its tiles), so drags snap instead of gliding when the
  user asks the OS for less motion. Near-zero duration rather than zero,
  so `transitionend` still fires. Mark your own animated elements with the
  same attribute to opt them in.

### Fixed

- **Stale hit-test rects while auto-scrolling.** Zone rects are cached at
  drag start, but `AutoScroll` moves the zones mid-drag - so hover
  highlighting and drops targeted where zones *sat at pickup*, not where
  the user sees them. A new payload-type-erased **rect-refresh channel**
  (`RectRefresh`, one per provider tree; nested providers share the
  outermost) fixes this: every provider registers a re-measure thunk that
  runs only while it has a drag in flight, and `AutoScroll` pings the
  channel after every scroll it performs and on any other scroll of its
  container (wheel/trackpad mid-drag). Custom layout mutators can ping it
  too via the new `use_rect_refresh()` hook.
- **Sortables and grids track scrolling too.** `SortableList` and
  `SortableGrid` are self-contained (no provider), so `AutoScroll` now
  *anchors* the refresh channel when it's the outermost participant and
  they join it. The grid re-measures plainly (tiles never transform), but
  the list can't: its rows carry live-preview transforms, often
  mid-transition, so `get_client_rect` reads displaced, interpolated boxes
  no subtraction can reliably invert. Instead the list *re-anchors*: the
  wrapper never transforms and rows never move within it mid-drag, so one
  wrapper measurement per ping gives the exact distance every cached base
  slot shifted. Pings from unrelated scroll surfaces measure zero movement
  and no-op; overlapping pings coalesce so the final scroll position is
  never left unapplied.

### Tests

- Runtime: nested providers share one refresh channel, a provider's thunk
  unregisters on unmount, and `AutoScroll` anchors the channel for
  provider-less sortables/grids. Browser: a drag that auto-scrolls its
  container hovers and drops on the *zone* that scrolled into place, and a
  sortable row dropped after auto-scrolling lands at the slot computed
  from the live scroll offset (both verified red without the fix, green
  with it).

## 2.2.0 - 2026-07-08

Cross-type drops, documented and de-trapped. Providers stay monomorphic -
that's the crate's core guarantee - but two things now make living next to
a second provider first-class.

### Added

- `ZoneRegistry::contains(id)` and `ZoneRegistry::ascend(current)`. The
  `ParentZone` context is shared across payload types, so a `DropZone<A>`
  nested inside a `DropZone<B>` records B's id as its parent - an id that
  only resolves in *B's* registry. `ascend` returns a zone's parent only
  when it's registered in the same registry.
- `ParentZone` is exported from the prelude, completing the public kit
  (`use_zone_registry`, `use_zone_id`, `ZoneRecord`) for building custom
  zones - including bridge zones registered in two type-worlds at once.
- README section **Mixing payload types**: when one provider with an enum
  payload is the answer, and when to bridge two providers by registering
  the same `ZoneId` (ids are process-global) in both registries with shared
  `mounted`/`rect` signals. Every drop stays typed; no erased channel.
- Gallery page **Standup**: tickets and teammates drag in separate
  providers; a shared agenda tray built from double registration (in ~40
  lines of user-land code) accepts both.

### Fixed

- Keyboard navigation could dead-end across type-worlds: ArrowLeft from a
  zone nested under a foreign-type zone entered the foreign parent's id,
  which this world's registry can't resolve - announcements degraded to
  "zone N" and Enter silently did nothing while the payload stayed held.
  Ascend now skips unresolvable parents (falling back to the previous
  sibling), and Enter's target resolution skips a hovered id that isn't in
  the registry (falling back to the first acceptable zone).
- The Playwright web server command used `--interactive false`-style flags,
  which dx 0.7 parses as a subcommand; switched to the `=` form.
- Zero clippy warnings again under Rust 1.96, which grew two lints since
  2.1.0 shipped: a range assertion in the autoscroll tests
  (`manual_range_contains`) and the gallery home's group tuple
  (`type_complexity`, now a named `NumberedGroup` alias).

### Tests

- Runtime: cross-type nesting records the foreign parent but `ascend`
  refuses it; a bridge's dual registration shares one rect between both
  registries, hit-tests in each world, delivers each drop through its own
  typed callback, and unregisters independently.
- Browser: a real pointer drag from each world lands on the shared bridge
  zone with typed delivery, while the foreign world's zones stay dark.

## 2.1.0 - 2026-07-08

Retargeted to Dioxus **0.7 stable**. 2.0 depended on the `0.8.0-alpha.0`
pre-release; because Cargo won't unify an `0.8` pre-release with `0.7.x`,
that made the crate unusable in the many projects on shipped 0.7. The code
needed no changes - only the dependency requirement moved. Verified against
`0.7.9`: library (all features), the wasm32 web build, every gallery
example, and the full test suite compile and pass with zero warnings. The
crate still declares `dioxus` with `default-features = false, features =
["minimal"]`, so it pulls in no renderer of its own. (Stores, which back the
in-app drag state, have been part of Dioxus since 0.7.0.)

## 2.0.1 - 2026-07-08

Docs-only patch. The README packaged into 2.0.0 still carried a stale
pre-release warning line and a malformed crates.io badge link, which
rendered on crates.io and the docs.rs front page. This release ships the
refreshed README: fixed badges, the 2.0 compatibility table, a prominent
link to the live gallery, highlighted code fences, and a precise claim
about JavaScript (the library ships none of its own; the optional `web`
feature uses `web-sys` bindings for pointer capture).

## 2.0.0 - 2026-07-08

The pointer-first release. In-app drag and drop now runs entirely on pointer
events and keyboard input with typed Rust payloads; native browser drag is
reserved for the app boundary, where `DataTransfer` is the only transport.
Styling is Tailwind-ready through presence-based data attributes, and the
project website is a new multi-page gallery that teaches every pattern.

### Breaking: one input model

- In-app components are pointer + keyboard only. Removed `PointerDraggable`,
  `DragInputMode`, `DragMode::Native`, `Draggable::native`, the `input`
  props on sortable/grid/board/multiselect components, and every in-app
  native `ondrag*` branch (`DropZone`, `SortableList`, `SortableGrid`,
  `BoardSlot`, `TreeNodeTarget`, `CanvasDropZone`). `Draggable` is now the
  one in-app drag source, and there is no mode to choose.
- Native `DataTransfer` remains exactly where the app boundary requires it:
  `FileDropZone`, `ExternalDropZone`, `ExternalDragSource`,
  `external::typed`, and `AutoScroll`'s dragover listener.
- `DropOutcome` gained two fields: `mode: DragMode` (`Pointer` or
  `Keyboard`) and `grab: Point` (the pointer offset inside the element at
  pickup; `element - grab` is the landing top-left). Manual `DropOutcome`
  literals must add both.
- `ZoneId::auto` and `DragId::auto` now start at 2^32 so auto-generated ids
  can never collide with explicit `u32`-range ids and silently replace a
  neighboring zone in the registry (previously dependent on mount order).

### Styling: presence-based data attributes

- Drag state is exposed as attributes that are present while active and
  absent otherwise, so Tailwind variants like `data-dragging:opacity-50`
  and CSS `[data-over]` selectors match only active elements: `data-over` +
  `data-active` on `DropZone`, `FileDropZone` and `ExternalDropZone`,
  `data-dragging` + `data-disabled` on `Draggable`, `data-dragging` +
  `data-drop-target` on sortable/grid items, `data-active` + `data-over` on
  `BoardSlot`, `data-selected` on `SelectableDraggable`, `data-intent` on
  `TreeNodeTarget`, `data-active` on `CanvasDropZone`, and
  `data-sort-handle` on touch grips.
- Forwarded `style` props merge after functional inline styles instead of
  replacing them, so `touch-action`, overlay positioning and the grid's
  `display: grid` survive while your declarations win per property.
- New `item_class` prop on `SortableGrid` for the library-rendered tile
  wrappers; `DragOverlay` forwards `class` and friends to the ghost.

### Pointer path

- New `web` feature: real pointer capture via `web-sys`, so a mouse drag
  stays glued to its source when the cursor leaves it. Off by default; the
  core stays dependency-free with a held-button recovery fallback, and a
  formal gesture state machine (`transition`, `GesturePhase`,
  `GestureEvent`, `GestureEffect`) drives every drag.
- New `SortableList::overlay` prop: a fixed-position ghost sized from the
  measured source row while the in-flow row becomes the live gap.
- Live preview translates rows by the measured slot pitch (margins and gaps
  included) so spacing never squashes mid-drag; hover hit-tests client
  coordinates against rects measured at drag start.
- A release outside a list or grid commits no reorder, and a drop over a
  rejecting zone falls through to an accepting zone stacked beneath it.
  Missed drops re-measure zones and retry within 48px for touch.
- `AutoScroll` follows default mouse pointer drags via held-button state,
  never scrolls on passive hover, stops when the pointer leaves the
  container, and takes an optional `active` gate.
- `Ctrl`/`Cmd` at release resolves pointer drops to `Copy` and `Alt` to
  `Link` (`effective_effect` is public); keyboard drops report the selected
  zone's center in `DropOutcome::element` and honor
  `CanvasKeyboardPlacement` (`Center`, `Origin`, `Fixed`).
- `BoardSlot` registers as a real zone (pointer, touch and keyboard can
  target it) and `apply_move` adjusts same-column forward insertions.
- `TreeNodeTarget`, `BoardSlot` and `CanvasDropZone` mirror changing props
  through signals so their one-time registry callbacks always read current
  values.
- `FileFilter::content_types` gained a real MIME matcher (exact types,
  `type/*`, `*/*`, structured `*+json` suffixes, parameters and case
  ignored, malformed patterns rejected), and `extensions` normalizes dots,
  case and whitespace.

### The gallery website

- `examples/gallery/` replaces the showcase, tailwind and canvas examples:
  a multi-page site with one page per pattern (fourteen in all), each
  pairing a live demo with a numbered walkthrough, an annotated snippet, a
  "New to Dioxus?" callout and a real API reference. Deployed to GitHub
  Pages by CI.
- The design follows the KI-U paper system: paper and ink scales, one
  forest-green accent, earth tones used sparingly, Poppins for UI text and
  Geist Mono for code, no glow shadows. Code panels are inverse-ink with a
  homegrown Rust/rsx highlighter (no JavaScript highlighter).
- The landing page hero is itself a demo: draggable paper scraps on a
  `CanvasDropZone`.
- Removed `examples/showcase.rs`, `examples/tailwind.rs` and
  `examples/canvas.rs`. The Playwright suite now drives headless fixtures
  in `examples/regressions.rs` on a single dev server, covering overlay
  geometry and cleanup, outside-release cancellation for lists and grids,
  autoscroll edge behavior, canvas grab-offset placement, drop
  fall-through, the Ctrl-drag copy convention, reorder buttons inside
  sortable rows, and the native boundary paths.

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

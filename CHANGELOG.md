# Changelog

## Unreleased

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
needed no changes — only the dependency requirement moved. Verified against
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

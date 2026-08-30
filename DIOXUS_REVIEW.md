# Dioxus 0.7 review of the current change set

Date: 2026-08-30
Branch reviewed: `fix/branch-reconciliation`
Status: **resolved — implementation and final self-review complete**

This is the consolidated result of three independent reviews of the current
working tree. The review criteria were:

- Dioxus correctness: props, hooks, effects, callbacks, context, mounted data,
  async work, RSX attributes, and keyed rendering follow Dioxus 0.7 behavior.
- Taste: the public API and implementation communicate one coherent model.
- Usability: a downstream application can adopt the crate without hidden
  lifecycle rules or surprising compatibility breaks.
- Complexity: machinery exists only where Dioxus requires it and hot paths do
  not do avoidable work.

The findings below preserve the original review evidence and the implementation
contract used to resolve it. A post-implementation review found seven uncovered
correctness regressions. The corrective stage below is now part of the
authoritative plan; the review cannot return to resolved until every named
regression and the deterministic release matrix pass.

## Reopened corrective stage

Completion: **done**

1. **Restore `DndContext::from_parts` compatibility.** A context made from
   caller-owned `Store<DragState<T>>` must continue to observe and mutate that
   shared store even when another wrapper or the caller changes it. Keep the
   new runtime phase for provider-owned terminal-event exclusivity without
   making legacy `dragging`, `settling`, cancellation, or equality split from
   the wrapped state. Regressions must cover direct store mutation and two
   wrappers around the same state and announcement.
2. **Protect Dioxus style-property attributes, not only textual `style`.**
   Dioxus 0.7 represents `touch_action:`, `transform:`, and other individual
   CSS properties as style-namespace attributes. Remove every property owned
   by an invariant before the later spread, while preserving unrelated user
   properties. Regressions must use real component RSX with individual
   `touch_action:` and `transform:` props.
3. **Make stable-sortable keyboard strategy semantics complete.**
   `SortStrategy::GridSwap` must emit `Placement::On` for pointer and keyboard
   input alike. A keyboard drag must also be able to reach the append target of
   a populated destination group. Regressions must exercise the public
   components with a swap group and a non-empty sibling group.
4. **Do not retain rejected drag-handle presses.** `DragHandle` must arm only a
   primary pointer press, and `Draggable` must consume the handle token before
   any press-validation return. A browser regression must right-click the
   handle and then left-click the non-handle surface without starting a drag.
5. **Keep the plan honest.** This document remains reopened until the focused
   regressions, all-feature Rust tests, Clippy, formatting, documentation,
   package checks, and Playwright suite pass. Only then may this section and
   the top-level status be marked resolved.
6. **Do not confuse a pointer self-drop with group append.** A populated
   group's background must remain an append target, but the active item's own
   pointer hit must terminate at that item as a no-op instead of falling
   through its rejected item zone to the enclosing group. Keep the active item
   out of keyboard target traversal while accepting pointer self-hits, and pin
   both behaviors in the component-level runtime suite.
7. **Document public records, not their private sidecars.** The `DragState`
   and `ZoneRecord` API tables and architecture guide must list only the
   source-compatible 3.x fields that downstream code can construct. Stable
   drag identity, tracked sessions, live proposed effects, terminal phase,
   and advanced target policy belong to private runtime sidecars and must be
   described through their public context, component, or registry APIs, never
   as public record fields or crate-private methods.

## Completed implementation plan

The following order is authoritative. Each implementation stage starts with a
failing regression and ends only when that regression passes. Aggregate green
tests do not substitute for the named cases.

### 1. Pin every failure with a regression before changing architecture

Completion: **done**

Add the smallest failing test for each confirmed defect:

- downstream integration tests that construct the 3.x `ZoneRecord` and
  `DragState` struct literals and compile legacy unit-return model helpers with
  warnings denied;
- local and multi-window tests proving a successful settled drop emits exactly
  one terminal event even if `cancel()` follows during the glide;
- tests proving `DropEffect::None` neither highlights nor delivers;
- list and grid browser tests that reorder stable-key nodes without remounting,
  then verify geometry, whole-row/tile pointer capture, handle capture, and
  scroll refresh use the node at its current index;
- browser coverage for cancelled-pointer-drag-then-click selection;
- runtime coverage for host sample withdrawal and browser coverage for native
  drag departure from `AutoScroll`;
- a generated N-world bridge test that changes its ID around a nested zone;
- public stable-sortable component tests for keyboard first/last insertion,
  cross-group movement, and an initially empty destination;
- an automatic/explicit sortable-group identity collision test.

No fix is complete unless its test fails on the current tree for the expected
reason and passes after the change.

### 2. Restore the 3.x public shapes and establish private sidecars

Completion: **done**

Do this before the behavioral fixes so later work is built on the final
ownership model rather than rewritten twice.

- Restore `ZoneRecord` and `DragState` exactly to their externally
  constructible shapes on `main`; remove the retroactive `#[non_exhaustive]`
  attributes from those two existing records.
- Keep `#[non_exhaustive]` on newly introduced extensible public enums and
  record structs only. Do not apply it to existing exhaustive APIs or identity
  newtypes.
- Store rich acceptance, allowed effects, and edge policy in a private
  registration-policy sidecar keyed by the full `ZoneRegistration` token, not
  only `ZoneId`. Public registration receives the legacy permissive policy;
  built-in components use an internal policy-aware registration path.
- Replace the new public `DragState` fields with a private shared drag-runtime
  sidecar containing stable identity, explicit/generated identity mode,
  session, proposed effect, source completion, monitor, and an explicit
  `Idle | Dragging | Settling` phase.
- Ensure every copy of a `DndContext` created from one construction shares the
  same sidecars. Preserve the legacy `from_parts` state/announcement behavior;
  newly created contexts may create their own new-feature sidecars.
- Keep public `records()`/`get()` results in the legacy `ZoneRecord` shape and
  perform policy-aware negotiation only on private registered candidates.

The downstream compile contracts from step 1 must pass before continuing.

### 3. Restore core drag/drop invariants on the sidecar model

Completion: **done**

- Make `DropEffect::None` an unconditional rejection before legacy acceptance,
  rich acceptance, hover, collision, settle claiming, or delivery callbacks.
- Remove `DropEffects::NONE`. Define `DropEffects::ALL` as the three enabled
  standard effects (or retain `STANDARD` as its explicit alias), and make the
  default coherent with that definition.
- Enforce terminal exclusivity through the private phase: `Dropped` commits the
  transition to `Settling` or `Idle`; later cancellation may clear visual
  settling but cannot emit `Cancelled` for that generation.
- Apply the same phase transition to local, tracked, untracked, joined-world,
  host-cancel, window-close, overlay-removal, and stale-transition paths.
- Clear or invalidate world settle metadata together with the shared phase so
  no presenter token survives a cancelled visual settle.

Run the local and multi-window terminal-event tests plus the complete existing
session/settle suite before continuing.

### 4. Repair Dioxus identity, event, context, and clock ownership

Completion: **done**

- Introduce an internal render-key type for legacy list/grid caches. Store
  `MountedData`, rect, and measurement generation by render key; maintain the
  current `index -> render key` projection for index-based public callbacks and
  algorithms. Guard async writes by key, node identity, and generation, and
  resolve pointer capture through the current projection.
- Validate duplicate caller keys when legacy `item_key` is supplied. Preserve
  the index fallback only for 3.x compatibility and keep its documented state
  and focus warning.
- Move the selectable pointer/click surface inside the draggable root so its
  handler runs before the root calls `stop_propagation`. Clear suppression on
  `on_drag_end(false)` while retaining it through a successful pointer drop
  until the trailing click is consumed.
- Give `bridge_drop_zone!` a real keyed component identity boundary. Verify the
  chosen macro-local or reusable internal boundary compiles for multiple macro
  invocations; registration, geometry, and `ParentZone` provisioning must be
  scoped so an ID change remounts descendants and cleans every old registry.
- Model `AutoScroll` clock ownership explicitly as pointer, native drag, or
  external host input. Pointer up/cancel, native drag leave/drop, and loss of
  either external sample field retire only the matching clock owner. A stale
  sample from one source must not stop a newer clock owned by another.

Run both list and grid browser regressions, the selection browser regression,
the bridge runtime regression, and external/native auto-scroll regressions.

### 5. Finalize the new stable-sortable contract before publishing it

Completion: **done**

The current group context cannot infer order from opaque `Element` children,
so keyboard intent needs explicit data rather than render-time bookkeeping.

- Require a stable position on directly composed `SortableItem`s;
  `SortableCollection` supplies it from enumeration. Carry the pickup position
  in the sortable payload and use the target's current position.
- For same-group keyboard drops, choose `Before` when the active position is
  after the target and `After` when it is before the target. For cross-group
  keyboard drops on an item, use a documented deterministic insertion rule;
  the group-background target supplies append.
- Represent group-background intent throughout the API, not only in
  `ReorderEvent`: update `DropPlacement`, layout projection, reorder helpers,
  callbacks, documentation, and examples together. `over: None` means append.
- Prototype the empty/background target against the existing hierarchical
  keyboard registry before selecting its implementation. It must accept an
  empty destination and background append without accidentally making ordinary
  Up/Down item navigation traverse group containers. Prefer a reusable internal
  registration primitive over copying `DropZone` logic.
- Put automatic group IDs in a mechanically disjoint namespace. Because this
  API is new, make the tuple field private and expose an explicit constructor
  limited to the reserved explicit range plus `auto()` for generated IDs.
- Narrow generic bounds: provider, group, and item require only their actual
  equality bounds; collection duplicate validation and projection retain
  `Eq + Hash` where hashing is performed.

Public-component tests must cover pointer reorder, keyboard before/after,
cross-group and empty-group delivery, direct composition, reactive props,
handles, stable child state, focus restoration, and the documented key
sequence. Pure helper tests are insufficient.

### 6. Remove avoidable API and hot-path friction without reopening semantics

Completion: **done**

- Implement `Display` and `std::error::Error` for `ApplyDropError` and document
  the checked helpers' error contract.
- Make `sortable_kernel` private and re-export the stable surface coherently
  from `sortable` and the prelude.
- Add a lazy monitor emission path that checks for listeners before constructing
  or cloning a payload snapshot while retaining FIFO reentrancy once dispatch
  begins.
- Build compact private policy candidates under the registry guard, release the
  guard, and only then invoke application acceptance/collision callbacks.
- Avoid cloning the payload for built-in collision strategies; construct the
  owned public collision request only for custom detectors that consume it.

Combine the policy candidate work with the sidecar design from step 2 rather
than performing two registry rewrites. No application callback may run while a
Dioxus signal borrow guard is held.

### 7. Reconcile documentation and run deterministic release gates

Completion: **done**

Only after steps 1-6 pass:

- update README, rustdoc, API and concept guides, changelog, examples, and this
  review to describe the final implementation exactly;
- run formatting and `git diff --check`;
- run all-feature, no-default, individual feature, MSRV, and wasm checks;
- run strict Clippy and rustdoc;
- run unit, runtime, multi-window, cross-VDOM, typed-transport, and both desktop
  application suites;
- run `cargo deny`, package contents verification, and publish dry-run;
- run the full Playwright suite, then rerun every affected browser scenario
  with retries disabled and repeated execution. A configured retry is not
  evidence that the scenario is deterministic.

## Release blockers

### 1. A keyboard drag suppresses the next real selection click

Location: `src/multiselect.rs`, `SelectableDraggable`, around lines 242-269.

`dragged` is armed by `Draggable::on_drag_start` for pointer and keyboard
drags. A pointer drag normally produces a trailing browser `click`, but a
keyboard drag does not. After keyboard pickup/drop, the next real mouse click
therefore consumes the stale flag and does not update the selection. Pointer
cancellation can leave the same stale state when the browser emits no click.

Correct fix:

1. Read the nearest `DndContext<Vec<K>>` in `SelectableDraggable`.
2. Arm click suppression only when `on_drag_start` observes
   `DragMode::Pointer`.
3. Add an outer `onpointerdown` handler that clears an old suppression token
   before a new pointer gesture starts. The pointerdown that began the drag
   occurs before the token is armed, so the immediate trailing click is still
   suppressed; a later genuine click begins with a new pointerdown and clears
   a token left by cancellation.
4. Replace the modality-blind boolean with a small, explicitly named
   `suppress_pointer_click` signal.
5. Protect the component-owned `onpointerdown` listener from the forwarded
   attribute spread in the same way as `onclick`.

Required regression tests:

- Keyboard pickup/drop followed by clicking another item selects it on the
  first click.
- A completed pointer drag still suppresses its immediate trailing click.
- Pointer cancellation with no browser click does not suppress the next
  genuine click.

### 2. Public identity props become stale after a Dioxus rerender

Locations include:

- `src/core/components/draggable.rs`, around lines 145-151
- `src/core/components/drop_zone.rs`, around lines 88-93 and 239-244
- `src/canvas.rs`, around lines 177-182
- `src/board.rs`, around lines 129-133
- `src/sortable_kernel.rs`, around lines 280-285
- `src/core/hooks.rs`, around lines 349-353

These props are copied into `use_hook` once. A later prop update panics only in
debug builds; release builds silently retain the old drag, zone, board, or
group identity. That is surprising for a normal Dioxus prop and can make the
rendered application disagree with its registry state.

Correct fix for components:

1. Keep auto-ID generation in the public outer component.
2. Resolve `explicit_id.unwrap_or(auto_id)` on every render.
3. Move registration, mounted-data, session, and cleanup hooks into a private
   `*Instance` component.
4. Render that implementation component with the resolved identity as its
   Dioxus `key`.
5. An identity change then follows the normal Dioxus lifecycle: the old keyed
   instance unmounts and unregisters, and the new instance mounts and
   registers atomically.

Correct fix for `use_bridge_world`, which cannot key itself:

1. Store the current `(zone_id, parent, registration)` as hook state.
2. React to `zone_id` and `parent` with `use_reactive!` plus an effect.
3. On change, unregister the old token before registering the replacement.
4. Make `BridgeGeometry::register` return a small RAII writer handle so the old
   geometry writer is removed when the registration changes or unmounts.
5. Route mounted and rect writes only through the current registration token.

Remove the debug-only freeze assertions after the lifecycle is reactive.

Required regression tests:

- Rerender each affected component with a different explicit ID and verify
  that only the new ID is registered and targetable.
- Change a bridge zone's ID and parent during an active drag; stale geometry
  and stale hierarchy must not survive.
- Verify old cleanup cannot unregister a newer same-ID registration.

### 3. Model helper return types break existing 3.x callers

Location: `src/core/model.rs`, `apply_clone_or_move` around line 200 and
`apply_list_clone_or_move` around line 246.

Both public helpers changed from returning `()` to returning
`Result<(), ApplyDropError>` while `Cargo.toml` still declares version 3.1.0.
This breaks callbacks that used the helper as their final expression and adds
`unused_must_use` failures for consumers that deny warnings.

Correct fix for the current 3.x release:

1. Restore the exact existing `apply_clone_or_move` and
   `apply_list_clone_or_move` signatures and legacy behavior.
2. Move the new checked behavior into `try_apply_clone_or_move` and
   `try_apply_list_clone_or_move`, returning `Result<(), ApplyDropError>`.
3. Export and document both checked helpers.
4. Do not add a deprecation warning to the old helpers in this minor release;
   that warning itself disrupts `-D warnings` consumers.
5. Reserve removal or signature changes for the next major version.

Required compatibility tests:

- Compile a callback whose final expression is each legacy helper and whose
  expected return type is `()`.
- Compile with warnings denied while calling the legacy helpers.
- Verify the `try_*` variants reject `Link` and `None` without mutation.

### 4. `AutoScroll.speed` silently changed units

Location: `src/autoscroll.rs`, `AutoScroll` around line 217. The stale contract
also appears in `README.md` around line 503.

The previous public prop used a default of `24.0` pixels per pointer event.
The current implementation uses `720.0` CSS pixels per second. Existing
`speed: 24.0` call sites still compile but become dramatically slower.

Correct fix for 3.x:

1. Keep `speed` with its old default of `24.0` and old nominal meaning.
2. Add an explicitly named optional prop such as
   `speed_px_per_second: Option<f64>` for the frame-rate-independent engine.
3. Compute internal velocity as
   `speed_px_per_second.unwrap_or(speed * 60.0)`. This preserves the old
   approximately-per-frame behavior at the historical 60 Hz assumption while
   allowing new callers to select exact units.
4. Make all internal physics use only the resolved pixels-per-second value.
5. Document `speed` as the compatibility prop and recommend
   `speed_px_per_second` for new code. Remove `speed` only in a major release.

Required regression tests:

- Default 3.x configuration remains close to its previous 60 Hz movement.
- Existing `speed: 24.0` does not become a 24 px/second crawl.
- Explicit pixels-per-second speed produces equivalent movement at different
  frame intervals.

## Correctness fixes

### 5. Rect refreshes retain a new Dioxus callback per scroll ping

Locations: `src/core/hooks.rs`, around lines 126-182, and
`src/core/registry.rs`, around lines 814-848.

Every active-drag refresh constructs `Callback::new`. Dioxus callbacks are
owned by their creation scope and their slots remain until that scope drops.
Autoscroll can therefore accumulate scope-owned callbacks at frame frequency
inside a long-lived provider.

Correct fix:

1. Change the internal completion parameter from `Callback<()>` to a plain
   `Rc<dyn Fn()>` (or accept `impl Fn() + 'static` and convert it once).
2. Clone that ordinary `Rc` into the measurement tasks.
3. Invoke it when the final task completes, then let all clones drop.
4. Keep the existing drag/session validation inside the closure so an old
   measurement batch cannot update a successor drag.
5. Use a Dioxus callback only at an actual component/event API boundary, not
   as an internal one-shot completion primitive.

Required regression test:

- Run thousands of refresh completions under one mounted provider and verify
  the Dioxus callback arena/owner count does not grow per refresh.

### 6. Sortable geometry can retain or resurrect removed nodes

Locations:

- `src/grid.rs`, caches around lines 104-168 and async writes around 273-285
- `src/sortable.rs`, shared refresh around lines 98-113, retention around
  332-337, and mount measurement around 685-697

`SortableGrid` does not prune caches when `len` decreases. Both list and grid
can also complete an old `get_client_rect()` after a row was removed or
replaced. The parent component owns those tasks, so removing the rendered row
does not cancel them.

Correct fix:

1. Give grid and list the same `use_effect(use_reactive!(|len| ...))` cache
   pruning behavior.
2. Track a per-index mount generation and the current `Rc<MountedData>`.
3. Capture `(index, generation, mounted)` before every async measurement.
4. After every `await`, write only if the index is still in range, the
   generation still matches, and `Rc::ptr_eq` confirms the same mounted node.
5. Add a generation to batch refreshes so an older batch cannot overwrite a
   newer one.
6. Before emitting `SortEvent`, enforce `to < current_len` even if a stale
   cache somehow exists.

Required regression tests:

- Shrink a grid and release inside a removed tile's old rectangle; no
  out-of-range event is emitted.
- Complete old and new measurements out of order; only the current node wins.
- Remove a list row while its measurement is pending; it is not reinserted.

### 7. Forwarded styles can replace behavior-critical CSS

Location: `src/core/components/mod.rs`, `merge_style` around lines 27-42, and
its uses in draggable, handle, overlay, FLIP animation, settle slots, and grid.

The helper removes only the first textual `style` attribute. Later style
attributes survive the spread and can replace the merged style wholesale.
It also always places user declarations last, allowing callers to override
behavioral properties such as `touch-action`, overlay position/transform,
FLIP transform, or settle visibility.

Correct fix:

1. Replace `merge_style` with a helper that removes **all** textual style
   attributes and concatenates them in their original order.
2. Provide two explicit policies:
   - user-last for configurable defaults such as grid column styling;
   - invariant-last for drag sensing, overlay positioning, FLIP transforms,
     and settle visibility.
3. Prefer a private inner wrapper for overlay/FLIP transforms so a caller can
   style or transform the public outer element without competing for the same
   CSS property.
4. Keep the existing reserved-attribute filtering. Dioxus 0.7 RSX does not
   permit moving the spread before later internal attributes, so spread-first
   is not a valid fix.
5. Update documentation so it accurately distinguishes configurable styles
   from component invariants.

Required regression tests:

- Multiple forwarded `style` attributes are all consumed and merged.
- `touch-action: auto` cannot disable an active drag sensor.
- Caller transforms do not replace overlay or FLIP transforms.
- Configurable grid declarations still allow the documented user override.

## Identity and list-key design

### 8. `StableItemKeys` is O(n²) and hides duplicate semantic IDs

Location: `src/sortable_kernel.rs`, around lines 19-47 and 290-305.

The current render-time reconciliation repeatedly performs a linear search
and `Vec::remove`. Equal IDs receive separate render keys even though drag
logic compares the IDs themselves and cannot distinguish the duplicates.

Correct fix:

1. Require `K: Eq + Hash` for the new, not-yet-published stable-ID kernel.
2. Use the caller's explicit render key directly. Dioxus already requires the
   application to identify list items, so a second generated-key identity
   layer is both redundant and easier to misuse.
3. Remove `StableItemKeys`, its render-time mutable reconciliation state, and
   the process-global counter entirely.
4. Detect duplicate semantic IDs and duplicate render keys in every build and
   fail with a clear
   message; at minimum use a debug assertion plus a documented hard
   precondition if release panic policy forbids the stronger check.
5. Document that `K` is the unique semantic identity, not merely item data
   that may compare equal.

Required tests:

- Reordering unique IDs preserves each row's Dioxus hook state and focus.
- Duplicate IDs are rejected clearly.
- Render keys come directly from the explicit caller identity without an
  internal reconciliation pass.

### 9. Legacy sortable components default to index keys

Locations: `src/sortable.rs`, around lines 317 and 503, and `src/grid.rs`,
around lines 72 and 197.

Index keys are unsafe for the stateful reorder use case these components
serve. Dioxus can attach hook state, focus, and mounted handles to positions
instead of domain items.

Correct fix:

1. Keep the fallback only where required for 3.x source compatibility.
2. Make the documentation state prominently that the fallback is safe only
   for stateless, position-identified rows.
3. Ensure every example with state, focus, or reordered backing data supplies
   `item_key`.
4. In the next major version, make a stable key callback mandatory or remove
   these index-based adapters in favor of the stable-ID kernel.
5. Do not pretend to synthesize domain identity internally; the component
   cannot infer it from an index.

Required tests:

- A keyed legacy row keeps its local state and focus after reorder.
- Documentation examples never demonstrate index keys for stateful rows.

## Public API and complexity fixes

### 10. `SortableItem` advertises a composition path users cannot construct

Location: `src/sortable_kernel.rs`, private `GroupContext` around line 246,
`SortableGroup` around line 271, and public `SortableItem` around line 318.

`SortableItem` requires a private context, while `SortableGroup` accepts no
children and always creates every item wrapper itself. A downstream user
cannot create the documented custom group layout without nesting duplicate
sortable behavior.

Correct fix before publishing the new API:

1. Make `SortableGroup` the provider/layout boundary: it accepts `children`
   and provides the private group context.
2. Let users render `SortableItem { id, ... }` beneath it.
3. Put the current `items + render` convenience behavior in a separately
   named component such as `SortableCollection` that composes
   `SortableGroup` and `SortableItem`.
4. Keep `GroupContext` private; consumers need the component seam, not raw
   registry internals.
5. Update examples to show both the convenient collection and a custom
   flex/grid layout.

Required tests:

- A downstream-style component can create a custom layout using only public
  APIs.
- The convenience component creates exactly one draggable/drop-zone wrapper
  per item.

### 11. Live labels make collision snapshots impure and unnecessarily costly

Location: `src/core/registry.rs`, `ZoneRecord::label`/`live_label` around
lines 60-63, snapshot hydration around lines 155-159 and 509-518, and
collision resolution around lines 712-745.

Every zone stores both a label snapshot and a live callback. Collision
snapshots invoke callbacks and clone labels for every zone on pointer moves,
although collision and acceptance do not inspect labels.

Correct fix:

1. Remove `live_label` from `ZoneRecord` and keep one registered label value.
2. Synchronize prop changes through `ZoneRegistry::sync_label` in a reactive
   effect, alongside the existing registration-policy synchronization.
3. Keep collision snapshots limited to geometry, hierarchy, effects, and
   acceptance data.
4. Read a label only after a target has been selected for an announcement or
   when a debug/inspection API explicitly requests records.
5. If same-render label freshness is considered mandatory, keep the live
   callback in a separate label table and invoke only the selected target's
   callback; never hydrate every collision candidate.

Required tests:

- Updating a label changes the next announcement.
- Collision resolution does not invoke any label callbacks.
- A pointer move over many zones does not clone every label.

### 12. Public scroll-coordinator handles expose no usable capability

Locations: `src/autoscroll.rs`, `ScrollContainerId` around line 16,
`ScrollCoordinator` around line 29, `use_scroll_coordinator` around line 95,
and their exports in `src/lib.rs`.

The types and hook are public and prelude-exported, but all coordinator
operations are private. The hook also panics without the private provider
arrangement. Downstream users can obtain a value but cannot use it.

Correct fix:

1. Make `ScrollContainerId`, `ScrollCoordinator`, and
   `use_scroll_coordinator` `pub(crate)`.
2. Remove them from the crate root/prelude.
3. Keep `AutoScroll` responsible for creating and using the coordinator.
4. Add a public RAII registration API later only if a demonstrated custom
   integration requires one.

### 13. `DndMonitor` is an inert public implementation handle

Locations: `src/core/monitor.rs`, `DndMonitor` around line 64, and
`DndContext::monitor` in `src/core/state.rs` around line 568.

The monitor type is public, but subscription, unsubscription, and emission
are crate-private. The supported public operation is already
`use_dnd_monitor`.

Correct fix:

1. Make `DndMonitor` and `DndContext::monitor` crate-private.
2. Keep `use_dnd_monitor` as the single public subscription API.
3. If imperative subscriptions are needed later, expose a deliberate method
   returning an RAII subscription guard rather than the raw implementation
   handle.

## Documentation corrections

### 14. README and API documentation drift from the implementation

Known mismatches:

- `README.md` says the Dioxus dependency enables only `minimal`; `Cargo.toml`
  enables `minimal`, `document`, and `mounted`.
- README autoscroll units/defaults describe the old behavior while the code
  uses the new frame-rate-independent engine.
- The README feature table omits the significant `desktop` feature.
- Style documentation says functional declarations survive while also saying
  user declarations win; conflicting properties make both claims impossible.
- Stable-ID documentation does not state that IDs must be unique.
- `SortableItem` documentation describes a provider seam that is currently
  inaccessible.

Correct fix:

1. Update documentation in the same changeset as the corresponding API fix,
   not before it.
2. State why `mounted` is a production requirement for geometry and why
   `document` is needed for renderer-independent evaluation.
3. Document exact autoscroll units and precedence between compatibility and
   pixels-per-second props.
4. Include `desktop` in the complete feature matrix.
5. Document unique identity/key requirements and the legacy index-key caveat.
6. Document which style properties are configurable and which are invariants
   placed on private wrappers or merged last.

## Areas explicitly cleared by the review

No further actionable defect was found in:

- Production propagation of Dioxus's `mounted` capability.
- `AutoScroll` tracking of `active` and `drag_pointer` through
  `use_reactive!`.
- Dynamic `DragOverlay::settle` behavior and reduced-motion styling.
- Board, bridge, sortable, and canvas acceptance callback freshness.
- Registry, monitor, and session callbacks running outside signal read guards.
- Signal guards across `await` points.
- FLIP and draggable measurement generation checks already added by the
  current changes.
- Linux pointer sampling no longer blocking the Dioxus UI executor.
- File picker/drop handling and the placement of `document::eval`.
- Multi-`VirtualDom` context seeding and callback runtime transfer.
- Handle-only activation and keyboard routing.
- The new `#[non_exhaustive]` coverage on extensible public enums and record
  structs. Public tuple/newtype IDs do not require that annotation.
- The reserved-attribute filtering approach. It is necessary under Dioxus
  0.7's RSX spread grammar; the implementation should be centralized, not
  replaced with invalid spread-first syntax.

## Resolution and final verification

All resolution criteria are satisfied on 2026-08-30:

1. Every release blocker and correctness fix above is implemented with a
   focused regression.
2. The stable-sortable API has one constructible composition model, explicit
   background/append intent, deterministic keyboard placement, disjoint group
   identities, and narrowed bounds.
3. Existing 3.x public record shapes, helper return types, and `AutoScroll`
   speed behavior are preserved; new behavior lives in private sidecars or new
   APIs.
4. README, rustdoc, API/concept guides, changelog, examples, and this review
   describe the final implementation.
5. The complete release matrix is green:

   - `cargo fmt --all -- --check` and `git diff --check` passed.
   - `cargo test --locked --all-features` passed: 162 library tests, 2
     downstream compatibility tests, 33 multi-window tests, 2 cross-VDOM seam
     tests, 82 runtime tests, 5 typed-transport tests, and 5 doctests. The 66
     renderer-gated doctests remained intentionally ignored.
   - `cargo test --locked --no-default-features --lib --tests` passed all 270
     applicable tests.
   - No-default `minimal`, `web`, `serde`, and `desktop` checks passed.
   - The `wasm32-unknown-unknown` web check passed.
   - MSRV `cargo +1.85.0 check --locked --no-default-features --lib` passed.
   - All-feature/all-target Clippy and rustdoc passed with warnings denied.
   - The standalone `desktop-multiwindow` and `desktop-showcase` manifests
     passed tests and strict all-target Clippy; the showcase ran 5 model tests.
   - `cargo deny check` passed advisories, bans, licenses, and sources. Its
     remaining duplicate/advisory-not-detected messages are configured
     dependency-policy warnings, not failures.
   - `cargo package --list --allow-dirty` produced the intended 138-file
     artifact, excluding this internal review and browser tooling.
   - `cargo publish --dry-run --locked --allow-dirty` packaged and compiled the
     crate successfully.
   - All 40 Playwright interaction scenarios passed in one run with retries
     disabled. Newly affected selection, keyed geometry/handle, native/external
     auto-scroll, dynamic identity, stable-sortable, and bridge cases also
     passed their focused retry-free runs during implementation.

Windows occasionally denied finalization of Cargo's incremental-cache session
directories. Cargo reported that only later cache reuse was affected; every
compiler, test, lint, documentation, package, and browser command completed
successfully.

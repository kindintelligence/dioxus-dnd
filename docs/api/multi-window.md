# Multi-window API reference

Desktop windowing glue for multi-window drag worlds (the `desktop` cargo
feature): one correctly ordered per-window provider, plus the lower-level
geometry feed and cross-window drag bridge it composes. This page also
covers the core world and model-lifetime APIs they drive.

Concept guide: [docs/concepts/multi-window.md](../concepts/multi-window.md).
Per-platform verification evidence: [PLATFORMS.md](../../PLATFORMS.md).

`MultiWindowProvider`, `use_window_geometry_feed` and `DragBridge` live in
`dioxus_dnd::desktop` and need `features = ["desktop"]`, which pulls
dioxus-desktop (wry/tao) and is therefore off by default. The provider is
also re-exported by the prelude when that feature is enabled.

Everything else on this page - `DndWorld`, `use_dnd_world`, `DndScope`,
`use_dnd_model`, `use_joined_window`, `JoinedWindow`, `WindowGeometry`,
`WindowKey`, `WindowRecord`, `ZoneLocation` - is core, always compiled and
platform-dependency-free. All except `WindowRecord` and `ZoneLocation` are
re-exported through the prelude; import those two from `dioxus_dnd::core`.

```rust,ignore
use dioxus_dnd::prelude::*;

fn board_window() -> Element {
    let world = use_dnd_world::<Card>();
    let model = use_dnd_model(Model::new);
    let popup_model = model.clone();
    let open = move |_| {
        let dom = world.vdom(popup).with_root_context(popup_model.clone());
        dioxus::desktop::window().new_window(dom, Default::default());
    };
    rsx! {
        MultiWindowProvider::<Card> {
            button { onclick: open, "Open" }
            // ... zones, overlay, live region ...
        }
    }
}

fn popup() -> Element {
    let model = use_context::<Model>();
    rsx! { MultiWindowProvider::<Card> { /* ... */ } }
}
```

## How the pieces divide

`dioxus_dnd::core::world` keeps the library dependency-free by consuming
window geometry it does not compute and host-reported pointer data it
cannot see. The `desktop` module is the other half for dioxus-desktop:
`MultiWindowProvider` is the normal per-window API, composed from these
lower-level pieces:

- `provider`: calls the feed in its own scope above `DndProvider`, then
  renders `DragBridge` inside that provider. This makes the ordering
  invariant structural.
- `feed`: `use_window_geometry_feed` samples this window's placement from
  tao events into the world's `WindowGeometry`.
- `bridge`: `DragBridge` gates which drags need host-side help (mouse and
  pen do; touch must be left alone) and composes the platform legs below.
- `platform` (sealed, not public API): backend policy plus the legs
  themselves. `windows` carries the WebView2 raw-input path; `fallback`
  keeps the portable tao mechanics (generation-bound cursor polling plus
  window-event release detection) shared. `linux` owns the runtime
  X11/Wayland decision and X11's held-button query for releases over
  desktop dead space; `macos` explicitly owns the still-unverified decision
  to use only the portable mechanics. This preserves one implementation of
  each shared leg without hiding genuinely platform-specific mechanics or
  capability policy.

## `MultiWindowProvider`

The one-per-window component for dioxus-desktop. It forwards `dir` to
`DndProvider`, supplies the window geometry before that provider joins, and
mounts `DragBridge<T>` after the join. Put app-styled children such as zones,
`DragOverlay` and `LiveRegion` inside it.

If it mounts with no `DndWorld<T>` in context, it emits one `tracing::warn!`
with the fix (`use_dnd_world` plus `DndWorld::vdom`) and falls back to the
provider's isolated single-window state. This catches the otherwise quiet
failure where every window works locally but never joins the others.
It also warns when nested beneath an existing `DndProvider<T>`: replace the
old provider instead of wrapping it, because only the outermost same-type
provider may join the world.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `dir` | `Direction` | `Ltr` | Forwarded to `DndProvider` for keyboard navigation and spatial ordering. |
| `children` | `Element` | required | Zones and app-styled provider children. |
| `phantom` | `PhantomData<T>` | `PhantomData` | Internal type marker; never set it. |

The manual feed/provider/bridge path below remains public for custom hosts
and integrations that need to insert their own host-side observations.

```rust,ignore
fn manually_wired_desktop_window() -> Element {
    use_window_geometry_feed(); // parent scope: available when join runs
    rsx! {
        DndProvider::<Card> {
            DragBridge::<Card> {} // child scope: consumes joined membership
            // ...
        }
    }
}
```

A non-tao host substitutes its own `WindowGeometry` provider and bridge,
driving the world through the host-side methods documented below.

## `use_window_geometry_feed`

`fn use_window_geometry_feed() -> WindowGeometry` - needs `desktop`.

Lower-level desktop hook used by `MultiWindowProvider`. Provides a
`WindowGeometry` for this window in context and keeps it fed
from tao events (position, size and scale, plus visibility eligibility, on
move, resize, scale change, cursor-enter and focus events). Call it ABOVE
the `DndProvider`, which picks the geometry up from context when it joins
the world. Returns the geometry handle, rarely needed directly.

Behavior notes:

- Eligibility follows `is_visible() && !is_minimized()`. A minimized or
  hidden window retains its last placement for a later restore but cannot
  win global hit-testing meanwhile.
- On Linux the feed starts ineligible and publishes nothing until tao's
  event-loop target identifies the backend actually in use, so plausible
  global placement is never exposed before the X11/Wayland decision.
- On Wayland, where a window cannot learn its own screen position, the feed
  leaves geometry cleared and this window drags per-window only. A failed
  position sample on X11 does not revise the backend decision; the next
  event may recover it.
- `CloseRequested` and `Destroyed` clear the geometry and mark the window
  ineligible.

## `DragBridge`

Lower-level component used by `MultiWindowProvider`: host-side eyes and ears for pointer drags
that leave the origin window. Render one INSIDE each window's
`DndProvider<T>`; it renders nothing. A provider that did not join a
`DndWorld` gets a no-op bridge. Needs `desktop`.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `phantom` | `std::marker::PhantomData<T>` | `PhantomData` | Internal type-parameter marker; never set this. |

### Which drags are bridged

Every leg is gated on the drag's `PointerKind`: only pointers WITHOUT
implicit capture (mouse, pen) are bridged. A touch drag is already streamed
whole to the origin webview by the browser's implicit capture; bridging it
too double-drives the drag from Windows' touch-synthesized mouse (a cursor
trailing the finger, plus synthesized button transitions that can end the
drag early). The world's bridging kill switch (below) vetoes every leg from
one gate.

All legs are idempotent per drag and bound to a composite drag generation
(world generation plus the tracked source session), captured at engagement
and re-validated immediately before every action, so a sleeper from drag N
can never drive replacement drag N+1.

### The per-platform truth table

Webview pointer events stop at the viewport edge, and while a button is
held every NON-origin window is fully event-blind (X11 implicit grab; the
equivalent AppKit/WKWebView strategy remains unverified) - probed and
confirmed on X11. The portable legs cover that shape: the origin polls the
global cursor (30ms ticks, generation-bound), and a blind window's first
pointer event mid-drag proves the button is up and completes the drop.
Linux's X11 button observer covers releases over no window at all, by
querying the root pointer's held-button mask over a first-party x11rb
connection. On Windows/WebView2 the shape is the OPPOSITE and both portable
legs are dead; the raw-input leg (`WM_INPUT` device events for pointer,
buttons and live modifier keys) exists for exactly that platform. It
requires `DeviceEventFilter::Never`, which the leg sets process-globally on
first use, on Windows only - apps with their own raw-input needs should
know the bridge flips it there. On Wayland neither global geometry nor the
global cursor exists by design; everything degrades to per-window drags,
which is the world's documented fallback.

### Shared window events

Regardless of which pointer leg is active, the bridge also routes
platform-neutral tao window events during a bridged drag:

- `ModifiersChanged` keeps the world's live modifiers current, so a
  modifier held at a host-side release resolves the same Copy/Link effect
  as a local drop.
- `Resized` and `ScaleFactorChanged` trigger `DndWorld::refresh_all_rects`,
  so a window resized mid-drag stays accurately hit-testable.
- On Windows, a model tripwire: the raw-input leg exists BECAUSE tao never
  delivers `CursorMoved`/`MouseInput` there (the WebView2 child HWND
  consumes them). If a WebView2 or tao update changes that routing, the
  bridge warns once per drag and deliberately does not act on the events,
  so the raw-input leg keeps sole ownership until the model is re-verified.

### The kill switch and diagnostics

Every leg honors the world's runtime bridging switch
(`DndWorld::set_bridging`; end users can set `DIOXUS_DND_NO_BRIDGE=1`
before launch, no rebuild). If a webview or OS update ever ships a
regression in these mechanics, the app can degrade to per-window drags -
the already-modeled Wayland behavior - instead of shipping broken
cross-window gestures. With `tracing` at `debug`, each leg logs when it
engages a drag (`leg` values `cursor-poller`, `release`, `x11-deadspace`
and `raw-input`, message "bridge leg engaged"), so a post-update bug report
arrives pre-triaged to the leg whose platform assumption moved.

## `DndWorld`

A drag world shared by several windows: one `DndContext<T>` every joined
provider re-provides, plus the window table cross-window hit-testing walks.
Cheap to copy; create a sibling VDOM with `world.vdom(root)`. Core, no
feature needed.

Dioxus desktop polls every window's `VirtualDom` on the main thread, and
signal storage is thread-local rather than runtime-local, so a `Signal`
(and therefore a `DndContext`) created in one window's runtime can be read,
written and subscribed from another's - a write in window A re-renders
window B through B's own scheduler. `DndWorld` builds on exactly that: the
payload crosses windows as a live Rust value, with no serialization and
none of the platform roulette of native HTML5 drag-and-drop.
(`DataTransfer` interop for drags that leave the app entirely stays in the
`external` module.)

**Coordinate spaces.** Everything zone-shaped stays in client CSS pixels of
its own window, exactly as in single-window use. The world adds one more
space: global desktop physical pixels, in which windows are located and
hit-tested. Each window's `WindowGeometry` carries the conversion: the
client area's top-left in physical px (`inner_position()` on desktop), the
window scale factor, and the client-area size in physical px. Conversion
happens only at the world boundary.

**Lifetimes.** A world's own state (the shared context and the window
table) is process-lived: it is created under an owner this crate holds for
the life of the app, not under any window's scope. Whichever window created
the world can close first and every other window keeps dragging -
cross-window between the survivors, single-window when only one remains.
Closing a joined window prunes it from the table and aborts an in-flight
drag that originated there (its coordinate anchor is gone); a drag merely
hovering that window's zones just loses the hover. The cost is a
deliberate, bounded leak: a handful of signals per world, once per app.

**Degradation.** Without geometry the world degrades gracefully: drags
behave exactly as single-window drags. This is also the honest Wayland
story, where a client can learn neither the cursor's global position nor
its own windows' positions.

Joining is automatic: a `DndProvider<T>` that finds a `DndWorld<T>` in
context joins it instead of creating isolated state; only a window's
outermost provider of `T` joins, and nested providers keep the usual
shadowing semantics.

### Construction and context

| Method | What it does |
|---|---|
| `new() -> DndWorld<T>` | Create a world with process-lived state. Must run inside a Dioxus app; prefer `use_dnd_world`, which also provides it in context. `Default` delegates here. |
| `vdom(root: fn() -> Element) -> VirtualDom` | Create a sibling VDOM with this world already seeded in root context. Chain `with_root_context(model)` and per-window data afterwards. |
| `context() -> DndContext<T>` | The shared drag context every joined provider re-provides. |

### Window lookup and geometry

| Method | What it does |
|---|---|
| `record(key: WindowKey) -> Option<WindowRecord<T>>` | Look up a joined window; `None` for unknown keys. Non-subscribing. |
| `windows() -> Vec<WindowRecord<T>>` | Every joined window, in join order. Subscribing read; its consumers are renderers and tests. |
| `window_under(global: Point) -> Option<WindowRecord<T>>` | The eligible window containing `global` (physical px), most recently focused first when several overlap. `None` while no live geometry contains the point. |
| `resolve_global(global: Point) -> Option<(WindowRecord<T>, Point)>` | Resolve a global point to (window, client-local point). `None` when no live window contains it. |
| `refresh_all_rects()` | Ask every joined window to re-measure its zones, each inside its own runtime. |

### Host-side drive

Entry points for glue that sees the pointer where webviews cannot; the
`desktop` feature's bridge legs call these, and a custom (non-tao) host
calls the same ones. All are gated by the kill switch: while bridging is
disabled they are inert.

| Method | What it does |
|---|---|
| `begin_from(key: WindowKey)` | Mark a drag as begun from `key` and reset stale presentation state. `Draggable` calls this at pickup; call it from custom drag sources so the world knows which window's client px `ctx.pointer()` is in. |
| `track_global(global: Point)` | Track an in-flight pointer drag from a host-reported cursor position (global physical px): updates the shared pointer (converted into the origin window's client px, the coordinate anchor everything else expects) and enters/leaves zones across every joined window. No-op when nothing is dragging or the origin window is unknown. |
| `drop_at_global(global: Point) -> Option<ZoneId>` | Complete an in-flight pointer drag at a host-reported position: last acceptable exact hit in registry order within whichever window contains the point (rejecting overlaps are skipped), else that window's 48px near-miss snap in its own CSS px, else cancel. Returns the receiving zone. A no-op returning `None` when nothing is dragging, so double delivery (webview pointerup plus host echo) is harmless. Requires `T: PartialEq`. |
| `cancel_drag()` | Abort an in-flight drag from the host side (a window manager signal, an escape hatch). No-op when nothing is dragging. Deliberately stays live under the kill switch. |
| `modifiers() -> Modifiers` | Modifiers currently associated with host delivery; empty outside an active world drag. |
| `update_modifiers(modifiers: Modifiers)` | Update the live modifiers for the active world drag. Late host events after completion are ignored. |
| `origin_window() -> Option<WindowKey>` | The window the in-flight drag started in. Glue uses it to tell "origin window, webview owns the events" from "foreign window, I am the drag's eyes". |
| `active_record() -> Option<WindowRecord<T>>` | The record of the window the in-flight drag started in. |
| `global_pointer() -> Option<Point>` | The in-flight pointer in global physical px. `None` until a world pointer can be resolved or after the world drag finishes. |

Every host leg converges on `track_global`, so overlapping legs are safe by
construction: same-tick reports are idempotent (every write is
equality-guarded, re-entering the current zone is a no-op), legs serialize
on the one event-loop thread, and a tick landing after a drop cannot
resurrect the drag. Drop and source callbacks may synchronously begin a
replacement; completion re-reads source-session ownership and live drag state
afterwards before clearing world metadata. Host legs separately revalidate
their captured composite generation immediately before calling into the world.

### Drag metadata

| Method | What it does |
|---|---|
| `source_location() -> Option<ZoneLocation>` | Window-qualified source zone of the active world drag. The legacy `DndContext` id accessors remain unchanged. |
| `over_location() -> Option<ZoneLocation>` | Window-qualified hover of the active world drag. |
| `drag_session() -> Option<DragSessionId>` | Current tracked pointer-drag session, if this world owns one. |

### Settle routing

In a joined world, only the elected window's overlay presents and finishes
a drop-settle glide; built-in delivery elects the receiving window
automatically, and the claim survives the origin window closing
mid-animation.

| Method | What it does |
|---|---|
| `claim_settle(key: WindowKey)` | Elect `key` to present the next world settle. Custom world delivery calls this before `DndContext::take_settling`; built-in delivery claims automatically. The claim is required, not advisory: a custom source that takes the settle without claiming gets no glide anywhere. |
| `finish_settle_from(key: WindowKey) -> bool` | Finish a custom or built-in settle from its elected window. Custom world overlays should use this rather than finishing the shared context directly, so world metadata is cleared with it. |
| `settling_in() -> Option<WindowKey>` | The window elected to present the current settle glide. |

### The bridging kill switch

| Method | What it does |
|---|---|
| `set_bridging(enabled: bool)` | Enable or disable host-side bridging at runtime - the lever for the day a webview or OS update ships a cross-window regression that a rebuild cannot wait for. While disabled, every host-drive entry point is inert and the `desktop` bridge legs stand down, so drags degrade to per-window, exactly the already-modeled Wayland behavior. Local drags, geometry, settle and delivery are untouched. `cancel_drag` deliberately stays live. |
| `bridging_enabled() -> bool` | Is host-side bridging currently enabled? |

The switch is owned by the world, not the desktop adapter, so a custom host
cannot keep driving a world whose app disabled bridging. Its initial value
comes from `DIOXUS_DND_NO_BRIDGE`, read once at world creation with opt-out
semantics: only an explicit non-empty, non-`0` value disables, so an unset
or neutered variable can never strand cross-window drags by accident.

## `use_dnd_world`

`fn use_dnd_world<T: Clone + 'static>() -> DndWorld<T>` - core.

Create a `DndWorld<T>` (process-lived, see the lifetimes note above) and
provide it in context, so providers in this window join it. Create sibling
windows with `world.vdom(root)`, which pre-seeds their root context. Call it
once, in any window.

## App and dynamic model ownership

Signals or stores placed in payloads and shared models must outlive every
window that can render them. The core API has two layers for that:

### `use_dnd_model`

`fn use_dnd_model<M: Clone + 'static>(init: impl FnOnce() -> M) -> M`

Runs `init` once under a process-lived owner pair, provides the resulting
model in context, and returns it. Seed the returned `M` into spawned windows
after `world.vdom(root)`. Its storage is deliberately process-lived, so a
copyable signal/store handle can never dangle regardless of close order; the
bounded cost is one owner pair per app-wide model.

Every owner-backed value needing that lifetime must be allocated
synchronously inside `init`. Passing in a signal created by `use_signal`
before calling `use_dnd_model` does not transfer ownership, and a later
allocation uses the owner current at that later call. Later app-lived state
can use its own `DndScope` retained inside process-lived model state.

The owner pair contains both Dioxus storage flavors. Signals allocate in
unsynchronized storage, while a `Store` allocates its subscription tree in
synchronized storage; retaining only the former leaves a store that can fail
after its creator window closes.

### `DndScope`

A cloneable, `Rc`-shared owner pair for dynamic state that really should be
reclaimed. Create one with `DndScope::new()` (or `Default`), then call
`scope.with(|| ...)` inside a Dioxus runtime to mint signals or stores under
it. The state remains live while any scope clone exists and is reclaimed when
the last clone drops. Do not drop that last clone while a read or write guard
from its storage is active: unsynchronized recycling may panic on the live
borrow, while synchronized recycling must wait for the lock guard.

Use this for per-window state such as a tray's cards. Teardown should first
remove or move every live handle from the shared app model, then drop the
scope as one operation. The
[`desktop-multiwindow` example](../../examples/desktop-multiwindow/src/main.rs)
shows that choreography.

## `use_joined_window` and `JoinedWindow`

`fn use_joined_window<T: Clone + 'static>() -> Option<JoinedWindow<T>>` -
core.

The enclosing provider's world membership, if it joined a world: the handle
desktop glue needs to bridge host-side input. Call it anywhere below the
`DndProvider`; it returns `None` under a provider that created isolated
state. Membership shadows like context does, so nested providers report
their own.

`JoinedWindow<T>` is a copyable handle with three public fields:

| Field | Type | Meaning |
|---|---|---|
| `world` | `DndWorld<T>` | The world this provider joined. |
| `key` | `WindowKey` | This window's key in the world table. |
| `geometry` | `WindowGeometry` | This window's geometry handle. |

| Method | What it does |
|---|---|
| `location(zone: ZoneId) -> ZoneLocation` | Qualify one of this window's local zone ids for world state. |
| `enter(location: ZoneLocation)` | Mark a window-qualified zone as hovered. Custom world-aware sources should use this rather than the legacy id-only context method. |
| `clear_hover()` | Clear both the qualified world hover and the legacy context hover. |
| `is_over(zone: ZoneId) -> bool` | Whether this exact window/zone pair owns the world hover. |
| `local_pointer() -> Option<Point>` | The latest global pointer converted into this window's client CSS coordinates. If geometry disappeared mid-gesture, the origin window retains its established context-local fallback. |

## `WindowGeometry`

One window's placement on the desktop, as reactive signals the host feeds:
client-area origin in global physical px, client-area size in physical px,
scale factor, a focus stamp, and an eligibility flag. Copy handle; create
one per window (the provider creates an inert one when none is in context)
and keep it updated from your windowing layer. Missing placement or host
ineligibility makes it inert: the window still drags internally, but cannot
take part in cross-window hit-testing. Core; on desktop,
`use_window_geometry_feed` feeds it for you.

| Method | What it does |
|---|---|
| `new() -> WindowGeometry` | A fresh, inert geometry owned by the current scope. Eligibility defaults on, so hosts that only feed placement keep working. `Default` delegates here. |
| `set(origin: Point, size: (f64, f64), scale: f64)` | Update the placement: client-area origin and size in global physical px, plus the scale factor. No-op writes are skipped, so it is safe from high-frequency window events. |
| `clear()` | Forget the placement (geometry became unavailable); the window keeps working as a single-window drag surface. |
| `set_eligible(eligible: bool)` | Include or exclude this window from global hit-testing without discarding its last known placement. |
| `eligible() -> bool` | Whether the host currently allows this window to receive a global drag. Subscribing, dead-safe read. |
| `mark_focused()` | Record that this window was just focused. Call it on focus events so overlapping windows resolve to the frontmost. |
| `live() -> bool` | Is the placement known and currently eligible for global hit-testing? Subscribing, dead-safe read that subscribes to each input independently, so an inert geometry still wakes when any capability arrives. |
| `to_global(client: Point) -> Option<Point>` | This window's client CSS px to global physical px. `None` until the placement is known. |
| `to_client(global: Point) -> Option<Point>` | Global physical px to this window's client CSS px. `None` until the placement is known. |
| `contains_global(global: Point) -> bool` | Does this eligible window's client area contain `global` (physical px)? Edge-inclusive; always false while placement is unknown or eligibility is off. Peeks rather than subscribes, so imperative hit-testing cannot subscribe its caller. |
| `scale() -> f64` | The window's scale factor (physical px per CSS px). |
| `focus_stamp() -> u64` | The monotonic focus stamp; higher is more recently focused, 0 means never focused. No z-order query exists on desktop, so focus recency approximates it for overlap ties. |

Reads degrade to "geometry unknown" when the underlying signals are gone: a
geometry's signals usually die with their window's `VirtualDom`, but a copy
inside a `WindowRecord` or a handler closure can race the pruning by one
event. On Windows that late read happens inside a Win32 callback, where a
panic cannot unwind and kills the process, so degrading is honest - stale
geometry is already a modeled state (Wayland).

## `WindowKey`

`WindowKey(pub u64)` identifies one joined window within a `DndWorld`.
Process-unique; `WindowKey::auto()` generates the next one. Ordinary derive
set (`Copy`, `Eq`, `Ord`, `Hash`), so it works as a map key. Core.

## `WindowRecord`

One window joined to a `DndWorld`: its geometry, its zone registry, and the
per-window handles drop delivery needs. Copy handle. Core.

| Field | Type | Meaning |
|---|---|---|
| `key` | `WindowKey` | The window's identity in the world table. |
| `geometry` | `WindowGeometry` | The window's placement handle. |
| `registry` | `ZoneRegistry<T>` | The window's own zone registry, which world hit-testing consults in that window's client px. |

The record also privately carries the window's settle flag (a drop landing
here settles iff this window has a settle-enabled overlay mounted) and a
re-measure callback created by the window's provider, so
`refresh_all_rects` runs each window's measurement inside its own runtime.

## `ZoneLocation`

A drop-zone identity qualified by the joined window that owns it:

| Field | Type | Meaning |
|---|---|---|
| `window` | `WindowKey` | The window whose registry holds the zone. |
| `zone` | `ZoneId` | The zone's id within that window. |

Legacy single-window APIs continue to expose plain `ZoneId`; worlds use
this richer identity so separate windows may safely reuse the same explicit
id without mirroring hovers or misrouting drops. Full derive set including
`Hash` and `Ord`. Core.

## Where the rest lives

`DndContext`, `ZoneRegistry`, `Point` and `ZoneId`:
[docs/api/core.md](core.md). The components each window renders
(`DndProvider`, `Draggable`, `DropZone`, `DragOverlay`):
[docs/api/drag-and-drop.md](drag-and-drop.md). `PointerKind` and its
`implicitly_captured` test, which the bridge gates on:
[docs/api/core.md](core.md). The payload-reactivity ownership rule and the
`use_dnd_model`/`DndScope` reference implementation:
[docs/concepts/multi-window.md](../concepts/multi-window.md) and the
[`desktop-multiwindow` example](../../examples/desktop-multiwindow/src/main.rs).
Per-platform verification detail:
[PLATFORMS.md](../../PLATFORMS.md).

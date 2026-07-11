# Multi-window desktop drags

On desktop, one app is often several windows - and a drag should not care.
Create a `DndWorld<T>` in your first window, hand it to the others, and
every joined window shares one drag: zones light up across windows, the
ghost hands off to whichever window the cursor is over, and the payload
arrives as a live Rust value through the same `on_drop` you already have.

API reference: [api/multi-window.md](../api/multi-window.md).
Live demos: this concept is desktop-only, so it runs in the repo instead of
the web gallery -
[`examples/desktop-showcase/`](../../examples/desktop-showcase/) drags live
signal-backed widgets between windows, and
[`examples/desktop-multiwindow/`](../../examples/desktop-multiwindow/) is
the board-and-N-trays app the platform verification drives.

## The mental model

One drag world, several windows:

- `DndWorld<T>` is one shared drag state spanning several windows, each
  window an independent `VirtualDom`. Dioxus desktop polls every window's
  `VirtualDom` on the main thread, and signal storage is thread-local
  rather than runtime-local, so a signal created in one window's runtime
  can be read, written and subscribed from another's: a write in window A
  re-renders window B through B's own scheduler. The world builds on
  exactly that. The payload crosses windows as a live Rust value, with no
  serialization and none of the platform roulette of native HTML5
  drag-and-drop.
- `use_dnd_world::<T>()` creates the world, once, in any window, and
  provides it in context. Hand the returned handle to sibling windows with
  `VirtualDom::with_root_context`.
- A `DndProvider<T>` that finds a `DndWorld<T>` in context joins it instead
  of creating isolated state. Only a window's outermost provider of `T`
  joins; nested providers keep their usual shadowing semantics.

Coordinates stay simple: everything zone-shaped remains in client CSS
pixels of its own window, exactly as in single-window use. The world adds
one more space, global desktop physical pixels, in which windows are
located and hit-tested. Each window's `WindowGeometry` carries the
conversion, and conversion happens only at the world boundary.

## A worked example

```rust,ignore
use dioxus::desktop::{window, Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus_dnd::desktop::{use_window_geometry_feed, DragBridge};
use dioxus_dnd::prelude::*;

fn board_window() -> Element {
    let world = use_dnd_world::<Card>();      // once, in any window
    use_window_geometry_feed();               // ABOVE the provider
    let open_tray = move |_| {
        window().new_window(
            VirtualDom::new(tray_window).with_root_context(world),
            Config::new().with_window(WindowBuilder::new().with_title("Tray")),
        );
    };
    rsx! {
        DndProvider::<Card> {                 // joins the world via context
            DragBridge::<Card> {}             // INSIDE the provider
            button { onclick: open_tray, "Open tray" }
            // ... zones, DragOverlay, LiveRegion ...
        }
    }
}

fn tray_window() -> Element {
    use_window_geometry_feed();
    rsx! {
        DndProvider::<Card> {                 // joins via root context
            DragBridge::<Card> {}
            // ... this window's zones ...
        }
    }
}
```

Drag a card from the board over a tray: the tray's zones carry
`data-active` and `data-over` like any local drag, the ghost leaves the
board window and reappears under the cursor in the tray (sized by the
origin-to-receiver scale ratio, so mixed-DPI setups hand off cleanly), and
the tray's `on_drop` receives the same `DropOutcome<Card>` a single-window
drop delivers.

## The two pieces of desktop glue

`DndWorld` is core and dependency-free: it consumes window geometry it does
not compute and host-reported pointer data it cannot see. The `desktop`
cargo feature (`dioxus_dnd::desktop`) supplies both from tao. The feature
pulls dioxus-desktop (wry/tao), so it is off by default and the core stays
dependency-free.

- `use_window_geometry_feed()`, called ABOVE the provider: feeds this
  window's position, size and scale from tao window events into a
  `WindowGeometry`. The provider picks that geometry up from context when
  it joins the world, which is why the order matters.
- `DragBridge::<T>`, rendered INSIDE the provider (it needs the join):
  host-side eyes and ears for pointer drags that leave the origin window.

The bridge exists because of how pointer input behaves mid-drag. Webview
pointer events stop at the viewport edge, and while a button is held every
non-origin window is fully event-blind - that is how pointer grabs work.
So:

- the origin window's bridge polls the global cursor to keep tracking the
  drag outside its own viewport;
- a blind window's first pointer event mid-"drag" proves the button is
  already up, and completes the drop right there;
- on Windows the shape is the opposite: the WebView2 child HWND consumes
  the mouse stream before tao ever sees it, so both portable legs are dead
  there and a third raw-input leg (`WM_INPUT`) carries pointer and live
  modifiers instead;
- on Linux the policy is decided at runtime from tao's actual backend. X11
  gets the portable legs plus a root pointer-button observer, so a release
  over desktop dead space (no window at all) cannot strand a drag. Wayland
  exposes neither global geometry nor a global cursor by design, so every
  global leg is off by policy and drags stay per-window. A transient X11
  sample miss is retried; it is never mistaken for Wayland.

Every leg is gated on the drag's `PointerKind`: mouse and pen are bridged,
touch never is. A touch drag is already streamed whole to the origin
webview by the browser's implicit capture; bridging it too double-drives
the drag from Windows' touch-synthesized mouse events.

## Payloads that own reactivity

The showcase puts a live `Signal` handle inside the payload, which is what
keeps the ghost's chart animating mid-drag. That is safe only when the
signal's storage outlives every window that can render it. A signal created
inside a window's component scope dies with that window; a surviving window
still holding the payload then reads a dead signal.

The cure is model-owned storage: a root `Owner<UnsyncStorage>` held in an
`Rc` that every window keeps alive, with every payload signal created under
it via `dioxus::core::with_owner`. The reference implementation is
`ModelOwner` in
[`examples/desktop-showcase/src/model.rs`](../../examples/desktop-showcase/src/model.rs):
every window retains the `Rc`, so the model earns the same close-order
guarantee as the world itself, and per-window state (a satellite's widget
list) gets its own owner so it is reclaimed promptly when that window
closes.

## The invariants

A few invariants hold the world together:

- **Identity is window-qualified.** Two windows may reuse the same explicit
  `ZoneId` without mirroring each other's hover highlight or misrouting a
  drop. The world tracks `ZoneLocation { window, zone }`; single-window
  code keeps using plain `ZoneId`, unchanged.
- **Receivers think in their own coordinates.** Edge highlights, tree drop
  intent and auto-scroll in the window under the cursor read the shared
  pointer converted into that window's client space, never the origin's.
- **Modifiers stay live across windows.** Ctrl/Cmd or Alt held at a
  host-side release resolves to the same Copy/Link effect as a local drop.
- **Hidden windows catch no drops.** A minimized or hidden window keeps its
  last geometry for a later restore but cannot win global hit-testing while
  it is ineligible.
- **The receiving window owns the drop-settle.** The glide presents in the
  window that took the drop, survives the origin window closing
  mid-animation, and only that window can finish it. Custom delivery code
  claims this with `DndWorld::claim_settle`.
- **Callbacks revalidate ownership.** Receiver and source callbacks may
  synchronously begin a replacement drag. The old result commits before
  receiver code; completion then re-reads source-session ownership and live
  drag state before clearing metadata. Host legs separately revalidate their
  composite generation immediately before acting. See
  [the callback invariant](architecture.md#callback-boundaries-are-generation-boundaries).

## Close order and degradation

Windows may close in any order. The world's state is process-lived, created
under an owner the crate holds for the life of the app rather than under
any window's scope, so whichever window created the world can close first
and the survivors keep dragging - cross-window between the survivors,
single-window when only one remains. The cost is a deliberate, bounded
leak: a handful of signals per world, once per app. Closing a joined window
prunes it from the table; a drag that originated there aborts (its
coordinate anchor is gone), while a window that was merely being hovered
just loses the hover.

Where geometry is unavailable, everything degrades to normal per-window
drags rather than breaking. Wayland is the designed-in case, and the same
degradation doubles as a kill switch: `DndWorld::set_bridging(false)` at
runtime, or `DIOXUS_DND_NO_BRIDGE=1` in the environment before launch (no
rebuild), stands every bridge leg down if a webview or OS update ever ships
a regression in these mechanics. With `tracing` at `debug`, each leg logs
when it engages a drag (`cursor-poller`, `release`, `x11-deadspace`,
`raw-input`), so a post-update bug report arrives pre-triaged to the leg
whose platform assumption moved.

## Platform verification

Verified per platform (2026-07); the full log - rigs, commits, what each
session exercised, and how each platform's bridge mechanics work - lives in
[PLATFORMS.md](../../PLATFORMS.md).

| Platform | Status |
|---|---|
| Linux/X11 | Verified end to end: cross-window hovers, ghost handoff, drops, dead-space release |
| Linux/Wayland | Cross-window impossible by OS design; drags gracefully stay per-window |
| Windows (WebView2) | Verified end to end: mouse and touch, modifiers, close-order churn |
| macOS (WKWebView) | Expected to work on the same APIs; not yet hand-verified |

## Gotchas

- **Order is load-bearing.** `use_window_geometry_feed()` goes ABOVE the
  provider (the provider reads the geometry from context at join);
  `DragBridge` goes INSIDE it (it needs the membership). Swap them and the
  window joins with inert geometry or the bridge no-ops.
- **Create drop-target windows visible.** Windows created hidden and shown
  later have broken drag-and-drop in WebView2 (wry#1639).
- **No window-scoped signals in payloads.** They die with their window; use
  the `ModelOwner` pattern above.
- **Touch is never bridged, on purpose.** Implicit capture already delivers
  the whole gesture to the origin webview; do not try to bridge around it.
- **The Windows leg flips a process-global tao setting.** Raw input needs
  `DeviceEventFilter::Never`; the bridge claims it exactly once, on Windows
  only. Apps with their own raw-input handling should know.
- **Cross-window drags are unit-testable.** The world-aware `DragSim`
  simulates whole cross-window arcs headlessly in CI; see
  [Testing](testing.md).

## Related

- [api/multi-window.md](../api/multi-window.md): every method of
  `DndWorld`, the glue components, and the geometry types.
- [Drag and drop](drag-and-drop.md): the components each window renders;
  nothing about them changes in a world.
- [Architecture](architecture.md): the context and registry a world shares.
- [Mixing payload types](mixing-payload-types.md): one world is one payload
  type; enums and bridge zones cover polymorphism.
- [PLATFORMS.md](../../PLATFORMS.md): the per-platform verification
  evidence.

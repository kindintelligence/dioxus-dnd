# Platform verification log

The README's [Platform notes](README.md#platform-notes) carry the one-line
per-platform verdicts; this file carries the evidence behind them - what was
exercised, on what rig, at what commit, and how each platform's bridge
mechanics actually work. It exists so a future regression report can be
compared against a concrete baseline instead of a vague "used to work".

Multi-window drags, verified per platform (2026-07):

## Linux/X11

Works end to end - cross-window hovers, ghost handoff, drops and release
over desktop dead space - with the `desktop` feature's geometry feed,
global-cursor poller and X11 root pointer-button observer. Polling is bound
to the drag's composite generation, so a sleeper from drag N cannot attach
to drag N+1.

**WSLg caveat**: under WSLg specifically, session state can corrupt
move-event button masks; the library debounces, but treat WSLg as a
smoke-test rig, not a verdict machine.

## Linux/Wayland

Cross-window is impossible by OS design (a client can learn neither its
windows' positions nor the global cursor); Tao's live backend selection
disables those global legs explicitly, rather than inferring Wayland from a
failed API call. Local drags stay fully active and geometry remains inert
without cursor-query error spam.

Verified under WSLg with Tao reporting Wayland; forced X11 was smoke-tested
separately rather than assumed from the environment (see the WSLg caveat
above for why neither run counts as a full verdict).

## Windows (WebView2)

Verified end to end at `f681b9c` (Win 11 Home ARM64 build 26200, single
1920x1200 monitor at 1.5x scale, 2026-07-10, real `SendInput`
mouse/keyboard and `InjectTouchInput` touch).

### What the session exercised

Board+3-tray drag chain in both directions, exactly one window highlighting
at a time, ghost handoff glued to the native pointer, dead-space cancel
with immediate same-source restart, drop then immediate re-drag,
Ctrl=Copy/Alt=Link resolved from modifiers changed outside the origin
viewport, mid-drag target resize refreshing world rects, hovered-window
close surviving the drag, origin close cancelling exactly once, minimized
windows rejecting hover and delivery, tray close/reopen churn, touch
interleaved with mouse (zero ghost-trajectory reversals), and the
board-first close-order regression with the surviving tray staying
interactive. Logs stayed free of ownership warnings, panics and fatal
callback exceptions; both sessions exited cleanly.

Multi-DPI monitors remain unexercised (single-monitor rig); the earlier
`60e642c` baseline evidence (same machine, 2026-07) still stands.

### How the bridge works there

WebView2 keeps streaming mouse events to the origin webview outside its
viewport, yet they target `<html>` (nothing retargets without pointer
capture) so no component hears them, and tao never sees
`CursorMoved`/`MouseInput` because the WebView2 child HWND consumes them.
The sealed library bridge uses raw input:
`DeviceEvent::Button`/`MouseMotion`/`Key` (the keyboard leg feeds live
modifiers - tao's `ModifiersChanged` never fires because the WebView2 child
HWND owns keyboard focus) through `use_wry_event_handler` plus
`set_device_event_filter(DeviceEventFilter::Never)` (the default
`Unfocused` filter never delivers - the foreground input owner is the
WebView2 process's HWND).

### Touch is never bridged

Implicit capture streams the whole gesture to the origin webview, so touch
needs none of this - and MUST NOT be bridged: Windows synthesizes mouse
input from touch (a cursor trailing the finger, spurious button
transitions), so bridging a touch drag double-drives it. The bridge gates
every bridge leg on the drag's `PointerKind` (recorded by `Draggable` at
pickup, `ctx.pointer_kind()`): bridge mouse and pen, never touch.

### Known trap

Windows created hidden then shown have broken DnD in WebView2
([wry#1639](https://github.com/tauri-apps/wry/issues/1639)), so create
drop-target windows visible.

## macOS (WKWebView)

Expected to work on the same reasoning (AppKit routes the whole drag
sequence to the mousedown view; `cursor_position` supported); not yet
hand-verified (call for testers:
[#20](https://github.com/kindintelligence/dioxus-dnd/issues/20)).

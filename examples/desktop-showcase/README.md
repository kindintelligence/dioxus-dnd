# Mission Control - the multi-window showcase

Live, signal-backed widgets you can drag between native windows - and they
**never stop animating, even inside the drag ghost, mid-flight between
windows**. That is the point: the drag payload is a live `Signal` handle
into shared model storage, not a serialized snapshot, which no OS drag
protocol could carry. See the design doc at
`docs/superpowers/specs/2026-07-10-desktop-showcase-design.md`.

## Run it

```sh
cd examples/desktop-showcase
cargo run
```

(Standalone package on purpose - dioxus-desktop's wry/tao stack must not
weigh down the main crate's builds. Linux needs the usual dioxus-desktop
system libs.)

### WSLg

From the repository root, launch the same charts example through WSLg/X11:

```sh
bash examples/desktop-showcase/run-wsl.sh
```

The launcher defaults to X11 so WSLg exposes the global coordinates needed
for full cross-window drag handoff. To check the intentional Wayland fallback:

```sh
GDK_BACKEND=wayland bash examples/desktop-showcase/run-wsl.sh
```

There is no separate WSL Rust implementation: both launchers use the same
`src/` code, including `use_dnd_model`, reclaimable `DndScope`s,
`DndWorld::vdom`, and `MultiWindowProvider`.

## Things to try

- **Open satellite** spawns tear-off windows; open as many as you like.
- Drag the telemetry chart into a satellite - watch it keep streaming in
  the ghost as it crosses the desktop gap.
- **Hold Ctrl while dropping** to clone: two independently ticking copies
  (the resolved `DropEffect::Copy`, fed by the raw-keyboard modifier leg
  on Windows). Alt (Link) is accepted and treated as Move here.
- Close windows in any order - including Mission Control first. Widgets
  keep running: the liveness ticker fails over to a surviving window
  (`ticker.rs`), and a closing satellite returns its widgets to the dock
  (`model.rs`, using `use_dnd_model` plus reclaimable `DndScope`s).
- Press **D** in any window to snap the demo filming layout
  (positions tuned for a 1920x1200 @1.5x screen, clamped elsewhere).

## Layout

- `model.rs` - shared live model: app-lived ownership, reclaimable satellite
  scopes, widget lifecycle, and move/clone semantics. Unit-tested across
  VirtualDoms.
- `ticker.rs` - the liveness engine: pure deterministic state advance
  (unit-tested) plus the claim/release failover hook.
- `widgets/` - one file per widget body (sparkline, stopwatch, deploy
  ring, pulse), all pure SVG/CSS.
- `theme.rs` - the ops-console theme, one CSS const.
- `layout.rs` - the D-key demo layout.
- `main.rs` - window wiring only.

# Mission Control: the multi-window showcase example

Date: 2026-07-10
Status: approved (design conversation, Chad + Claude)
Scope: new example + filming assets only. Zero library changes.

## Goal

A polished, runnable showcase for the `desktop` feature's multi-window
drag-and-drop, built for two audiences at once:

1. **The README hero GIF** (~12 s, loopable, legible at ~800 px): one
   frame no serialization-based library can produce - a chart that keeps
   streaming *inside the drag ghost* while crossing between windows.
2. **Hands-on users** (`cargo run` from the example directory): an app
   worth poking at, with N satellite windows, any close order, and the
   Ctrl-clone trick discoverable by accident.

The demo doubles as an architecture proof: every widget's payload
carries a live `Signal` handle - a thing that cannot be serialized -
so "live payloads, not serialization" is visible, not claimed.

## Deliverables

- `examples/desktop-showcase/` - standalone package (own `Cargo.toml` +
  lockfile, same pattern as `examples/desktop-multiwindow/`), binary
  `desktop-showcase`.
- A scripted "cinematography" scenario for the existing SendInput rig
  (slow, eased drags; identical take every run). Lives with the rig,
  not in the repo.
- `assets/showcase.gif` (README hero) + an extended-cut mp4 for the
  Discord/showcase post (3 windows + close-Mission-Control-first beat).
- README hero section: GIF at top, one-paragraph claim, link to the
  example.

## Storyboard (hero GIF)

1. **Open:** Mission Control window (left, dark ops-console) with four
   live widgets - streaming sparkline (cyan), ticking stopwatch
   (amber), deploy progress ring (green), pulse monitor (red). One
   Satellite window (right), empty: "Drop a live widget here."
2. **Money shot:** slow drag of the sparkline out of Mission Control.
   It keeps streaming inside the ghost, crosses the desktop gap, the
   Satellite zone glows cyan, drop, settle-glide in - never missing a
   sample.
3. **Modifier flex:** Ctrl-drag the stopwatch across; it drops as a
   clone - two stopwatches ticking in two windows. (Exercises the
   Windows raw-keyboard modifier leg shipped in f681b9c.)
4. **Return + loop:** drag the sparkline back home; both windows idle
   with everything alive; loop point.

Extended cut adds: a second satellite, then closing Mission Control
first while satellites keep running (the model-owner lifetime work from
b0b2cec, visible).

## App design

### Windows

- **Mission Control**: wide dark window, header with app title, live
  window-count pill ("3 windows joined"), "Open satellite" button, and
  a 2x2 widget grid (one `DropZone`).
- **Satellite N**: small window, one `DropZone`, empty-state coaching
  text. Spawned via `VirtualDom::with_root_context` exactly like the
  workhorse example (world + model owner + satellite identity).
- **Demo layout key**: pressing `D` in any window snaps all windows to
  the filming layout (positions/sizes via `dioxus_desktop::window()`
  handles; fixed physical coordinates tuned for a 1920x1200 @1.5x
  screen, clamped to the work area elsewhere). One keypress =
  reproducible cinematography.

### Widgets

All pure CSS/SVG + signals; no new dependencies.

| Widget | Live behavior | Accent |
|---|---|---|
| Sparkline | SVG polyline, 60-sample smooth random walk, ~20 fps | cyan |
| Stopwatch | mm:ss.t monospace, 10 Hz | amber |
| Deploy ring | SVG stroke-dashoffset percentage, loops with phase label | green |
| Pulse | ECG-style trace + BPM readout, occasional beat wobble | red |

`Widget { id: u32, kind: WidgetKind, state: Signal<WidgetState> }` is
the drag payload type (`T = Widget`). The signal handle is what makes
the ghost live: `DragOverlay { match_source: true }` renders the same
card component, which reads `state` and therefore subscribes and
rerenders on every tick, wherever the ghost is presenting.

### Model and lifetime

Same `Rc<ModelOwner>` pattern as the workhorse (proven by the
close-order regression): model-owned signal storage shared by all
windows, per-satellite reclaimable owners, cards returned to Mission
Control's dock list if a satellite closes (and to the invisible model
if Mission Control itself is gone - never lost, never dangling).

### Ticker (liveness engine) with failover

One logical ticker advances all widget states (~50 ms interval). To
survive any window close order:

- A process-wide claim (`AtomicBool` or equivalent in the model owner)
  marks "ticker is running".
- Every window runs a small `use_future`: try to claim; if claimed,
  drive ticks until this window's runtime drops (release claim in
  `use_drop`); if not claimed, poll ~500 ms and adopt the ticker when
  the claim frees.
- Result: widgets keep animating no matter which windows close, with a
  worst-case half-second hiccup on ticker handoff.

### Drop semantics

- Default drop = **move** (widget leaves source list, joins target).
- **Ctrl (Copy effect)** = clone: new `id`, state forked into a fresh
  signal seeded from the source's current values, then both run
  independently. `DropOutcome::effect` drives this in the example's
  `on_drop`.
- Alt (Link) is accepted but treated as move (documented in a code
  comment; the example doesn't need a third semantic).

## Visual direction

Dark ops-console: near-black blue base (#0B0E14 family), frosted-glass
cards (subtle translucent fill, 1 px hairline borders), one neon accent
per widget kind, soft outer glows, generous type sizes tuned for a
800 px GIF. Drop zones show a dashed eligible outline during any drag
and glow the incoming widget's accent on hover. Ghost = same card with
lifted shadow/stronger glow. Flat dark desktop wallpaper for filming.
Implementation uses the frontend-design skill; visual quality bar is
"screenshot could pass for a commercial app".

## Filming plan

- Cameraman: the existing `%TEMP%\dnd-wtest` rig (real `SendInput`),
  new `film` scenario with eased glide curves and beat timings matching
  the storyboard. Deterministic take, re-recordable after any tweak.
- Capture: ffmpeg (gdigrab) or ScreenToGif on this machine; GIF via
  palettegen for the README (target < 10 MB), mp4 for the extended cut.
- Layout: `D` key first, then the scripted take.

## Verification

1. **Spike first (gate for the whole design):** before any polish,
   prove a ticking signal inside the payload rerenders the ghost
   mid-drag across windows on this machine. If it doesn't, stop and
   reassess (library finding, separate work).
2. Standard example gates: `cargo test/clippy/build --locked` for the
   new package; strict clippy; fmt on new files; `git diff --check`.
3. One scripted rig drag against the showcase (smoke, not the full
   matrix - the workhorse owns the matrix).
4. The workhorse example and its rig stay byte-identical.

## Non-goals

- No library API changes (any gap discovered = separate finding).
- No web build of the showcase, no sound, no fake "integrations".
- No replacing `desktop-multiwindow` - it remains the verification
  target.
- No multi-DPI claims in the GIF caption (still unexercised).

## Risks

- **Ghost liveness assumption** - mitigated by the spike (step 1).
- **GIF size** vs. 20 fps animations - mitigate with 12-15 fps capture,
  tight palette, dark flat background; fall back to animated WebP if
  needed (GitHub renders it).
- **Windows-only demo-layout coordinates** - clamp to work area and
  document; the layout key is a convenience, not a requirement.

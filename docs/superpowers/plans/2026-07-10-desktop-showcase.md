# Mission Control Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `examples/desktop-showcase/` — the "Mission Control" multi-window demo whose live signal-backed widgets keep animating inside the drag ghost across windows — per `docs/superpowers/specs/2026-07-10-desktop-showcase-design.md`.

**Architecture:** Standalone example package (same pattern as `desktop-multiwindow`), modular source split: model (shared signal storage + move/clone semantics), ticker (failover liveness engine), one file per widget body, theme, demo layout. The drag payload `Widget` carries a `Signal<WidgetState>` handle, which is what makes the ghost live. Zero library changes.

**Tech Stack:** dioxus 0.7 desktop, dioxus-dnd path dep with `serde desktop` features, pure CSS/SVG (no new deps), existing `%TEMP%\dnd-wtest` SendInput rig for live verification.

## Global Constraints

- Zero changes under `src/` (library) and zero changes to `examples/desktop-multiwindow/` — the workhorse and its rig stay byte-identical.
- No new crate dependencies beyond what `desktop-multiwindow` uses (drop `serde` if unused).
- Every commit: imperative subject + reasoned body, **no Co-Authored-By trailer**.
- Gates for the package: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo build --bins` (then `--locked` once the lockfile is committed), rustfmt-clean new files, `git diff --check`.
- All cargo commands on this machine need `$env:RUSTC_WRAPPER=''` (sccache is broken).
- Windows titles must be `dioxus-dnd - mission control` / `dioxus-dnd - satellite N` (rig-matchable, distinct from the workhorse's `board`/`tray N`).
- The spike (Task 3) is a HARD GATE: if the ghost does not visibly update mid-drag, stop and report — do not polish.

## File Structure

```
examples/desktop-showcase/
  Cargo.toml            — standalone package, empty [workspace]
  src/main.rs           — launch, Chrome shell, window components, wiring only
  src/model.rs          — WidgetKind, WidgetState, Widget, Model, ModelOwner,
                          move/clone semantics, satellite retire (close-order safe)
  src/ticker.rs         — WidgetState::advanced (pure) + use_ticker failover hook
  src/theme.rs          — STYLE const (all CSS)
  src/layout.rs         — demo layout epoch → per-window snap positions
  src/widgets/mod.rs    — WidgetCard (card chrome + accent + body dispatch)
  src/widgets/sparkline.rs, stopwatch.rs, ring.rs, pulse.rs — body components
```

---

### Task 1: Package skeleton + model core

**Files:**
- Create: `examples/desktop-showcase/Cargo.toml`, `src/main.rs` (stub), `src/model.rs`

**Interfaces (produced, used by every later task):**

```rust
// model.rs
pub enum WidgetKind { Sparkline, Stopwatch, Ring, Pulse }   // Clone, Copy, PartialEq, Debug
pub struct WidgetState {
    pub ticks: u64,        // 50ms ticks since creation (stopwatch: mm:ss.t)
    pub samples: Vec<f64>, // sparkline window, capped at 60, 0..1
    pub level: f64,        // deploy ring 0..1, wraps
    pub bpm: f64,          // pulse readout
    pub seed: u64,         // xorshift PRNG state (no rand dep)
}                          // Clone, PartialEq, Debug
pub struct Widget { pub id: u32, pub kind: WidgetKind, pub state: Signal<WidgetState> } // Clone, Copy, PartialEq
pub struct Satellite { pub n: u32, pub zone: ZoneId, pub widgets: Signal<Vec<Widget>> } // Clone, Copy, PartialEq
pub struct Model { pub dock: Signal<Vec<Widget>>, pub satellites: Signal<Vec<Satellite>>, pub layout_epoch: Signal<u32> } // Clone, Copy, PartialEq
pub struct ModelOwner { pub model: Model, /* root Owner + per-satellite owners, ticker claim */ }
impl ModelOwner {
    pub fn new() -> Rc<Self>;                       // dock seeded with the 4 widgets
    pub fn new_satellite(&self, n: u32) -> Satellite;
    pub fn close_satellite(&self, s: Satellite) -> bool; // widgets → dock, owner reclaimed, repeat-inert
    pub fn deliver(&self, w: Widget, to: ZoneId, effect: DropEffect); // Move: relocate; Copy: clone_widget then relocate clone
    pub fn clone_widget(&self, w: &Widget) -> Widget;    // fresh id, forked Signal on the root owner
    pub fn claim_ticker(&self) -> bool;                  // AtomicBool CAS
    pub fn release_ticker(&self);
}
pub const DOCK: ZoneId = ZoneId(1);
```

- [ ] Step 1: Write `Cargo.toml` mirroring `desktop-multiwindow`'s (path dep `../..`, features `["desktop"]` only — no serde needed), package `desktop-showcase`.
- [ ] Step 2: Write `model.rs` per the interface above, following the workhorse's proven `ModelOwner` shape (root `Owner<UnsyncStorage>` + `RefCell<HashMap<ZoneId, Owner>>` for satellites, guards acquired before mutation in `close_satellite`). `deliver` mirrors the workhorse `move_card` fallback: unknown zone → dock, a widget can never vanish.
- [ ] Step 3: Unit tests in `model.rs` using the workhorse's cross-VDOM test pattern (creator VDOM parks `Rc<ModelOwner>`; satellite VDOMs with `use_drop` cleanup): (a) satellite close returns widgets to dock and repeat-close is inert; (b) `clone_widget` forks state — mutating the clone's signal leaves the source unchanged; (c) `deliver` with `DropEffect::Copy` grows the target by an independent widget while the source keeps the original.
- [ ] Step 4: `main.rs` stub: `fn main()` launching one window that renders `rsx! { "showcase skeleton" }` so the package builds.
- [ ] Step 5: Run `cargo test` + `cargo clippy --all-targets -- -D warnings` in the package. Expected: tests pass, clippy clean.
- [ ] Step 6: Commit (`Add the desktop-showcase model core`), including the generated `Cargo.lock`.

### Task 2: Ticker — pure advance + failover hook

**Files:**
- Create: `examples/desktop-showcase/src/ticker.rs`
- Modify: `src/main.rs` (mod decl)

**Interfaces:**
- Produces: `impl WidgetState { pub fn advanced(&self) -> WidgetState }` (pure, in ticker.rs as an extension or in model.rs — implementer's choice, document where); `pub fn use_ticker(owner: Rc<ModelOwner>)` hook installed by every window.

Behavior of `advanced`: `ticks + 1`; xorshift64 the seed; push a new sparkline sample = previous ± small step, clamped 0..1, window capped at 60; `level = (level + 0.004) % 1.0`; `bpm` drifts inside 58..102. Deterministic given `seed` — unit-testable.

`use_ticker`: `use_future` loop — if `claim_ticker()` succeeds (or already held by this window), every 50 ms write `advanced()` into every widget signal reachable from dock + all satellites (peek the lists; `try_write`-degrade on teardown races: skip, never panic); if not held, retry claim every 500 ms. `use_drop` releases the claim so a surviving window adopts it (≤500 ms hiccup).

- [ ] Step 1: Failing unit tests for `advanced`: ticks increment, samples cap at 60, level wraps below 1.0, determinism (same seed → same next state), bpm stays in band over 10k steps.
- [ ] Step 2: Implement `advanced` + xorshift; tests pass.
- [ ] Step 3: Implement `use_ticker` + claim methods on ModelOwner.
- [ ] Step 4: Package gates (test + clippy). Commit (`Add the showcase liveness ticker with window failover`).

### Task 3: Minimal two-window app + SPIKE GATE (ghost liveness)

**Files:**
- Modify: `examples/desktop-showcase/src/main.rs` (real windows), create `src/widgets/mod.rs` + `src/widgets/sparkline.rs` (minimal, unstyled)

**Interfaces:**
- Consumes: Task 1 model, Task 2 ticker.
- Produces: `WidgetCard` component `#[component] pub fn WidgetCard(widget: Widget) -> Element` (renders `.card` with `data-kind`, dispatches body by kind); window fns `mission_control()` / `satellite_window()` with the `DndProvider`/`DragBridge`/`use_window_geometry_feed`/`DragOverlay{ match_source: true }` wiring copied from the workhorse's `Chrome`.

- [ ] Step 1: Wire mission control (dock `DropZone` listing dock widgets as `Draggable{ payload: widget }` around `WidgetCard`) + "Open satellite" button + satellite windows, exactly the workhorse's context-passing shape. Sparkline body: SVG polyline from `state().samples` (unstyled is fine here).
- [ ] Step 2: Build and launch with CDP (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9773`, log tee). Open one satellite.
- [ ] Step 3: **SPIKE:** rig-drive a real SendInput drag of the sparkline from mission control toward the satellite; while HELD over the satellite, sample the ghost's innerHTML twice ~600 ms apart via CDP in the satellite page. Expected: the two samples DIFFER (polyline points moved) → ghost is live. Also confirm drop delivers and the widget keeps animating in the satellite.
- [ ] Step 4: If samples are identical → STOP the plan, report the finding (library-level), do not proceed to polish.
- [ ] Step 5: Commit (`Stand up the showcase two-window spike`), noting the observed evidence in the body.

### Task 4: Full widget set + theme

**Files:**
- Create: `src/widgets/stopwatch.rs`, `src/widgets/ring.rs`, `src/widgets/pulse.rs`, `src/theme.rs`
- Modify: `src/widgets/mod.rs` (dispatch all four), `src/main.rs` (use STYLE)

**Interfaces:**
- Each body: `#[component] pub fn XBody(state: Signal<WidgetState>) -> Element`.
- Stopwatch: `ticks` → `mm:ss.t` monospace. Ring: SVG circle, `stroke-dasharray`/`offset` from `level`, phase label (Build/Test/Ship by level thirds). Pulse: ECG polyline segment scrolled by `ticks`, BPM readout.
- Theme per spec: #0B0E14-family base, frosted cards, hairline borders, per-kind neon accents (cyan/amber/green/red) via `data-kind` attribute selectors, dashed eligible outline on `.zone[data-active]`, accent glow on `[data-over]`, ghost = lifted shadow/glow. Use the frontend-design skill for this task.

- [ ] Step 1: Implement the three bodies + dispatch. Package gates.
- [ ] Step 2: Write `theme.rs` (all CSS in one const, like the workhorse's STYLE), apply classes in components. Launch and eyeball all four widgets animating, hover states, ghost styling in both windows.
- [ ] Step 3: Commit (`Dress the showcase in the mission-control theme`).

### Task 5: Ctrl-clone, N satellites, close-order semantics, D-key layout

**Files:**
- Create: `src/layout.rs`
- Modify: `src/main.rs`, `src/model.rs` (only if `deliver` needs a fix)

**Interfaces:**
- `on_drop` in both window kinds: `owner.deliver(o.payload, o.to, o.effect)` — Copy clones (fresh ticking twin), Move relocates, Link treated as Move (code comment).
- `layout.rs`: `pub fn apply(role: WindowRole, epoch: u32)` — `WindowRole::MissionControl | Satellite(n)`; positions for 1920x1200@1.5x filming layout, clamped to monitor work area; each window watches `model.layout_epoch` in a `use_effect` and snaps itself; `D` keydown on the Chrome root bumps the epoch.
- Satellite close: `use_drop` → `close_satellite` (Task 1) — widgets return to dock.

- [ ] Step 1: Wire deliver/effect, D-key epoch + per-window snap, keyboard focusability of the Chrome root (`tabindex: "0"`, autofocus).
- [ ] Step 2: Live check with the rig + hands-on: Ctrl-drag clones (two stopwatches ticking), satellite close returns widgets, D snaps layout, mission-control-first close leaves satellites animating (ticker failover observable: ≤0.5 s hiccup).
- [ ] Step 3: Package gates. Commit (`Complete the showcase interactions and demo layout`).

### Task 6: Gates, rig smoke scenario, example README

**Files:**
- Create: `examples/desktop-showcase/README.md` (run instructions, D-key, Ctrl-clone, what it demonstrates + spec link)
- Rig (in `%TEMP%\dnd-wtest`, not committed): `showcase` scenario — open satellite, drag sparkline across, assert single hover owner + delivery + ghost-liveness sample, Ctrl-drag stopwatch, assert clone count.

- [ ] Step 1: Full package gates with `--locked` (lockfile already committed): test, clippy `-D warnings`, build `--bins`, rustfmt on all new files, `git diff --check`. Root crate gates untouched (verify `git status` clean outside the example).
- [ ] Step 2: Run the rig `showcase` scenario end to end; log audit (same fatal-signature grep as Slice F) on the captured app log.
- [ ] Step 3: Commit (`Document and smoke the desktop showcase`).

### Task 7: Film scenario + capture attempt

**Files:**
- Rig: `film` scenario (eased glides, storyboard beats, D-layout first).
- Create (only if capture succeeds): `assets/showcase.gif`; Modify: `README.md` hero section.

- [ ] Step 1: Script the storyboard take (sparkline over-and-back with mid-air pauses, Ctrl-stopwatch clone beat) with slow eased `glide` calls.
- [ ] Step 2: Check for a capture tool (`ffmpeg -version`, ScreenToGif). If present: record the take, palettegen GIF < 10 MB, add README hero section, commit (`Land the multi-window showcase hero`). If absent: deliver the scenario + exact capture instructions and report the gap instead of faking it.

## Self-Review

- Spec coverage: storyboard beats → Tasks 3/5/7; widgets/theme → 4; ticker failover → 2; demo layout key → 5; verification spike → 3 (hard gate); non-goals respected (no library changes anywhere; workhorse untouched). Extended-cut mp4 deferred with the capture attempt in Task 7 — acceptable, GIF is primary.
- No placeholder steps: each step names exact behavior, files, and checks; load-bearing signatures pinned in Interfaces blocks.
- Type consistency: `Widget`/`WidgetState`/`ModelOwner` names match across tasks; `deliver(payload, to, effect)` used consistently.

# The renderer contract

- Date: 2026-07-10 (line references at commit `4f36063`)
- Purpose: enumerate every browser/DOM behavior this crate depends on, so a
  non-webview render target (Blitz / dioxus-native today, anything else
  tomorrow) can be assessed dependency-by-dependency instead of by trial
  and error.
- Companion: `BLITZ-TRACKING-ISSUE.md` (draft issue body mapping each
  module to works-as-is / needs-native-impl / N-A-under-Blitz).

**Honesty rule**: "Blitz status" below records only what has been verified
or is structurally certain. Everything else is marked **unknown** - do not
promote an unknown to "works" without running it.

## Summary table

| # | Behavior | Depended on by | Without it | Blitz status |
|---|---|---|---|---|
| 1 | Pointer events (`pointerdown/move/up/cancel`) | the entire pointer path | no pointer drags at all | unknown |
| 2 | Native pointer capture (`setPointerCapture`, `web` feature) | capture-solid mouse drags | falls back to capture substitute; already optional | unknown (web-sys downcast almost certainly N/A) |
| 3 | `MountedData::get_client_rect` | every rect measurement | no hit-testing: drags never find zones | unknown |
| 4 | CSS transitions + `transitionend` | settle glide, FLIP | settle may never complete; FLIP jumps | unknown |
| 5 | `prefers-reduced-motion` media query | all motion | motion plays for users who opted out | unknown |
| 6 | CSS animation + `animationend` (hold clock) | touch hold-to-drag | `Auto` loses long-press pickup only | unknown |
| 7 | `tabindex` / keyboard focus / `keydown` | keyboard drags | no keyboard operation | unknown |
| 8 | `aria-live` region | screen-reader voice | silent drags for SR users | unknown (AccessKit mapping unverified) |
| 9 | `data-*` attributes as styling contract | all visual states | unstyled states if attr selectors don't match | unknown, structurally likely (Stylo) |
| 10 | `onvisible` / IntersectionObserver | virtualized-list recipe | recipe dead; core unaffected | unknown |
| 11 | HTML5 `DataTransfer` / `DragEvent` | native boundary modules | file drops, drag-out, typed transport dead | structurally N/A until Blitz has an OS-DnD story |
| 12 | `touch-action` CSS | touch scroll-vs-drag arbitration | page pans fight drags on touch | unknown |
| 13 | `position: fixed` + `transform` | ghost, capture substitute | ghost mispositioned / substitute dead | unknown |
| 14 | HTML file input + `document::eval` | `FileDropZone` click-to-choose | picker path dead; OS drops still work if #11 does | structurally N/A without a native file-dialog bridge |

## 1. Pointer events and their fields

The whole pointer path is driven by `pointerdown/move/up/cancel` with
`pointer_id`, `pointerType`, client coordinates, and button state.

- `Draggable`: `src/core/components/draggable.rs:400,480,538,554`
- Sortable rows: `src/sortable.rs:531,560,572,661`; grid tiles:
  `src/grid.rs:213`
- `PointerKind::from_pointer_type` (bridging policy rides `pointerType`):
  `src/core/types.rs` (`from_pointer_type`)

Without it: nothing drags by pointer. Keyboard drags (7) would be the only
input. This is the load-bearing dependency; everything else degrades,
this one doesn't.

Blitz status: **unknown** (mouse events presumably exist; whether they are
delivered as pointer events with `pointer_id`/`pointerType`, and whether
`pointercancel` ever fires, is unverified).

## 2. Native pointer capture (`web` feature only)

`src/core/platform.rs:33` (`set_pointer_capture`) and `:77` (release) via a
`MountedData::downcast::<web_sys::Element>()`. Already optional by design:
without the `web` feature the same components render a full-viewport
"capture substitute" while a drag is in flight
(`src/core/components/draggable.rs:762`, `src/sortable.rs:606`,
`src/grid.rs:266`) - which is exactly how dioxus-desktop runs today.

Without it: nothing new breaks that the substitute doesn't already cover;
mouse releases outside the app window reconcile by held-button state
instead of committing.

Blitz status: the `web_sys::Element` downcast is **structurally N/A**
(no DOM element behind `MountedData`); the substitute path's own
dependencies are (1) and (13).

## 3. Rect measurement: `MountedData::get_client_rect`

The registry measures every zone, draggable, overlay and scroll container
through `onmounted` + `get_client_rect`:

- Zones: `src/core/components/drop_zone.rs:149,247,367`
- Draggables (grab offset, ghost size):
  `src/core/components/draggable.rs:231,449`
- Overlay settle target: `src/animate.rs:109`
- Auto-scroll containers: `src/autoscroll.rs:212`
- Canvas: `src/canvas.rs:247`

Without it: rects stay empty, hit-testing finds nothing, drags pick up and
never drop. (This is a documented failure mode already: dioxus-web without
the `mounted` feature behaves exactly this way - see `Cargo.toml`'s
dev-dependency comment.)

Blitz status: **unknown**. dioxus-native advertises some `MountedData`
support; whether `get_client_rect` returns real layout rects is
unverified.

## 4. CSS transitions + `transitionend`

The settle glide is an inline `transform` transition whose completion
handshake is `ontransitionend` (`src/core/components/overlay.rs:365,390`);
`FlipItem` reorder glides are inline transitions too (`src/animate.rs:41,67,76`).
The near-zero-duration trick for reduced motion exists precisely because
`transitionend` still fires for a 0.01ms transition (`src/a11y.rs:131`).

Without it: `FlipItem` degrades to a jump (visual only), but a settle that
starts and never hears `transitionend` **wedges the settle state** - the
overlay component has explicit unmount recovery
(`src/core/components/overlay.rs:144`) but no wall-clock timeout. This is
the one dependency whose absence corrupts behavior rather than degrading
it; if the target renderer lacks `transitionend`, settle must be disabled
or given a core-side timeout.

Blitz status: **unknown** (Stylo parses transitions; whether Blitz runs
them and synthesizes `transitionend` is unverified).

## 5. `prefers-reduced-motion`

Honored as an injected `@media` rule, not a JS query:
`src/a11y.rs:133` (`REDUCED_MOTION_CSS`), consumed at `src/animate.rs:143`,
`src/core/components/overlay.rs:175`, `src/sortable.rs:488`.

Without it: motion plays for users who asked the OS for none - an
accessibility regression, not a functional one.

Blitz status: **unknown**.

## 6. CSS animation + `animationend`: the touch hold clock

`TouchSense::Auto`'s long-press pickup is timed by a zero-size element
running a no-op CSS animation; `animationend` is the alarm
(`src/core/components/pointer.rs:34,47,53`). Deliberately renderer-honest
already: the component's docs state that where CSS animations don't run,
`Auto` quietly loses only its long-press path - sideways pulls still drag,
`TouchSense::Immediate` is untouched.

Blitz status: **unknown**. (If Blitz gains nothing here, a core-side
fallback timer is a plausible small fix; the element-lifecycle-as-timer
design was chosen for webviews, not against timers.)

## 7. `tabindex`, keyboard focus, `keydown`

Keyboard drags ride focusable sources: `src/core/components/draggable.rs:599`
(`tabindex`), `:602` (`onkeydown`); sortable rows carry the same contract
(`src/core/components/draggable.rs:428` comment). The a11y reorder buttons
(`src/a11y.rs:101,114`) are plain clicks and carry no extra dependency.

Without it: no keyboard operation - which is the crate's compliance story,
so a renderer without focus/keydown is an accessibility non-starter, not a
degraded mode.

Blitz status: **unknown** (some focus work exists for form controls;
`tabindex` on arbitrary elements + `keydown` routing unverified).

## 8. `aria-live` announcements

`src/a11y.rs:27-42`: a visually-hidden `aria-live="polite"` / `role="status"`
region voices pickup/move/drop. Depends on the renderer mapping DOM-ish
attributes to a platform accessibility tree.

Without it: drags work but are silent to screen readers.

Blitz status: **unknown** (Blitz integrates AccessKit; whether live-region
semantics are mapped is unverified).

## 9. `data-*` attributes as the styling contract

All visual state is presence-based data attributes, styled by the consumer:
`data-over`/`data-active` (`src/core/components/drop_zone.rs:136-137`,
`src/board.rs:264-265`, `src/canvas.rs:241`), `data-dragging`/`data-disabled`
(`src/core/components/draggable.rs:386-387`), `data-reorder`
(`src/a11y.rs:101,114`), `data-dnd-motion` (`src/animate.rs:150`).

Without attribute-selector styling: functional behavior is intact but every
state is invisible. Setting the attribute itself is plain VDOM; the risk
is only on the CSS side.

Blitz status: **unknown, structurally likely to work** - attribute
selectors are core Stylo. Left unverified per the honesty rule.

## 10. `onvisible` / IntersectionObserver (virtualized lists)

Core has no dependency; the *documented recipe* for virtualized lists
(README "Virtualized lists", `README.md:914`; gallery Archive page,
`examples/gallery/pages/archive.rs:98,211`) drives windowing from
`onvisible` because dioxus-web 0.7 delivers no element-level scroll events
(`src/autoscroll.rs:10-17` module docs). Auto-scroll itself observes
scrolling through the events that cause it, not through `scroll` events.

Without it: the virtualization recipe is dead; plain long lists and
auto-scroll (which scrolls via `MountedData::scroll`,
`src/autoscroll.rs:218`) are unaffected to the extent (3) holds.

Blitz status: **unknown**.

## 11. HTML5 `DataTransfer` / `DragEvent` (the native boundary)

Everything that crosses the app boundary rides the native drag protocol:

- OS file drops: `src/files.rs:264-288` (`ondragover/enter/leave/drop`)
- Drag-out to other apps: `src/dragout.rs:141,195` (`ondragstart` +
  outbound `DataTransfer`)
- Typed/plain interop transport: `src/external.rs:72,115-118` and
  `external::typed` (`serde` feature)

Without it: these three modules are inert. In-app drags (the entire core)
never touch `DataTransfer` by design - the 2.0 "one input model" split
exists precisely so the pointer path has zero native-DnD dependency.

Blitz status: **structurally N/A** until Blitz/winit exposes an OS
drag-and-drop story; nothing this crate can shim.

## 12. `touch-action`

Scroll-vs-drag arbitration on touch is CSS-declared:
`touch-action: pan-y pinch-zoom` for `TouchSense::Auto`
(`src/core/components/pointer.rs:22`), `none` for `Immediate` (`:25`), and
`none` on every capture-substitute shield
(`src/core/components/draggable.rs:762`, `src/sortable.rs:606`,
`src/grid.rs:266`).

Without it: on a touch-capable renderer, native panning and drag gestures
fight (the browser starts a pan mid-drag,
`src/core/components/draggable.rs:578` comment). On a renderer with no
native touch panning there is nothing to arbitrate and the dependency
vanishes.

Blitz status: **unknown** (depends entirely on whether Blitz implements
its own touch scrolling).

## 13. `position: fixed` + `transform` layout semantics

The drag ghost is a `position: fixed` element following the pointer
(`src/core/components/overlay.rs:22`), the settle glide is pure `transform`
on top of a stationary layout rect (`overlay.rs:200,360-365`), and the
capture substitute is a fixed full-viewport shield (see 2).

Without correct fixed-positioning/transform: the ghost detaches from the
pointer or the substitute fails to cover the viewport - both visually
fatal for pointer drags.

Blitz status: **unknown, structurally likely** (core CSS layout), left
unverified per the honesty rule.

## 14. HTML file input + `document::eval`

`FileDropZone` keeps its headless wrapper and renders a hidden
`input[type=file][multiple]` (`src/files.rs`). Its wrapper click handler uses
Dioxus's document evaluator to clear and click that input; `onchange` then
reads `FormEvent::files()` and enters the same filtering/callback path as a
drop. No CSS or layout behavior is part of this dependency.

Without it: clicking the zone cannot open a picker. The OS-drop path remains
independent and continues to work to the extent (11) holds; `FileFilter`
itself is pure Rust.

Blitz status: **structurally N/A in the current implementation**. A native
renderer needs a file-dialog API that can return Dioxus `FileData`; it has no
browser file input or JavaScript document to evaluate.

## What core explicitly does NOT depend on

Worth stating so ports don't defend against ghosts: no `document.*` /
`window.*` globals, no JS `eval`, no timers (`setTimeout`) on the drag
path, no element-level `scroll` events, no native HTML5 DnD for in-app
drags, and - outside the optional `web` feature - no `web-sys` at all.
The gesture machine (`src/core/machine.rs`), modifiers
(`src/core/modifiers.rs`), registry, world, and `DragSim` test harness are
pure Rust and renderer-independent by construction.

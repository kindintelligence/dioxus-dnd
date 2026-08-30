# Auto-scroll API reference

Edge-scrolling for drags, the missing piece for long lists and tall
boards: `AutoScroll` wraps any scrollable container and scrolls it while a
drag hovers within `threshold` px of an edge, pings the tree's
rect-refresh channel after every scroll so hit-testing tracks the
movement, and reports the offset through `on_scroll`. One continuous clock
drives pixels-per-second velocity even while the pointer is stationary.
`ScrollAxis` selects the axes. Nested coordination is automatic, and
`edge_delta` is the pure velocity math underneath.

Concept guide: [docs/concepts/autoscroll.md](../concepts/autoscroll.md).

```rust,ignore
AutoScroll {
    style: "height: 300px; overflow-y: auto;",
    for item in long_list {
        Row { item }
    }
}
```

Scrolling and measuring go through Dioxus's `MountedData`, with no
JavaScript eval, so the same code works in web and desktop webviews.
`dragover` (native boundary drags) and active `pointermove` events (in-app
pointer drags via `Draggable` and the sortable components) feed pointer
positions. The latest active sample starts a 16ms clock; while the pointer
sits within `threshold` px of an edge, the container keeps moving at up to
the resolved CSS-pixels-per-second velocity, scaled by proximity and elapsed
time.

## `AutoScroll`

A scrollable container that scrolls itself while a drag hovers near its
edges. Renders a wrapper `div` and forwards arbitrary attributes (`class`,
`style`, `id`, ...) to it. Mounted, wheel, drag, and pointer listeners are
component-owned and cannot be replaced through that forwarded list. Give it
the `overflow` CSS yourself, and
consider `overscroll-behavior: contain` alongside it, so a wheel or touch
scroll that hits the container's end mid-drag doesn't chain into scrolling
the page. (The edge-scrolling itself is programmatic, clamps at the
container's bounds, and never chains.)

| Prop | Type | Default | What it does |
|---|---|---|---|
| `threshold` | `f64` | `48.0` | Edge band size in px. A drag hovering within this distance of an edge scrolls the container. |
| `speed` | `f64` | `24.0` | 3.x compatibility speed in nominal pixels per 60 Hz frame. Used only when `speed_px_per_second` is absent. |
| `speed_px_per_second` | `Option<f64>` | `None` | Exact maximum velocity in CSS px per second. Takes precedence over `speed`; recommended for new code. |
| `axis` | `ScrollAxis` | `Y` | Axes to scroll: `Y` for lists, `X` for strips, `Both` for 2D panes. |
| `active` | `Option<bool>` | `None` | External drag-state gate for the pointer path. `Some(true)` scrolls on any pointer movement, `Some(false)` suppresses it, `None` uses the built-in contact heuristic. |
| `drag_pointer` | `Option<Point>` | `None` | Pointer supplied by a host that tracks movement outside this element's DOM event stream, in this window's client coordinates. Read only while `active` is `Some(true)`; pass the matching drag's live active state so a retained coordinate cannot scroll idle or settling content. |
| `on_scroll` | `Option<EventHandler<Point>>` | `None` | Fired with the container's scroll offset when a sample sees it changed - after the component's own scrolling, a wheel or trackpad scroll, or pointer movement over the container - following the rect-refresh ping. Drive a windowed (virtualized) list from `offset.y`. |

Behavior notes:

- One continuous clock with several pointer feeds. Native boundary drags
  start it through `dragover`; the handler never calls `prevent_default`,
  so drop permission stays the business of the zones inside. In-app drags
  start it through `pointermove`, gated by contact:
  mouse drags report held buttons, touch and pen commonly report pressure
  during contact (and some platforms expose held buttons for them too).
  `active` overrides the heuristic in both directions.
- The pointer must be inside the container to scroll it, edges inclusive.
  Under pointer capture the container keeps receiving bubbled
  `pointermove` events with the cursor far outside; without this gate the
  delta would pin to full velocity and the container would scroll forever.
- Scrolls with `ScrollBehavior::Instant` through the mounted handle, one
  async scroll in flight at a time. Movement is velocity multiplied by
  elapsed time, with elapsed time capped at 100ms so a suspended tab cannot
  produce a giant catch-up jump.
- Nested ownership. The outermost `AutoScroll` creates a coordinator and
  nested instances inherit it. The smallest containing surface moves first.
  If it reaches a boundary, it marks itself blocked and the next containing
  surface receives subsequent movement until a fresh pointer sample makes
  the inner surface eligible again.
- Rect refresh. Scrolling this container moves everything inside it, so
  cached hit-test rects go stale the moment it scrolls. The component
  create-or-inherits the tree's rect-refresh channel: with a `DndProvider`
  above, it joins that provider's channel; without one (self-contained
  sortables, plain pages) it anchors a channel itself so the components
  inside can register. After every scroll it performs or observes, it
  calls `refresh_all()`: zone registries re-measure and sortables
  re-anchor their cached slots. Participants without a drag in flight
  ignore the ping, so it is free from high-frequency sources.
- The initial offset is reported at mount (restored scroll positions
  exist), so windowing starts aligned.
- A host-driven receiver may be event-blind while another surface owns the
  pointer; `drag_pointer` routes its client-space feed through the same
  scroll path as DOM pointer movement, behind the explicit
  `active: Some(true)` gate. Multi-window desktop drags feed it the shared
  pointer converted into this window's client coordinates.

## Scroll observation

Scroll observation (the rect-refresh ping and the `on_scroll` prop) rides
the events that cause or accompany scrolling - wheel, pointer contact
moves, and the auto-scrolls this component performs - each of which
samples the offset through `MountedData` and reports when it changed. It
has to work this way: dioxus-web 0.7 never delivers element-level `scroll`
events to `onscroll` handlers, and its eval channel drops messages that
resolve after the receiver parked, so neither a Rust `onscroll` nor a JS
listener bridge can carry the signal.

- Wheel events reach the element under the cursor regardless of pointer
  capture, and the sample's async offset read resolves after the browser
  applied the scroll the event causes, so wheel and trackpad scrolling
  report correctly whether idle or mid-drag.
- `pointermove` samples on every move, contact or hover, so the window
  trues up after scrollbar drags and programmatic scrolls the moment the
  pointer stirs.
- The known blind spot is a scroll no event accompanies (a programmatic
  scroll-to-index with the pointer at rest): the code that initiates one
  should update its own state, and the next pointer or wheel activity
  trues everything up. The gallery's archive page covers the gap with
  `onvisible` sentinels on its rows.

## `ScrollAxis`

Which axes to auto-scroll.

| Variant | Meaning |
|---|---|
| `Y` | Vertical only, the default (lists). |
| `X` | Horizontal only (strips). |
| `Both` | Both axes (2D panes). |

## `edge_delta`

```rust,ignore
pub fn edge_delta(pos: Point, rect: Rect, threshold: f64, speed: f64, axis: ScrollAxis) -> (f64, f64)
```

The pure per-axis velocity math behind `AutoScroll`, public for unit-testing
ramps or driving a custom scroller with the same behavior. Returns `(vx,
vy)`, each in `-speed..=speed`, for a pointer at `pos` inside `rect`:

- Outside `rect` (edges inclusive), the delta is `(0.0, 0.0)`.
- On each allowed axis the pointer scrolls toward whichever edge is
  nearer, so a container narrower than `2 * threshold`, where the pointer
  sits inside both bands at once, still scrolls both ways instead of one
  edge always winning.
- The magnitude ramps linearly with depth into the band:
  `(depth / threshold).clamp(0.0, 1.0) * speed`, with the divisor floored
  at 1 so a zero threshold cannot divide by zero.

`frame_delta(velocity, elapsed_seconds)` converts that velocity into one
clock tick's movement and clamps elapsed time to 100ms.

## Where the rest lives

`use_rect_refresh()` and the `RectRefresh` channel (`refresh_all` and
registration): [docs/api/core.md](core.md), which also covers `Point` and
`Rect`. The sortable components that re-anchor on the ping:
[docs/api/sortable-lists.md](sortable-lists.md). The multi-window pointer
feed behind `drag_pointer`:
[docs/api/multi-window.md](multi-window.md).

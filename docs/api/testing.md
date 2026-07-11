# Testing API reference

Headless test driver - drag-and-drop in CI, no browser: `DragSimProbe`
captures a provider's drag world and `DragSim` drives whole pointer
interactions through the production delivery path inside a `VirtualDom`.

Concept guide: [docs/concepts/testing.md](../concepts/testing.md).

The drag state machine is plain Rust over signals, so a whole pointer
interaction can run headless: pick up, hover, drop, assert. The one thing a
headless run lacks is layout, so *you place the zone rects* - which makes
tests deterministic instead of flaky. Mount a `DragSimProbe` inside the
provider under test, grab the `DragSim` it captured, and drive:

```rust,ignore
use dioxus_dnd::test::{drag_sim, rerender, DragSimProbe};

fn test_app() -> Element {
    rsx! {
        DndProvider::<Card> {
            DragSimProbe::<Card> {}
            ShelfApp {}   // the component you're testing
        }
    }
}

let mut dom = VirtualDom::new(test_app);
dom.rebuild_in_place();
let mut sim = drag_sim::<Card>();

sim.place(&dom, SHELF, Rect::new(0.0, 100.0, 200.0, 80.0));
sim.pick_up(&dom, card.clone());
sim.move_to(&dom, Point::new(100.0, 140.0));
assert_eq!(sim.over(&dom), Some(SHELF));
rerender(&mut dom);
assert!(dioxus_ssr::render(&dom).contains("data-over"));
assert_eq!(sim.release(&dom), Some(SHELF));   // your on_drop just ran
```

Or as one line for the common arc: `simulate_drag`.

## `DragSimProbe`

Captures a `DragSim<T>` for the enclosing provider. Mount one inside the
`DndProvider<T>` of your *test* app; it renders nothing. Retrieve the
handle with `drag_sim` after `rebuild_in_place`.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `phantom` | `PhantomData<T>` | default | Internal type marker; never set this. |

Captured handles live in a thread-local slot keyed by payload type: one
slot per type per thread, and the most recently mounted probe wins. That is
exactly right for one `VirtualDom` per test; with two providers of the same
`T` (multi-window tests), mount the probe in the dom you intend to drive,
or mount it last.

## `drag_sim`

```rust,ignore
pub fn drag_sim<T: Clone + PartialEq + 'static>() -> DragSim<T>
```

Returns the handle the most recent `DragSimProbe<T>` captured. Panics when
no probe for `T` has mounted - add `DragSimProbe::<T> {}` inside the
provider and `rebuild_in_place` first.

## `DragSim`

Headless driver for one provider's drag world. `Copy` and `Clone`; the
gesture methods take `&mut self`, so bind it `let mut sim`. Every method
takes the `VirtualDom` so the underlying signal operations run inside its
runtime; call `rerender` between actions and markup assertions.

Placing geometry:

| Method | Returns | What it does |
|---|---|---|
| `place(&dom, zone, rect)` | `()` | Gives a zone its client `Rect` - the headless stand-in for layout. Panics when no zone with this id is registered. |
| `place_in(&dom, window, zone, rect)` | `()` | `place` for a zone living in another joined window's registry; `rect` is in **that window's** client px. Panics when this sim's provider joined no world, the window is unknown, or the zone is not registered there. |

Driving the gesture:

| Method | Returns | What it does |
|---|---|---|
| `pick_up(&dom, payload)` | `()` | Begins a pointer drag carrying `payload`, from no particular zone. |
| `pick_up_from(&dom, payload, from)` | `()` | Begins a pointer drag reporting `from: Option<ZoneId>` as the source zone; it arrives in `DropOutcome::from`. |
| `move_to(&dom, point)` | `()` | Moves the pointer: updates the tracked position and enters or leaves zones by hit-testing the placed rects, the same logic the pointer gesture runs per `pointermove`. |
| `release(&dom)` | `Option<ZoneId>` | Releases at the current pointer position with the `Move` effect. Returns the zone that received the drop, or `None` when the drag cancelled. |
| `release_as(&dom, effect)` | `Option<ZoneId>` | `release` with an explicit `DropEffect` - simulate the Ctrl-held copy drop with `DropEffect::Copy`. |
| `cancel(&dom)` | `()` | Aborts the drag, as Escape or a pointer cancel would. |

A sim drag starts at the coordinate origin with a zero grab offset and a
`Move` base effect; `move_to` sets every position after that.

Observing state:

| Method | Returns | What it does |
|---|---|---|
| `over(&dom)` | `Option<ZoneId>` | The zone currently hovered. |
| `dragging(&dom)` | `bool` | Is a drag in flight? |
| `payload(&dom)` | `Option<T>` | The in-flight payload, if any. |
| `announcement(&dom)` | `String` | The latest screen-reader announcement. |
| `completions(&dom)` | `Vec<bool>` | Exactly-once source completion results observed by the simulated source: `true` per delivered drop, `false` per cancel. What a `Draggable`'s `on_drag_end` would see. |
| `window_key()` | `Option<WindowKey>` | The key this sim's provider joined its world under, when it did. |

### Release semantics

Drops go through the *production* delivery path - acceptance filters,
`DropOutcome` construction, closest-edge enrichment, settle routing -
shared with `Draggable` itself, not a reimplementation. Releases mirror
the pointer gesture: an exact hit wins; otherwise the drop snaps to the
closest acceptable zone whose edge is within 48px (the touch forgiveness),
else the drag cancels and `release` returns `None`.

Not simulated: pointer capture, auto-scroll, and the re-measure that
precedes the real snap (headless rects are wherever you placed them). The
crate covers those browser behaviors with its Playwright suite; see the
concept guide.

### Cross-window simulation

When the sim's provider joined a `DndWorld` (see
[docs/api/multi-window.md](multi-window.md)), the sim is world-aware and
moves and releases resolve across windows, like the gesture:

- `pick_up` anchors the world drag to this sim's window.
- `move_to` resolves through the world first: a zone hit in any joined
  window is authoritative, a point inside a foreign window but outside its
  zones clears the hover, and an unresolved point falls back to the local
  registry.
- A `release` the world resolves into a foreign window delivers there,
  with the 48px snap running in the target window's own CSS px.

Build one `VirtualDom` per simulated window, share the world via
`with_root_context`, feed each window's geometry by hand, and place
foreign zones with `place_in`. `tests/multiwindow.rs` shows the full
pattern, including a `rerender` per dom before markup assertions.

## `rerender`

```rust,ignore
pub fn rerender(dom: &mut VirtualDom)
```

Flushes pending reactivity so the tree reflects the simulated state - call
between driver actions and markup assertions (`dioxus_ssr::render`).

## `simulate_drag`

```rust,ignore
pub fn simulate_drag<T: Clone + PartialEq + 'static>(
    dom: &mut VirtualDom,
    payload: T,
    from: Option<ZoneId>,
    path: &[Point],
) -> Option<ZoneId>
```

One whole pointer drag: picks `payload` up (from `from`), glides through
`path`, releases at its last point, re-rendering between steps so zone
reactions run just as they would live. Returns the receiving zone, or
`None` when the drag cancelled. Needs a mounted `DragSimProbe<T>`; an
empty `path` releases at the pickup point.

```rust,ignore
let landed = simulate_drag(&mut dom, card, Some(READING), &[Point::new(100.0, 140.0)]);
assert_eq!(landed, Some(FINISHED));
```

## Where the rest lives

`Point`, `Rect`, `ZoneId`, `WindowKey` and the registry:
[docs/api/core.md](core.md). `DropEffect` and the modifier convention:
[docs/api/drop-effects.md](drop-effects.md). `DndWorld`, window geometry
and joining: [docs/api/multi-window.md](multi-window.md). The delivered
`DropOutcome`: [docs/api/drag-and-drop.md](drag-and-drop.md).

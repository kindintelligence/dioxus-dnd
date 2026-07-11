# Testing

Drag-and-drop logic usually hides behind a browser: real pointers, real
layout, flaky end-to-end suites. Here the drag state machine is plain Rust
over signals, so a whole pointer interaction - pick up, hover, drop - runs
inside a `VirtualDom` and asserts in microseconds. In CI, no browser.

API reference: [api/testing.md](../api/testing.md). Working examples: the
crate's own suites, `tests/runtime.rs` (single window) and
`tests/multiwindow.rs` (cross-window drags), drive the same simulator.

## The mental model

Two pieces from `dioxus_dnd::test`:

- `DragSimProbe<T>` mounts inside the `DndProvider<T>` of your test app.
  It renders nothing and captures a handle to that provider's drag world.
- `DragSim<T>`, retrieved with `drag_sim::<T>()`, drives that world the way
  the pointer gesture does: the same context your zones read, the same
  delivery code your `on_drop` handlers hang off.

The one thing a headless run lacks is layout: nothing measures the zones.
So you place the zone rects yourself with `sim.place(...)`, the headless
stand-in for layout. That inversion is the point: geometry is exactly what
the test says, every run, so tests are deterministic instead of flaky.

## A worked example

Wrap the component under test in a provider with a probe:

```rust,ignore
use dioxus_dnd::prelude::*;
use dioxus_dnd::test::{drag_sim, rerender, DragSimProbe};

fn test_app() -> Element {
    rsx! {
        DndProvider::<Card> {
            DragSimProbe::<Card> {}
            Shelves {}   // the component under test
        }
    }
}
```

Then drive a full arc and assert at every stage:

```rust,ignore
let mut dom = VirtualDom::new(test_app);
dom.rebuild_in_place();
let mut sim = drag_sim::<Card>();

// Headless layout: the test owns the geometry.
sim.place(&dom, READING, Rect::new(0.0, 0.0, 200.0, 80.0));
sim.place(&dom, FINISHED, Rect::new(0.0, 100.0, 200.0, 80.0));

sim.pick_up_from(&dom, card.clone(), Some(READING));
sim.move_to(&dom, Point::new(100.0, 140.0));
assert_eq!(sim.over(&dom), Some(FINISHED));

rerender(&mut dom);
assert!(dioxus_ssr::render(&dom).contains("data-over"));

assert_eq!(sim.release(&dom), Some(FINISHED));  // your on_drop just ran
// ...assert your model moved the card.
```

`release` returns the zone that received the drop, or `None` when the drag
cancelled, so the assertion and the state change land in one line.

## Drops run the production path

`release` is not a shortcut around your logic. It ends in the same delivery
code as `Draggable` itself: acceptance filters run, the release mirrors the
pointer gesture's forgiveness (the last acceptable exact hit in registry
order wins, rejecting overlaps are skipped, otherwise the drop snaps to the
closest acceptable zone whose edge is within 48px, else the drag cancels),
the receiving zone's closest-edge enrichment fills
`DropOutcome::edge`, and the full `DropOutcome` - coordinates, effect,
`from` and `to` - is constructed and handed to `on_drop`. A test that
asserts on the outcome is exercising production behavior, not a mock.

Two consequences worth testing on purpose:

- A zone whose `accepts` rejects the payload does not take the drop; the
  release snaps past it or cancels, exactly as a finger would experience.
- A release in the gutter between zones, within 48px of an acceptable
  edge, still lands. Place your rects with a gap and assert the snap.

Modifier behavior is one call away: `release_as(&dom, DropEffect::Copy)`
simulates the Ctrl-held copy drop, and the outcome's `effect` field carries
it, so the copy-vs-move branch in your handler is testable without a
keyboard.

## The one-line arc

When the test cares about the destination, not the journey, `simulate_drag`
runs the whole arc - pick up, glide through a path, release at its last
point - re-rendering between steps so zone reactions run as they would
live:

```rust,ignore
let landed = simulate_drag(&mut dom, card, Some(READING), &[Point::new(100.0, 140.0)]);
assert_eq!(landed, Some(FINISHED));
```

## Asserting on rendered markup

The styling contract is data attributes (`data-active`, `data-over`,
`data-edge`, `data-dragging`), and SSR output shows them. Call
`rerender(&mut dom)` to flush pending reactivity, then render with
`dioxus_ssr::render(&dom)` and assert on the string. This is how you prove
zones light up on pickup, the hovered zone highlights, and everything
unlights after the drop - the visual states users style, verified without
a screen. `dioxus-ssr` is a dev-dependency: `dioxus-ssr = "0.7"`.

The sim also exposes the live state directly: `over`, `dragging`,
`payload`, `announcement` (the latest screen-reader string), and
`completions` (what the source's `on_drag_end` observed, `true` per
delivered drop, `false` per cancel).

## Cross-window arcs

`DragSim` is world-aware. When the provider under test joined a `DndWorld`
(see [Multi-window desktop drags](multi-window.md)), the sim resolves moves
and releases across every joined window, like the real gesture: build one
`VirtualDom` per simulated window, share the world through
`with_root_context`, feed each window's geometry by hand, and place foreign
zones with `sim.place_in(&dom, window_key, zone, rect)` in that window's
own client px. A `move_to` past a window edge lights the other window's
zone, and a release there delivers with client coordinates in the target
window's space. `tests/multiwindow.rs` is the full pattern.

## What still needs a browser

The simulator covers state, delivery and markup. It does not simulate
pointer capture, auto-scroll, or the re-measure that precedes the real
near-miss snap (headless rects sit wherever you placed them). Those are
browser behaviors, so the crate covers them the honest way: a Playwright
suite (`tests/browser/`) drives real headless-browser fixtures from
`examples/regressions.rs` - overlay geometry, releases outside a list
committing no reorder, autoscroll edges, drop fall-through past rejecting
zones, real CDP touch gestures, and more. `cargo test` runs the Rust
layer; `npm install && npm run test:web` runs the browser layer. Your own
app usually needs only the Rust layer, because the browser-dependent parts
are the crate's job to keep working.

## Gotchas

- **Last probe wins.** Probes register per payload type per thread, and the
  most recently mounted one is what `drag_sim` returns. With two providers
  of the same `T` (multi-window tests), mount the probe in the window you
  drive, or mount it last.
- **`rerender` before markup assertions.** Sim actions write signals; the
  tree is stale until you flush. State queries (`over`, `dragging`) are
  live either way.
- **Place after `rebuild_in_place`.** `place` panics when the zone is not
  registered yet. A zone that mounts mid-test (a virtualized row) needs its
  own `place` before it can be hit.
- **No re-measure.** The live gesture re-measures zones before the snap;
  headless rects are wherever you put them. If your test's layout "moves",
  call `place` again.
- **Bind `let mut sim`.** The gesture methods take `&mut self`. `DragSim`
  is `Copy`, so grabbing a fresh handle later is also fine.

## Related

- [Drag and drop](drag-and-drop.md): the components and data attributes
  these tests exercise.
- [Multi-window desktop drags](multi-window.md): the world the cross-window
  sim drives.
- [Accessibility](accessibility.md): the announcement strings
  `sim.announcement` lets you assert.
- [Debugging](debugging.md): when a test fails and you need to see the
  registry the sim saw.

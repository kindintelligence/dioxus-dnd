# Auto-scroll

A drag cannot reach content that is scrolled out of view, and mid-drag the
user has no free hand for the scrollbar. `AutoScroll` closes the loop: wrap
the scrollable container, and a drag hovering near its edge scrolls it, at
a speed ramped by how close to the edge the pointer sits.

API reference: [api/autoscroll.md](../api/autoscroll.md).
Live demos: the
[Podcast queue](https://kindintelligence.github.io/dioxus-dnd/podcast-queue)
page is the plainest; the
[Archive](https://kindintelligence.github.io/dioxus-dnd/archive) page runs
it against a 10,000-row virtualized list.

## The mental model

`AutoScroll` renders a `div` and forwards arbitrary attributes (`class`,
`style`, ...) to it. You give it the `overflow` CSS; it gives the div
behavior. Two invisible bands, `threshold` px deep (default 48), line the
container's edges on the scrolling axis (`axis`, default `ScrollAxis::Y`).
While a drag's pointer sits inside a band, each arriving event scrolls the
container toward that edge by up to `speed` px (default 24), scaled
linearly by depth: just inside the band creeps, at the very edge moves at
full speed.

Scrolling is event-driven, not timer-driven. Native boundary drags (an OS
file inbound, text dragged from another app) feed `dragover`, which
browsers fire continuously while hovering, so they scroll steadily. In-app
pointer drags feed `pointermove`, so they advance as the pointer moves. A
contact heuristic gates the pointer path - held buttons for mouse,
pressure for touch and pen - so passively mousing around never scrolls
anything. Keyboard drags carry no pointer and never auto-scroll: scroll
first, then pick up.

Everything runs through Dioxus `MountedData` measurements, no JavaScript
eval, so the same code works in web and desktop webviews.

## A worked example

The gallery's podcast queue: a sortable list taller than its window.

```rust,ignore
AutoScroll {
    class: "max-h-52 overflow-y-auto rounded-xl",
    SortableList {
        len: rows.read().len(),
        touch_handle: true,
        on_sort: move |ev: SortEvent| apply_sort(&mut rows.write(), ev),
        render: move |ix: usize| rsx! { "{rows.read()[ix]}" },
    }
}
```

Drag a row toward the top or bottom edge and the list scrolls under it;
release, and the drop lands on the slot the user sees. Note there is no
`DndProvider` here: the sortable is self-contained, and `AutoScroll`
anchors the shared rect-refresh channel for it (next section).

## Scrolling invalidates every rect

Hit-testing runs on measured rects: zones measure at mount and at pickup,
sortables cache their row slots. Scrolling the container moves all of that
content without telling anyone, so the cached geometry goes stale the
moment the container scrolls, and a drop would land where things sat at
pickup rather than where the user sees them.

So after every scroll it performs or observes (its own edge-scrolling, a
wheel or trackpad scroll mid-drag), `AutoScroll` pings the tree's
rect-refresh channel: every `DropZone` registry re-measures its zones, and
`SortableList` / `SortableGrid` re-anchor their cached slots against the
wrapper's movement. Hover highlighting and the eventual drop then track
what is actually on screen.

The ping is free while idle. Each participant gates on its own drag state,
so a provider or sortable without a drag in flight ignores it, which is
what makes the channel safe to ping from high-frequency sources.

Channel ownership is create-or-inherit: under a `DndProvider` the
`AutoScroll` joins the provider's channel, and standalone it anchors one
itself so the components inside can register. If you move layout under a
live drag some other way - a custom scroll surface, a collapsing panel -
grab the channel with `use_rect_refresh()` and call `refresh_all()` from
your own event.

## Watching the offset

`on_scroll` fires with the container's scroll offset whenever a sample
sees it changed, always after the rect-refresh ping. The archive demo
drives a 10,000-row windowed list from it:

```rust,ignore
AutoScroll {
    style: "height: 420px; overflow-y: auto;",
    on_scroll: move |offset: Point| scroll_top.set(offset.y),
    // full-height canvas, translated window of ~40 DropZone rows
}
```

Observation rides the events that cause or accompany scrolling - wheel,
pointer contact moves, the auto-scrolls the component performs - because
dioxus-web 0.7 delivers no element-level scroll events (the API reference
records the platform detail). One blind spot follows: a scroll that no
event accompanies, such as a programmatic scroll-to-index with the pointer
at rest, goes unreported until the next wheel or pointer activity trues
everything up. Code that initiates one should update its own state. The
archive page covers the gap with `onvisible` sentinels on its rows.

## Gating it from outside

The contact heuristic is right for the built-in components. When a parent
tracks drag state itself, `active` overrides it:

- `active: Some(false)` suppresses pointer-driven scrolling entirely, for
  example while a drop-settle animation runs.
- `active: Some(true)` scrolls on any pointer movement, held buttons or
  not. Pass it only while your drag really is in flight.
- With `active: Some(true)`, `drag_pointer` feeds positions from outside
  the DOM event stream. Multi-window desktop drags use this: a window that
  is not the drag's origin receives no DOM events mid-drag, so the world
  feeds it the shared pointer in that window's client coordinates.

## Gotchas

- **The overflow CSS is yours.** `AutoScroll` renders a plain div; without
  `overflow-y: auto` (or similar) nothing scrolls and the component does
  nothing visible.
- **Add `overscroll-behavior: contain`.** A wheel or touch scroll that
  hits the container's end mid-drag chains into scrolling the page. The
  edge-scrolling itself is programmatic, clamps at the container's bounds,
  and never chains, but user scrolling does.
- **`Some(true)` means always.** With `active: Some(true)` every pointer
  movement over the container scrolls, drag or no drag. Feed it live drag
  state, not a latched flag; the same rule keeps a retained `drag_pointer`
  from scrolling idle or settling content.
- **Pointer outside means no scroll.** The pointer must be inside the
  container (edges inclusive). Under pointer capture the container keeps
  receiving bubbled moves with the cursor far away; those never scroll.
- **Keyboard drags don't auto-scroll.** They carry no pointer. In a
  virtualized list, a keyboard drag reaches only the mounted window:
  scroll first, then pick up.

## Related

- [Drag and drop](drag-and-drop.md): the zones and drags being scrolled.
- [Sortable lists](sortable-lists.md): `touch_handle` keeps rows
  finger-scrollable inside a scroll container.
- [Virtualized lists](virtualized-lists.md): the full windowing pattern
  behind the archive demo.
- [Multi-window desktop drags](multi-window.md): where `drag_pointer`
  comes from.

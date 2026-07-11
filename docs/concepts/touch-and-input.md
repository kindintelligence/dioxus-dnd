# Touch and input

One input model covers every device: in-app drags run on pointer events,
which unify mouse, touch and pen into a single stream, and every
provider-backed drag source is keyboard-operable on top. Nothing here uses
the browser's native HTML5 drag, so there is no native drag image to fight
and no touch shim to install.

API reference: the `touch` and `threshold` props in
[api/drag-and-drop.md](../api/drag-and-drop.md); this guide is the model
behind them. Live demo: the
[Podcast queue](https://kindintelligence.github.io/dioxus-dnd/podcast-queue)
page runs touch grips inside a scrollable, auto-scrolling list.

## The mental model

A press is not a drag. Every drag surface feeds the same small gesture
machine, and the machine promotes a press to a drag by device-specific
rules:

- **A mouse promotes on plain travel**: 8 CSS px in any direction (the
  `threshold` prop). Below that, a release is a click, so clicks stay
  clicks - no accidental drags from a twitchy press.
- **A finger or a pen under `TouchSense::Auto`** (the default) promotes on
  a short hold - 250ms with the finger still - or on a sideways-dominant
  pull (more horizontal than vertical) past the threshold. A
  vertical-dominant pull resolves the press as scroll intent: the machine
  steps aside and the browser pans the page. An exact diagonal counts as
  scroll.

Keyboard is the third path, present on every typed source: Space or Enter
picks up, arrows choose a zone, Space drops, Escape cancels, and the same
`on_drop` fires with `mode: DragMode::Keyboard`. See
[Accessibility](accessibility.md).

## Sharing the screen with scrolling

`Auto` works because of what the element declares *before* any gesture:
it carries `touch-action: pan-y pinch-zoom`, so the browser knows vertical
pans are its to take, and pinch-zoom stays allowed (two fingers were never
a drag, and zooming is an accessibility floor). Text selection and the iOS
callout are pinned down too, so a long-press cannot start selecting instead
of dragging.

The moment a drag begins, the item owns the touch. `touch-action` is only
consulted at gesture start, so the crate also cancels every further touch
move for the drag's duration - the page stays put under the drag, and a
pan can never start mid-flight. Nothing to configure: scrolling a list and
dragging its rows stop fighting by default.

## `TouchSense::Immediate`

The opt-out restores `touch-action: none`: the surface owns every touch
from the first pixel, and any travel past the threshold drags - no hold, no
sideways rule. The cost is that finger-scrolling *across* the element is
disabled.

Right for surfaces that never sit in a scrollable view - a full-screen
canvas, a game board - or when a vertical pull must begin a drag instantly:

```rust,ignore
Draggable::<Piece> { payload: piece, touch: TouchSense::Immediate, PieceFace {} }
```

Mouse drags are identical under both settings; `TouchSense` decides what
fingers and pens mean, and under `Immediate` every device promotes on plain
travel. `SortableGrid` tiles are always immediate (grids rarely scroll by
dragging across their own tiles); `Draggable` and whole-row `SortableList`
take the `touch` prop.

## Touch handles on sortables

`touch_handle: true` on `SortableList` confines pointer drags to a leading
grip while the rows themselves keep finger-scrolling. The grip is always
immediate - a grip *is* an explicit statement of intent, so there is
nothing to disambiguate - and it is exposed as `[data-sort-handle]` for
styling. The default grip renders a braille-dots glyph; pass the `handle`
callback for your own content.

```rust,ignore
SortableList { len, render, on_sort, touch_handle: true,
    class: "[&_[data-sort-handle]]:w-6 [&_[data-sort-handle]]:cursor-grab",
}
```

Use it when you want the explicit affordance, or when rows are dense enough
that even hold-or-sideways feels risky next to scrolling.

## Pointer capture

Where do move events go once the cursor leaves the element mid-drag? Dioxus
0.7 exposes no pointer-capture API, so the answer depends on platform and
features (this summarizes the README's "Platform notes"):

- **Web with the `web` feature** (recommended for web): the crate grabs
  real pointer capture on press. The drag stays glued to the source no
  matter where the cursor goes, and a release anywhere commits the drop.
- **Web without it** (the dependency-free default): nothing retargets, so
  the crate compensates. Straying off the surface does not cancel the drag,
  and a mouse released outside is reconciled when the cursor returns, via
  held-button state. Best-effort: a release that never returns will not
  commit.
- **Desktop (dioxus-desktop)**: the capture API does not exist there, so
  `Draggable`, `SortableList` and `SortableGrid` render a full-viewport
  capture substitute while a drag is in flight - an invisible fixed layer
  that keeps pointer events flowing to the drag's handlers anywhere in the
  window. Verified on Linux (WebKitGTK).
- **Touch and pen are unaffected everywhere**: the browser implicitly
  captures them to the source element, whole gesture included.

## `PointerKind`

At pickup the crate records which device started the drag - `Mouse`,
`Touch` or `Pen`, mapped from the DOM `pointerType`. Host-side glue (the
multi-window bridge above all) reads it to decide which drags need input
bridging: a touch contact is implicitly captured, so the source element
keeps receiving the whole gesture and bridging it again would double-drive
the drag; mouse and pen go blind at the viewport edge whenever native
capture is unavailable, so they must be bridged. The enum is
non-exhaustive, and `implicitly_captured()` encodes the safe default for
kinds it has never heard of. See [Multi-window](multi-window.md).

## Gotchas

- **`Immediate` inside a scrollable list is a scroll trap.** Rows that own
  every touch cannot be finger-scrolled across; keep `Auto` there, or use
  `touch_handle`.
- **The hold-or-sideways rule covers pens too.** Under `Auto`, a pen
  promotes like a finger (hold or sideways pull); only a mouse promotes on
  plain travel.
- **Two fingers never drag.** Pinch-zoom stays allowed under `Auto` by
  design.
- **Self-contained sortables have no keyboard drag.** `SortableList` /
  `SortableGrid` run without a provider; pair them with `ReorderButtons`
  for the no-gesture path. See [Accessibility](accessibility.md).
- **The capture substitute is `position: fixed`.** A transformed ancestor
  becomes its containing block and clips it - the standard caveat, shared
  with the overlay.

## Related

- [Drag and drop](drag-and-drop.md): the components these gestures drive.
- [Accessibility](accessibility.md): the keyboard path and
  `ReorderButtons`.
- [Sortable lists](sortable-lists.md): whole-row versus handle dragging.
- [Multi-window](multi-window.md): what host glue does with `PointerKind`.

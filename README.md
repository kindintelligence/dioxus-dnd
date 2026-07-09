# dioxus-dnd

[![Crates.io](https://img.shields.io/crates/v/dioxus-dnd.svg)](https://crates.io/crates/dioxus-dnd)
[![Documentation](https://docs.rs/dioxus-dnd/badge.svg)](https://docs.rs/dioxus-dnd)
[![Downloads](https://img.shields.io/crates/d/dioxus-dnd.svg)](https://crates.io/crates/dioxus-dnd)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![Dioxus 0.7](https://img.shields.io/badge/dioxus-0.7-0E6B63)](https://dioxuslabs.com)
[![MSRV 1.85](https://img.shields.io/badge/rustc-1.85%2B-orange.svg)](https://releases.rs/docs/1.85.0/)
[![Tests](https://img.shields.io/badge/tests-156%20passing-brightgreen.svg)](CHANGELOG.md)

**Pick it up. Put it anywhere.** Modular, accessible drag and drop for
[Dioxus](https://dioxuslabs.com): one small core, one module per drop
pattern, use only what you need. Keyboard accessible by default, touch-ready
out of the box, live drop previews, and auto-scroll. The library ships no
JavaScript of its own: everything is Rust, and the optional `web` feature
reaches browser pointer capture through `web-sys` bindings.

**[See every pattern live](https://kindintelligence.github.io/dioxus-dnd/)**:
the gallery pairs eighteen interactive demos with plain-language
walkthroughs and API references, and it is built with this crate.

## Why this crate

- **Typed payloads, no serialization.** In-app drags carry any `Clone` Rust
  value through Dioxus context. No `DataTransfer` string round-trips, no
  JSON, no ids-as-strings.
- **One input model.** Pointer events serve mouse, touch and pen; keyboard
  drag and drop is built into every draggable. There is no native/hybrid
  mode to choose for app UI.
- **Native where it must be.** OS file drops, external text/links/HTML, and
  drag-out to other applications use the real `DataTransfer` protocol,
  because that boundary demands it.
- **Headless and Tailwind-ready.** No CSS ships. Drag state is exposed as
  presence-based data attributes (`data-dragging`, `data-over`, ...), so
  `data-over:border-blue-500` or plain `[data-over]` selectors are all the
  styling wiring you need.
- **Accessible by default.** Space picks up, arrows navigate zones
  spatially, Escape cancels, and `LiveRegion` voices it to screen readers.

## Install

```sh
cargo add dioxus-dnd
```

| dioxus-dnd | Dioxus | Rust |
|---|---|---|
| 2.1 – 2.4 | **0.7** (verified against `0.7.9`) | 1.85+ |
| 2.0 | 0.8 alpha (`0.8.0-alpha.0`) | 1.85+ |

The crate depends on `dioxus` with `default-features = false, features =
["minimal"]`, so it adds no renderer and no extra dependencies of its own.
The optional `web` feature is the only exception: it pulls in `web-sys` for
native pointer capture (see [Feature flags](#feature-flags)).

## Quick start

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[derive(Clone, PartialEq)]
struct Card { id: u32, title: String }

#[component]
fn App() -> Element {
    rsx! {
        DndProvider::<Card> {
            Draggable::<Card> {
                payload: Card { id: 1, title: "hello".into() },
                class: "card",
                "Drag me"
            }
            DropZone::<Card> {
                on_drop: move |o: DropOutcome<Card>| {
                    tracing::info!("got card {} at {:?}", o.payload.id, o.element);
                },
                accepts: move |c: Card| c.id != 0,
                class: "bin",
                "Drop here"
            }
        }
    }
}
```

Both components forward arbitrary attributes (`class`, `style`, and so on)
to their wrapper `div`.

## Pick your pattern

| Module | Pattern | Payload transport |
|---|---|---|
| `core` | `Draggable` to `DropZone` with any `Clone` payload; closest-edge insertion indicators (`edge`) | Rust `Store` context |
| `sortable` | reorder within one list, with live preview (`SortableList`) | self-contained (indices) |
| `grid` | 2D tile reorder or swap (`SortableGrid`) | self-contained (indices) |
| `board` | kanban and cross-container moves (`BoardColumn`, `BoardItem`, `BoardSlot`) | context (`BoardPayload<T>`) |
| `tree` | nested drops with before/after/into intent (`TreeNodeTarget`) | context |
| `canvas` | free-position drops with snap and bounds (`CanvasDropZone`) | context |
| `multiselect` | drag N selected items as one (`SelectableDraggable`) | context (`Vec<K>`) |
| `files` | OS file drops (`FileDropZone`, `FileFilter`) | native event (`evt.files()`) |
| `external` | text, URLs and HTML dropped in from other apps | native `DataTransfer` |
| `dragout` | drag text, links and HTML out to other apps (`ExternalDragSource`) | native `DataTransfer` |
| `autoscroll` | edge-scrolling containers (`AutoScroll`) | n/a |
| `a11y` | screen-reader announcements (`LiveRegion`), no-drag reordering (`ReorderButtons`) | n/a |
| `animate` | FLIP reorder transitions (`FlipItem`) | n/a |
| `debug` | dev-only zone inspector (`DndDebugOverlay`) | n/a |

## How it works

In-app payloads travel through a shared `Store<DragState<T>>` in Dioxus
context: any `Clone` type, zero serialization. Stores (Dioxus 0.7's
fine-grained reactivity) give each state field its own lazy subscription, so
a component that reads `dnd.over()` in render to highlight a zone reruns
only when the hovered zone changes, not on every pointer move.

Native events are used only for what requires the browser/OS boundary: OS
files, external text/links/HTML, and content dragged out to another app.
In-app drag sources use pointer events plus keyboard controls, which avoids
the browser's native drag image and keeps visual state under your control.

The provider also carries a zone registry. Every mounted `DropZone` records
its id, label, drop callback, acceptance filter and DOM handle there; that
registry powers keyboard navigation and pointer hit-testing, and it is
public (`use_zone_registry`) if you want to build your own interaction on
top. The pointer gesture lifecycle itself is a formal, exhaustively tested
state machine (`core::machine`), also public.

## Styling (Tailwind-ready)

The library ships no CSS and no theme. Every component forwards `class` to
its wrapper, and drag state is exposed as data attributes that are
**present while active and absent otherwise**, never `="false"`, so both
plain CSS (`[data-dragging] { ... }`) and Tailwind's presence-based
variants (`data-dragging:opacity-50`) work directly:

| Attribute | Found on | Present while |
|---|---|---|
| `data-dragging` | `Draggable`, `SortableList` / `SortableGrid` items | that element's payload is being dragged |
| `data-drop-target` | `SortableList` / `SortableGrid` items | hovered as the drop slot |
| `data-over` | `DropZone`, `FileDropZone`, `ExternalDropZone` | a (compatible) drag hovers the zone |
| `data-active` | `DropZone`, `BoardSlot`, `CanvasDropZone` | a compatible drag is in flight anywhere; reveal your targets |
| `data-intent` | `TreeNodeTarget` | hovered; valued `"before" \| "after" \| "into"` |
| `data-selected` | `SelectableDraggable` | the item is selected |
| `data-disabled` | `Draggable` | dragging is disabled |

Context-backed attributes follow mouse, touch, pen and keyboard drags alike.
Native boundary components (`FileDropZone`, `ExternalDropZone`) reflect
browser drag events from outside the app. With Tailwind that composes into
complete drag styling with no extra state:

```rust,ignore
DndProvider::<Card> {
    Draggable::<Card> {
        payload: card,
        class: "rounded-lg border p-3 data-dragging:opacity-40 data-dragging:cursor-grabbing",
        "Drag me"
    }
    DropZone::<Card> {
        on_drop: handle_drop,
        class: "rounded-xl border-2 border-dashed border-transparent p-4
                data-active:border-gray-300 data-over:border-blue-500 data-over:bg-blue-50",
        "Drop here"
    }
}
```

`SortableList` and `SortableGrid` render their own item wrappers; those
wrappers are where `data-dragging` / `data-drop-target` live. For lists,
style those wrappers from the list's forwarded root `class` with direct
child selectors. `SortableGrid` also has an `item_class` prop for its tile
wrappers:

```rust,ignore
SortableList {
    len, render, on_sort,
    class: "[&>*]:rounded [&>*]:border [&>*]:bg-white [&>*]:p-2
            [&>[data-dragging]]:opacity-40
            [&>[data-drop-target]]:border-blue-500",
}
```

Value selectors work too, e.g. tree insertion indicators:
`data-[intent=before]:border-t-2 data-[intent=into]:bg-blue-50
data-[intent=after]:border-b-2`.

For insertion indicators on a *bare* zone, opt into the closest-edge
signal: `DropZone { edge: EdgeSet::Vertical, ... }` carries
`data-edge="top" | "bottom"` live while an acceptable pointer drag hovers
(`Horizontal` tracks left/right, `All` every side), and the delivered
`DropOutcome::edge` records the edge held at release - so the handler maps
`Top` to "insert before" without re-deriving geometry. The pure function
behind it, `edge_of(point, rect, edges)`, is public for custom zones. The
gallery's *Itinerary* page builds a drop-above/drop-below list with it.

The drag ghost styles the same way; `DragOverlay` forwards `class` to its
wrapper while positioning stays functional:

```rust,ignore
DragOverlay::<Card> { settle: true, class: "rotate-3 scale-105 shadow-xl", GhostCard {} }
```

`settle: true` is the drop animation: on a successful pointer drop the
ghost glides from the release point into the receiving zone instead of
vanishing (tune with `duration`/`easing`; honors `prefers-reduced-motion`;
cancelled drags and keyboard drops never settle).

To style *children* of a state-carrying wrapper, either mark the wrapper a
group (`SortableGrid`'s `item_class: "group"`, or a list root selector such
as `class: "[&>*]:group"`) and use `group-data-dragging:opacity-40` on
inner elements, or, with Tailwind v4, use the `in-*` variant from inside
with no wrapper class at all: `in-data-dragging:italic` inside your
`render` content reacts to the row's drag state with zero wiring.

One mechanic worth knowing: a forwarded `style` is *merged after* any
functional inline style (`touch-action` on `Draggable`, positioning on
`DragOverlay`, the `display: grid` layout on `SortableGrid`) rather than
replacing it; your declarations win per property, the functional ones
survive. So grid spacing is just `class: "gap-2"`, and custom column tracks
are `style: "grid-template-columns: 2fr 1fr 1fr;"`.

Not using Tailwind? The same contract serves plain CSS: `[data-over]`,
`[data-intent="into"]`, `[data-sort-handle]`, and so on.

## Accessibility (built in, not opt-in)

Drag-and-drop is where accessibility usually goes to die: pointer-only
interactions, silent state changes, motion nobody asked for. This crate
treats the accessible path as the same path - every capability below works
on every `Draggable` and `DropZone` with no extra wiring, and the pieces
you *do* wire (labels, one `LiveRegion`) are one prop each.

### Keyboard operation

Every `Draggable` is focusable (`tabindex="0"`) and fully operable without
a pointer:

- **Space / Enter** picks the item up
- **Up / Down** cycles drop zones at the current level (spatial order,
  top-to-bottom then left-to-right, so arrows match what the eye sees)
- **Right / Left** descends into a zone's nested zones or ascends to the
  parent (nesting is detected automatically when `DropZone`s contain
  `DropZone`s; in flat apps these fall back to next/previous - the
  WAI-ARIA tree convention)
- **Space / Enter** drops into the selected zone
- **Escape** cancels - there is no keyboard trap; focus never leaves the
  item, and every drag can be abandoned

Keyboard drags drive the same context as pointer drags, so zones light
their `data-active`/`data-over` styling identically, drops deliver the
same `DropOutcome` (with `mode: DragMode::Keyboard`), and the drop-settle
animation and closest-edge signal degrade gracefully rather than
misbehaving.

### Screen readers

Elements carry `role="button"` and `aria-roledescription="draggable"`, so
assistive tech announces what the thing *is*. Render one `LiveRegion::<T>`
per provider - a visually-hidden `aria-live="polite"` region - and every
step of a keyboard drag is voiced without stealing focus, including the
instructions:

```rust,ignore
DndProvider::<Card> {
    LiveRegion::<Card> {}
    Draggable::<Card> { payload: card, label: "Ship it", /* ... */ }
    DropZone::<Card>  { label: "Done", on_drop, /* ... */ }
}
// "Picked up Ship it. Use arrow keys to choose a drop target,
//  Enter to drop, Escape to cancel."
// "Over Done." / "Over Done, inside Sprint board." (nested zones name
//  their parent)  then "Dropped in Done." or "Drag cancelled."
```

Dead ends are voiced too ("No drop targets available.", "No drop target
selected."), and custom flows push their own messages with
`dnd.announce(...)`. Every phrase is localizable - see
[Localization](#localization). In virtualized lists, forward
`aria-setsize`/`aria-posinset` so position is announced against the full
list, not the rendered window (the gallery's *Archive* page shows this).

### Reordering without any drag at all

`a11y::ReorderButtons` renders real move-up/move-down `<button>`s with
localized `aria-label`s ("Move Piranesi up"), disabled at the list edges,
emitting the same `SortEvent` as drag-reordering - so one `on_sort` serves
pointer drags, keyboard drags, and plain button presses. This is the
strongest fallback there is: no gesture of any kind required.

### RTL layouts

Pass `dir: Direction::Rtl` on the provider and keyboard navigation
mirrors: spatial order runs right-to-left within a row, and the
descend/ascend arrows swap so "into" is always the arrow pointing along
reading order (the WAI-ARIA tree convention):

```rust,ignore
DndProvider::<Card> { dir: Direction::Rtl, /* ... */ }
```

### Reduced motion

Everything the crate animates - `SortableList`'s live preview,
`FlipItem`'s glide, `DragOverlay`'s drop-settle - marks its moving
elements with `data-dnd-motion` and ships a
`prefers-reduced-motion: reduce` override, so drags snap instead of
gliding when the user asks the OS for less motion. Nothing to configure;
mark your own animated elements with the same attribute to opt them in.
(The override uses a near-zero duration rather than zero, so
`transitionend`-driven cleanup still runs.)

### Motor forgiveness

Presses only become drags after an 8px movement threshold (clicks stay
clicks), near-miss releases snap to the closest acceptable zone whose edge
is within 48px, and touch auto-senses by default - vertical swipes scroll,
a short hold or sideways pull drags - so scrolling a list and dragging its
rows don't fight (see [Touch](#touch); `touch_handle` grips remain for
lists that want an explicit affordance).

### What this means for compliance

The crate covers the interaction-layer criteria a drag-and-drop feature
usually fails. Mapping to **WCAG 2.2**:

| Criterion | How it's met |
|---|---|
| 2.1.1 Keyboard (A) | complete keyboard path on every draggable, built in |
| 2.1.2 No Keyboard Trap (A) | focus stays on the item; Escape always cancels |
| 2.5.7 Dragging Movements (AA) | keyboard operation on everything, plus `ReorderButtons` as a no-gesture, single-pointer alternative for reordering |
| 4.1.2 Name, Role, Value (A) | `role="button"` + `aria-roledescription="draggable"`, accessible names from `label` props, real buttons in `ReorderButtons` |
| 4.1.3 Status Messages (AA) | `LiveRegion`'s polite live region announces every state change without moving focus |
| 2.3.3 Animation from Interactions (AAA) | `prefers-reduced-motion` honored across every built-in animation |

Honest scope: a library can't make your *app* compliant. You still own
focus visibility (2.4.7), contrast and target sizes for your rendered
items, and passing meaningful `label`s - the crate makes those the only
things left to do.

## Localization

Every phrase the crate voices - the keyboard announcements,
`ReorderButtons` labels and row fallbacks, the `SelectionCount` badge -
reads a `DndStrings` from context, with English built in. Each field owns
a whole sentence as a function, so translations reorder, inflect and
pluralize freely; nothing is concatenated for them. Provide one anywhere
above your drag UI and override only what you translate:

```rust,ignore
use_context_provider(|| DndStrings {
    picked_up: Rc::new(|name| t!("picked-up", name: name)),   // dioxus-i18n
    dropped_in: Rc::new(|name| t!("dropped-in", name: name)),
    cancelled: Rc::new(|| t!("cancelled")),
    ..Default::default()
});
```

The crate stays dependency-free: the closures call whatever produces a
`String` - dioxus-i18n's `t!` (shown; the Fluent-based crate the Dioxus
docs recommend) or a `match` on your own locale signal. Have them *read*
the locale rather than re-providing the struct on switch: components
capture `DndStrings` once at mount, but every phrase is a fresh call, so
the very next announcement speaks the new language.

Two things to pair with it: pass your item and zone `label`s through the
same translation layer (the crate voices the names you give it), and set
`dir: Direction::Rtl` for right-to-left locales so the keyboard's spatial
navigation matches the mirrored layout. Custom components can voice
themselves consistently via `use_dnd_strings()`. The gallery's *Packing
list* page shows the full dioxus-i18n wiring - inline Fluent catalogs, a
live English/Spanish toggle, and a visible mirror of the announcement
channel. (`DndDebugOverlay` is intentionally not localized; it's a
dev-only tool.)

## Touch

Every in-app drag pattern uses pointer events for mouse, touch and pen,
plus keyboard controls where a typed provider is involved.

- `Draggable` is the drag source for `DropZone` targets. `BoardItem`,
  `SelectableDraggable` and `TreeNodeTarget` build on the same machinery,
  so boards, multi-select and trees are touch-ready as-is. Missed pointer
  drops re-measure zones and retry with a closest-center fallback, so drops
  in the gutter between zones still land.
- `SortableList` and `SortableGrid` carry their own built-in pointer path,
  avoiding the browser's native drag image during reorders.
- Native components stay native because they cross the app boundary:
  `FileDropZone`, `ExternalDropZone`, `ExternalDragSource` and
  `external::typed` use `DataTransfer` for file drops, external drops,
  drag-out and cross-window interop.

Touch surfaces auto-sense by default (`TouchSense::Auto`): a `Draggable`
or whole-row `SortableList` carries `touch-action: pan-y`, so a vertical
swipe keeps scrolling the page, while a short hold (250ms with the finger
still) or a sideways pull picks the item up - and from that moment the
item owns the touch, so the page stays put under the drag. Nothing to
configure, no scroll trap, and mouse drags are exactly as before (the
hold-or-sideways rule applies only to fingers and pens; a mouse promotes
on plain 8px travel).

Two opt-outs when `Auto` isn't the right call:

- `touch: TouchSense::Immediate` restores `touch-action: none` - the
  surface owns every touch from the first pixel and any 8px travel drags.
  Right for surfaces that never sit in a scrollable view (a full-screen
  canvas, a game board), or when a vertical pull must begin instantly.
- `touch_handle: true` on sortables confines pointer drags to a leading
  grip (always immediate - a grip *is* an explicit statement of intent)
  while the rows themselves keep scrolling. The default grip is exposed
  as `[data-sort-handle]`, so style it from the list root class or plain
  CSS:

```rust,ignore
SortableList { len, render, on_sort, touch_handle: true,
    class: "[&_[data-sort-handle]]:w-6 [&_[data-sort-handle]]:cursor-grab",
}
```

## Sortable lists with live preview

```rust,ignore
let mut items = use_signal(|| vec!["alpha".to_string(), "beta".into(), "gamma".into()]);
rsx! {
    SortableList {
        len: items.read().len(),
        render: move |ix: usize| rsx! { "{items.read()[ix]}" },
        on_sort: move |ev: SortEvent| apply_sort(&mut items.write(), ev),
    }
}
```

While you drag, the row slides toward its landing slot and its neighbors
translate out of the way: a live preview of the final order (disable with
`live_preview: false`). An optional `overlay` prop renders a floating ghost
pinned to the pointer while the in-flow row becomes the gap.

## Canvas drops

`CanvasDropZone` is the free-position primitive for node editors,
whiteboards and floor planners. Start in-app moves with `Draggable`; the
completed `CanvasDrop` gives you both the raw canvas-relative pointer and
the corrected top-left position:

- `pointer`: where the pointer landed inside the canvas.
- `position`: `pointer - grab`, then optional snap and bounds.
- `Bounds` clamps the returned top-left point. It does not know the dropped
  element's own width or height; use `Bounds::clamp_item` or
  `Bounds::clamp_rect` when you want the whole item inside.

Keyboard drops use the selected target's measured center by default. Set
`keyboard: CanvasKeyboardPlacement::Origin` or
`CanvasKeyboardPlacement::Fixed(point)` when keyboard placement should be
explicit.

```rust,ignore
CanvasDropZone::<Node> {
    snap: SnapGrid(16.0),
    bounds: Bounds { width: 640.0, height: 360.0 },
    on_drop: move |drop: CanvasDrop<Node>| {
        place_node(drop.payload.id, drop.position);
    },
    for node in nodes.read().clone() {
        Draggable::<Node> {
            payload: node,
            style: "position: absolute;",
            NodeView {}
        }
    }
}
```

For richer constraints such as "keep the whole node inside the canvas", use
`Bounds::clamp_item` for simple bounds or the composable modifier chain
(`apply_modifiers`, `DragModifier::KeepInside`, `ModifierCtx`) with the
element size you know in your app. The pure helpers `client_to_canvas`,
`canvas_to_client` and `canvas_position` are available when wiring custom
interactions.

## Auto-scroll

Wrap any scrollable container in `AutoScroll` and drags hovering within
`threshold` px of an edge (default 48) scroll it by up to `speed` px per
event (default 24), ramped by proximity. Works for in-app pointer drags and
native boundary drags alike. Pass `active: Some(false)` when a parent
tracks drag state and wants to suppress scrolling. Pure `MountedData`, no
JavaScript eval.

```rust,ignore
AutoScroll { style: "max-height: 300px; overflow-y: auto;",
    for row in rows { /* ... */ }
}
```

Scrolling moves everything inside the container, so `AutoScroll` also pings
the rect-refresh channel after every scroll (its own or the user's wheel
mid-drag): drop-zone registries re-measure, and `SortableList` /
`SortableGrid` - which need no provider; `AutoScroll` anchors the channel
for them - re-anchor their cached row slots against the wrapper's movement.
Hover highlighting and the eventual drop land on what the user actually
sees, not where things sat at pickup. If you move layout under a live drag
some other way - a custom scroll surface, a collapsing panel - grab the
channel yourself with `use_rect_refresh()` and call `refresh_all()` from
your event. Participants without a drag in flight ignore the ping, so it's
free to call from high-frequency sources.

## Modifier keys

The file-manager convention works out of the box: holding **Ctrl/Cmd**
during a drag forces a Copy effect, **Alt** forces Link, and the resolved
value arrives in `DropOutcome::effect`, so your `on_drop` can branch on
move-vs-copy. `effective_effect` is public if you need the same resolution
in custom handlers.

For simple zone models, `apply_clone_or_move` applies that convention to a
`HashMap<ZoneId, Vec<T>>`. Give it an identity function so moves can remove
the source item, and a clone hook for assigning a fresh id on copy:

```rust,ignore
DropZone::<Card> {
    on_drop: move |outcome: DropOutcome<Card>| {
        apply_clone_or_move(
            &mut cards_by_zone.write(),
            outcome,
            |card| card.id,
            |mut card| {
                card.id = next_id();
                next_id += 1;
                card
            },
        );
    },
    "Drop here"
}
```

For two plain lists, use `apply_list_clone_or_move` and pass the source
list directly.

## Multi-select

```rust,ignore
let selection = use_selection::<FileId>();
rsx! {
    DndProvider::<Vec<FileId>> {
        for f in files { SelectableDraggable::<FileId> { item: f.id, selection, FileRow { f } } }
        DropZone::<Vec<FileId>> { on_drop: move |o| trash(o.payload), "Trash" }
        DragOverlay::<Vec<FileId>> { SelectionCount::<FileId> {} }
    }
}
```

Click selects one, Ctrl/Cmd+click toggles. Dragging a selected item carries
the whole selection; dragging an unselected one carries just itself.

## File drops

```rust,ignore
FileDropZone {
    filter: FileFilter::new()
        .extensions(["png", "jpg"])
        .content_types(["image/*"])
        .max_size(5_000_000)
        .max_files(10),
    on_files: move |drop: FileDrop| async move {
        for f in drop.files {
            let bytes = f.read_bytes().await?; // web
            // or f.path()                     // desktop
        }
    },
    on_rejected: move |bad: Vec<(FileData, FileRejection)>| { /* toast */ },
    "Drop images"
}
```

`FileFilter::content_types` supports exact MIME types (`application/pdf`),
top-level wildcards (`image/*`), all typed files (`*/*`) and structured
suffix wildcards (`application/*+json`, `*/*+json`). Matching is
case-insensitive and ignores MIME parameters such as `; charset=utf-8`.
`FileFilter::extensions` is also case-insensitive and accepts extensions
with or without a leading dot. Filters are a UX affordance, not a security
boundary: validate real bytes server-side.

## Dragging out

```rust,ignore
ExternalDragSource {
    content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
    "Drag this link to another tab"
}
```

`OutboundContent` covers text, links (written as `text/uri-list` plus
`text/plain` plus `text/html`), rich HTML with a plain-text fallback, and
raw custom `(format, data)` pairs. Generated HTML anchors escape their
content and refuse `javascript:`-style schemes.

With the `serde` feature the boundary also speaks **types**:
`TypedDragSource` serializes a payload to JSON on the drag's
`DataTransfer` (always alongside a legible `text/plain` fallback), and
`TypedDropZone` decodes drops back to the type - ignoring untyped drags
and reporting undecodable JSON through `on_invalid`. That is the wire for
drags between two *separate* apps; between windows of one app, read on.

## Multi-window desktop drags

On desktop, one app is often several windows - and a drag should not care.
Create a `DndWorld<T>` in your first window, hand it to the others, and
every joined window shares one drag: zones light up across windows, the
ghost hands off to whichever window the cursor is over (scale-aware on
mixed-DPI setups), and the payload arrives as a live Rust value - no
serialization, no `DataTransfer`, same `on_drop` you already have.

```rust,ignore
fn board_window() -> Element {
    let world = use_dnd_world::<Card>();          // once, in any window
    let open_tray = move |_| {
        dioxus::desktop::window().new_window(
            VirtualDom::new(tray_window).with_root_context(world),
            Default::default(),
        );
    };
    rsx! { DndProvider::<Card> { /* joins the world via context */ } }
}

fn tray_window() -> Element {
    rsx! { DndProvider::<Card> { /* joins via root context */ } }
}
```

Two pieces of glue make it spatial, both plain app code today (see
`examples/desktop-multiwindow/` for a working two-window board-and-tray,
probe binary included):

- **Geometry**: feed each window's position/size/scale into a
  `WindowGeometry` from tao's window events, so the world can hit-test
  windows in desktop coordinates.
- **The bridge**: webview pointer events stop at the viewport edge, and
  while a button is held every *other* window is event-blind on every OS
  (that's how pointer grabs work). So the origin window's glue polls the
  global cursor (`cursor_position()`) to keep the drag tracking outside
  its own viewport, and a blind window receiving its first pointer event
  mid-drag - proof the button was released - completes the drop through
  `DndWorld::drop_at_global`.

Windows may close in **any order**: the world's state is process-lived, a
window closing mid-drag aborts a drag that started there (and merely
clears the hover if it was only being hovered), and where geometry is
unavailable - Wayland forbids it by design - everything gracefully
degrades to normal per-window drags. Headless tests drive all of it: the
world-aware `DragSim` simulates whole cross-window arcs in CI.

## Nesting

Sortables inside sortables, boards inside boards: inner drag scopes stop
propagation on drag start and drop, so each level owns its own gestures.
Nested `DropZone`s discover their parents automatically through context,
which is what powers hierarchical keyboard traversal. No configuration
needed.

## Mixing payload types

A provider is monomorphic on purpose: a `Task` drag can only land on a
`DropZone<Task>`, checked at compile time. "Polymorphic" needs come in two
shapes, with different answers.

**Several payload shapes, one drag world.** A tree whose nodes are files or
folders, a list mixing cards and separators: make the payload an enum. The
zone's `accepts` filters variants, the drop handler matches on them, and
the compile-time guarantee stays intact:

```rust,ignore
#[derive(Clone, PartialEq)]
enum Node { File(u64), Folder(u64) }

DropZone::<Node> {
    accepts: move |n: Node| matches!(n, Node::Folder(_)),
    on_drop: move |o: DropOutcome<Node>| match o.payload {
        Node::File(id) => { /* … */ }
        Node::Folder(id) => { /* … */ }
    },
}
```

**Two independent drag worlds sharing one target.** Sometimes two providers
genuinely coexist - tasks and teammates, say - and one region should accept
drops from both. That's `BridgeDropZone<A, B>`: one element holding the
*same* `ZoneId` in both worlds' registries (ids are process-global,
registries per-type), sharing its `mounted`/`rect` signals. Each world's
machinery (hit-testing, keyboard navigation) finds the zone on its own,
acceptance is per-world (`accepts_a`/`accepts_b`), and each drop arrives
through its own typed callback (`on_drop_a`/`on_drop_b`) - no downcasts, no
shared erased channel:

```rust,ignore
BridgeDropZone::<Task, Person> {
    label: "Standup agenda",
    on_drop_a: move |o: DropOutcome<Task>| { /* … */ },
    on_drop_b: move |o: DropOutcome<Person>| { /* … */ },
    "Drop a task or a teammate"
}
```

The gallery's *Standup* page shows it live. For *three or more* worlds,
generate a component for your exact type list with the
`bridge_drop_zone!` macro - each row is one world, with its own optional
acceptance filter and required typed drop callback:

```rust,ignore
dioxus_dnd::bridge_drop_zone!(pub StandupZone {
    (Task, accepts_task, on_drop_task),
    (Person, accepts_person, on_drop_person),
    (Alert, accepts_alert, on_drop_alert),
});
```

(Rust has no variadic generics, so the component is generated per concrete
type list - which is also why `BridgeDropZone<A, B>` stops at two.) Under
both sits `use_bridge_world`, public too: call it once per world with a
shared id and `mounted`/`rect` signals to build something custom.

## Virtualized lists

Rows can be zones and zones can churn: a windowed (virtualized) list
registers a `DropZone` per rendered row and unregisters it on recycle, and
the registry keeps up - zones measure themselves the moment they mount, so
a row that scrolls into view *mid-drag* is hit-testable immediately. Give
rows stable index-derived ids (`ZoneId(BASE + index)`) so a recycled row
re-registers as itself.

Two practical notes for the windowing itself. dioxus-web 0.7 delivers no
element-level scroll events, so drive your window from `onvisible` on the
rendered rows (each crossing of the container's clip reports an
IntersectionObserver rect that, with the row's canvas position, recovers
the scroll offset - Dioxus's documented virtual-list tool) plus
`AutoScroll`'s `on_scroll` for its own edge-scrolling during drags. The
gallery's *Archive* page runs the full pattern at 10,000 rows, keyboard
navigation included.

## Debug overlay (dev-only)

When a zone won't light up or a drop lands somewhere surprising, render
`DndDebugOverlay::<T>` inside the provider: every registered zone draws as
a tinted, labeled outline pinned over the page, rejecting zones dim and go
dashed while a drag is in flight, the hovered zone fills live (pointer and
keyboard alike), and a status chip counts zones the registry hasn't
measured. What it draws *is* the registry - a missing or misplaced outline
means hit-testing sees exactly the same wrong thing.

It is a **development tool**: unstyled chrome over your UI, not localized.
Gate it out of release builds yourself:

```rust,ignore
if cfg!(debug_assertions) {
    DndDebugOverlay::<Card> {}
}
```

## Examples and website

The [live gallery](https://kindintelligence.github.io/dioxus-dnd/) is
`examples/gallery/` in this repo: a multi-page site with one page per
pattern, each pairing a live demo with a plain-language walkthrough and an
API reference. It deploys to GitHub Pages from CI.

```sh
dx serve --example gallery --platform web --features web
```

There is also a focused board example:

```sh
dx serve --example kanban --platform web --features web
```

## Testing

**Your drag-and-drop is unit-testable.** The drag state machine is plain
Rust over signals, so `dioxus_dnd::test` runs whole pointer interactions
inside a `VirtualDom` - in CI, no browser. Mount a `DragSimProbe` in your
test app, place the zone rects (the headless stand-in for layout - which
makes tests deterministic), and drive:

```rust,ignore
use dioxus_dnd::test::{drag_sim, rerender, simulate_drag, DragSimProbe};

fn test_app() -> Element {
    rsx! {
        DndProvider::<Card> {
            DragSimProbe::<Card> {}
            Shelves {}   // the component under test
        }
    }
}

let mut dom = VirtualDom::new(test_app);
dom.rebuild_in_place();
let mut sim = drag_sim::<Card>();

sim.place(&dom, FINISHED, Rect::new(0.0, 100.0, 200.0, 80.0));
sim.pick_up_from(&dom, card.clone(), Some(READING));
sim.move_to(&dom, Point::new(100.0, 140.0));
assert_eq!(sim.over(&dom), Some(FINISHED));
rerender(&mut dom);
assert!(dioxus_ssr::render(&dom).contains("data-over"));

assert_eq!(sim.release(&dom), Some(FINISHED));  // your on_drop just ran
// ...assert your model moved the card.
```

Or the whole arc in one line:
`simulate_drag(&mut dom, card, Some(READING), &[Point::new(100.0, 140.0)])`.
Drops run the *production* delivery path - acceptance filters, the 48px
near-miss snap, closest-edge enrichment, `DropOutcome` construction - and
`release_as(&dom, DropEffect::Copy)` simulates the Ctrl-held copy. This
crate's own runtime tests drive the same simulator.

The Rust tests cover pure state, SSR output and geometry helpers. Pointer
capture is browser behavior, so the web path also has Playwright
regressions driving the headless fixtures in `examples/regressions.rs`:
sortable overlay geometry and cleanup, releases outside a list or grid
committing no reorder, autoscroll edge behavior, canvas grab-offset
placement, drop fall-through past rejecting zones, the Ctrl-drag copy
convention, reorder buttons inside sortable rows, the native boundary
paths, a bridge zone receiving typed drops from two payload worlds, drops
landing on zones - and sortable slots - that auto-scrolled into place
mid-drag, the touch auto-sensor (real CDP touch gestures: swipes scroll,
holds and sideways pulls drag, promoted drags pin the page), and
`FlipItem`'s synchronously-armed glide.

```sh
cargo test
npm install && npm run test:web
```

## Feature flags

- `serde`: enables the typed `DataTransfer` transport for drags that must
  cross **app** boundaries - `TypedDragSource`/`TypedDropZone` and the
  underlying `external::typed::{store, retrieve}` (JSON, wire-compatible
  with dioxus-html's own helpers). Multi-window drags within one app need
  no feature: `DndWorld` is core and carries live Rust values.
- `web`: enables native **pointer capture** (via `web-sys`, pinned to the
  version `dioxus-web` uses) so mouse pointer-drags stay glued to the drag
  source even when the cursor leaves it, and lets `FlipItem` arm its glide
  synchronously on the real element (no paint-timing dependency). Off by
  default: the core stays dependency-free, and both fall back to
  best-effort paths (see [Platform notes](#platform-notes)). Enable it for
  web builds: `features = ["web"]` in your `Cargo.toml`. Touch and pen
  never need pointer capture.

## Platform notes

- **Mouse pointer drags.** Dioxus 0.7 exposes no pointer-capture API, so
  the behavior depends on the `web` feature:
  - **With `web`** (recommended for web): the crate grabs real pointer
    capture on press, so the drag stays glued to the source no matter where
    the cursor goes; release anywhere commits the drop.
  - **Without it** (dependency-free default): straying off the drag surface
    no longer cancels the drag, and a mouse released outside is reconciled
    when the cursor returns (via held-button state). Best-effort; a release
    that never returns won't commit.
  - Touch and pen are unaffected either way; the browser implicitly
    captures them.
- **Desktop pointer drags** (dioxus-desktop) run without native pointer
  capture - the `web` feature's capture API doesn't exist there - so
  `Draggable`, `SortableList` and `SortableGrid` render a full-viewport
  capture substitute while a drag is in flight, which keeps mouse drags
  tracking anywhere in the window. Verified on Linux (WebKitGTK).
- **Multi-window drags**, verified per platform (2026-07):
  - **Linux/X11**: works end to end - cross-window hovers, ghost handoff,
    drops - with the example's geometry feed and cursor-polling bridge.
    (Under WSLg specifically, session state can corrupt move-event button
    masks; the library debounces, but treat WSLg as a smoke-test rig,
    not a verdict machine.)
  - **Linux/Wayland**: cross-window is impossible by OS design (a client
    can learn neither its windows' positions nor the global cursor);
    the world detects missing geometry and degrades to per-window drags.
    Verified: drags park at the window edge and recover cleanly.
  - **Windows (WebView2)**: verified end to end (Win 11 ARM64, 1.5x
    scale, 2026-07) - cross-window hovers, ghost handoff, drops both
    directions, dead-space cancel, tray close/reopen mid-drag - but the
    bridge needs a third leg there. WebView2 keeps streaming mouse
    events to the origin webview outside its viewport, yet they target
    `<html>` (nothing retargets without pointer capture) so no component
    hears them, and tao never sees `CursorMoved`/`MouseInput` because
    the WebView2 child HWND consumes them. The example bridges via raw
    input: `DeviceEvent::Button`/`MouseMotion` through
    `use_wry_event_handler` plus
    `set_device_event_filter(DeviceEventFilter::Never)` (the default
    `Unfocused` filter never delivers - the foreground input owner is
    the WebView2 process's HWND). Touch needs none of this - implicit
    capture streams the whole gesture to the origin webview - and MUST
    NOT be bridged: Windows synthesizes mouse input from touch (a
    cursor trailing the finger, spurious button transitions), so
    bridging a touch drag double-drives it. The example gates every
    bridge leg on the drag's `PointerKind` (recorded by `Draggable` at
    pickup, `ctx.pointer_kind()`): bridge mouse and pen, never touch.
    Known trap unchanged: windows created hidden then shown have broken
    DnD in WebView2 (wry#1639), so create drop-target windows visible.
  - **macOS (WKWebView)**: expected to work on the same reasoning
    (AppKit routes the whole drag sequence to the mousedown view;
    `cursor_position` supported); not yet hand-verified.
- **Windows desktop file drops** have a history of platform quirks in
  wry-based webviews. Test on your target and consider a file input
  fallback. Note the tradeoff wry imposes on Windows: its drop handler
  and HTML5 drag-and-drop are mutually exclusive per window
  (`with_disable_drag_drop_handler`), so a window using the typed
  `DataTransfer` transport there gives up native file drops.
- **`animate::FlipItem`** with the `web` feature arms its glide
  synchronously on the real element (invert, forced style flush, release),
  so it cannot race the browser's paint schedule. Without `web` it falls
  back to animating through two renders; that fallback is the one code
  path whose behavior depends on browser paint timing rather than pure
  logic - validate it in your target renderer.

## Prior art

The Dioxus ecosystem has several dnd crates with different philosophies:
`dioxus-dnd-kit` (mouse-synthesized, layout-stability focused), `taino-dnd`
(framework-agnostic core, pointer-events) and `dioxus-nox-dnd` (headless
sortable primitives). This crate's live-preview displacement, collision
fallback, modifier chain and gesture state machine were informed by reading
them, and by dnd-kit and react-beautiful-dnd before that. What it does that
the others do not: the native boundary path (OS file drops, drag-out to
other apps, copy/move effects) alongside touch and keyboard, across
fourteen patterns - and multi-window desktop drags with live payloads, a
story neither dnd-kit nor pragmatic-drag-and-drop tells either.

## License

Licensed under the [MIT license](LICENSE-MIT).

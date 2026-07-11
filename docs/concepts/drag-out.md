# Dragging out

The mirror of [External content in](external-content.md): your content
leaving for another browser tab, the URL bar, a text editor, or any
application that accepts the standard `DataTransfer` formats. Wrap the
element in `ExternalDragSource`, describe the content once, and the crate
writes every representation that fits.

API reference: [api/drag-out.md](../api/drag-out.md).
Live demo: the [Share](https://kindintelligence.github.io/dioxus-dnd/share)
page exercises both directions of the app boundary.

## The mental model

The receiving application shares no code with you. The only language you
have in common is the `DataTransfer`: `(format, string)` pairs written at
drag start, read by whoever catches the drop. `OutboundContent` describes
what leaves; `ExternalDragSource` writes it and advertises a drop effect.
No provider is needed - the browser is the transport, not the shared
context.

Because different targets read different formats, each content shape
writes everything that fits:

- `Text` writes `text/plain`.
- `Url` writes `text/uri-list` and `text/plain`, and with a title also a
  `text/html` anchor - so URL bars, editors and rich targets each find a
  representation they understand.
- `Html` writes `text/html` plus a `text/plain` fallback for targets that
  do not take markup.
- `Custom` writes raw `(format, data)` pairs verbatim, in order, for
  proprietary formats.

## A complete example

A link card users can drag into another tab, the URL bar, or a chat app:

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[component]
fn ShareCard() -> Element {
    rsx! {
        ExternalDragSource {
            content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
            class: "cursor-grab rounded-xl border p-4",
            div { "Dioxus" }
            div { "dioxuslabs.com" }
        }
    }
}
```

Dropping this on a URL bar navigates there (`text/uri-list`), on a text
editor pastes the URL (`text/plain`), and on a rich target inserts a
titled link (`text/html`). The `effect` prop defaults to `Copy`, which is
what outbound drags almost always mean: the content is not leaving your
app, a copy of it is.

## The generated anchor is guarded

Only the `text/html` representation of a `Url` is an injection surface;
the plain-text formats are not markup, so the raw URL travels verbatim
there. For the anchor, both fields are escaped for their context (the URL
as an attribute value, the title as text content), and URLs with schemes
that can execute script when the HTML lands in another app - `javascript:`,
`data:`, `vbscript:`, matched case-insensitively and ignoring leading
whitespace, the way browsers resolve them - lose their `href` entirely.
A hostile URL still ships as inert text; it cannot carry an active link
into the target.

`Custom` pairs get none of this. They are written verbatim, so escaping
and scheme judgment are yours.

## Typed drag-out

With the `serde` feature, `TypedDragSource<T>` serializes any `Serialize`
payload to JSON on the drag's `DataTransfer`, under
`external::typed::MIME`, always alongside a `text/plain` fallback so
non-typed targets (text editors, other apps) still receive something
legible. The fallback defaults to the JSON itself; set `fallback_text` for
a friendlier string. The receiving end is `TypedDropZone` - yours, in
another app - or any consumer of dioxus-html's wire-compatible `retrieve`.
See [External content in](external-content.md) for the receiving side.

Serialization failures are contained: the drag degrades to carrying only
the fallback text, and reports through `on_error` if wired.

This JSON wire exists for drags between two separate apps: separate tabs,
separate deployments, separate processes. Between windows of one app, a
`DndWorld` carries live Rust payloads with no serialization and no
`Serialize` bound; see [Multi-window drags](multi-window.md).

## Gotchas

- **The ghost is the browser's.** Outbound drags are native, so the drag
  image is the browser's rendering of the element. `DragOverlay` reads the
  in-app context and never applies here.
- **No keyboard path.** The native drag protocol has no keyboard
  operation, and the crate's keyboard machinery cannot cross the app
  boundary. If the content matters, offer a copy-to-clipboard affordance
  alongside the drag.
- **Drag start stops propagation.** A source nested inside another
  element's drag handlers owns the drag; an ancestor source does not also
  write.
- **`serde` gates the typed half only.** `ExternalDragSource` and
  `OutboundContent` are always available; `TypedDragSource` needs
  `features = ["serde"]`.

## Related

- [External content in](external-content.md): the mirror direction,
  including `TypedDropZone` and the typed wire format.
- [Drop effects](drop-effects.md): the `DropEffect` vocabulary the
  `effect` prop advertises.
- [Multi-window drags](multi-window.md): when both ends are windows of
  one app, skip serialization entirely.

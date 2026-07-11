# External content in

Every other pattern in the crate handles drags your app started. This one
handles drags the outside world starts: selected text, a link pulled from
another tab, rich content from another application. The browser is the
transport here, not the shared context, so the machinery is different: no
provider, no payload type parameter, the real `DataTransfer` protocol.

API reference: [api/external-content.md](../api/external-content.md).
Live demo: the [Share](https://kindintelligence.github.io/dioxus-dnd/share)
page exercises both directions of the app boundary.

## The mental model

An external drag carries no Rust value, because the thing in flight was
never yours. What the browser delivers is a `DataTransfer`: a bag of
`(format, string)` entries, each a different representation of the same
content. `ExternalDropZone` absorbs the HTML5 boilerplate (the
`preventDefault` dance, enter/leave depth counting so hovering a child does
not flicker the hover state) and decodes that bag into `ExternalPayload`
values:

- `Url` from `text/uri-list`: links dragged from the URL bar, bookmarks,
  other tabs. One URL per line; `#` lines are comments and are skipped.
- `Html` from `text/html`: rich content, such as a selection dragged off a
  page.
- `Text` from `text/plain`.

The decoded list is ordered most specific first. No provider wraps any of
this, and there is no `accepts` filter, because the browser hides the data
while a drag hovers (`DataTransfer` protected mode): what actually arrived
can only be judged at drop time.

In-app drags never reach these zones. The core `Draggable` moves on pointer
events, not the native drag protocol, so an `ExternalDropZone` only ever
sees drags that crossed the app boundary - or came from an
`ExternalDragSource`, which is native by design (see
[Dragging out](drag-out.md)).

## A complete example

A reading list that accepts links dragged in from anywhere:

```rust,ignore
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[component]
fn ReadingList() -> Element {
    let mut links = use_signal(Vec::<String>::new);
    rsx! {
        ExternalDropZone {
            class: "rounded-xl border-2 border-dashed p-6 data-over:border-blue-500",
            on_drop: move |d: ExternalDrop| {
                if let Some(url) = d.url() {
                    links.write().push(url.to_string());
                }
            },
            "Drop a link here"
        }
        for url in links() {
            div { "{url}" }
        }
    }
}
```

While any drag hovers, the wrapper div carries `data-over="true"` (absent
otherwise), so highlighting is a CSS selector away. `on_hover` fires with
`true`/`false` on enter and leave when you need the signal in Rust.

## Reading the drop

`ExternalDrop` hands you everything the source offered: `payloads` (all
representations, most specific first), `files` (external drags can carry
those too), and `client`/`element` drop coordinates. Three helpers cover
the common questions:

- `best()` is the most specific payload, whatever it is.
- `url()` is the first URL, already parsed out of the `text/uri-list`.
- `text()` is the first plain-text payload.

A drop that decodes to nothing (no known formats, no files) is swallowed:
`on_drop` never fires for it. If files are your actual subject, prefer
`FileDropZone` from [File drops](file-drops.md), which adds filtering; the
`files` field here exists so a mixed drag loses nothing.

## Every payload is hostile

External payloads are attacker-controlled input, full stop.

- `ExternalPayload::Html` is arbitrary markup. Rendering it raw via
  `dangerous_inner_html` is stored or reflected XSS; sanitize first.
- `ExternalPayload::Url` may use any scheme, including `javascript:` and
  `data:`. Scheme-check before navigating to it or building an anchor
  from it.

The outbound direction guards this for you (generated anchors escape and
refuse dangerous schemes); inbound, the judgment is yours.

## Typed drops between your own apps

With the `serde` feature the boundary also speaks types. `TypedDropZone<T>`
decodes drops whose `DataTransfer` carries JSON under
`external::typed::MIME` (`application/json`) into a `TypedDrop<T>`. The
sending side is `TypedDragSource` from [Dragging out](drag-out.md), which
serializes a `Serialize` payload at drag start. Together they are the wire
between two separate Dioxus apps: two browser tabs running different
deployments, a web app and a desktop app, yours and someone else's.

Delivery is honest about the three cases: a drag with no typed entry at
all is silently ignored (it was never a typed drag), a typed entry that
decodes as `T` fires `on_drop`, and a typed entry whose JSON fails to
decode fires `on_invalid` with the error. The raw `external::typed::store`
and `retrieve` functions are public for custom handlers, and the format is
wire-compatible with dioxus-html's own `DataTransfer::store`/`retrieve`
helpers.

Between windows of one app, do not reach for this. A `DndWorld` carries
live Rust payloads through shared state with no serialization and no
`Serialize` bound; see [Multi-window drags](multi-window.md). The typed
transport exists precisely for the case `DndWorld` cannot reach: two
processes that share no memory.

## Gotchas

- **Hover cannot see the data.** `DataTransfer` protected mode hides
  content until drop, and Dioxus exposes no `types` list, so `data-over`
  lights for any drag hovering the zone - including drags a `TypedDropZone`
  will end up ignoring. Acceptance happens at drop time only.
- **Absent formats read as empty strings.** The DOM's `getData` returns
  `""` rather than null for formats a drag does not carry. `classify` and
  `typed::retrieve` both guard against this; treat the empty string as
  "not present" in any custom handler too.
- **Windows desktop trades file drops for `DataTransfer`.** wry's drop
  handler and HTML5 drag-and-drop are mutually exclusive per window, so a
  desktop window using the typed transport there gives up native file
  drops. See the platform notes in the crate README.
- **`serde` gates the typed half only.** `ExternalDropZone`,
  `ExternalDrop`, `ExternalPayload` and `classify` are always available;
  `TypedDropZone`, `TypedDrop` and `external::typed` need
  `features = ["serde"]`.

## Related

- [Dragging out](drag-out.md): the mirror direction, including the typed
  sending side.
- [File drops](file-drops.md): OS file drops with filtering.
- [Multi-window drags](multi-window.md): live payloads between windows of
  one app, no serialization.

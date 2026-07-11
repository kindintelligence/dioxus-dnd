# Drag-out API reference

Drag *out* of your app, the mirror of the `external` module: wrap content
in `ExternalDragSource` and users can drag it into other browser tabs, the
URL bar, text editors, or any application that accepts the standard
`DataTransfer` formats.

Concept guide: [docs/concepts/drag-out.md](../concepts/drag-out.md).
No provider needed - the browser is the transport here, not the shared
context.

```rust,ignore
ExternalDragSource {
    content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
    "Drag this link to another tab"
}
```

## `ExternalDragSource`

Makes its children draggable out of the app, populating the native
`DataTransfer` on drag start. Renders a wrapper `div` with the native
`draggable` attribute and forwards arbitrary attributes (`class`, `style`,
`id`, ...) to it.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `content` | `OutboundContent` | required | The content written to the drag's `DataTransfer`. |
| `effect` | `DropEffect` | `Copy` | Effect advertised to the receiving application (`effectAllowed`). `Copy` is what outbound drags almost always mean. |
| `disabled` | `bool` | `false` | Disable without unmounting: the wrapper renders `draggable: false` and drag start becomes a no-op. |

On drag start the source stops the event's propagation (a source nested in
another element's drag handlers owns the drag), writes each `(format,
data)` entry of `content` in order, and sets the allowed effect. Each
write is best-effort; a format the platform rejects does not abort the
rest.

Being a native drag, the drag image is the browser's rendering of the
element. The in-app `DragOverlay` reads the shared context and does not
apply here.

## `OutboundContent`

What to place on the outbound `DataTransfer`:

| Variant | Formats written, in order |
|---|---|
| `Text(String)` | `text/plain`. |
| `Url { url, title: Option<String> }` | `text/uri-list` and `text/plain`, both carrying the raw URL verbatim. With a title, also `text/html` as an anchor (see below). |
| `Html { html, fallback_text }` | `text/html`, then the fallback as `text/plain` for targets that don't take HTML. |
| `Custom(Vec<(String, String)>)` | The raw `(format, data)` pairs, verbatim, in order. No escaping is applied. |

Methods:

| Method | What it does |
|---|---|
| `url(url, title: Option<&str>)` | Convenience constructor for `OutboundContent::Url`. |
| `entries()` | The `(format, data)` pairs this content writes, in order. Pure, for testability. |

The generated `Url` anchor is the one injection surface, and it is
guarded: the URL is escaped as an attribute value and the title as text
content, and for dangerous schemes (`javascript:`, `data:`, `vbscript:`)
the `href` is omitted entirely, so a hostile URL can't carry an active
link into the target. Scheme matching is lenient the way browsers are:
leading ASCII whitespace and control characters are ignored and the
comparison is case-insensitive. The plain-text formats are not markup, so
the raw URL still travels verbatim there.

## `TypedDragSource` (`serde` feature)

Makes its children draggable with a **typed** payload on the native
`DataTransfer`: `payload` (`T: Serialize + Clone + PartialEq + 'static`)
is serialized to JSON under `external::typed::MIME` at drag start, always
alongside a `text/plain` fallback so non-typed targets (text editors,
other apps) still receive something legible. The receiving end is
`TypedDropZone` in [docs/api/external-content.md](external-content.md) -
yours, in another app - or any consumer of dioxus-html's wire-compatible
`retrieve`. For drags between windows of ONE app, prefer a `DndWorld`:
live Rust payloads, no serialization (see
[docs/api/multi-window.md](multi-window.md)).

| Prop | Type | Default | What it does |
|---|---|---|---|
| `payload` | `T` | required | The value serialized onto the drag's `DataTransfer`. |
| `fallback_text` | `Option<String>` | `None` | The `text/plain` fallback written alongside the JSON. Defaults to the JSON itself, which pastes legibly into text targets. |
| `effect` | `DropEffect` | `Copy` | Effect advertised to the receiving application. |
| `disabled` | `bool` | `false` | Disable without unmounting. |
| `on_error` | `Option<EventHandler<String>>` | `None` | Fired when the payload fails to serialize at drag start. |

Serialization failures are contained: the drag degrades to carrying only
the fallback text (when one was given explicitly) and reports through
`on_error` if wired. Like `ExternalDragSource`, the wrapper is a `div`
with forwarded attributes, and drag start stops propagation.

## Where the rest lives

The inbound mirror (`ExternalDropZone`, `classify`, `TypedDropZone`, the
`external::typed` store/retrieve functions and the `MIME` constant):
[docs/api/external-content.md](external-content.md). The `DropEffect`
vocabulary: [docs/api/drop-effects.md](drop-effects.md). `DndWorld` and
cross-window drags within one app:
[docs/api/multi-window.md](multi-window.md).

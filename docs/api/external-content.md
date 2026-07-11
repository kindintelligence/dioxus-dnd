# External content API reference

Drops arriving from outside your app - selected text, links dragged from
another tab, content from other applications - decoded from the native
`DataTransfer`, plus typed serde payloads over the same bridge for interop
scenarios the Rust-side context can't reach.

Concept guide:
[docs/concepts/external-content.md](../concepts/external-content.md).
For drags between elements of your own app, prefer the core context: it
carries any `Clone` type with zero serialization. Reach for this module
when the browser is the transport.

```rust,ignore
ExternalDropZone {
    on_drop: move |d: ExternalDrop| {
        if let Some(url) = d.url() { add_link(url); }
    },
    "Drop a link or text here"
}
```

## `ExternalDropZone`

A zone accepting drops that originate outside the app. Renders a wrapper
`div`, forwards arbitrary attributes (`class`, `style`, `id`, ...) to it,
and absorbs the HTML5 boilerplate: `preventDefault` on dragover and
dragenter, and enter/leave depth counting so hovering a child element does
not flicker the hover state.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `on_drop` | `EventHandler<ExternalDrop>` | required | Fired with the decoded drop. Not fired when the drop decodes to nothing (no known formats, no files). |
| `on_hover` | `Option<EventHandler<bool>>` | `None` | Fired with `true`/`false` on hover enter/leave. |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-over` | any drag hovers the zone; valued `"true"`, absent otherwise |

The attribute lights for any drag, not just acceptable ones: during hover
the browser hides the drag's data (`DataTransfer` protected mode), so
there is nothing to filter on until drop.

## `ExternalDrop`

A decoded external drop, delivered to `on_drop`:

| Field | Type | Meaning |
|---|---|---|
| `payloads` | `Vec<ExternalPayload>` | All representations the browser offered, most specific first. |
| `files` | `Vec<FileData>` | Files, if the drag carried any (also see the `files` module). |
| `client` | `Point` | Drop position in viewport coordinates. |
| `element` | `Point` | Drop position relative to the zone's element. |

Methods:

| Method | Returns | Meaning |
|---|---|---|
| `best()` | `Option<&ExternalPayload>` | The most specific text-ish payload, if any. |
| `url()` | `Option<&str>` | The first URL payload, parsed out of `text/uri-list`. |
| `text()` | `Option<&str>` | The first plain-text payload. |

## `ExternalPayload`

Content the browser handed us from an external drag, best-effort decoded
in order of specificity:

| Variant | Source format | Meaning |
|---|---|---|
| `Url(String)` | `text/uri-list` | Links dragged from the URL bar, bookmarks, other tabs. One variant per URL line; `#` comment lines are skipped. |
| `Html(String)` | `text/html` | Rich content, e.g. a selection dragged from a page. |
| `Text(String)` | `text/plain` | Plain text. |

**Untrusted input.** These payloads come from outside your app and are
fully attacker-controlled. `Html` is arbitrary markup - sanitize before
rendering via `dangerous_inner_html` (raw insertion is stored or reflected
XSS). `Url` may carry a `javascript:` or `data:` scheme - scheme-check
before navigating to it or building an anchor from it.

## `classify`

`classify(evt: &DragEvent) -> Vec<ExternalPayload>` decodes an incoming
drag event's `DataTransfer` into `ExternalPayload`s: every non-empty,
non-comment line of `text/uri-list` as a `Url`, then `text/html`, then
`text/plain`, skipping empty entries (the DOM's `getData` returns `""`
rather than null for absent formats). Public for custom native drop
handlers that want the same decoding.

## Typed transport (`serde` feature)

Everything below requires `features = ["serde"]`. It carries typed
payloads over the native `DataTransfer` - JSON encoded under
`typed::MIME` - for when the browser must carry the data, e.g. dragging
between two separate Dioxus apps. Between windows of ONE app, prefer a
`DndWorld`: live Rust payloads, no serialization (see
[docs/api/multi-window.md](multi-window.md)). The sending side is
`TypedDragSource` in [docs/api/drag-out.md](drag-out.md).

## `TypedDropZone`

A zone accepting typed drags: JSON under `typed::MIME` decoded to `T`
(`T: DeserializeOwned + Clone + PartialEq + 'static`) and delivered as a
`TypedDrop<T>`. Handles the same HTML5 boilerplate as `ExternalDropZone`
(prevented defaults, depth counting, wrapper `div` with forwarded
attributes).

| Prop | Type | Default | What it does |
|---|---|---|---|
| `on_drop` | `EventHandler<TypedDrop<T>>` | required | Fired with the decoded payload on a successful typed drop. |
| `on_invalid` | `Option<EventHandler<String>>` | `None` | Fired when a drop carried a `typed::MIME` entry that failed to decode as `T` (the decode error, for diagnostics). |
| `on_hover` | `Option<EventHandler<bool>>` | `None` | Fired with `true`/`false` on hover enter/leave. |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-over` | any drag hovers the zone; valued `"true"`, absent otherwise |

One honest limitation, spec-imposed: during hover the payload is
unreadable (`DataTransfer` protected mode) and Dioxus exposes no `types`
list, so `data-over` lights for ANY drag hovering the zone - acceptance
can only be judged at drop time. At drop, the three cases are handled
distinctly: drags with no typed entry at all are ignored silently (not a
typed drag, not ours), a decodable entry fires `on_drop`, and an entry
whose JSON fails to decode as `T` fires `on_invalid`.

## `TypedDrop`

A successfully decoded typed drop:

| Field | Type | Meaning |
|---|---|---|
| `payload` | `T` | The decoded payload. It crossed an app boundary and is untrusted input - validate it like any other. |
| `client` | `Point` | Drop position in viewport coordinates. |
| `element` | `Point` | Drop position relative to the zone's element. |

## `external::typed`

The functions under the component wrappers, public for custom handlers.

| Item | Signature | What it does |
|---|---|---|
| `MIME` | `&str` | `"application/json"`, the single format typed payloads travel under. Hardcoded so the wire stays compatible with dioxus-html's own `DataTransfer::store`/`retrieve` helpers. |
| `store` | `fn(&DragEvent, &T) -> Result<(), String>` where `T: Serialize` | Serialize `value` to JSON and store it on the drag's `DataTransfer`. Call in `ondragstart`. |
| `retrieve` | `fn(&DragEvent) -> Result<Option<T>, String>` where `T: DeserializeOwned` | Read and decode the typed payload from a drop. Call in `ondrop`. |
| `store_in` | `fn(&DataTransfer, &T) -> Result<(), String>` | `store` against a `DataTransfer` directly; the building block and the testable seam. |
| `retrieve_from` | `fn(&DataTransfer) -> Result<Option<T>, String>` | `retrieve` against a `DataTransfer` directly. |

`retrieve` semantics: `Ok(None)` when the drag carries no `MIME` entry
(not a typed drag); `Err` when it does but the JSON doesn't decode as `T`.
"No entry" includes the empty string: the DOM's `getData` returns `""` for
absent formats rather than null, so on web every untyped drag reads as
`Some("")` - the same reality `classify` guards against.

## Where the rest lives

The outbound mirror (`ExternalDragSource`, `OutboundContent`,
`TypedDragSource`): [docs/api/drag-out.md](drag-out.md). OS file drops
with filtering: [docs/api/file-drops.md](file-drops.md). `DndWorld` for
windows of one app: [docs/api/multi-window.md](multi-window.md). `Point`
and the `client_point`/`element_point` helpers these zones use:
[docs/api/core.md](core.md).

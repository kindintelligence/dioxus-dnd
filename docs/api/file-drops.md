# File drops API reference

OS file drops and click-to-choose uploads - native boundary payloads that
arrive *in their event* (`evt.files()`) rather than through the shared
context: `FileDropZone`, the declarative `FileFilter`, its `FileRejection`
reasons, and the `FileDrop` batch delivered on success.

Concept guide: [docs/concepts/file-drops.md](../concepts/file-drops.md).
Works on web and desktop; on desktop `FileData::path` gives you the real
filesystem path, on web you read contents with `read_bytes()` /
`read_string()` / `byte_stream()`.

```rust,ignore
rsx! {
    FileDropZone {
        filter: FileFilter::new().extensions(["png", "jpg"]).max_size(5_000_000),
        on_files: move |drop: FileDrop| async move {
            for f in drop.files {
                let bytes = f.read_bytes().await.unwrap();
                // ...
            }
        },
        "Click to choose images or drop them here"
    }
}
```

## `FileDropZone`

A zone that accepts files dragged in from the operating system or selected
from the native file picker opened by clicking it. Independent of
`DndContext` - these files don't come from inside your app, so no provider is
required. Renders a wrapper `div`, forwards arbitrary attributes (`class`,
`style`, `id`, ...) to it, and adds no visual styles of its own.

| Prop | Type | Default | What it does |
|---|---|---|---|
| `filter` | `Option<FileFilter>` | `None` | Acceptance rules applied to dropped and selected files; everything is accepted when omitted. |
| `on_files` | `EventHandler<FileDrop>` | required | Fired with accepted dropped or selected files, only if at least one passed. |
| `on_rejected` | `Option<EventHandler<Vec<(FileData, FileRejection)>>>` | `None` | Fired with rejected dropped or selected files paired with their reasons, only if at least one failed. |
| `on_hover` | `Option<EventHandler<bool>>` | `None` | Fired with `true` when a drag enters the zone, `false` when it leaves or the drop lands. |

Data attributes:

| Attribute | Present while |
|---|---|
| `data-over` | an OS drag hovers the zone; valued `"true"`, absent otherwise |

Unlike the context-backed attributes on `DropZone`, `data-over` here
reflects real browser drag events from outside the app: an in-app pointer
drag never sets it. An enter/leave depth counter keeps it stable while the
drag crosses the zone's children.

Behavior notes:

- Clicking the wrapper opens a native multi-file picker. A hidden file input
  provides this behavior without adding layout or visual styling.
- Extension rules plus exact and top-level-wildcard MIME rules are mirrored
  to the input's advisory `accept` value. The complete `FileFilter` always
  runs after selection because native pickers cannot express every rule.
- Picker selection uses the same callbacks as dropping. It supplies `(0, 0)`
  for both points because no drop location exists. Cancelling is silent.
- The zone calls `prevent_default()` on `dragover`; without that the
  browser never delivers the drop and opens the file instead. You write no
  ceremony yourself.
- A drop that delivers no files (dragged text, for example) fires neither
  callback.
- Handlers can be async: `on_files` can await `read_bytes()` per file
  directly; Dioxus spawns the future.

## `FileFilter`

Declarative acceptance rules for dropped or picker-selected files. A builder: start from
`FileFilter::new()` (or `Default`), chain rules; a file must pass every
rule you set, and an empty filter accepts everything. `Clone`, `Debug`,
`PartialEq`.

| Method | Rule | Rejection |
|---|---|---|
| `extensions(iter)` | The file name must end in one of these extensions. Case-insensitive (ASCII), leading dot optional, whitespace trimmed; entries that normalize to empty are dropped. The extension must terminate the name: `png.txt` does not pass `["png"]`. | `Extension` |
| `content_types(iter)` | The reported MIME type must match one of these patterns (forms below). | `ContentType` |
| `max_size(bytes)` | Reject files larger than this many bytes; a file of exactly `bytes` passes. | `TooLarge` |
| `max_files(n)` | Accept at most `n` files per incoming batch; applied by `partition`, ignored by `check`. | `TooMany` |

`content_types` pattern forms:

| Pattern | Matches |
|---|---|
| `application/pdf` | exactly that type |
| `image/*` | any subtype under `image` |
| `*/*` | any file that reports a well-formed `type/subtype` |
| `application/*+json` | any `application` subtype carrying the `+json` structured suffix (`application/ld+json`, `application/vnd.api+json`), not `application/json` itself |
| `*/*+json` | the `+json` suffix under any top-level type |

Matching normalizes both sides: ASCII case is ignored and MIME parameters
(`; charset=utf-8`) are stripped. Wildcards match whole slash-delimited
parts, so `imageevil/png` never matches `image/*`. A subtype-only wildcard
without a structured suffix (`*/json`) is not supported and matches
nothing, as do malformed patterns (`image`, `image/`, `/png`). A file that
reports no content type fails every `content_types`-restricted filter,
including `*/*`.

Checking, directly usable outside the component:

- `check(&FileData) -> Result<(), FileRejection>` runs the per-file rules
  in order - extension, then content type, then size - and reports the
  first failure. It ignores `max_files`.
- `partition(Vec<FileData>) -> (Vec<FileData>, Vec<(FileData, FileRejection)>)`
  splits a batch applying every rule, preserving order. `max_files` counts
  accepted files only: a file rejected on another rule does not consume a
  slot, so valid files behind it still land.

**Advisory, not a security boundary.** These rules match on the browser-
and OS-reported name, content type and size, all of which are
attacker-controllable: a `.exe` can be renamed `photo.png` and report
`content_type: "image/png"`, and `size` is self-reported. Use the filter
for UX (rejecting obviously wrong drops early), but validate the actual
bytes server-side or via content sniffing before trusting a file.

## `FileDrop`

A batch of dropped or selected files delivered to `on_files`:

| Field | Type | Meaning |
|---|---|---|
| `files` | `Vec<FileData>` | The accepted files. `FileData` is Dioxus's platform file handle (`dioxus::html::FileData`), not a type of this crate. |
| `client` | `Point` | Pointer position in client (viewport) coordinates at drop time; `(0, 0)` for picker selections. |
| `element` | `Point` | Pointer position relative to the drop zone element; `(0, 0)` for picker selections. |

On every renderer `FileData` exposes `name()`, `size()`, `content_type()`
and `last_modified()`. Contents differ: web reads them with
`read_bytes()`, `read_string()` or `byte_stream()`; desktop additionally
has the real filesystem path via `path()`.

## `FileRejection`

Why a file was rejected by a `FileFilter`, delivered alongside the file in
`on_rejected`:

| Variant | Meaning |
|---|---|
| `Extension` | Name does not end in an allowed extension. |
| `ContentType` | Reported MIME type matched no allowed pattern; files reporting no type also land here. |
| `TooLarge` | Larger than `max_size` bytes. |
| `TooMany` | Arrived after the batch already held `max_files` accepted files. |

Non-exhaustive: new acceptance rules mean new rejection reasons, so keep a
wildcard arm with a generic "not accepted" message.

## Platform notes

- **Web** reads contents through the async `FileData` methods; there are
  no filesystem paths.
- **Desktop** exposes `path()`; hand it to `std::fs` or another process
  without copying bytes through the webview.
- **Windows desktop file drops** have a history of platform quirks in
  wry-based webviews. Test on your target and use the same zone's picker when
  OS drops are unreliable. wry also makes its drop handler and HTML5
  drag-and-drop mutually exclusive per window, so a Windows window using the
  typed `DataTransfer` transport gives up native file drops but can still use
  click-to-choose.

## Where the rest lives

`Point` and the `client_point` / `element_point` helpers that build one
from a native `DragEvent` (for custom native zones):
[docs/api/core.md](core.md). Text, links and HTML dropped in from other
apps: [docs/api/external-content.md](external-content.md).

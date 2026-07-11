# File drops

Files dragged in from the operating system: the desktop, a file manager,
another app's window. The payload cannot travel through Rust context here,
because the drag starts outside your app; it arrives inside the browser's
own drag event. `FileDropZone` handles the native ceremony, filters what
arrives, and hands your handler real files.

API reference: [api/file-drops.md](../api/file-drops.md).
Live demo: the
[Upload](https://kindintelligence.github.io/dioxus-dnd/upload) page.

## The mental model

Every other zone in the crate reads the shared drag context. `FileDropZone`
reads nothing from it, which changes three things:

- **No provider, no payload type.** Drop the component anywhere; it needs
  no `DndProvider`. The "payload" is `Vec<FileData>`, Dioxus's platform
  file handle, pulled straight from the native event (`evt.files()`).
- **`data-over` reflects browser events.** Context-backed zones light up
  for pointer, touch and keyboard drags alike; this zone lights up for real
  OS drag events only. An in-app pointer drag can never trigger it, and an
  OS drag can never trigger an in-app `DropZone`.
- **Two callbacks split every drop.** Files that pass the filter reach
  `on_files` as one `FileDrop` batch; files that fail reach `on_rejected`,
  each paired with a `FileRejection` naming the reason. Honest feedback
  costs one match statement.

## A complete example

```rust,ignore
use dioxus::html::FileData;
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

#[component]
fn Uploader() -> Element {
    rsx! {
        FileDropZone {
            filter: FileFilter::new()
                .content_types(["image/*"])
                .max_size(5_000_000)
                .max_files(6),
            on_files: move |drop: FileDrop| async move {
                for f in drop.files {
                    let bytes = f.read_bytes().await.unwrap(); // web
                    // or: let path = f.path();                // desktop
                }
            },
            on_rejected: move |bad: Vec<(FileData, FileRejection)>| {
                // show each f.name() with its reason
            },
            class: "rounded-xl border-2 border-dashed p-8 data-over:border-blue-500",
            "Drop images here"
        }
    }
}
```

The handler can be async: Dioxus spawns the future for you, so `on_files`
awaits `read_bytes` for each file directly, with no separate effect system
to route reads through.

## Filtering

`FileFilter` is a builder with four rules; a file must pass all of them:

- `extensions(["png", "jpg"])` allow-lists by file name ending.
  Case-insensitive, leading dot optional, whitespace trimmed, and the
  extension must terminate the name: `png.txt` does not pass a `png`
  filter.
- `content_types(["image/*"])` allow-lists by the reported MIME type.
  Patterns cover exact types (`application/pdf`), top-level wildcards
  (`image/*`), all typed files (`*/*`), and structured suffix wildcards
  (`application/*+json`, `*/*+json`). Matching ignores case and MIME
  parameters such as `; charset=utf-8`, and wildcards match whole
  slash-delimited parts, so `imageevil/png` never sneaks past `image/*`.
- `max_size(bytes)` rejects files larger than the cap.
- `max_files(n)` accepts at most `n` files per drop.

Rejections do not consume slots: a file bounced on type does not count
against `max_files`, so valid files behind it still land. An omitted
`filter` prop, or an empty `FileFilter::new()`, accepts everything.

## A UX affordance, not a security boundary

The filter matches on the browser- and OS-reported name, content type and
size, all of which are attacker-controllable. A renamed executable can be
called `photo.png` and report `image/png`, and the size is self-reported.
Use the filter to bounce obviously wrong drops early with a clear reason;
validate the actual bytes server-side, or by content sniffing, before
trusting a file.

## Reading what landed

`FileDrop.files` holds `FileData` values, Dioxus's platform file handle.
Metadata (`name()`, `size()`, `content_type()`, `last_modified()`) is
available everywhere; the contents differ by renderer:

- **Web** has no filesystem paths. Read contents with `read_bytes()`,
  `read_string()` or the chunked `byte_stream()`.
- **Desktop** exposes the real path through `path()`, so you can hand the
  file to `std::fs` or another process without copying its bytes through
  the webview.

The same handler compiles for both; the `FileDrop` also carries `client`
and `element` drop coordinates when placement matters.

## Styling the zone

While an OS drag hovers, the wrapper div carries `data-over="true"` and
drops it otherwise, so the classic highlight needs no handler: Tailwind
`data-over:border-blue-500`, plain CSS `[data-over]`. An enter/leave depth
counter keeps the attribute stable while the drag crosses the zone's
children, so nested markup does not make it flicker. When styling needs
real state, `on_hover` fires `true` on enter and `false` on leave or drop.

The zone is fully headless: borders, tints and result chips in the demo
are all page styling over `data-over` and the two callbacks.

## Gotchas

- **A missing MIME type fails any type filter.** A file that reports no
  content type is rejected by every `content_types` filter, including
  `*/*`. Filter by extension instead when your sources report types
  unreliably.
- **Suffix wildcards do not cover the bare type.** `application/*+json`
  matches `application/ld+json` but not `application/json`; list both when
  you want both. Subtype wildcards without a suffix (`*/json`) are not
  supported and match nothing.
- **Empty drops are silent.** A drag that delivers no files (dragged text,
  for example) fires neither callback. Text and links from other apps are
  a different concept: [External content in](external-content.md).
- **Windows desktop file drops have webview quirks.** wry-based webviews
  have a history of platform issues here; test on your target and consider
  a file input fallback. wry also makes its drop handler and HTML5
  drag-and-drop mutually exclusive per window, so a Windows window using
  the typed `DataTransfer` transport gives up native file drops.

## Related

- [External content in](external-content.md): the other inbound native
  boundary, for text, links and HTML from other apps.
- [Drag and drop](drag-and-drop.md): the in-app context machinery this
  zone deliberately does not use.
- [Styling](styling.md): the full data-attribute contract.

//! Drops arriving from *outside* your app - selected text, links dragged from
//! another tab, content from other applications - plus typed serde payloads
//! over the `DataTransfer` bridge for interop scenarios the Rust-side
//! context can't reach.
//!
//! For drags between elements of your own app, prefer the core context: it
//! carries any `Clone` type with zero serialization. Reach for this module
//! when the *browser* is the transport.

use dioxus::html::HasFileData;
use dioxus::prelude::*;

use crate::core::{client_point, element_point, Point};

/// Content the browser handed us from an external drag, best-effort decoded
/// in order of specificity.
///
/// **Untrusted input.** These payloads come from outside your app and are
/// fully attacker-controlled. Treat them like any other external data:
/// - [`ExternalPayload::Html`] is arbitrary markup - sanitize it before
///   rendering via `dangerous_inner_html` (raw insertion is stored/reflected
///   XSS).
/// - [`ExternalPayload::Url`] may carry a `javascript:` or `data:` scheme -
///   scheme-check before navigating to it or building an anchor from it.
#[derive(Debug, Clone, PartialEq)]
pub enum ExternalPayload {
    /// `text/uri-list` - links dragged from the URL bar, bookmarks, other tabs.
    /// May use any scheme; validate before use.
    Url(String),
    /// `text/html` - rich content (e.g. a selection dragged from a page).
    /// Arbitrary untrusted markup; sanitize before rendering.
    Html(String),
    /// `text/plain`.
    Text(String),
}

/// A decoded external drop.
#[derive(Clone, PartialEq)]
pub struct ExternalDrop {
    /// All representations the browser offered, most specific first.
    pub payloads: Vec<ExternalPayload>,
    /// Files, if the drag carried any (also see [`crate::files`]).
    pub files: Vec<dioxus::html::FileData>,
    pub client: Point,
    pub element: Point,
}

impl ExternalDrop {
    /// The most specific text-ish payload, if any.
    pub fn best(&self) -> Option<&ExternalPayload> {
        self.payloads.first()
    }

    /// First URL payload, parsed out of `text/uri-list` (one URL per line,
    /// `#` lines are comments).
    pub fn url(&self) -> Option<&str> {
        self.payloads.iter().find_map(|p| match p {
            ExternalPayload::Url(u) => Some(u.as_str()),
            _ => None,
        })
    }

    /// First plain-text payload.
    pub fn text(&self) -> Option<&str> {
        self.payloads.iter().find_map(|p| match p {
            ExternalPayload::Text(t) => Some(t.as_str()),
            _ => None,
        })
    }
}

/// Decode an incoming drag event's `DataTransfer` into [`ExternalPayload`]s.
pub fn classify(evt: &DragEvent) -> Vec<ExternalPayload> {
    let dt = evt.data_transfer();
    let mut out = Vec::new();
    if let Some(uris) = dt.get_data("text/uri-list") {
        for line in uris.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                out.push(ExternalPayload::Url(line.to_string()));
            }
        }
    }
    if let Some(html) = dt.get_data("text/html") {
        if !html.is_empty() {
            out.push(ExternalPayload::Html(html));
        }
    }
    if let Some(text) = dt.get_data("text/plain") {
        if !text.is_empty() {
            out.push(ExternalPayload::Text(text));
        }
    }
    out
}

/// A zone accepting drops that originate outside the app.
///
/// While a drag hovers the zone the div carries `data-over="true"` (absent
/// otherwise) for styling without `on_hover` wiring.
#[component]
pub fn ExternalDropZone(
    on_drop: EventHandler<ExternalDrop>,
    /// Fired with `true`/`false` on hover enter/leave.
    #[props(default)]
    on_hover: Option<EventHandler<bool>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut depth = use_signal(|| 0u32);

    rsx! {
        div {
            "data-over": if depth() > 0 { "true" },
            ondragover: move |evt: DragEvent| {
                evt.prevent_default();
            },
            ondragenter: move |evt: DragEvent| {
                evt.prevent_default();
                let d = depth() + 1;
                depth.set(d);
                if d == 1 {
                    if let Some(h) = &on_hover {
                        h.call(true);
                    }
                }
            },
            ondragleave: move |_| {
                let d = depth().saturating_sub(1);
                depth.set(d);
                if d == 0 {
                    if let Some(h) = &on_hover {
                        h.call(false);
                    }
                }
            },
            ondrop: move |evt: DragEvent| {
                evt.prevent_default();
                depth.set(0);
                if let Some(h) = &on_hover {
                    h.call(false);
                }
                let payloads = classify(&evt);
                let files = evt.files();
                if payloads.is_empty() && files.is_empty() {
                    return;
                }
                on_drop.call(ExternalDrop {
                    payloads,
                    files,
                    client: client_point(&evt),
                    element: element_point(&evt),
                });
            },
            ..attributes,
            {children}
        }
    }
}

/// Typed payloads over the native `DataTransfer` (JSON-encoded under
/// [`typed::MIME`], wire-compatible with dioxus-html's own
/// `store`/`retrieve`). Useful when the browser must carry the data - e.g.
/// dragging between two separate Dioxus apps - at the cost of requiring
/// `Serialize`/`Deserialize`. (Between windows of ONE app, prefer a
/// [`crate::core::DndWorld`]: live Rust payloads, no serialization.)
///
/// Component wrappers: [`TypedDropZone`] here and
/// [`crate::dragout::TypedDragSource`] on the outbound side.
#[cfg(feature = "serde")]
pub mod typed {
    use dioxus::html::DataTransfer;
    use dioxus::prelude::*;

    /// The format typed payloads travel under. A single hardcoded MIME
    /// keeps the wire format compatible with dioxus-html's own
    /// `DataTransfer::store`/`retrieve` helpers.
    pub const MIME: &str = "application/json";

    /// Store a typed payload on a `DataTransfer` directly. The building
    /// block behind [`store()`]; also the testable seam.
    pub fn store_in<T: serde::Serialize>(dt: &DataTransfer, value: &T) -> Result<(), String> {
        let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
        dt.set_data(MIME, &json)
    }

    /// Retrieve a typed payload from a `DataTransfer` directly.
    /// `Ok(None)` when the drag carries no [`MIME`] entry (not a typed
    /// drag); `Err` when it does but the JSON doesn't decode as `T`.
    /// "No entry" includes the empty string: the DOM's `getData` returns
    /// `""` for absent formats rather than null, so on web every untyped
    /// drag reads as `Some("")` (the same reality [`super::classify`]
    /// guards against).
    pub fn retrieve_from<T: for<'de> serde::Deserialize<'de>>(
        dt: &DataTransfer,
    ) -> Result<Option<T>, String> {
        match dt.get_data(MIME) {
            Some(json) if !json.trim().is_empty() => serde_json::from_str(&json)
                .map(Some)
                .map_err(|e| e.to_string()),
            _ => Ok(None),
        }
    }

    /// Store a typed payload on the drag's `DataTransfer`. Call in `ondragstart`.
    pub fn store<T: serde::Serialize>(evt: &DragEvent, value: &T) -> Result<(), String> {
        store_in(&evt.data_transfer(), value)
    }

    /// Retrieve a typed payload from a drop's `DataTransfer`. Call in `ondrop`.
    pub fn retrieve<T: for<'de> serde::Deserialize<'de>>(
        evt: &DragEvent,
    ) -> Result<Option<T>, String> {
        retrieve_from(&evt.data_transfer())
    }
}

/// A successfully decoded typed drop, as delivered by [`TypedDropZone`].
#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq)]
pub struct TypedDrop<T> {
    /// The decoded payload. Like every external payload it crossed an app
    /// boundary and is untrusted input - validate it like any other.
    pub payload: T,
    /// Pointer position in client (viewport) coordinates at drop time.
    pub client: Point,
    /// Pointer position relative to the zone's element.
    pub element: Point,
}

/// A zone accepting typed drags (see [`typed`]) - JSON under
/// [`typed::MIME`] decoded to `T` and delivered as a [`TypedDrop`].
///
/// Handles the HTML5 boilerplate like [`ExternalDropZone`]: `preventDefault`
/// on dragover, enter/leave depth counting, and `data-over="true"` while
/// hovered. One honest limitation, spec-imposed: during hover the payload
/// is unreadable (`DataTransfer` protected mode) and dioxus exposes no
/// `types` list, so `data-over` lights for ANY drag hovering the zone -
/// acceptance can only be judged at drop time. Drags with no typed entry
/// at all are ignored silently at drop; drags whose JSON fails to decode
/// as `T` fire `on_invalid` with the decode error.
#[cfg(feature = "serde")]
#[component]
pub fn TypedDropZone<T: serde::de::DeserializeOwned + Clone + PartialEq + 'static>(
    /// Fired with the decoded payload on a successful typed drop.
    on_drop: EventHandler<TypedDrop<T>>,
    /// Fired when a drop carried a [`typed::MIME`] entry that failed to
    /// decode as `T` (the decode error, for diagnostics).
    #[props(default)]
    on_invalid: Option<EventHandler<String>>,
    /// Fired with `true`/`false` on hover enter/leave.
    #[props(default)]
    on_hover: Option<EventHandler<bool>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut depth = use_signal(|| 0u32);

    rsx! {
        div {
            "data-over": if depth() > 0 { "true" },
            ondragover: move |evt: DragEvent| {
                evt.prevent_default();
            },
            ondragenter: move |evt: DragEvent| {
                evt.prevent_default();
                let d = depth() + 1;
                depth.set(d);
                if d == 1 {
                    if let Some(h) = &on_hover {
                        h.call(true);
                    }
                }
            },
            ondragleave: move |_| {
                let d = depth().saturating_sub(1);
                depth.set(d);
                if d == 0 {
                    if let Some(h) = &on_hover {
                        h.call(false);
                    }
                }
            },
            ondrop: move |evt: DragEvent| {
                evt.prevent_default();
                depth.set(0);
                if let Some(h) = &on_hover {
                    h.call(false);
                }
                match typed::retrieve::<T>(&evt) {
                    Ok(Some(payload)) => on_drop.call(TypedDrop {
                        payload,
                        client: client_point(&evt),
                        element: element_point(&evt),
                    }),
                    // No typed entry: not a typed drag - not ours.
                    Ok(None) => {}
                    Err(e) => {
                        if let Some(h) = &on_invalid {
                            h.call(e);
                        }
                    }
                }
            },
            ..attributes,
            {children}
        }
    }
}

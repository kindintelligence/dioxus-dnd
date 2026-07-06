//! Drag *out* of your app - the mirror of [`crate::external`]. Wrap content
//! in [`ExternalDragSource`] and users can drag it into other browser tabs,
//! the URL bar, text editors, or any application that accepts the standard
//! `DataTransfer` formats.
//!
//! ```text
//! ExternalDragSource {
//!     content: OutboundContent::url("https://dioxuslabs.com", Some("Dioxus")),
//!     a { href: "https://dioxuslabs.com", "Dioxus" }
//! }
//! ```
//!
//! No provider needed - the browser is the transport here, not the shared
//! context. (For dragging typed Rust payloads between two of *your own*
//! windows, see `external::typed` with the `serde` feature.)

use dioxus::prelude::*;

use crate::core::DropEffect;

/// What to place on the outbound `DataTransfer`.
#[derive(Debug, Clone, PartialEq)]
pub enum OutboundContent {
    /// Plain text (`text/plain`).
    Text(String),
    /// A link: written as `text/uri-list` *and* `text/plain` (and, with a
    /// title, `text/html` as an anchor) so maximal targets understand it.
    Url {
        url: String,
        /// Optional human title, used for the HTML representation.
        title: Option<String>,
    },
    /// Rich content: `text/html` plus a plain-text fallback.
    Html {
        html: String,
        /// Written as `text/plain` for targets that don't take HTML.
        fallback_text: String,
    },
    /// Raw `(format, data)` pairs, written verbatim in order.
    Custom(Vec<(String, String)>),
}

impl OutboundContent {
    /// Convenience constructor for [`OutboundContent::Url`].
    pub fn url(url: impl Into<String>, title: Option<&str>) -> Self {
        Self::Url {
            url: url.into(),
            title: title.map(str::to_string),
        }
    }

    /// The `(format, data)` pairs this content writes, in order. Pure, for
    /// testability.
    pub fn entries(&self) -> Vec<(String, String)> {
        match self {
            OutboundContent::Text(t) => vec![("text/plain".into(), t.clone())],
            OutboundContent::Url { url, title } => {
                let mut out = vec![
                    ("text/uri-list".into(), url.clone()),
                    ("text/plain".into(), url.clone()),
                ];
                if let Some(title) = title {
                    out.push((
                        "text/html".into(),
                        format!(r#"<a href="{url}">{title}</a>"#),
                    ));
                }
                out
            }
            OutboundContent::Html {
                html,
                fallback_text,
            } => vec![
                ("text/html".into(), html.clone()),
                ("text/plain".into(), fallback_text.clone()),
            ],
            OutboundContent::Custom(pairs) => pairs.clone(),
        }
    }
}

/// Makes its children draggable *out of the app*, populating the native
/// `DataTransfer` on drag start.
#[component]
pub fn ExternalDragSource(
    /// The content written to the drag's `DataTransfer`.
    content: OutboundContent,
    /// Effect advertised to the receiving application. Defaults to `Copy`,
    /// which is what outbound drags almost always mean.
    #[props(default = DropEffect::Copy)]
    effect: DropEffect,
    /// Disable without unmounting.
    #[props(default)]
    disabled: bool,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    rsx! {
        div {
            draggable: !disabled,
            ondragstart: move |evt: DragEvent| {
                if disabled {
                    return;
                }
                evt.stop_propagation();
                let dt = evt.data_transfer();
                for (format, data) in content.entries() {
                    let _ = dt.set_data(&format, &data);
                }
                dt.set_effect_allowed(effect.as_str());
            },
            ..attributes,
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_content_covers_all_formats() {
        let c = OutboundContent::url("https://example.com", Some("Example"));
        let e = c.entries();
        assert_eq!(e[0].0, "text/uri-list");
        assert_eq!(e[1], ("text/plain".into(), "https://example.com".into()));
        assert!(e[2].1.contains(r#"href="https://example.com""#));

        // no title → no html entry
        assert_eq!(OutboundContent::url("https://x.y", None).entries().len(), 2);
    }
}

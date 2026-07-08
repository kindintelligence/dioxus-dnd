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
                // `text/uri-list` / `text/plain` are plain-text formats, so the
                // raw url is written verbatim. Only the `text/html` anchor is an
                // injection surface: escape both fields for their context, and
                // omit the `href` for dangerous schemes (javascript:/data:/…)
                // so a hostile url can't carry an active link into the target.
                let mut out = vec![
                    ("text/uri-list".into(), url.clone()),
                    ("text/plain".into(), url.clone()),
                ];
                if let Some(title) = title {
                    let anchor = if is_safe_href(url) {
                        format!(
                            r#"<a href="{}">{}</a>"#,
                            escape_html_attr(url),
                            escape_html_text(title)
                        )
                    } else {
                        format!("<a>{}</a>", escape_html_text(title))
                    };
                    out.push(("text/html".into(), anchor));
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

/// Escape a string for use inside a double-quoted HTML attribute value.
fn escape_html_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Escape a string for use as HTML text content.
fn escape_html_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Is this url safe to place in an anchor `href`? Rejects the schemes that can
/// execute script when the dragged HTML lands in another app
/// (`javascript:`, `data:`, `vbscript:`), matching leniently: leading ASCII
/// whitespace and control characters are ignored and the scheme is
/// case-insensitive, mirroring how browsers resolve a url.
fn is_safe_href(url: &str) -> bool {
    let trimmed = url.trim_start_matches(|c: char| c.is_ascii_whitespace() || c.is_control());
    let lower = trimmed.to_ascii_lowercase();
    !["javascript:", "data:", "vbscript:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
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

    #[test]
    fn url_html_entry_escapes_attribute_and_text() {
        // A url with a query string (`&`) and a title with markup must not
        // break or inject into the generated anchor.
        let c = OutboundContent::url("https://x.y/?a=1&b=\"2\"", Some("A & B <img src=x>"));
        let html = &c.entries()[2].1;
        assert_eq!(
            html,
            r#"<a href="https://x.y/?a=1&amp;b=&quot;2&quot;">A &amp; B &lt;img src=x&gt;</a>"#
        );
        // Plain-text formats still carry the raw url.
        assert_eq!(c.entries()[1].1, "https://x.y/?a=1&b=\"2\"");
    }

    #[test]
    fn url_html_entry_drops_href_for_dangerous_schemes() {
        for bad in [
            "javascript:alert(1)",
            "  JavaScript:alert(1)",
            "data:text/html,<script>",
            "vbscript:msgbox",
        ] {
            let c = OutboundContent::url(bad, Some("click"));
            let html = &c.entries()[2].1;
            assert!(!html.contains("href="), "{bad} kept an href: {html}");
            assert_eq!(html, "<a>click</a>");
        }
        // Ordinary schemes keep the href.
        assert!(
            OutboundContent::url("mailto:a@b.c", Some("mail")).entries()[2]
                .1
                .contains("href=")
        );
    }
}

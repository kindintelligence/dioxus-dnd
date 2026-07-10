//! OS file drops - the one drop type where the payload arrives *in the native
//! event* (`evt.files()`) rather than through the shared context.
//!
//! Works on web and desktop. On desktop, [`dioxus::html::FileData::path`]
//! gives you the real filesystem path; on web you read contents with
//! `read_bytes()` / `read_string()` / `byte_stream()`.
//!
//! ```text
//! rsx! {
//!     FileDropZone {
//!         filter: FileFilter::new().extensions(["png", "jpg"]).max_size(5_000_000),
//!         on_files: move |drop: FileDrop| async move {
//!             for f in drop.files {
//!                 let bytes = f.read_bytes().await.unwrap();
//!                 // ...
//!             }
//!         },
//!         "Drop images here"
//!     }
//! }
//! ```

use dioxus::html::{FileData, HasFileData};
use dioxus::prelude::*;

use crate::core::{client_point, element_point, Point};

/// A batch of dropped files plus where they landed.
#[derive(Clone, PartialEq)]
pub struct FileDrop {
    pub files: Vec<FileData>,
    /// Pointer position in client (viewport) coordinates.
    pub client: Point,
    /// Pointer position relative to the drop zone element.
    pub element: Point,
}

/// Why a file was rejected by a [`FileFilter`].
///
/// Non-exhaustive: new acceptance rules mean new rejection reasons, so
/// keep a wildcard arm with a generic "not accepted" message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileRejection {
    /// Extension not in the allow-list.
    Extension,
    /// MIME type not in the allow-list.
    ContentType,
    /// Larger than `max_size` bytes.
    TooLarge,
    /// Batch exceeded `max_files`; this file was over the limit.
    TooMany,
}

/// Declarative acceptance rules for dropped files.
///
/// **Advisory, not a security boundary.** These rules match on the browser-
/// and OS-reported name, content type and size, all of which are
/// attacker-controllable: a `.exe` can be renamed `photo.png` and report
/// `content_type: "image/png"`, and `size` is self-reported. Use the filter
/// for UX (rejecting obviously wrong drops early), but validate the actual
/// bytes server-side or via content sniffing before trusting a file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileFilter {
    extensions: Vec<String>,
    content_types: Vec<String>,
    max_size: Option<u64>,
    max_files: Option<usize>,
}

impl FileFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allow only these extensions (case-insensitive, leading dot optional).
    pub fn extensions<I, S>(mut self, exts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.extensions = exts
            .into_iter()
            .map(|s| normalize_extension(&s.into()))
            .filter(|s| !s.is_empty())
            .collect();
        self
    }

    /// Allow only these MIME types.
    ///
    /// Supported patterns:
    ///
    /// - exact types: `"application/pdf"`
    /// - top-level wildcards: `"image/*"`
    /// - all typed files: `"*/*"`
    /// - structured suffix wildcards: `"application/*+json"` and `"*/*+json"`
    pub fn content_types<I, S>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.content_types = types
            .into_iter()
            .map(|s| normalize_content_type(&s.into()))
            .filter(|s| !s.is_empty())
            .collect();
        self
    }

    /// Reject files larger than this many bytes.
    pub fn max_size(mut self, bytes: u64) -> Self {
        self.max_size = Some(bytes);
        self
    }

    /// Accept at most this many files per drop.
    pub fn max_files(mut self, n: usize) -> Self {
        self.max_files = Some(n);
        self
    }

    /// Check a single file against the rules (ignores `max_files`).
    pub fn check(&self, file: &FileData) -> Result<(), FileRejection> {
        if !self.extensions.is_empty() {
            // ASCII-lowercase to match `normalize_extension`; a full Unicode
            // `to_lowercase()` here could case-fold a non-ASCII filename
            // differently from the (ASCII-lowered) extension and spuriously
            // mismatch. File extensions are ASCII in practice.
            let name = file.name().to_ascii_lowercase();
            let ok = self
                .extensions
                .iter()
                .any(|ext| name.ends_with(&format!(".{ext}")));
            if !ok {
                return Err(FileRejection::Extension);
            }
        }
        if !self.content_types.is_empty() {
            let ct = file
                .content_type()
                .map(|s| normalize_content_type(&s))
                .unwrap_or_default();
            let ok = self
                .content_types
                .iter()
                .any(|allowed| content_type_matches(allowed, &ct));
            if !ok {
                return Err(FileRejection::ContentType);
            }
        }
        if let Some(max) = self.max_size {
            if file.size() > max {
                return Err(FileRejection::TooLarge);
            }
        }
        Ok(())
    }

    /// Split a batch into `(accepted, rejected)` applying every rule.
    pub fn partition(
        &self,
        files: Vec<FileData>,
    ) -> (Vec<FileData>, Vec<(FileData, FileRejection)>) {
        let mut ok = Vec::new();
        let mut bad = Vec::new();
        for file in files {
            if let Some(max) = self.max_files {
                if ok.len() >= max {
                    bad.push((file, FileRejection::TooMany));
                    continue;
                }
            }
            match self.check(&file) {
                Ok(()) => ok.push(file),
                Err(why) => bad.push((file, why)),
            }
        }
        (ok, bad)
    }
}

fn normalize_extension(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn normalize_content_type(content_type: &str) -> String {
    content_type
        .split_once(';')
        .map(|(base, _)| base)
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}

fn split_content_type(content_type: &str) -> Option<(&str, &str)> {
    let (ty, subtype) = content_type.split_once('/')?;
    if ty.is_empty() || subtype.is_empty() || subtype.contains('/') {
        return None;
    }
    Some((ty, subtype))
}

fn content_type_matches(pattern: &str, content_type: &str) -> bool {
    let Some((pattern_type, pattern_subtype)) = split_content_type(pattern) else {
        return false;
    };
    let Some((actual_type, actual_subtype)) = split_content_type(content_type) else {
        return false;
    };

    if pattern_type == "*" && pattern_subtype == "*" {
        return true;
    }
    if pattern_type == "*" && !pattern_subtype.starts_with("*+") {
        return false;
    }
    if pattern_type != "*" && pattern_type != actual_type {
        return false;
    }
    if pattern_subtype == "*" {
        return true;
    }
    if let Some(suffix) = pattern_subtype.strip_prefix("*+") {
        return actual_subtype
            .rsplit_once('+')
            .map(|(_, actual_suffix)| actual_suffix == suffix)
            .unwrap_or(false);
    }

    pattern_subtype == actual_subtype
}

/// A zone that accepts files dragged in from the operating system.
///
/// Independent of `DndContext` - file drops don't come from inside your app,
/// so no provider is required.
///
/// While a drag hovers the zone the div carries `data-over="true"` (absent
/// otherwise), so the classic "highlight the dropzone" style needs no
/// `on_hover` wiring: Tailwind `data-over:border-blue-500`, CSS
/// `[data-over]`.
#[component]
pub fn FileDropZone(
    /// Acceptance rules; everything is accepted when omitted.
    #[props(default)]
    filter: Option<FileFilter>,
    /// Fired with the accepted files of a drop (only if at least one passed).
    on_files: EventHandler<FileDrop>,
    /// Fired with the rejected files of a drop, if any.
    #[props(default)]
    on_rejected: Option<EventHandler<Vec<(FileData, FileRejection)>>>,
    /// Fired with `true` when a drag hovers the zone, `false` when it leaves.
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
                // Required: without preventDefault the browser never delivers
                // the drop (it would open the file instead).
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
                let files = evt.files();
                if files.is_empty() {
                    return;
                }
                let (accepted, rejected) = match &filter {
                    Some(f) => f.partition(files),
                    None => (files, Vec::new()),
                };
                if !rejected.is_empty() {
                    if let Some(h) = &on_rejected {
                        h.call(rejected);
                    }
                }
                if !accepted.is_empty() {
                    on_files.call(FileDrop {
                        files: accepted,
                        client: client_point(&evt),
                        element: element_point(&evt),
                    });
                }
            },
            ..attributes,
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::html::NativeFileData;
    use std::path::PathBuf;
    use std::pin::Pin;

    /// Minimal test double for the platform file object.
    struct MockFile {
        name: &'static str,
        size: u64,
        content_type: Option<&'static str>,
    }

    impl NativeFileData for MockFile {
        fn name(&self) -> String {
            self.name.to_string()
        }
        fn size(&self) -> u64 {
            self.size
        }
        fn last_modified(&self) -> u64 {
            0
        }
        fn path(&self) -> PathBuf {
            PathBuf::new()
        }
        fn content_type(&self) -> Option<String> {
            self.content_type.map(str::to_string)
        }
        fn read_bytes(
            &self,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<bytes::Bytes, dioxus::CapturedError>>>>
        {
            Box::pin(std::future::ready(Ok(bytes::Bytes::new())))
        }
        fn byte_stream(
            &self,
        ) -> Pin<
            Box<
                dyn futures_util::Stream<Item = Result<bytes::Bytes, dioxus::CapturedError>>
                    + Send
                    + 'static,
            >,
        > {
            Box::pin(futures_util::stream::empty())
        }
        fn read_string(
            &self,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<String, dioxus::CapturedError>>>>
        {
            Box::pin(std::future::ready(Ok(String::new())))
        }
        fn inner(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn file(name: &'static str, size: u64, ct: Option<&'static str>) -> FileData {
        FileData::new(MockFile {
            name,
            size,
            content_type: ct,
        })
    }

    #[test]
    fn extension_filter_is_case_insensitive() {
        let f = FileFilter::new().extensions(["png", ".JPG", " gif "]);
        assert!(f.check(&file("Photo.PNG", 10, None)).is_ok());
        assert!(f.check(&file("photo.jpg", 10, None)).is_ok());
        assert!(f.check(&file("clip.GIF", 10, None)).is_ok());
        assert_eq!(
            f.check(&file("notes.txt", 10, None)),
            Err(FileRejection::Extension)
        );
        // extension must match at the end, not merely appear
        assert_eq!(
            f.check(&file("png.txt", 10, None)),
            Err(FileRejection::Extension)
        );
    }

    #[test]
    fn content_type_wildcards() {
        let f = FileFilter::new().content_types(["image/*", "application/pdf"]);
        assert!(f.check(&file("a", 1, Some("image/webp"))).is_ok());
        assert!(f.check(&file("b", 1, Some("application/pdf"))).is_ok());
        assert_eq!(
            f.check(&file("c", 1, Some("text/plain"))),
            Err(FileRejection::ContentType)
        );
        // missing content type fails a type-restricted filter
        assert_eq!(
            f.check(&file("d", 1, None)),
            Err(FileRejection::ContentType)
        );
    }

    #[test]
    fn content_type_wildcards_match_whole_type_only() {
        let f = FileFilter::new().content_types(["image/*"]);
        assert!(f.check(&file("a", 1, Some("image/svg+xml"))).is_ok());
        assert_eq!(
            f.check(&file("b", 1, Some("imageevil/png"))),
            Err(FileRejection::ContentType)
        );
        assert_eq!(
            f.check(&file("c", 1, Some("application/image"))),
            Err(FileRejection::ContentType)
        );
        assert_eq!(
            f.check(&file("d", 1, Some("image/png/extra"))),
            Err(FileRejection::ContentType)
        );
    }

    #[test]
    fn content_type_matching_normalizes_case_whitespace_and_parameters() {
        let f = FileFilter::new().content_types([" Application/PDF ", "text/plain"]);
        assert!(f.check(&file("a", 1, Some("application/pdf"))).is_ok());
        assert!(f
            .check(&file("b", 1, Some("TEXT/PLAIN; charset=utf-8")))
            .is_ok());
    }

    #[test]
    fn content_type_all_wildcard_accepts_any_typed_file() {
        let f = FileFilter::new().content_types(["*/*"]);
        assert!(f.check(&file("a", 1, Some("image/png"))).is_ok());
        assert!(f
            .check(&file("b", 1, Some("application/octet-stream")))
            .is_ok());
        assert_eq!(
            f.check(&file("c", 1, None)),
            Err(FileRejection::ContentType)
        );
    }

    #[test]
    fn content_type_structured_suffix_wildcards() {
        let app_json = FileFilter::new().content_types(["application/*+json"]);
        assert!(app_json
            .check(&file("a", 1, Some("application/ld+json")))
            .is_ok());
        assert!(app_json
            .check(&file("b", 1, Some("application/vnd.api+json")))
            .is_ok());
        assert_eq!(
            app_json.check(&file("c", 1, Some("text/ld+json"))),
            Err(FileRejection::ContentType)
        );
        assert_eq!(
            app_json.check(&file("d", 1, Some("application/json"))),
            Err(FileRejection::ContentType)
        );

        let any_json = FileFilter::new().content_types(["*/*+json"]);
        assert!(any_json
            .check(&file("e", 1, Some("application/problem+json")))
            .is_ok());
        assert!(any_json
            .check(&file("f", 1, Some("model/gltf+json")))
            .is_ok());
        assert_eq!(
            any_json.check(&file("g", 1, Some("application/json"))),
            Err(FileRejection::ContentType)
        );
    }

    #[test]
    fn malformed_content_type_patterns_do_not_match() {
        let f = FileFilter::new().content_types(["image", "image/", "/png", "image/png/extra"]);
        assert_eq!(
            f.check(&file("a", 1, Some("image/png"))),
            Err(FileRejection::ContentType)
        );
    }

    #[test]
    fn unsupported_subtype_only_wildcard_does_not_match() {
        let f = FileFilter::new().content_types(["*/json"]);
        assert_eq!(
            f.check(&file("a", 1, Some("application/json"))),
            Err(FileRejection::ContentType)
        );
        assert_eq!(
            f.check(&file("b", 1, Some("text/json"))),
            Err(FileRejection::ContentType)
        );
    }

    #[test]
    fn size_limit() {
        let f = FileFilter::new().max_size(100);
        assert!(f.check(&file("ok", 100, None)).is_ok());
        assert_eq!(
            f.check(&file("big", 101, None)),
            Err(FileRejection::TooLarge)
        );
    }

    #[test]
    fn partition_applies_count_after_other_rules() {
        let f = FileFilter::new().extensions(["png"]).max_files(2);
        let batch = vec![
            file("a.png", 1, None),
            file("b.txt", 1, None), // rejected on extension, doesn't consume a slot
            file("c.png", 1, None),
            file("d.png", 1, None), // over the count
        ];
        let (ok, bad) = f.partition(batch);
        assert_eq!(
            ok.iter().map(|f| f.name()).collect::<Vec<_>>(),
            vec!["a.png", "c.png"]
        );
        assert_eq!(bad.len(), 2);
        assert_eq!(bad[0].1, FileRejection::Extension);
        assert_eq!(bad[1].1, FileRejection::TooMany);
    }
}

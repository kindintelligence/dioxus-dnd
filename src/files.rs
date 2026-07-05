//! OS file drops — the one drop type where the payload arrives *in the native
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Allow only these extensions (case-insensitive, no leading dot).
    pub fn extensions<I, S>(mut self, exts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.extensions = exts.into_iter().map(|s| s.into().to_lowercase()).collect();
        self
    }

    /// Allow only these MIME types. A trailing `/*` wildcard is supported
    /// (`"image/*"`).
    pub fn content_types<I, S>(mut self, types: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.content_types = types.into_iter().map(|s| s.into().to_lowercase()).collect();
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
            let name = file.name().to_lowercase();
            let ok = self
                .extensions
                .iter()
                .any(|ext| name.ends_with(&format!(".{ext}")));
            if !ok {
                return Err(FileRejection::Extension);
            }
        }
        if !self.content_types.is_empty() {
            let ct = file.content_type().unwrap_or_default().to_lowercase();
            let ok = self.content_types.iter().any(|allowed| {
                if let Some(prefix) = allowed.strip_suffix("/*") {
                    ct.starts_with(prefix)
                } else {
                    ct == *allowed
                }
            });
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

/// A zone that accepts files dragged in from the operating system.
///
/// Independent of `DndContext` — file drops don't come from inside your app,
/// so no provider is required.
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
        let f = FileFilter::new().extensions(["png", "JPG"]);
        assert!(f.check(&file("Photo.PNG", 10, None)).is_ok());
        assert!(f.check(&file("photo.jpg", 10, None)).is_ok());
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

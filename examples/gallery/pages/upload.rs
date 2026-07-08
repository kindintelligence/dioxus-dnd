//! Upload: live demo plus how the pattern works.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use crate::ui::*;

#[component]
pub fn UploadPage() -> Element {
    rsx! {
        PageIntro {
            kicker: "Beyond the window",
            title: "Upload",
            lead: "FileDropZone is the one drop where the payload arrives in the native event rather than through context: real files from the desktop, filtered before your handler ever runs.",
        }
        UploadDemo {}
        DocBlock { title: "How it works",
            Prose {
                p {
                    "FileFilter is a builder: extensions, content_types, max_size, max_files. Accepted files reach on_files as a FileDrop; everything else reaches on_rejected as (FileData, FileRejection) pairs, so the reason (wrong type, too large, too many) is yours to show."
                }
                p {
                    "MIME matching is strict where it matters: image/* matches the slash-delimited prefix, structured suffixes like */*+json work, parameters and case are ignored, and bogus types like imageevil/png do not sneak through."
                }
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Advisory, not a security boundary:",
                        "names, types and sizes are attacker-controllable; validate real bytes server-side.",
                    ),
                    (
                        "data-over highlights the zone",
                        "during a native hover, same contract as in-app zones.",
                    ),
                    ("Web reads bytes, desktop reads paths;", "the same handler compiles for both."),
                    (
                        "Windows desktop file drops have webview quirks;",
                        "test your target and consider an input fallback.",
                    ),
                ],
            }
        }
    }
}

const SNIPPET: &str = r#"FileDropZone {
    filter: FileFilter::new()
        .content_types(["image/*"])
        .max_size(5_000_000)
        .max_files(6),
    on_files: move |drop: FileDrop| async move {
        for f in drop.files {
            let bytes = f.read_bytes().await?;   // web; f.path() on desktop
        }
    },
    on_rejected: move |bad: Vec<(FileData, FileRejection)>| show_reasons(bad),
    "Drop images here"
}"#;

// --- 13. upload (OS file drop, native) ---------------------------------------

#[component]
fn UploadDemo() -> Element {
    let mut accepted = use_signal(Vec::<String>::new);
    let mut refused = use_signal(Vec::<String>::new);
    rsx! {
        Section {
            title: "Upload",
            note: "Drag images from your desktop. Anything that isn't an image, or weighs over 5 MB, bounces with the reason on its chip. (Native OS drag; in-page pointer drags can't cross the app boundary.)",
            tag: "FileFilter",
            FileDropZone {
                filter: FileFilter::new()
                                                                                    .content_types(["image/*"])
                                                                                    .max_size(5_000_000)
                                                                                    .max_files(6),
                on_files: move |drop: FileDrop| {
                    accepted.write().extend(drop.files.iter().map(|f| f.name()));
                },
                on_rejected: move |bad: Vec<(dioxus::html::FileData, FileRejection)>| {
                    refused
                        .write()
                        .extend(
                            bad
                                .into_iter()
                                .map(|(f, why)| {
                                    let reason = match why {
                                        FileRejection::ContentType => "not an image",
                                        FileRejection::TooLarge => "over 5 MB",
                                        FileRejection::TooMany => "past the 6-file limit",
                                        FileRejection::Extension => "wrong extension",
                                    };
                                    format!("{} · {reason}", f.name())
                                }),
                        );
                },
                class: "flex min-h-28 flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-white/15 p-4 text-center transition data-over:border-[#D97D55] data-over:bg-[#D97D55]/15",
                if accepted.read().is_empty() && refused.read().is_empty() {
                    p { class: "text-sm font-medium text-[#b8ab93]", "Drop images here to upload" }
                    p { class: "text-[12px] text-[#9c8f77]", "Up to 6 images, 5 MB each" }
                } else {
                    if !accepted.read().is_empty() {
                        div { class: "flex flex-wrap justify-center gap-1.5",
                            for n in accepted.read().clone() {
                                span { class: "inline-flex items-center gap-1.5 rounded-md bg-[#B8C4A9]/20 px-2 py-1 text-[11px] font-medium text-[#c9d4ba]",
                                    span { DocGlyph {} }
                                    "{n}"
                                }
                            }
                        }
                    }
                    if !refused.read().is_empty() {
                        div { class: "flex flex-wrap justify-center gap-1.5",
                            for m in refused.read().clone() {
                                span { class: "inline-flex items-center rounded-md bg-[#D97D55]/20 px-2 py-1 text-[11px] font-medium text-[#eda87f]",
                                    "{m}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

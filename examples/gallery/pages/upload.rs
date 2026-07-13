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
            lead: "Files come from outside your app, so this is the first pattern that crosses the browser boundary: FileDropZone handles native drops and click-to-choose uploads, filters what arrives, and hands your code real files. No provider, no payload type.",
        }
        UploadDemo {}
        DocBlock { title: "How it works",
            Steps {
                steps: vec![
                    (
                        "The file rides the event.",
                        "Dropped and picker-selected files arrive in native browser events rather than through Rust context. FileDropZone handles the drag-over ceremony, opens the native chooser when clicked, and exposes data-over for drag-hover styling.",
                    ),
                    (
                        "Filter before your code runs.",
                        "FileFilter is a builder: allowed extensions, allowed MIME types, a size cap, a count cap. Files that pass reach on_files as a FileDrop; the rest reach on_rejected paired with the reason, so honest feedback costs a match statement.",
                    ),
                    (
                        "Read where you run.",
                        "On web, read contents with read_bytes, read_string or byte_stream. On desktop, FileData::path gives the real filesystem path. The same handler compiles for both.",
                    ),
                ],
            }
        }
        DocBlock { title: "Use it",
            CodeBlock { code: SNIPPET }
            Prose {
                p {
                    "The zone is fully headless: this demo's dashed border, hover tint and result chips are all page styling on top of the data-over attribute and the two callbacks."
                }
            }
            DioxusNote {
                p {
                    "Event handlers can be async: on_files here could await read_bytes for each file directly. Dioxus spawns the future for you; there is no separate effect system to route file reads through."
                }
            }
        }
        DocBlock { title: "The API",
            PropsTable {
                title: "FileDropZone props",
                rows: vec![
                    ("on_files", "EventHandler<FileDrop>, required", "The accepted files from a drop or picker selection. Picker selections use zero coordinates."),
                    ("filter", "Option<FileFilter>", "Acceptance rules; everything is accepted when omitted."),
                    ("on_rejected", "EventHandler<Vec<(FileData, FileRejection)>>", "The files that failed, each paired with why."),
                    ("on_hover", "EventHandler<bool>", "True on drag enter, false on leave, when styling needs more than data-over."),
                ],
            }
            PropsTable {
                title: "FileFilter builder",
                rows: vec![
                    (".extensions([\"png\", \"jpg\"])", "", "Allow-list by file extension; case-insensitive, leading dot optional."),
                    (".content_types([\"image/*\"])", "", "Allow-list by MIME type. Supports exact types, image/* wildcards, */* for any typed file, and structured suffixes like application/*+json."),
                    (".max_size(5_000_000)", "", "Reject files over this many bytes."),
                    (".max_files(6)", "", "Accept at most this many per incoming batch; extras reject as TooMany."),
                ],
            }
            PropsTable {
                title: "FileRejection variants",
                rows: vec![
                    ("Extension", "", "Name didn't end in an allowed extension."),
                    ("ContentType", "", "Reported MIME type didn't match the allow-list (missing types fail a restricted filter)."),
                    ("TooLarge", "", "Bigger than max_size."),
                    ("TooMany", "", "Arrived after the max_files quota was already full."),
                ],
            }
        }
        DocBlock { title: "Good to know",
            ApiNotes {
                notes: vec![
                    (
                        "Advisory, not a security boundary:",
                        "names, types and sizes are attacker-controllable; a renamed executable can claim to be a PNG. Validate real bytes server-side.",
                    ),
                    (
                        "MIME matching is strict where it matters:",
                        "wildcards match whole slash-delimited parts, so imageevil/png never sneaks past image/*; parameters and case are ignored.",
                    ),
                    (
                        "Rejections don't consume slots:",
                        "a file bounced on type doesn't count against max_files, so valid files behind it still land.",
                    ),
                    (
                        "Windows desktop file drops have webview quirks;",
                        "test your target; clicking the same zone uses its native file-input fallback.",
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
    "Click to choose images or drop them here"
}"#;

// --- 13. upload (OS file drop, native) ---------------------------------------

#[component]
fn UploadDemo() -> Element {
    let mut accepted = use_signal(Vec::<String>::new);
    let mut refused = use_signal(Vec::<String>::new);
    rsx! {
        Section {
            title: "Upload",
            note: "Click to choose images or drag them from your desktop. Anything that isn't an image, or weighs over 5 MB, bounces with the reason on its chip.",
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
                                        // FileRejection is non-exhaustive:
                                        // future rules land here.
                                        _ => "not accepted",
                                    };
                                    format!("{} · {reason}", f.name())
                                }),
                        );
                },
                class: "flex min-h-28 cursor-pointer flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed border-[#7A776C]/30 p-4 text-center transition data-over:border-[#1C4A38] data-over:bg-[#1C4A38]/15",
                if accepted.read().is_empty() && refused.read().is_empty() {
                    p { class: "text-sm font-medium text-[#45423B]", "Click to choose images or drop them here" }
                    p { class: "text-[12px] text-[#7A776C]", "Up to 6 images, 5 MB each" }
                } else {
                    if !accepted.read().is_empty() {
                        div { class: "flex flex-wrap justify-center gap-1.5",
                            for n in accepted.read().clone() {
                                span { class: "inline-flex items-center gap-1.5 rounded-md bg-[#6C9984]/20 px-2 py-1 text-[11px] font-medium text-[#1C4A38]",
                                    span { DocGlyph {} }
                                    "{n}"
                                }
                            }
                        }
                    }
                    if !refused.read().is_empty() {
                        div { class: "flex flex-wrap justify-center gap-1.5",
                            for m in refused.read().clone() {
                                span { class: "inline-flex items-center rounded-md bg-[#F1D9D1] px-2 py-1 text-[11px] font-medium text-[#8B3A2E]",
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

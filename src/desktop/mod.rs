//! Desktop windowing glue for multi-window drag worlds (`desktop` feature).
//!
//! [`crate::core::world`] keeps the library dependency-free by consuming
//! window geometry it does not compute and host-reported pointer data it
//! cannot see. This module is the other half for dioxus-desktop: the two
//! pieces every window of a multi-window app needs, promoted from the
//! `desktop-multiwindow` example after the per-platform behavior was
//! probed and hand-verified (Linux/X11 and Windows 11/WebView2; macOS is
//! expected to work on the same APIs but not yet hand-verified).
//!
//! ```rust,ignore
//! use dioxus_dnd::desktop::{use_window_geometry_feed, DragBridge};
//! use dioxus_dnd::prelude::*;
//!
//! fn any_window() -> Element {
//!     // ABOVE the provider: the provider picks the geometry up from
//!     // context when it joins the world.
//!     use_window_geometry_feed();
//!     rsx! {
//!         DndProvider::<Card> {
//!             DragBridge::<Card> {}   // BELOW the provider: needs the join
//!             // ... your zones, overlay, live region ...
//!         }
//!     }
//! }
//! ```
//!
//! # How the pieces divide
//!
//! - [`feed`]: [`use_window_geometry_feed`] samples this window's
//!   placement from tao events into the world's `WindowGeometry`.
//! - [`bridge`]: [`DragBridge`] gates which drags need host-side help
//!   (mouse and pen do; touch must be left alone) and composes the
//!   platform legs below.
//! - `platform`: the legs themselves. `windows` carries the WebView2
//!   raw-input path; `fallback` carries the portable tao path (cursor
//!   polling plus foreign-window release detection) that X11 and macOS
//!   ride today. Per-OS modules split out of `fallback` when their
//!   behavior actually diverges (explicit Wayland capability detection,
//!   a verified macOS strategy) - not before.
//!
//! # The per-platform truth table
//!
//! Webview pointer events stop at the viewport edge, and while a button
//! is held every NON-origin window is fully event-blind (X11 implicit
//! grab / AppKit event routing / engine mouse capture) - probed and
//! confirmed on those stacks. The portable legs cover that shape. On
//! Windows/WebView2 the shape is the OPPOSITE and both portable legs are
//! dead; the raw-input leg exists for exactly that platform (mechanics
//! documented in `platform::windows`). On Wayland neither global
//! geometry nor the global cursor exists by design; everything degrades
//! to per-window drags, which is the world's documented fallback.
//!
//! Every leg is gated on the drag's [`crate::core::PointerKind`]: only
//! pointers WITHOUT implicit capture (mouse, pen) are bridged. A touch
//! drag is already streamed whole to the origin webview by the browser's
//! implicit capture; bridging it too double-drives the drag from
//! Windows' touch-synthesized mouse (a cursor trailing the finger, plus
//! synthesized button transitions that can end the drag early).

mod bridge;
mod feed;
mod platform;

pub use bridge::DragBridge;
pub use feed::use_window_geometry_feed;

//! Desktop windowing glue for multi-window drag worlds (`desktop` feature).
//!
//! [`crate::core::world`] keeps the library dependency-free by consuming
//! window geometry it does not compute and host-reported pointer data it
//! cannot see. This module is the other half for dioxus-desktop: the two
//! pieces every window of a multi-window app needs, promoted from the
//! `desktop-multiwindow` example after the per-platform behavior was
//! probed and hand-verified (Linux/X11, Linux/Wayland policy, and Windows
//! 11/WebView2; macOS is expected to work on the same APIs but not yet
//! hand-verified).
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
//! - `platform`: sealed backend policy plus the legs themselves. `windows`
//!   carries the WebView2 raw-input path; `fallback` keeps the portable Tao
//!   mechanics (generation-bound cursor polling plus window-event release
//!   detection) shared. `linux` owns the runtime X11/Wayland decision and
//!   X11's held-button query for releases over desktop dead space; `macos`
//!   explicitly owns the still-unverified decision to use only the portable
//!   mechanics. This preserves one implementation of each shared leg without
//!   hiding genuinely platform-specific mechanics or capability policy.
//!
//! # The per-platform truth table
//!
//! Webview pointer events stop at the viewport edge, and while a button
//! is held every NON-origin window is fully event-blind (X11 implicit
//! grab; the equivalent AppKit/WKWebView strategy remains unverified) -
//! probed and confirmed on X11. The portable legs cover that shape, while
//! Linux's X11 button observer covers releases over no window at all. On
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
//!
//! # The kill switch
//!
//! Every leg also honors the world's runtime bridging switch
//! ([`crate::core::DndWorld::set_bridging`]; end users can set
//! `DIOXUS_DND_NO_BRIDGE=1` before launch, no rebuild). If a webview or
//! OS update ever ships a regression in these mechanics, the app can
//! degrade to per-window drags - the already-modeled Wayland behavior -
//! instead of shipping broken cross-window gestures. With `tracing` at
//! `debug`, each leg logs when it engages a drag (`cursor-poller` /
//! `release` / `x11-deadspace` / `raw-input`), so a post-update bug
//! report arrives pre-triaged to the leg whose platform assumption
//! moved.

mod bridge;
mod feed;
mod platform;

pub use bridge::DragBridge;
pub use feed::use_window_geometry_feed;

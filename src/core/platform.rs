//! Platform capability shims.
//!
//! Everything `web-sys` in this crate lives here, behind the `web` cargo
//! feature. With the feature off, these functions compile to nothing and the
//! pointer paths fall back to their capture-free reconciliation (a held-button
//! recovery plus not cancelling a drag that merely strays off the element).
//!
//! The one capability we need that Dioxus 0.8 does not expose is **pointer
//! capture**: routing every later event for a pointer to one element until
//! release, so a mouse drag survives the cursor leaving that element. Touch and
//! pen get this implicitly from the browser; mouse does not, and there is no
//! `setPointerCapture` on Dioxus' event or `MountedData`. This shim reaches the
//! real DOM element through `MountedData::downcast` and calls it directly.

use dioxus::prelude::*;

/// Route every subsequent event for `pointer_id` to `node` until the pointer is
/// released, regardless of where the pointer travels. The native fix for "the
/// mouse left the element, so move/up events stopped arriving." No-op without
/// the `web` feature (or on non-web renderers, where the downcast yields
/// `None`).
///
/// Capture a *stable* wrapper element rather than the event's target: Dioxus'
/// web renderer delegates events at the document root (so `currentTarget` is
/// the root, not your element), and a target child can re-render mid-drag
/// (live-preview transforms), which would drop the capture.
pub(crate) fn capture_pointer(node: &MountedData, pointer_id: i32) {
    #[cfg(feature = "web")]
    if let Some(el) = node.downcast::<web_sys::Element>() {
        // Fails only for an already-released/invalid pointer id - harmless.
        let _ = el.set_pointer_capture(pointer_id);
    }
    #[cfg(not(feature = "web"))]
    {
        let _ = (node, pointer_id);
    }
}

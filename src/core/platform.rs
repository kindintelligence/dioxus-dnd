//! Platform capability shims.
//!
//! Everything `web-sys` in this crate lives here, behind the `web` cargo
//! feature. With the feature off, these functions compile to nothing and the
//! pointer paths fall back to their capture-free reconciliation (a held-button
//! recovery plus not cancelling a drag that merely strays off the element).
//!
//! The one capability we need that Dioxus 0.7 does not expose is **pointer
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
/// Returns whether capture was actually taken - callers use it to decide
/// whether a capture *substitute* (the full-viewport layer) is needed.
pub(crate) fn capture_pointer(node: &MountedData, pointer_id: i32) -> bool {
    #[cfg(feature = "web")]
    if let Some(el) = node.downcast::<web_sys::Element>() {
        // Fails only for an already-released/invalid pointer id - harmless.
        return el.set_pointer_capture(pointer_id).is_ok();
    }
    #[cfg(not(feature = "web"))]
    {
        let _ = (node, pointer_id);
    }
    false
}

/// Run one FLIP handoff on the real DOM element, synchronously: write the
/// inverted transform, force a style-and-layout flush (reading a layout
/// metric does this per spec), then write the rest style with its transition
/// armed - the browser is now guaranteed to start the glide from the old
/// position. This is the classic no-library FLIP sequence; it removes the
/// paint-timing dependency of the render-twice fallback, where the release
/// races the browser painting the inverted frame.
///
/// `rest_style` must equal the style the caller renders for its at-rest
/// state, so the virtual DOM's view of the attribute stays truthful.
///
/// Returns `false` without the `web` feature (or on non-web renderers,
/// where the downcast yields `None`); the caller then falls back to
/// animating through renders.
pub(crate) fn flip_transform(node: &MountedData, invert_style: &str, rest_style: &str) -> bool {
    #[cfg(feature = "web")]
    if let Some(el) = node.downcast::<web_sys::Element>() {
        let _ = el.set_attribute("style", invert_style);
        let _ = el.client_width();
        let _ = el.set_attribute("style", rest_style);
        return true;
    }
    let _ = (node, invert_style, rest_style);
    false
}

/// Release pointer capture for `pointer_id` from `node`.
///
/// Browsers release capture automatically on pointerup/pointercancel, but an
/// explicit release makes the normal cleanup path obvious and keeps future
/// platform shims symmetric with [`capture_pointer`]. No-op without `web`.
pub(crate) fn release_pointer(node: &MountedData, pointer_id: i32) {
    #[cfg(feature = "web")]
    if let Some(el) = node.downcast::<web_sys::Element>() {
        // Fails if capture is already gone or this element never had it.
        let _ = el.release_pointer_capture(pointer_id);
    }
    #[cfg(not(feature = "web"))]
    {
        let _ = (node, pointer_id);
    }
}

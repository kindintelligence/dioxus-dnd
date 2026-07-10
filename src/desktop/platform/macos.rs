//! macOS policy, deliberately isolated and runtime-unverified. AppKit/
//! WKWebView is expected to use the shared cursor poller plus Tao release
//! fallback, but this remains a strategy rather than a verification claim
//! until it receives a real macOS pass.

use dioxus::prelude::*;

use crate::core::{DndContext, JoinedWindow};

use super::{fallback, GlobalCapability};

pub(super) fn use_global_capability() -> Signal<GlobalCapability> {
    use_signal(|| GlobalCapability::Available)
}

pub(super) fn use_portable_legs<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
    capability: Signal<GlobalCapability>,
) {
    fallback::use_cursor_poller_leg(joined, ctx, capability);
    fallback::use_release_leg(joined, ctx, capability);
}

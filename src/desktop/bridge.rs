//! The bridge component: decides WHICH drags need host-side help and
//! composes the platform legs that provide it. Platform mechanics live
//! in `platform`; this file knows only the gates.

use dioxus::prelude::*;

use crate::core::{use_joined_window, DndContext, JoinedWindow};

use super::platform;

/// Does the current drag need host-side bridging from this window?
/// Mouse and pen do (they go blind at the viewport edge without native
/// capture); touch must be left to the browser's implicit capture - see
/// the module docs on double-driving.
pub(super) fn bridged<T: Clone + 'static>(ctx: &DndContext<T>) -> bool {
    ctx.dragging() && !ctx.pointer_kind().implicitly_captured()
}

/// The cross-window drag bridge: host-side eyes and ears for pointer
/// drags that leave the origin window (see the module docs for the
/// per-platform mechanics). Render one INSIDE each window's
/// `DndProvider<T>`; it renders nothing. A provider that did not join a
/// [`crate::core::DndWorld`] gets a no-op bridge.
#[component]
pub fn DragBridge<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
) -> Element {
    let _ = phantom;
    let Some(joined) = use_joined_window::<T>() else {
        return rsx! {};
    };
    use_legs(joined);
    rsx! {}
}

/// Install every leg for this window. Split from the component so the
/// hook sequence reads as one unit: all legs share the same gates, all
/// legs are idempotent per drag, and exactly one window (the origin)
/// acts on any of them.
fn use_legs<T: Clone + PartialEq + 'static>(joined: JoinedWindow<T>) {
    let ctx = joined.world.context();
    platform::windows::use_raw_input_leg(joined, ctx);
    platform::fallback::use_cursor_poller_leg(joined, ctx);
    platform::fallback::use_foreign_release_leg(joined, ctx);
}

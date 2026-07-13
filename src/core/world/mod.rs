//! Multi-window drag worlds: one shared drag state spanning several
//! windows of a desktop app, each window an independent `VirtualDom`.
//!
//! Dioxus desktop polls every window's `VirtualDom` on the main thread, and
//! signal storage is thread-local rather than runtime-local, so a `Signal`
//! (and therefore a [`DndContext`](crate::core::DndContext)) created in one window's runtime can be
//! read, written and subscribed from another's - a write in window A
//! re-renders window B through B's own scheduler. `DndWorld` builds on
//! exactly that: the payload crosses windows as a live Rust value, with no
//! serialization and none of the platform roulette of native HTML5
//! drag-and-drop. (`DataTransfer` interop for drags that leave the app
//! entirely stays in [`crate::external`].)
//!
//! # Coordinate spaces
//!
//! Everything zone-shaped stays in **client CSS pixels of its own window**,
//! exactly as in single-window use. The world adds one more space: **global
//! desktop physical pixels**, in which windows are located and hit-tested.
//! Each window's [`WindowGeometry`] carries the conversion: the client
//! area's top-left in physical px (`inner_position()` on desktop), the
//! window scale factor, and the client-area size in physical px. Conversion
//! happens only at the world boundary.
//!
//! # Wiring
//!
//! With the `desktop` feature, render `MultiWindowProvider<T>` once in every
//! window. It installs the geometry feed above the drag provider and the host
//! bridge below it, so their required ordering is structural. Create sibling
//! VDOMs with [`DndWorld::vdom`] so the world cannot be omitted; chain the app
//! model and legitimate per-window context afterwards:
//!
//! ```text
//! fn main_window() -> Element {
//!     let world = use_dnd_world::<Card>();
//!     let model = use_dnd_model(Model::new);
//!     let popup_model = model.clone();
//!     let open = move |_| {
//!         let dom = world.vdom(popup).with_root_context(popup_model.clone());
//!         dioxus::desktop::window().new_window(dom, Default::default());
//!     };
//!     rsx! {
//!         MultiWindowProvider::<Card> {
//!             button { onclick: open, "Open" }
//!             // zones, overlay, live region
//!         }
//!     }
//! }
//!
//! fn popup() -> Element {
//!     let model = use_context::<Model>();
//!     rsx! { MultiWindowProvider::<Card> { /* ... */ } }
//! }
//! ```
//!
//! `MultiWindowProvider` warns once if it mounts without a world in context;
//! that window otherwise falls back to isolated drag state.
//!
//! Custom windowing hosts use the manual path: provide a [`WindowGeometry`]
//! above `DndProvider<T>`, update it from host move/resize/focus events, and
//! render the host bridge inside the provider after it joins. A provider that
//! finds a world joins it instead of creating isolated state (nested providers
//! keep today's shadowing semantics: only a window's outermost provider of `T`
//! joins). Sample placement in global physical pixels and call
//! [`WindowGeometry::set`]; call [`WindowGeometry::mark_focused`] on focus so
//! overlapping windows resolve to the frontmost. **Without geometry the world
//! degrades gracefully**: drags behave exactly as single-window drags (this is
//! also the honest Wayland story, where a client can learn neither the cursor's
//! global position nor its own windows' positions).
//!
//! # Lifetimes: close windows in any order
//!
//! A world's own state (the shared context and the window table) is
//! **process-lived**: it is created under an owner this module holds for
//! the life of the app, not under any window's scope. Whichever window
//! created the world can close first and every other window keeps
//! dragging - cross-window between the survivors, single-window when only
//! one remains. Closing a joined window prunes it from the table and
//! aborts an in-flight drag that originated there (its coordinate anchor
//! is gone). The cost is a deliberate, bounded leak: a handful of signals
//! per world, once per app.

use dioxus::prelude::*;

mod drag;
mod geometry;
mod host;
mod joined;
mod settle;
mod state;

pub use geometry::{WindowGeometry, WindowKey};
pub use joined::JoinedWindow;
pub(crate) use joined::{WorldHit, WorldMembership};
pub use state::{DndWorld, WindowRecord, ZoneLocation};

/// Create a `DndWorld<T>` (process-lived - see the module docs on
/// lifetimes) and provide it in context, so providers in this window join
/// it. Create sibling windows with [`DndWorld::vdom`]. Call it once, in any
/// window.
pub fn use_dnd_world<T: Clone + 'static>() -> DndWorld<T> {
    use_hook(|| provide_context(DndWorld::<T>::new()))
}

/// The enclosing provider's world membership, if it joined a world - the
/// handle desktop glue needs to bridge host-side input (see
/// [`DndWorld::track_global`] / [`DndWorld::drop_at_global`]). Call it
/// anywhere below the `DndProvider`.
pub fn use_joined_window<T: Clone + 'static>() -> Option<JoinedWindow<T>> {
    try_use_context::<WorldMembership<T>>().and_then(|m| m.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct WorldSlot(Rc<RefCell<Option<DndWorld<String>>>>);

    fn world_creator() -> Element {
        let slot = use_context::<WorldSlot>();
        slot.0.replace(Some(use_dnd_world::<String>()));
        rsx! {}
    }

    fn seeded_sibling() -> Element {
        let slot = use_context::<WorldSlot>();
        slot.0.replace(Some(use_context::<DndWorld<String>>()));
        rsx! {}
    }

    #[test]
    fn vdom_seeds_the_world_root_context() {
        let created = WorldSlot::default();
        let mut creator = VirtualDom::new(world_creator).with_root_context(created.clone());
        creator.rebuild_in_place();
        let world = created.0.take().expect("creator published its world");

        drop(creator);
        let seen = WorldSlot::default();
        let mut sibling = world.vdom(seeded_sibling).with_root_context(seen.clone());
        sibling.rebuild_in_place();

        assert!(seen.0.take().is_some_and(|seen| seen == world));
    }
}

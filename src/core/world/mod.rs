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
//! ```text
//! // main window: create the world (root scope - it must outlive every
//! // joining window), spawn siblings with it in their root context
//! fn main_window() -> Element {
//!     let world = use_dnd_world::<Card>();
//!     // dioxus_desktop::window().new_window(
//!     //     VirtualDom::new(popup).with_root_context(world), Default::default());
//!     rsx! { DndProvider::<Card> { /* ... */ } }   // joins via context
//! }
//!
//! fn popup() -> Element {
//!     rsx! { DndProvider::<Card> { /* ... */ } }   // joins via root context
//! }
//! ```
//!
//! A `DndProvider<T>` that finds a `DndWorld<T>` in context joins it
//! instead of creating isolated state (nested providers keep today's
//! shadowing semantics: only a window's outermost provider of `T` joins).
//! Feed each window's [`WindowGeometry`] from your windowing layer - on
//! desktop, sample `inner_position()` / `inner_size()` / `scale_factor()`
//! on move/resize events and call [`WindowGeometry::set`]; call
//! [`WindowGeometry::mark_focused`] on focus so overlapping windows resolve
//! to the frontmost. **Without geometry the world degrades gracefully**:
//! drags behave exactly as single-window drags (this is also the honest
//! Wayland story, where a client can learn neither the cursor's global
//! position nor its own windows' positions).
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
/// it. Pass the returned handle to sibling windows via
/// `VirtualDom::with_root_context`. Call it once, in any window.
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

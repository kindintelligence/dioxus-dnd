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
//! # What the bridge actually bridges (the per-platform truth table)
//!
//! Webview pointer events stop at the viewport edge, and while a button is
//! held every NON-origin window is fully event-blind (X11 implicit grab /
//! AppKit event routing / engine mouse capture) - probed and confirmed on
//! those stacks. So:
//!
//! - While a drag is in flight and the cursor is outside the ORIGIN
//!   window, the origin's bridge polls the global cursor (tao
//!   `cursor_position`, works even where events are dead) and feeds
//!   [`crate::core::DndWorld::track_global`] - ghost and zone hovers keep
//!   working across windows.
//! - A NON-origin window receiving any pointer event mid-drag proves the
//!   button was released (it was blind while held): its bridge completes
//!   the drop at that position via
//!   [`crate::core::DndWorld::drop_at_global`]. Releases back inside the
//!   origin window arrive as normal pointerups; both paths are idempotent.
//!
//! Windows/WebView2 amendment (probed on Win 11): the engine capture
//! there is the OPPOSITE shape. The origin webview keeps receiving the
//! full mouse stream while the button is held - including moves and the
//! release outside its own viewport - but those events target `<html>`
//! (nothing retargets without pointer capture), so no component handler
//! ever hears them; and tao never fires `CursorMoved`/`MouseInput` at
//! all, because the WebView2 child HWND consumes the messages before the
//! tao window sees them. Both legs above are therefore dead on Windows.
//! The third leg fixes it one layer lower: tao registers Windows raw
//! input (`WM_INPUT`) on the event loop's thread target, which no HWND
//! can swallow, and dioxus-desktop forwards `Event::DeviceEvent` to
//! EVERY wry event handler (only `WindowEvent`s are per-window
//! filtered). The origin's bridge hears the raw button-up wherever it
//! happens and completes the drop at `cursor_position()`; raw motion
//! retracks mid-drag at event rate. Requires
//! `DeviceEventFilter::Never`: the default `Unfocused` registers without
//! `RIDEV_INPUTSINK`, and the foreground input owner is the WebView2
//! process's HWND, so raw input never arrives otherwise. **The bridge
//! sets that filter process-globally on first use** - documented no-op
//! outside Windows, but if your app has its own raw-input needs, know
//! the bridge flips it.
//!
//! Every leg is gated on the drag's [`crate::core::PointerKind`]: only
//! pointers WITHOUT implicit capture (mouse, pen) are bridged. A touch
//! drag is already streamed whole to the origin webview by the browser's
//! implicit capture; bridging it too double-drives the drag from
//! Windows' touch-synthesized mouse (a cursor trailing the finger, plus
//! synthesized button transitions that can end the drag early).
//!
//! Wayland: a client can learn neither its windows' positions nor the
//! global cursor, by design. The geometry feed leaves geometry cleared,
//! `cursor_position()` errors leave the bridge inert, and drags
//! gracefully stay per-window - the world's documented degradation.

use dioxus::prelude::*;
use dioxus_desktop::tao::event::{DeviceEvent, ElementState, Event, WindowEvent};
use dioxus_desktop::tao::event_loop::DeviceEventFilter;
use dioxus_desktop::{use_wry_event_handler, window};

use crate::core::{use_joined_window, Point, WindowGeometry};

/// Provide a [`WindowGeometry`] for this window and keep it fed from tao
/// events (position/size/scale on move/resize/focus). Call it ABOVE the
/// `DndProvider`, which picks the geometry up from context when it joins
/// the world. Returns the geometry handle (rarely needed directly).
///
/// On Wayland, where a window cannot learn its own screen position, the
/// feed leaves geometry cleared and this window drags per-window only.
pub fn use_window_geometry_feed() -> WindowGeometry {
    let geometry = use_context_provider(WindowGeometry::new);
    let desktop = window();
    let sample = use_callback(move |_: ()| {
        let scale = desktop.scale_factor();
        let size = desktop.inner_size();
        match desktop.inner_position() {
            Ok(pos) => geometry.set(
                Point::new(pos.x as f64, pos.y as f64),
                (size.width as f64, size.height as f64),
                scale,
            ),
            Err(_) => geometry.clear(),
        }
    });
    use_hook(move || {
        sample.call(());
        geometry.mark_focused();
    });
    // WindowEvents arrive pre-filtered to the registering window.
    use_wry_event_handler(move |event, _| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. } => sample.call(()),
                WindowEvent::Focused(true) => {
                    geometry.mark_focused();
                    sample.call(());
                }
                _ => {}
            }
        }
    });
    geometry
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
    let ctx = joined.world.context();

    // Touch drags need NO bridging - see the module docs.
    let bridged = move || ctx.dragging() && !ctx.pointer_kind().implicitly_captured();

    // Third leg: raw-input release detection + event-rate tracking
    // (Windows; inert elsewhere). DeviceEvents reach every window's
    // handler; the origin gate keeps exactly one bridge acting.
    let filter_set = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
    use_wry_event_handler(move |event, target| {
        if !filter_set.get() {
            filter_set.set(true);
            target.set_device_event_filter(DeviceEventFilter::Never);
        }
        if !bridged() || joined.world.origin_window() != Some(joined.key) {
            return;
        }
        let Event::DeviceEvent { event, .. } = event else {
            return;
        };
        let released = matches!(
            event,
            DeviceEvent::Button { button: 1, state: ElementState::Released, .. }
        );
        if !released && !matches!(event, DeviceEvent::MouseMotion { .. }) {
            return;
        }
        // Wayland has no global cursor; the error leaves this leg inert.
        let Ok(pos) = window().cursor_position() else {
            return;
        };
        let global = Point::new(pos.x, pos.y);
        // Inside the origin viewport the webview owns the gesture (the
        // capture substitute feeds moves, the Draggable's pointerup
        // finishes drops). Outside it, this leg is the drag's ears.
        if joined.geometry.contains_global(global) {
            return;
        }
        if released {
            joined.world.drop_at_global(global);
        } else {
            joined.world.track_global(global);
        }
    });

    // Origin-side poller: spawned when a drag starts, ends itself when
    // the drag does. ~30ms keeps the ghost smooth without busy-waiting.
    // Skipped entirely for captured (touch) drags.
    use_effect(move || {
        if !bridged() {
            return;
        }
        if joined.world.origin_window() != Some(joined.key) {
            return;
        }
        let desktop = window();
        spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                if !ctx.dragging() {
                    break;
                }
                // Wayland: no global cursor by design - the bridge simply
                // never engages and drags stay per-window.
                let Ok(pos) = desktop.cursor_position() else {
                    break;
                };
                let global = Point::new(pos.x, pos.y);
                // Inside the origin window the webview owns the stream;
                // outside it, the poller is the drag's eyes.
                if !joined.geometry.contains_global(global) {
                    joined.world.track_global(global);
                }
            }
        });
    });

    // Foreign-side release detection (X11/AppKit; dead on Windows where
    // the raw-input leg covers it). Gated off for captured (touch)
    // drags: the origin webview hears the touch release itself, and a
    // foreign window's synthesized-mouse events must not complete the
    // drop at the trailing cursor position.
    use_wry_event_handler(move |event, _| {
        let dragging_foreign = bridged()
            && joined.world.origin_window().is_some()
            && joined.world.origin_window() != Some(joined.key);
        if !dragging_foreign {
            return;
        }
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    // Physical window px -> this window's CSS px -> global.
                    let scale = joined.geometry.scale().max(f64::EPSILON);
                    let client = Point::new(position.x / scale, position.y / scale);
                    if let Some(global) = joined.geometry.to_global(client) {
                        joined.world.drop_at_global(global);
                    }
                }
                WindowEvent::MouseInput { state: ElementState::Released, .. } => {
                    if let Ok(pos) = window().cursor_position() {
                        joined.world.drop_at_global(Point::new(pos.x, pos.y));
                    }
                }
                _ => {}
            }
        }
    });

    rsx! {}
}

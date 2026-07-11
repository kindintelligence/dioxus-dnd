//! The geometry feed: this window's placement, sampled from tao into the
//! world's coordinate space.

use dioxus::prelude::*;
use dioxus_desktop::tao::event::{Event, WindowEvent};
use dioxus_desktop::{use_wry_event_handler, window};

use crate::core::{Point, WindowGeometry};

use super::platform::{self, GlobalCapability};

/// Provide a [`WindowGeometry`] for this window and keep it fed from tao
/// events (position/size/scale and visibility eligibility on
/// move/resize/focus). Call it ABOVE the
/// `DndProvider`, which picks the geometry up from context when it joins
/// the world. Returns the geometry handle (rarely needed directly).
///
/// On Wayland, where a window cannot learn its own screen position, the
/// feed leaves geometry cleared and this window drags per-window only.
pub fn use_window_geometry_feed() -> WindowGeometry {
    let geometry = use_context_provider(|| {
        let geometry = WindowGeometry::new();
        // Linux must not expose plausible global placement before Tao's
        // event-loop target identifies the backend actually in use.
        geometry.set_eligible(false);
        geometry
    });
    let capability = platform::use_global_capability();
    let desktop = window();
    let sample = use_callback(move |_: ()| {
        if !capability.peek().available() {
            geometry.set_eligible(false);
            geometry.clear();
            return;
        }
        let eligible = desktop.is_visible() && !desktop.is_minimized();
        geometry.set_eligible(eligible);
        if !eligible {
            // Minimized/hidden windows retain their last placement for a
            // later restore, but cannot win global hit-testing meanwhile.
            return;
        }
        let scale = desktop.scale_factor();
        let size = desktop.inner_size();
        match desktop.inner_position() {
            Ok(pos) => geometry.set(
                Point::new(pos.x as f64, pos.y as f64),
                (size.width as f64, size.height as f64),
                scale,
            ),
            Err(_) => {
                // A failed sample does not revise the backend decision. X11
                // may recover on the next event; Wayland never reaches here.
                geometry.set_eligible(false);
                geometry.clear();
            }
        }
    });
    use_effect(move || match capability() {
        GlobalCapability::Available => {
            // The detected capability owns the first geometry publication.
            geometry.mark_focused();
            sample.call(());
        }
        GlobalCapability::Unknown | GlobalCapability::Unavailable => {
            geometry.set_eligible(false);
            geometry.clear();
        }
    });
    // WindowEvents arrive pre-filtered to the registering window.
    use_wry_event_handler(move |event, _| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::CursorEntered { .. } => sample.call(()),
                WindowEvent::Focused(true) => {
                    if capability.peek().available() {
                        geometry.mark_focused();
                    }
                    sample.call(());
                }
                WindowEvent::Focused(false) => sample.call(()),
                WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                    geometry.set_eligible(false);
                    geometry.clear();
                }
                _ => {}
            }
        }
    });
    geometry
}

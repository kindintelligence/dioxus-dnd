//! The geometry feed: this window's placement, sampled from tao into the
//! world's coordinate space.

use dioxus::prelude::*;
use dioxus_desktop::tao::event::{Event, WindowEvent};
use dioxus_desktop::{use_wry_event_handler, window};

use crate::core::{Point, WindowGeometry};

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

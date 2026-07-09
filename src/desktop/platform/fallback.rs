//! The portable tao legs: cursor polling and foreign-window release
//! detection. This is the shape probed on X11 (and expected on macOS,
//! unverified): webview pointer events stop at the viewport edge, and
//! while a button is held every NON-origin window is fully event-blind -
//! so the origin polls the global cursor to keep tracking, and a blind
//! window receiving its first pointer event mid-drag (proof the button
//! was released) completes the drop. Both paths are idempotent with the
//! webview's own pointerup and with the Windows raw-input leg.

use dioxus::prelude::*;
use dioxus_desktop::tao::event::{ElementState, Event, WindowEvent};
use dioxus_desktop::{use_wry_event_handler, window};

use crate::core::{DndContext, JoinedWindow, Point};

use super::super::bridge::bridged;

/// Origin-side poller: spawned when a drag starts, ends itself when the
/// drag does. ~30ms keeps the ghost smooth without busy-waiting. Skipped
/// entirely for captured (touch) drags.
pub(crate) fn use_cursor_poller_leg<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
) {
    use_effect(move || {
        if !bridged(&ctx) {
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
}

/// Foreign-side release detection. Gated off for captured (touch) drags:
/// the origin webview hears the touch release itself, and a foreign
/// window's synthesized-mouse events must not complete the drop at the
/// trailing cursor position.
pub(crate) fn use_foreign_release_leg<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
) {
    use_wry_event_handler(move |event, _| {
        let dragging_foreign = bridged(&ctx)
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
}

//! The Windows/WebView2 leg: raw input, because nothing else works there.
//!
//! Probed on Win 11 (see the repo's UPSTREAM-multiwindow notes for the
//! recordings): the origin webview keeps receiving the full mouse stream
//! while the button is held - including moves and the release outside
//! its own viewport - but those events target `<html>` (nothing
//! retargets without pointer capture), so no component handler ever
//! hears them; and tao never fires `CursorMoved`/`MouseInput` at all,
//! because the WebView2 child HWND consumes the messages before the tao
//! window sees them. Both portable legs are therefore dead on Windows.
//!
//! This leg goes one layer lower: tao registers Windows raw input
//! (`WM_INPUT`) on the event loop's thread target, which no HWND can
//! swallow, and dioxus-desktop forwards `Event::DeviceEvent` to EVERY
//! wry event handler (only `WindowEvent`s are per-window filtered). The
//! origin's bridge hears the raw button-up wherever it happens and
//! completes the drop at `cursor_position()` - the same
//! global-physical-px source the poller uses; raw motion retracks
//! mid-drag at event rate, out-pacing the 30ms poller.
//!
//! Requires `DeviceEventFilter::Never`: the default `Unfocused`
//! registers without `RIDEV_INPUTSINK`, and the foreground input owner
//! is the WebView2 process's HWND, so raw input never arrives otherwise.
//! **The leg sets that filter process-globally on first use** -
//! documented no-op outside Windows, but apps with their own raw-input
//! needs should know the bridge flips it.

use dioxus::prelude::*;
use dioxus_desktop::tao::event::{DeviceEvent, ElementState, Event};
use dioxus_desktop::tao::event_loop::DeviceEventFilter;
use dioxus_desktop::{use_wry_event_handler, window};

use crate::core::{DndContext, JoinedWindow, Point};

use super::super::bridge::bridged;

/// Raw-input release detection + event-rate tracking. DeviceEvents reach
/// every window's handler; the origin gate keeps exactly one bridge
/// acting.
pub(crate) fn use_raw_input_leg<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
) {
    let filter_set = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
    use_wry_event_handler(move |event, target| {
        if !filter_set.get() {
            filter_set.set(true);
            target.set_device_event_filter(DeviceEventFilter::Never);
        }
        if !bridged(&ctx) || joined.world.origin_window() != Some(joined.key) {
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
}

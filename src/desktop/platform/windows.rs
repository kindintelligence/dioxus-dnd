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

use dioxus_desktop::tao::event::{DeviceEvent, ElementState, Event};
use dioxus_desktop::tao::event_loop::DeviceEventFilter;
use dioxus_desktop::{use_wry_event_handler, window};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::{DndContext, JoinedWindow, Point};

use super::super::bridge::{bridged, current_bridged_generation, BridgeGeneration};

static DEVICE_FILTER_CLAIMED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterSetupAction {
    Install,
    AlreadyInstalled,
}

fn filter_setup_action(was_claimed: bool) -> FilterSetupAction {
    if was_claimed {
        FilterSetupAction::AlreadyInstalled
    } else {
        FilterSetupAction::Install
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawInputKind {
    Motion,
    PrimaryRelease,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawInputAction {
    Track,
    Drop,
}

fn raw_input_action(
    kind: RawInputKind,
    captured: BridgeGeneration,
    live: Option<BridgeGeneration>,
    needs_bridge: bool,
    owns_origin: bool,
    outside_origin: bool,
) -> Option<RawInputAction> {
    if live != Some(captured) || !needs_bridge || !owns_origin || !outside_origin {
        return None;
    }
    match kind {
        RawInputKind::Motion => Some(RawInputAction::Track),
        RawInputKind::PrimaryRelease => Some(RawInputAction::Drop),
        RawInputKind::Other => None,
    }
}

/// Raw-input release detection + event-rate tracking. DeviceEvents reach
/// every window's handler; the origin gate keeps exactly one bridge
/// acting.
pub(crate) fn use_raw_input_leg<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
) {
    use_wry_event_handler(move |event, target| {
        if !cfg!(target_os = "windows") {
            return;
        }
        if filter_setup_action(DEVICE_FILTER_CLAIMED.swap(true, Ordering::AcqRel))
            == FilterSetupAction::Install
        {
            // The process-global first claim owns raw-input installation;
            // mounting or retiring any individual window cannot re-arm it.
            target.set_device_event_filter(DeviceEventFilter::Never);
        }
        if !bridged(&ctx) || joined.world.origin_window() != Some(joined.key) {
            return;
        }
        let Event::DeviceEvent { event, .. } = event else {
            return;
        };
        let kind = match event {
            DeviceEvent::Button {
                button: 1,
                state: ElementState::Released,
                ..
            } => RawInputKind::PrimaryRelease,
            DeviceEvent::MouseMotion { .. } => RawInputKind::Motion,
            _ => RawInputKind::Other,
        };
        if kind == RawInputKind::Other {
            return;
        }
        // Capture the composite generation before cursor lookup. An untracked
        // `session: None` is authoritative only with its mandatory world id;
        // replacement `begin_from` mints a different id and makes this inert.
        let Some(generation) = current_bridged_generation(joined, &ctx) else {
            return;
        };
        let Ok(pos) = window().cursor_position() else {
            return;
        };
        let global = Point::new(pos.x, pos.y);
        // Inside the origin viewport the webview owns the gesture (the
        // capture substitute feeds moves, the Draggable's pointerup
        // finishes drops). Outside it, this leg is the drag's ears.
        let live = current_bridged_generation(joined, &ctx);
        let action = raw_input_action(
            kind,
            generation,
            live,
            bridged(&ctx),
            joined.world.origin_window() == Some(joined.key),
            !joined.geometry.contains_global(global),
        );
        // The captured generation owns this raw observation. Completion or
        // replacement invalidates it immediately before any world mutation.
        match action {
            Some(RawInputAction::Drop) => joined.world.drop_at_global(global),
            Some(RawInputAction::Track) => {
                joined.world.track_global(global);
                None
            }
            None => None,
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DragSessionId;

    #[test]
    fn device_filter_setup_is_process_once() {
        let first = filter_setup_action(false);
        let claimed = first == FilterSetupAction::Install;

        assert_eq!(first, FilterSetupAction::Install);
        assert_eq!(
            filter_setup_action(claimed),
            FilterSetupAction::AlreadyInstalled
        );
    }

    #[test]
    fn raw_input_action_requires_the_captured_live_generation() {
        let drag_n = BridgeGeneration {
            world: 10,
            session: Some(DragSessionId(20)),
        };
        let drag_n_plus_one = BridgeGeneration {
            world: 11,
            session: Some(DragSessionId(21)),
        };

        assert_eq!(
            raw_input_action(RawInputKind::Motion, drag_n, Some(drag_n), true, true, true),
            Some(RawInputAction::Track)
        );
        assert_eq!(
            raw_input_action(
                RawInputKind::PrimaryRelease,
                drag_n,
                Some(drag_n),
                true,
                true,
                true
            ),
            Some(RawInputAction::Drop)
        );
        assert_eq!(
            raw_input_action(
                RawInputKind::PrimaryRelease,
                drag_n,
                Some(drag_n_plus_one),
                true,
                true,
                true
            ),
            None
        );

        let untracked_n = BridgeGeneration {
            world: 30,
            session: None,
        };
        let untracked_n_plus_one = BridgeGeneration {
            world: 31,
            session: None,
        };
        assert_eq!(
            raw_input_action(
                RawInputKind::Motion,
                untracked_n,
                Some(untracked_n),
                true,
                true,
                true
            ),
            Some(RawInputAction::Track)
        );
        assert_eq!(
            raw_input_action(
                RawInputKind::Motion,
                untracked_n,
                Some(untracked_n_plus_one),
                true,
                true,
                true
            ),
            None
        );
        assert_eq!(
            raw_input_action(RawInputKind::PrimaryRelease, drag_n, None, true, true, true),
            None
        );
    }

    #[test]
    fn raw_input_action_requires_every_ownership_gate() {
        let drag = BridgeGeneration {
            world: 5,
            session: None,
        };
        for (kind, bridge, origin, outside) in [
            (RawInputKind::PrimaryRelease, false, true, true),
            (RawInputKind::PrimaryRelease, true, false, true),
            (RawInputKind::PrimaryRelease, true, true, false),
            (RawInputKind::Other, true, true, true),
        ] {
            assert_eq!(
                raw_input_action(kind, drag, Some(drag), bridge, origin, outside),
                None
            );
        }
    }
}

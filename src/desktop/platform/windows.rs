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

use dioxus::prelude::{use_hook, Modifiers};
use dioxus_desktop::tao::event::{DeviceEvent, ElementState, Event};
use dioxus_desktop::tao::event_loop::DeviceEventFilter;
use dioxus_desktop::tao::keyboard::KeyCode;
use dioxus_desktop::{use_wry_event_handler, window};
use std::cell::Cell;
use std::rc::Rc;
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

// Live modifiers need the same raw layer as the pointer (probed on Win 11,
// same recordings): tao never fires `ModifiersChanged` because the WebView2
// child HWND owns keyboard focus, and the origin's streamed `<html>`-target
// events carry the state but reach no component handler. The
// `DeviceEventFilter::Never` registration below already includes keyboards,
// so `DeviceEvent::Key` arrives here regardless of focus; each physical
// modifier side is tracked so releasing one Ctrl while the other is held
// cannot clear the state.
const CONTROL_SIDES: u8 = 0b0000_0011;
const ALT_SIDES: u8 = 0b0000_1100;
const SHIFT_SIDES: u8 = 0b0011_0000;
const SUPER_SIDES: u8 = 0b1100_0000;

fn raw_modifier_bit(key: KeyCode) -> Option<u8> {
    Some(match key {
        KeyCode::ControlLeft => 1 << 0,
        KeyCode::ControlRight => 1 << 1,
        KeyCode::AltLeft => 1 << 2,
        KeyCode::AltRight => 1 << 3,
        KeyCode::ShiftLeft => 1 << 4,
        KeyCode::ShiftRight => 1 << 5,
        KeyCode::SuperLeft => 1 << 6,
        KeyCode::SuperRight => 1 << 7,
        _ => return None,
    })
}

fn apply_raw_key(mask: u8, key: KeyCode, pressed: bool) -> u8 {
    match raw_modifier_bit(key) {
        Some(bit) if pressed => mask | bit,
        Some(bit) => mask & !bit,
        None => mask,
    }
}

fn mask_modifiers(mask: u8) -> Modifiers {
    let mut mods = Modifiers::empty();
    if mask & CONTROL_SIDES != 0 {
        mods.insert(Modifiers::CONTROL);
    }
    if mask & ALT_SIDES != 0 {
        mods.insert(Modifiers::ALT);
    }
    if mask & SHIFT_SIDES != 0 {
        mods.insert(Modifiers::SHIFT);
    }
    if mask & SUPER_SIDES != 0 {
        mods.insert(Modifiers::META);
    }
    mods
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
    let key_mask = use_hook(|| Rc::new(Cell::new(0u8)));
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
        if let Event::DeviceEvent {
            event: DeviceEvent::Key(key),
            ..
        } = event
        {
            // Track modifier keys even while idle so a drag that starts with
            // one already held sees it; every window's mask converges on the
            // same stream, and only the origin's live drag feeds the world.
            let mask = apply_raw_key(
                key_mask.get(),
                key.physical_key,
                key.state == ElementState::Pressed,
            );
            key_mask.set(mask);
            if bridged(&ctx)
                && joined.world.origin_window() == Some(joined.key)
                && current_bridged_generation(joined, &ctx).is_some()
            {
                joined.world.update_modifiers(mask_modifiers(mask));
            }
            return;
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
    fn raw_key_mask_tracks_paired_modifier_sides() {
        let mut mask = 0;
        mask = apply_raw_key(mask, KeyCode::ControlLeft, true);
        assert_eq!(mask_modifiers(mask), Modifiers::CONTROL);
        mask = apply_raw_key(mask, KeyCode::ControlRight, true);
        mask = apply_raw_key(mask, KeyCode::ControlLeft, false);
        assert_eq!(mask_modifiers(mask), Modifiers::CONTROL);
        mask = apply_raw_key(mask, KeyCode::ControlRight, false);
        assert_eq!(mask_modifiers(mask), Modifiers::empty());
    }

    #[test]
    fn raw_key_mask_maps_each_modifier_and_ignores_other_keys() {
        assert_eq!(
            mask_modifiers(apply_raw_key(0, KeyCode::AltRight, true)),
            Modifiers::ALT
        );
        assert_eq!(
            mask_modifiers(apply_raw_key(0, KeyCode::ShiftLeft, true)),
            Modifiers::SHIFT
        );
        assert_eq!(
            mask_modifiers(apply_raw_key(0, KeyCode::SuperLeft, true)),
            Modifiers::META
        );
        assert_eq!(apply_raw_key(0, KeyCode::KeyA, true), 0);
        assert_eq!(apply_raw_key(0, KeyCode::ControlLeft, false), 0);
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

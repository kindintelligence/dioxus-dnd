//! Portable Tao mechanics used by explicit platform policy: cursor polling
//! plus native/foreign release detection. Linux enables them only for X11;
//! macOS currently enables the same strategy but remains runtime-unverified.
//! Wayland never reaches either leg, so an API call failing once is a
//! transient sample failure rather than backend detection.

use dioxus::prelude::*;
use dioxus_desktop::tao::event::{ElementState, Event, MouseButton, WindowEvent};
use dioxus_desktop::{use_wry_event_handler, window};

use crate::core::{DndContext, JoinedWindow, Point};

use super::super::bridge::{
    bridged, current_bridged_generation, current_generation, subscribed_generation,
    BridgeGeneration,
};
use super::GlobalCapability;

fn poller_run_current(
    expected: BridgeGeneration,
    current: Option<BridgeGeneration>,
    bridge_active: bool,
    owns_origin: bool,
    capability_available: bool,
    geometry_live: bool,
) -> bool {
    current == Some(expected)
        && bridge_active
        && owns_origin
        && capability_available
        && geometry_live
}

pub(super) fn poller_owns_generation<T: Clone + 'static>(
    joined: JoinedWindow<T>,
    ctx: &DndContext<T>,
    capability: Signal<GlobalCapability>,
    expected: BridgeGeneration,
) -> bool {
    poller_run_current(
        expected,
        current_generation(joined),
        bridged(ctx),
        joined.world.origin_window() == Some(joined.key),
        capability.peek().available(),
        joined.geometry.live(),
    ) && joined
        .world
        .is_drag_generation(expected.world, expected.session)
}

/// Origin-side poller. A resource cancels its prior task when any reactive
/// gate changes; the captured generation check remains load-bearing for a
/// sleeper racing cancellation or a rapid replacement drag.
pub(crate) fn use_cursor_poller_leg<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
    capability: Signal<GlobalCapability>,
) {
    let _poller = use_resource(move || {
        // Subscribe the resource to every policy input that can start or stop
        // a run. The composite world/source generation is captured separately
        // as the run's authority token.
        let capability_available = capability().available();
        let bridge_active = bridged(&ctx);
        let owns_origin = joined.world.origin_window() == Some(joined.key);
        let geometry_live = joined.geometry.live();
        let generation = subscribed_generation(joined);
        let should_poll = generation.is_some_and(|generation| {
            poller_run_current(
                generation,
                current_generation(joined),
                bridge_active,
                owns_origin,
                capability_available,
                geometry_live,
            ) && joined
                .world
                .is_drag_generation(generation.world, generation.session)
        });
        let desktop = window();
        async move {
            let Some(generation) = generation.filter(|_| should_poll) else {
                return;
            };

            loop {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                if !poller_owns_generation(joined, &ctx, capability, generation) {
                    break;
                }
                let Ok(pos) = desktop.cursor_position() else {
                    // X11 capability was established independently. Skip one
                    // failed sample and let the next tick recover the run.
                    continue;
                };
                let global = Point::new(pos.x, pos.y);
                if !joined.geometry.contains_global(global) {
                    // This captured session and origin window own the update.
                    // Recheck immediately before mutation so drag N cannot
                    // attach a late tick to replacement drag N+1.
                    if poller_owns_generation(joined, &ctx, capability, generation) {
                        joined.world.track_global(global);
                    }
                }
            }
        }
    });
}

fn release_owns_generation<T: Clone + 'static>(
    joined: JoinedWindow<T>,
    ctx: &DndContext<T>,
    capability: Signal<GlobalCapability>,
    expected: BridgeGeneration,
    foreign_only: bool,
) -> bool {
    let origin = joined.world.origin_window();
    current_bridged_generation(joined, ctx) == Some(expected)
        && capability.peek().available()
        && joined.geometry.live()
        && joined.world.record(joined.key).is_some()
        && origin.is_some_and(|origin| !foreign_only || origin != joined.key)
}

/// Tao release detection. A primary release can arrive at the origin while
/// its implicit X11 grab is outside the viewport, so it is not foreign-only;
/// the first cursor event in a foreign window remains the fallback proof that
/// the release happened while that window was blind.
pub(crate) fn use_release_leg<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
    capability: Signal<GlobalCapability>,
) {
    use_wry_event_handler(move |event, _| {
        let Some(generation) = current_bridged_generation(joined, &ctx) else {
            return;
        };
        if !capability.peek().available() || !joined.geometry.live() {
            return;
        }
        let Event::WindowEvent { event, .. } = event else {
            return;
        };
        match event {
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let Ok(pos) = window().cursor_position() else {
                    return;
                };
                // The captured composite generation owns this release.
                // Completion or replacement invalidates it before action.
                if release_owns_generation(joined, &ctx, capability, generation, false) {
                    joined.world.drop_at_global(Point::new(pos.x, pos.y));
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if !release_owns_generation(joined, &ctx, capability, generation, true) {
                    return;
                }
                // Physical window px -> this window's CSS px -> global.
                let scale = joined.geometry.scale().max(f64::EPSILON);
                let client = Point::new(position.x / scale, position.y / scale);
                if let Some(global) = joined.geometry.to_global(client) {
                    if release_owns_generation(joined, &ctx, capability, generation, true) {
                        joined.world.drop_at_global(global);
                    }
                }
            }
            WindowEvent::CursorEntered { .. } => {
                let Ok(pos) = window().cursor_position() else {
                    return;
                };
                // Geometry resampling for CursorEntered stays in `feed`; this
                // is only the foreign-release fallback at the global cursor.
                if release_owns_generation(joined, &ctx, capability, generation, true) {
                    joined.world.drop_at_global(Point::new(pos.x, pos.y));
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DragSessionId;

    #[test]
    fn poller_run_is_bound_to_its_captured_generation() {
        let drag_n = BridgeGeneration {
            world: 41,
            session: Some(DragSessionId(10)),
        };
        let drag_n_plus_one = BridgeGeneration {
            world: 42,
            session: Some(DragSessionId(11)),
        };

        assert!(poller_run_current(
            drag_n,
            Some(drag_n),
            true,
            true,
            true,
            true
        ));
        assert!(!poller_run_current(
            drag_n,
            Some(drag_n_plus_one),
            true,
            true,
            true,
            true
        ));
        assert!(!poller_run_current(drag_n, None, true, true, true, true));

        let untracked_n = BridgeGeneration {
            world: 50,
            session: None,
        };
        let untracked_n_plus_one = BridgeGeneration {
            world: 51,
            session: None,
        };
        assert!(poller_run_current(
            untracked_n,
            Some(untracked_n),
            true,
            true,
            true,
            true
        ));
        assert!(!poller_run_current(
            untracked_n,
            Some(untracked_n_plus_one),
            true,
            true,
            true,
            true
        ));
    }

    #[test]
    fn poller_requires_every_ownership_gate() {
        let drag = BridgeGeneration {
            world: 7,
            session: None,
        };
        for gates in [
            (false, true, true, true),
            (true, false, true, true),
            (true, true, false, true),
            (true, true, true, false),
        ] {
            assert!(!poller_run_current(
                drag,
                Some(drag),
                gates.0,
                gates.1,
                gates.2,
                gates.3
            ));
        }
    }
}

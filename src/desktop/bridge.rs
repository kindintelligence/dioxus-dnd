//! The bridge component: decides WHICH drags need host-side help and
//! composes the platform legs that provide it. Platform mechanics live
//! in `platform`; this file knows only the gates.

use dioxus::prelude::*;
use dioxus_desktop::tao::event::{Event, WindowEvent};
use dioxus_desktop::tao::keyboard::ModifiersState as TaoModifiers;
use dioxus_desktop::use_wry_event_handler;

use crate::core::{use_joined_window, DndContext, DragSessionId, JoinedWindow, PointerKind};

use super::platform;

/// Does the current drag need host-side bridging from this window?
/// Mouse and pen do (they go blind at the viewport edge without native
/// capture); touch must be left to the browser's implicit capture - see
/// the module docs on double-driving. The world's kill switch
/// ([`crate::core::DndWorld::set_bridging`]) vetoes everything: when an
/// upstream update ships a bridge regression, every leg must stand down
/// from this one gate rather than each growing its own check.
fn bridge_needed(dragging: bool, pointer_kind: PointerKind, bridging_enabled: bool) -> bool {
    dragging && bridging_enabled && !pointer_kind.implicitly_captured()
}

pub(super) fn bridged<T: Clone + 'static>(joined: JoinedWindow<T>, ctx: &DndContext<T>) -> bool {
    bridge_needed(
        ctx.dragging(),
        ctx.pointer_kind(),
        joined.world.bridging_enabled(),
    )
}

/// A host observation's complete authority token. The world generation is
/// mandatory for every `begin_from`; a tracked source session adds its
/// exactly-once completion generation when one exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BridgeGeneration {
    pub(super) world: u64,
    pub(super) session: Option<DragSessionId>,
}

pub(super) fn current_generation<T: Clone + 'static>(
    joined: JoinedWindow<T>,
) -> Option<BridgeGeneration> {
    let (world, session) = joined.world.drag_generation_peek()?;
    joined
        .world
        .is_drag_generation(world, session)
        .then_some(BridgeGeneration { world, session })
}

pub(super) fn subscribed_generation<T: Clone + 'static>(
    joined: JoinedWindow<T>,
) -> Option<BridgeGeneration> {
    let (world, session) = joined.world.drag_generation()?;
    joined
        .world
        .is_drag_generation(world, session)
        .then_some(BridgeGeneration { world, session })
}

pub(super) fn current_bridged_generation<T: Clone + 'static>(
    joined: JoinedWindow<T>,
    ctx: &DndContext<T>,
) -> Option<BridgeGeneration> {
    if !bridged(joined, ctx) {
        return None;
    }
    current_generation(joined)
}

fn map_modifiers(native: TaoModifiers) -> Modifiers {
    let mut mapped = Modifiers::empty();
    if native.shift_key() {
        mapped.insert(Modifiers::SHIFT);
    }
    if native.control_key() {
        mapped.insert(Modifiers::CONTROL);
    }
    if native.alt_key() {
        mapped.insert(Modifiers::ALT);
    }
    if native.super_key() {
        mapped.insert(Modifiers::META);
    }
    mapped
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
    use_legs(joined);
    rsx! {}
}

/// Install every leg for this window. Split from the component so the
/// hook sequence reads as one unit: all legs share the same gates, all
/// legs are idempotent per drag, and exactly one window (the origin)
/// acts on any of them.
fn use_legs<T: Clone + PartialEq + 'static>(joined: JoinedWindow<T>) {
    let ctx = joined.world.context();
    let capability = platform::use_global_capability();
    use_shared_window_events(joined, ctx);
    platform::use_pointer_legs(joined, ctx, capability);
}

/// Route platform-neutral Tao observations that remain useful regardless of
/// which pointer leg is active. The handler contains no OS dispatch.
fn use_shared_window_events<T: Clone + PartialEq + 'static>(
    joined: JoinedWindow<T>,
    ctx: DndContext<T>,
) {
    use_wry_event_handler(move |event, _| {
        let Event::WindowEvent { event, .. } = event else {
            return;
        };
        match event {
            WindowEvent::ModifiersChanged(modifiers)
                // The live current, non-captured generation owns modifier
                // state; a late idle/touch/replaced event is inert.
                if current_bridged_generation(joined, &ctx).is_some()
                    && joined.world.record(joined.key).is_some() =>
            {
                joined.world.update_modifiers(map_modifiers(*modifiers));
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
                if current_generation(joined).is_some()
                    && joined.world.record(joined.key).is_some() =>
            {
                // Rect refresh belongs only to the active world generation and
                // a still-joined window; teardown and superseded events no-op.
                joined.world.refresh_all_rects();
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implicitly_captured_touch_never_bridges() {
        assert!(bridge_needed(true, PointerKind::Mouse, true));
        assert!(bridge_needed(true, PointerKind::Pen, true));
        assert!(!bridge_needed(true, PointerKind::Touch, true));
        assert!(!bridge_needed(false, PointerKind::Mouse, true));
    }

    #[test]
    fn kill_switch_vetoes_every_bridgeable_pointer() {
        assert!(!bridge_needed(true, PointerKind::Mouse, false));
        assert!(!bridge_needed(true, PointerKind::Pen, false));
        assert!(!bridge_needed(true, PointerKind::Touch, false));
        assert!(!bridge_needed(false, PointerKind::Mouse, false));
    }

    #[test]
    fn tao_modifiers_map_to_drop_effect_modifiers() {
        let native =
            TaoModifiers::SHIFT | TaoModifiers::CONTROL | TaoModifiers::ALT | TaoModifiers::SUPER;
        let mapped = map_modifiers(native);

        assert!(mapped.contains(Modifiers::SHIFT));
        assert!(mapped.contains(Modifiers::CONTROL));
        assert!(mapped.contains(Modifiers::ALT));
        assert!(mapped.contains(Modifiers::META));
        assert_eq!(map_modifiers(TaoModifiers::empty()), Modifiers::empty());
    }
}

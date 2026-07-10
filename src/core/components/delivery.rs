//! THE drop path: payload delivery to a receiving zone, shared by the
//! `Draggable` pointer gesture, host-side drops, and the test harness.

use dioxus::prelude::*;

use crate::core::hooks::SettleFlag;
use crate::core::registry::ZoneRegistry;
use crate::core::state::DndContext;
use crate::core::types::{DragMode, DropEffect, DropOutcome, Point, ZoneId};

/// How many CONSECUTIVE moves must report no held buttons before the
/// lost-release recovery synthesizes a pointer-up. Move events carry the
/// display server's button state mask, which some pipelines corrupt for
/// isolated events (WSLg's RDP translation is the documented case) - one
/// bogus "empty" move must not phantom-drop a drag. A genuinely lost
/// release produces a steady empty stream, so the debounce costs a few
/// milliseconds, not correctness.
pub(crate) const RELEASE_RECOVERY_MOVES: u8 = 3;

/// Deliver the in-flight payload to `target`: acceptance check, settle
/// routing, outcome construction, the zone's callback. THE drop path - the
/// `Draggable` pointer gesture and [`crate::test::DragSim`] both end here,
/// so headless tests exercise exactly what production drops run.
pub(crate) fn deliver_drop<T: Clone + PartialEq + 'static>(
    registry: ZoneRegistry<T>,
    dnd: &mut DndContext<T>,
    settle_flag: Option<SettleFlag<T>>,
    target: ZoneId,
    point: Point,
    effect: DropEffect,
) -> bool {
    let Some(record) = registry.get(target) else {
        return false;
    };
    let Some(p) = dnd.payload() else {
        return false;
    };
    if !record.accepts_payload(&p) {
        return false;
    }
    let target_rect = registry.cached_rect(target);
    let origin = target_rect.map(|r| r.origin()).unwrap_or_default();
    let mode = dnd.mode();
    let grab = dnd.grab();
    // A settle-enabled overlay glides the ghost into the target zone:
    // route the drop through the settling take so the payload stays
    // readable while it animates. Pointer drops only - a keyboard drag
    // renders no positioned ghost to glide.
    let settle_to = match settle_flag {
        Some(f) if mode == DragMode::Pointer && *f.armed.peek() => target_rect,
        _ => None,
    };
    let taken = match settle_to {
        Some(to) => dnd.take_settling(to),
        None => dnd.take(),
    };
    if let Some((p, from)) = taken {
        record.on_drop.call(DropOutcome {
            payload: p,
            from,
            to: target,
            effect,
            mode,
            client: point,
            element: point - origin,
            grab,
            // The receiving zone fills this in when it opted in.
            edge: None,
        });
        return true;
    }
    false
}

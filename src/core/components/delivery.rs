//! THE drop path: payload delivery to a receiving zone, shared by the
//! `Draggable` pointer gesture, host-side drops, and the test harness.

use crate::core::hooks::SettleFlag;
use crate::core::registry::ZoneRegistry;
use crate::core::state::DndContext;
use crate::core::types::{DragMode, DragSessionId, DropEffect, DropOutcome, Point, ZoneId};
use crate::core::world::{DndWorld, WindowKey};

/// How many CONSECUTIVE moves must report no held buttons before the
/// lost-release recovery synthesizes a pointer-up. Move events carry the
/// display server's button state mask, which some pipelines corrupt for
/// isolated events (WSLg's RDP translation is the documented case) - one
/// bogus "empty" move must not phantom-drop a drag. A genuinely lost
/// release produces a steady empty stream, so the debounce costs a few
/// milliseconds, not correctness.
pub(crate) const RELEASE_RECOVERY_MOVES: u8 = 3;

/// How a successful delivery commits the source lifecycle before receiver
/// user code runs. Receiver callbacks may synchronously remove the source or
/// start a replacement drag, so completing afterwards without a generation
/// guard is too late.
pub(crate) enum DropCompletion<'a, T: Clone + 'static> {
    None,
    Local(DragSessionId),
    World {
        world: &'a DndWorld<T>,
        session: Option<DragSessionId>,
    },
}

impl<T: Clone + 'static> Copy for DropCompletion<'_, T> {}
impl<T: Clone + 'static> Clone for DropCompletion<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Clone + 'static> DropCompletion<'_, T> {
    fn commit(self, dnd: &mut DndContext<T>) {
        match self {
            Self::None => {}
            Self::Local(session) => {
                dnd.commit_source(session, true);
            }
            Self::World {
                world,
                session: Some(session),
            } => {
                world.commit_session(session, true);
            }
            Self::World {
                world: _,
                session: None,
            } => {}
        }
    }

    fn finalize(self, dnd: &mut DndContext<T>) {
        match self {
            Self::None => {}
            Self::Local(session) => {
                dnd.finalize_source(session);
            }
            Self::World {
                world,
                session: Some(session),
            } => {
                world.finalize_session(session);
            }
            Self::World {
                world,
                session: None,
            } => world.finish_untracked(true),
        }
    }
}

/// Settle capability for one delivery. World deliveries carry the receiving
/// window so it is elected before the shared context enters settling.
pub(crate) struct SettleRoute<'a, T: Clone + 'static> {
    pub(crate) flag: Option<SettleFlag<T>>,
    pub(crate) owner: Option<(&'a DndWorld<T>, WindowKey)>,
}

/// Acceptance-aware release selection shared by the live pointer path, host
/// delivery, and `DragSim`, so overlap fall-through cannot diverge between
/// production and the headless driver.
pub(crate) fn resolve_release_target<T: Clone + 'static>(
    registry: ZoneRegistry<T>,
    payload: &T,
    point: Point,
    max_distance: f64,
) -> Option<ZoneId> {
    registry.hit_test_closest(point, payload, max_distance)
}

/// Deliver the in-flight payload to `target`: acceptance check, settle
/// routing, outcome construction, the zone's callback. THE drop path - the
/// `Draggable` pointer gesture and [`crate::test::DragSim`] both end here,
/// so headless tests exercise exactly what production drops run.
pub(crate) fn deliver_drop<T: Clone + PartialEq + 'static>(
    registry: ZoneRegistry<T>,
    dnd: &mut DndContext<T>,
    settle: SettleRoute<'_, T>,
    completion: DropCompletion<'_, T>,
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
    let settle_to = match settle.flag {
        Some(f) if mode == DragMode::Pointer && f.is_armed() => target_rect,
        _ => None,
    };
    let taken = match settle_to {
        Some(to) => {
            if let Some((world, key)) = settle.owner {
                world.claim_settle(key);
            }
            dnd.take_settling(to)
        }
        None => dnd.take(),
    };
    if let Some((p, from)) = taken {
        completion.commit(dnd);
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
        completion.finalize(dnd);
        return true;
    }
    false
}

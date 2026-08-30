//! THE drop path: payload delivery to a receiving zone, shared by the
//! `Draggable` pointer gesture, host-side drops, and the test harness.

use crate::core::hooks::SettleFlag;
use crate::core::registry::ZoneRegistry;
use crate::core::session::DragCompletion;
use crate::core::state::DndContext;
use crate::core::types::{
    edge_of, DragMode, DragSessionId, DropEffect, DropOutcome, Point, Rect, ZoneId,
};
use crate::core::world::{DndWorld, WindowKey};
use crate::core::{DropQuery, DropReceipt};

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
            } => world.finish_untracked(DragCompletion::Dropped),
        }
    }
}

/// Settle capability for one delivery. World deliveries carry the receiving
/// window so it is elected before the shared context enters settling.
pub(crate) struct SettleRoute<'a, T: Clone + 'static> {
    pub(crate) flag: Option<SettleFlag<T>>,
    pub(crate) owner: Option<(&'a DndWorld<T>, WindowKey)>,
}

pub(crate) fn drop_query<T: Clone + 'static>(
    dnd: &DndContext<T>,
    payload: T,
    proposed_effect: DropEffect,
) -> DropQuery<T> {
    DropQuery {
        payload,
        source: dnd.source(),
        proposed_effect,
        mode: dnd.mode(),
        pointer_kind: dnd.pointer_kind(),
        drag_id: dnd.drag_id(),
    }
}

fn active_rect<T: Clone + 'static>(dnd: &DndContext<T>, point: Point) -> Option<Rect> {
    let source = dnd.source_rect()?;
    let grab = dnd.grab();
    Some(Rect::new(
        point.x - grab.x,
        point.y - grab.y,
        source.width,
        source.height,
    ))
}

/// Resolve a live drag through the registry's configured collision and
/// acceptance policies.
pub(crate) fn resolve_drag_target<T: Clone + 'static>(
    registry: ZoneRegistry<T>,
    dnd: &DndContext<T>,
    point: Point,
    proposed_effect: DropEffect,
    max_distance: f64,
) -> Option<ZoneId> {
    let payload = dnd.payload()?;
    let query = drop_query(dnd, payload, proposed_effect);
    registry
        .resolve(&query, point, active_rect(dnd, point), max_distance)
        .map(|(zone, _)| zone)
}

pub(crate) fn resolve_drag_hover<T: Clone + 'static>(
    registry: ZoneRegistry<T>,
    dnd: &DndContext<T>,
    point: Point,
    proposed_effect: DropEffect,
) -> Option<ZoneId> {
    let payload = dnd.payload()?;
    let query = drop_query(dnd, payload, proposed_effect);
    registry
        .resolve_hover(&query, point, active_rect(dnd, point), dnd.over())
        .map(|(zone, _)| zone)
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
    let Some(p) = dnd.payload() else {
        return false;
    };
    let query = drop_query(dnd, p.clone(), effect);
    let Some(negotiated) = registry.negotiate_zone(target, &query) else {
        return false;
    };
    let effect = negotiated.effect;
    let drag = dnd
        .monitor_has_listeners()
        .then(|| dnd.snapshot())
        .flatten();
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
        let outcome = DropOutcome {
            payload: p,
            from,
            to: target,
            effect,
            mode,
            client: point,
            element: point - origin,
            grab,
            edge: match (mode, negotiated.edge, target_rect) {
                (DragMode::Pointer, Some(edges), Some(rect)) => Some(edge_of(point, rect, edges)),
                _ => None,
            },
        };
        if let Some(drag) = drag {
            dnd.emit_dropped(DropReceipt {
                drag,
                outcome: outcome.clone(),
            });
        }
        negotiated.record.on_drop.call(outcome);
        completion.finalize(dnd);
        return true;
    }
    false
}

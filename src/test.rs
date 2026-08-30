#![doc = include_str!("../docs/api/testing.md")]

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

use dioxus::prelude::*;

use crate::core::components::{
    deliver_drop, drop_query, resolve_drag_hover, resolve_drag_target, DropCompletion, SettleRoute,
};
use crate::core::hooks::SettleFlag;
use crate::core::monitor::CancelReason;
use crate::core::world::{JoinedWindow, WorldHit, WorldMembership};
use crate::core::{
    use_dnd, use_zone_registry, DndContext, DragCompletion, DropEffect, Point, Rect, WindowKey,
    ZoneId, ZoneRegistry,
};

thread_local! {
    /// Handles captured by [`DragSimProbe`], keyed by payload type. One
    /// slot per type per thread: the most recently mounted probe wins,
    /// which is exactly right for one `VirtualDom` per test.
    static SIMS: RefCell<HashMap<TypeId, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

/// Captures a [`DragSim`] for the enclosing provider. Mount one inside the
/// `DndProvider<T>` of your *test* app (it renders nothing), then retrieve
/// the handle with [`drag_sim`] after `rebuild_in_place`.
#[component]
pub fn DragSimProbe<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
) -> Element {
    let _ = phantom;
    let completions = use_signal(Vec::<bool>::new);
    let completion = use_callback(move |dropped| {
        let mut completions = completions;
        completions.write().push(dropped);
    });
    let sim = DragSim {
        dnd: use_dnd::<T>(),
        registry: use_zone_registry::<T>(),
        settle: try_use_context::<SettleFlag<T>>(),
        membership: try_use_context::<WorldMembership<T>>().and_then(|m| m.0),
        completion,
        completions,
    };
    use_hook(move || {
        SIMS.with_borrow_mut(|m| {
            m.insert(TypeId::of::<T>(), Box::new(sim));
        });
    });
    rsx! {}
}

/// The handle the most recent [`DragSimProbe<T>`] captured.
///
/// # Panics
/// Panics when no probe for `T` has mounted - add `DragSimProbe::<T> {}`
/// inside the provider and `rebuild_in_place` first.
pub fn drag_sim<T: Clone + PartialEq + 'static>() -> DragSim<T> {
    SIMS.with_borrow(|m| {
        m.get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<DragSim<T>>())
            .copied()
    })
    .expect("no DragSim captured: mount DragSimProbe::<T> inside the provider and rebuild first")
}

/// Headless driver for one provider's drag world. Every method takes the
/// `VirtualDom` so the underlying signal operations run inside its runtime;
/// call [`rerender`] between actions and markup assertions.
pub struct DragSim<T: Clone + 'static> {
    dnd: DndContext<T>,
    registry: ZoneRegistry<T>,
    settle: Option<SettleFlag<T>>,
    /// The provider's world membership, when it joined a `DndWorld` -
    /// moves and releases then resolve across windows, like the gesture.
    membership: Option<JoinedWindow<T>>,
    completion: Callback<bool>,
    completions: Signal<Vec<bool>>,
}

impl<T: Clone + 'static> Copy for DragSim<T> {}
impl<T: Clone + 'static> Clone for DragSim<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Clone + PartialEq + 'static> DragSim<T> {
    /// Give a zone its client rect - the headless stand-in for layout.
    ///
    /// # Panics
    /// Panics when no zone with this id is registered.
    pub fn place(&self, dom: &VirtualDom, zone: ZoneId, rect: Rect) {
        dom.in_runtime(|| {
            assert!(
                self.registry.contains(zone),
                "place: no zone {} registered",
                zone.0
            );
            let mut registry = self.registry;
            registry.set_rect(zone, rect);
        });
    }

    /// The key this sim's provider joined its world under, when it did.
    pub fn window_key(&self) -> Option<WindowKey> {
        self.membership.map(|j| j.key)
    }

    /// [`Self::place`] for a zone living in another joined window's
    /// registry - `rect` is in **that window's** client px.
    ///
    /// # Panics
    /// Panics when this sim's provider joined no world, the window is
    /// unknown, or the zone isn't registered there.
    pub fn place_in(&self, dom: &VirtualDom, window: WindowKey, zone: ZoneId, rect: Rect) {
        let world = self
            .membership
            .expect("place_in: this provider joined no DndWorld")
            .world;
        dom.in_runtime(|| {
            let rec = world
                .record(window)
                .unwrap_or_else(|| panic!("place_in: no window {} joined", window.0));
            assert!(
                rec.registry.contains(zone),
                "place_in: no zone {} in window {}",
                zone.0,
                window.0
            );
            let mut registry = rec.registry;
            registry.set_rect(zone, rect);
        });
    }

    /// Begin a pointer drag carrying `payload`, from no particular zone.
    pub fn pick_up(&mut self, dom: &VirtualDom, payload: T) {
        self.pick_up_from(dom, payload, None);
    }

    /// Begin a pointer drag, reporting `from` as the source zone
    /// (arrives in `DropOutcome::from`).
    pub fn pick_up_from(&mut self, dom: &VirtualDom, payload: T, from: Option<ZoneId>) {
        let mut dnd = self.dnd;
        let membership = self.membership;
        dom.in_runtime(|| {
            let session = dnd.start_tracked(
                payload,
                from,
                Point::default(),
                Point::default(),
                DropEffect::Move,
                self.completion,
            );
            // Like the gesture: a world drag anchors to this window.
            if dnd.is_session(session) {
                if let Some(j) = membership {
                    j.world.begin_from(j.key);
                }
            }
        });
    }

    /// Move the pointer: updates the tracked position and enters/leaves
    /// zones by hit-testing the placed rects - the same logic the pointer
    /// gesture runs per `pointermove`.
    pub fn move_to(&mut self, dom: &VirtualDom, point: Point) {
        let mut dnd = self.dnd;
        let registry = self.registry;
        let membership = self.membership;
        dom.in_runtime(|| {
            let session = dnd.active_session();
            dnd.update_pointer(point);
            if session.is_some_and(|session| !dnd.is_session(session)) {
                return;
            }
            let query = dnd
                .payload()
                .map(|payload| drop_query(&dnd, payload, dnd.effect()));
            // Same resolution order as the gesture: world hits (any
            // window) are authoritative, unresolved points fall back to
            // the local registry.
            match membership {
                Some(joined) => match query
                    .as_ref()
                    .map(|query| joined.zone_under_query(point, query))
                    .unwrap_or(WorldHit::Unresolved)
                {
                    WorldHit::Zone(location) => joined.enter(location),
                    WorldHit::Window => joined.clear_hover(),
                    WorldHit::Unresolved => {
                        match resolve_drag_hover(registry, &dnd, point, dnd.effect()) {
                            Some(zone) => joined.enter(joined.location(zone)),
                            None => joined.clear_hover(),
                        }
                    }
                },
                None => match resolve_drag_hover(registry, &dnd, point, dnd.effect()) {
                    Some(zone) => dnd.enter(zone),
                    None => {
                        if let Some(over) = dnd.over() {
                            dnd.leave(over);
                        }
                    }
                },
            }
        });
    }

    /// Release at the current pointer position. Returns the zone that
    /// received the drop, or `None` when the drag cancelled (no acceptable
    /// zone under the pointer or within the provider's configured recovery
    /// radius).
    pub fn release(&mut self, dom: &VirtualDom) -> Option<ZoneId> {
        self.release_as(dom, DropEffect::Move)
    }

    /// [`Self::release`] with an explicit effect - simulate the Ctrl-held
    /// copy drop with `DropEffect::Copy`.
    pub fn release_as(&mut self, dom: &VirtualDom, effect: DropEffect) -> Option<ZoneId> {
        let mut dnd = self.dnd;
        let registry = self.registry;
        let settle = self.settle;
        let membership = self.membership;
        dom.in_runtime(|| {
            let point = dnd.pointer();
            let session = dnd.active_session();
            // A release the world resolves into a foreign window delivers
            // there, mirroring the gesture (the snap runs in the target
            // window's own CSS px). Headless rects are placed, so the
            // gesture's pre-snap re-measure is skipped as documented.
            if let Some(j) = membership {
                let _ = j.zone_under(point);
                if let Some((rec, local)) = j.foreign_window_under(point) {
                    let target = dnd.payload().and_then(|payload| {
                        let query = drop_query(&dnd, payload, effect);
                        rec.registry
                            .resolve(
                                &query,
                                local,
                                j.world.active_rect_in(rec, local),
                                rec.registry.release_policy().recovery_radius,
                            )
                            .map(|(zone, _)| zone)
                    });
                    let delivered = target
                        .filter(|t| {
                            deliver_drop(
                                rec.registry,
                                &mut dnd,
                                SettleRoute {
                                    flag: Some(rec.settle),
                                    owner: Some((&j.world, rec.key)),
                                },
                                DropCompletion::World {
                                    world: &j.world,
                                    session,
                                },
                                *t,
                                local,
                                effect,
                            )
                        })
                        .is_some();
                    if !delivered {
                        match session {
                            Some(session) => {
                                j.world.finish_session(
                                    session,
                                    DragCompletion::Cancelled(CancelReason::NoTarget),
                                );
                            }
                            None => j.world.finish_untracked(DragCompletion::Cancelled(
                                CancelReason::NoTarget,
                            )),
                        }
                        return None;
                    }
                    return target;
                }
            }
            let target = resolve_drag_target(
                registry,
                &dnd,
                point,
                effect,
                registry.release_policy().recovery_radius,
            );
            let delivered = target
                .filter(|t| match membership {
                    Some(j) => deliver_drop(
                        registry,
                        &mut dnd,
                        SettleRoute {
                            flag: settle,
                            owner: Some((&j.world, j.key)),
                        },
                        DropCompletion::World {
                            world: &j.world,
                            session,
                        },
                        *t,
                        point,
                        effect,
                    ),
                    None => deliver_drop(
                        registry,
                        &mut dnd,
                        SettleRoute {
                            flag: settle,
                            owner: None,
                        },
                        match session {
                            Some(session) => DropCompletion::Local(session),
                            None => DropCompletion::None,
                        },
                        *t,
                        point,
                        effect,
                    ),
                })
                .is_some();
            if !delivered {
                match membership {
                    Some(j) => match session {
                        Some(session) => {
                            j.world.finish_session(
                                session,
                                DragCompletion::Cancelled(CancelReason::NoTarget),
                            );
                        }
                        None => j
                            .world
                            .finish_untracked(DragCompletion::Cancelled(CancelReason::NoTarget)),
                    },
                    None => match session {
                        Some(session) => {
                            dnd.cancel_session(session, CancelReason::NoTarget);
                        }
                        None => dnd.cancel_with_reason(CancelReason::NoTarget),
                    },
                }
                return None;
            }
            target
        })
    }

    /// Abort the drag, as Escape or a pointer cancel would.
    pub fn cancel(&mut self, dom: &VirtualDom) {
        let mut dnd = self.dnd;
        let membership = self.membership;
        dom.in_runtime(|| {
            let session = dnd.active_session();
            match membership {
                Some(j) => match session {
                    Some(session) => {
                        j.world
                            .finish_session(session, DragCompletion::Cancelled(CancelReason::User));
                    }
                    None => {
                        j.world
                            .finish_untracked(DragCompletion::Cancelled(CancelReason::User));
                    }
                },
                None => match session {
                    Some(session) => {
                        dnd.cancel_session(session, CancelReason::User);
                    }
                    None => dnd.cancel(),
                },
            }
        });
    }

    /// Exactly-once source completion results observed by the simulated
    /// source (`true` for delivered, `false` for cancelled).
    pub fn completions(&self, dom: &VirtualDom) -> Vec<bool> {
        dom.in_runtime(|| self.completions.read().clone())
    }

    /// The zone currently hovered.
    pub fn over(&self, dom: &VirtualDom) -> Option<ZoneId> {
        dom.in_runtime(|| self.dnd.over())
    }

    /// Is a drag in flight?
    pub fn dragging(&self, dom: &VirtualDom) -> bool {
        dom.in_runtime(|| self.dnd.dragging())
    }

    /// The in-flight payload, if any.
    pub fn payload(&self, dom: &VirtualDom) -> Option<T> {
        dom.in_runtime(|| self.dnd.payload())
    }

    /// The latest screen-reader announcement.
    pub fn announcement(&self, dom: &VirtualDom) -> String {
        dom.in_runtime(|| self.dnd.announcement())
    }
}

/// Flush pending reactivity so the tree reflects the simulated state -
/// call between driver actions and markup assertions
/// (`dioxus_ssr::render`).
pub fn rerender(dom: &mut VirtualDom) {
    dom.process_events();
    dom.render_immediate(&mut dioxus::core::NoOpMutations);
}

/// One whole pointer drag: pick `payload` up (from `from`), glide through
/// `path`, release at its last point, re-rendering between steps so zone
/// reactions run just as they would live. Returns the receiving zone, or
/// `None` when the drag cancelled. Needs a mounted [`DragSimProbe<T>`];
/// an empty `path` releases at the pickup point.
pub fn simulate_drag<T: Clone + PartialEq + 'static>(
    dom: &mut VirtualDom,
    payload: T,
    from: Option<ZoneId>,
    path: &[Point],
) -> Option<ZoneId> {
    let mut sim = drag_sim::<T>();
    sim.pick_up_from(dom, payload, from);
    rerender(dom);
    for p in path {
        sim.move_to(dom, *p);
        rerender(dom);
    }
    let delivered = sim.release(dom);
    rerender(dom);
    delivered
}

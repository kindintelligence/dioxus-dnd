//! Headless test driver - drag-and-drop in CI, no browser.
//!
//! The drag state machine is plain Rust over signals, so a whole pointer
//! interaction can run inside a `VirtualDom`: pick up, hover, drop, assert.
//! The one thing a headless run lacks is layout, so *you place the zone
//! rects* - which makes tests deterministic instead of flaky.
//!
//! Mount a [`DragSimProbe`] inside the provider under test, grab the
//! [`DragSim`] it captured, and drive:
//!
//! ```text
//! fn test_app() -> Element {
//!     rsx! {
//!         DndProvider::<Card> {
//!             DragSimProbe::<Card> {}
//!             ShelfApp {}   // the component you're testing
//!         }
//!     }
//! }
//!
//! let mut dom = VirtualDom::new(test_app);
//! dom.rebuild_in_place();
//! let mut sim = drag_sim::<Card>();
//!
//! sim.place(&dom, SHELF, Rect::new(0.0, 100.0, 200.0, 80.0));
//! sim.pick_up(&dom, card.clone());
//! sim.move_to(&dom, Point::new(100.0, 140.0));
//! assert_eq!(sim.over(&dom), Some(SHELF));
//! rerender(&mut dom);
//! assert!(dioxus_ssr::render(&dom).contains("data-over"));
//! assert_eq!(sim.release(&dom), Some(SHELF));   // your on_drop just ran
//! ```
//!
//! Or as one line for the common arc: [`simulate_drag`].
//!
//! Drops go through the *production* delivery path - acceptance filters,
//! `DropOutcome` construction, closest-edge enrichment, settle routing -
//! shared with `Draggable` itself, not a reimplementation. Releases mirror
//! the pointer gesture: an exact hit wins; otherwise the drop snaps to the
//! closest acceptable zone whose edge is within 48px (the touch
//! forgiveness), else the drag cancels. Not simulated: pointer capture,
//! auto-scroll, and the re-measure that precedes the real snap (headless
//! rects are wherever you placed them).

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

use dioxus::prelude::*;

use crate::core::components::{deliver_drop, DropCompletion, SettleRoute};
use crate::core::hooks::SettleFlag;
use crate::core::world::{JoinedWindow, WorldHit, WorldMembership};
use crate::core::{
    use_dnd, use_zone_registry, DndContext, DropEffect, Point, Rect, WindowKey, ZoneId,
    ZoneRegistry,
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
            dnd.start_tracked(
                payload,
                from,
                Point::default(),
                Point::default(),
                DropEffect::Move,
                self.completion,
            );
            // Like the gesture: a world drag anchors to this window.
            if let Some(j) = membership {
                j.world.begin_from(j.key);
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
            dnd.update_pointer(point);
            // Same resolution order as the gesture: world hits (any
            // window) are authoritative, unresolved points fall back to
            // the local registry.
            match membership {
                Some(joined) => match joined.zone_under(point) {
                    WorldHit::Zone(location) => joined.enter(location),
                    WorldHit::Window => joined.clear_hover(),
                    WorldHit::Unresolved => match registry.hit_test(point) {
                        Some(zone) => joined.enter(joined.location(zone)),
                        None => joined.clear_hover(),
                    },
                },
                None => match registry.hit_test(point) {
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
    /// zone under the pointer, and none with an edge within the 48px
    /// snap).
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
                    let target = rec.registry.hit_test(local).or_else(|| {
                        dnd.payload()
                            .and_then(|p| rec.registry.hit_test_closest(local, &p, 48.0))
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
                                j.world.finish_session(session, false);
                            }
                            None => j.world.finish_untracked(false),
                        }
                        return None;
                    }
                    return target;
                }
            }
            let target = registry.hit_test(point).or_else(|| {
                dnd.payload()
                    .and_then(|p| registry.hit_test_closest(point, &p, 48.0))
            });
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
                            j.world.finish_session(session, false);
                        }
                        None => j.world.finish_untracked(false),
                    },
                    None => match session {
                        Some(session) => {
                            dnd.cancel_session(session);
                        }
                        None => dnd.cancel(),
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
                        j.world.finish_session(session, false);
                    }
                    None => j.world.finish_untracked(false),
                },
                None => match session {
                    Some(session) => {
                        dnd.cancel_session(session);
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

#![doc = include_str!("../../docs/api/drop-effects.md")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::rc::Rc;

use dioxus::prelude::{provide_context, use_hook};
use dioxus::signals::{AnyStorage, Owner, SyncStorage, UnsyncStorage};

use super::{DropEffect, DropOutcome, ZoneId};

thread_local! {
    /// Owner pairs backing models created by [`use_dnd_model`]. App-wide
    /// models deliberately outlive every window: a copyable signal/store
    /// handle must never dangle because the window that created it closed.
    /// Bounded in normal use to one scope per app-wide model. `ManuallyDrop`
    /// is load-bearing: ordinary thread-local values are destroyed when their
    /// creator thread exits, which would make a transferred `SyncSignal`
    /// dangle before process exit.
    static MODEL_OWNERS: RefCell<Vec<ManuallyDrop<DndScope>>> = const { RefCell::new(Vec::new()) };
}

/// An explicit lifetime for Dioxus signals and stores created outside a
/// component scope.
///
/// A scope owns both storage flavors Dioxus state may allocate: ordinary
/// signals use [`UnsyncStorage`], while a [`Store`](struct@dioxus::prelude::Store)
/// keeps its subscription tree in [`SyncStorage`]. Create state inside
/// [`with`](Self::with), then retain a clone of the scope for exactly as long
/// as that state may be used. Storage is reclaimed when the last clone drops.
///
/// Use [`use_dnd_model`] instead for an app-wide model shared by windows. A
/// `DndScope` is intended for dynamic state whose lifetime really should end,
/// such as the contents owned by one spawned window.
///
/// `with` must run inside a Dioxus runtime, like the state constructors it
/// contains.
///
/// Do not drop the last scope clone while any owned read or write guard is
/// live. Unsynchronized storage cannot be recycled through an active
/// `RefCell` borrow and synchronized storage must wait for its lock guard.
///
/// ```no_run
/// use dioxus::prelude::*;
/// use dioxus_dnd::prelude::DndScope;
///
/// fn app() -> Element {
///     let scope = use_hook(DndScope::new);
///     let count = use_hook(|| scope.with(|| Signal::new(0)));
///     rsx! { "{count}" }
/// }
/// ```
#[must_use = "keep a DndScope alive while using state created under it"]
#[derive(Clone)]
pub struct DndScope {
    owners: Rc<DndScopeOwners>,
}

struct DndScopeOwners {
    unsync: Owner<UnsyncStorage>,
    sync: Owner<SyncStorage>,
}

impl DndScope {
    /// Create an empty scope. Mint every signal or store it owns with
    /// [`Self::with`].
    pub fn new() -> Self {
        Self {
            owners: Rc::new(DndScopeOwners {
                unsync: UnsyncStorage::owner(),
                sync: SyncStorage::owner(),
            }),
        }
    }

    /// Run `init` with this scope as the current owner for both Dioxus
    /// storage flavors.
    ///
    /// Owner restoration is unwind-safe: a panic from `init` is resumed only
    /// after both Dioxus owner overrides have returned normally and restored
    /// their previous values.
    pub fn with<R>(&self, init: impl FnOnce() -> R) -> R {
        let result = dioxus::core::with_owner(self.owners.unsync.clone(), || {
            dioxus::core::with_owner(self.owners.sync.clone(), || {
                catch_unwind(AssertUnwindSafe(init))
            })
        });
        match result {
            Ok(value) => value,
            Err(panic) => resume_unwind(panic),
        }
    }
}

impl Default for DndScope {
    fn default() -> Self {
        Self::new()
    }
}

/// Create and provide an app-wide model whose Dioxus state survives every
/// window close order.
///
/// `init` runs once for this component instance under a paired, process-lived
/// [`DndScope`]. The returned model is also provided in context. Seed spawned
/// windows with the model by chaining `with_root_context(model)` after
/// [`DndWorld::vdom`](crate::core::DndWorld::vdom).
/// The process lifetime is deliberate: copyable signal and store handles do
/// not carry an ownership guard, so tying storage to a particular window (or
/// to an `Rc` callers must remember to propagate) can leave a survivor holding
/// a dangling handle.
///
/// Every signal, store, or other owner-backed value that needs this lifetime
/// must be allocated synchronously inside `init`. Wrapping a handle created
/// earlier does not reparent it, and an allocation performed later uses
/// whichever owner is current then. For later app-lived allocations, mint
/// them under a new [`DndScope`] and retain that scope in process-lived model
/// state.
///
/// Call this once for each app-wide model. Use [`DndScope`] for dynamic state
/// that should be reclaimed before process exit.
///
/// # Allocation boundary
///
/// The following compiles, but does **not** give `cards` process lifetime:
/// it was already owned by the component before `use_dnd_model` ran.
///
/// ```no_run
/// use dioxus::prelude::*;
/// use dioxus_dnd::prelude::use_dnd_model;
///
/// #[derive(Clone, Copy)]
/// struct Model {
///     cards: Signal<Vec<String>>,
/// }
///
/// fn app() -> Element {
///     let cards = use_signal(Vec::<String>::new);
///     let _model = use_dnd_model(|| Model { cards }); // not reparented
///     rsx! {}
/// }
/// ```
///
/// Allocate the signal inside the initializer instead:
///
/// ```
/// use dioxus::prelude::*;
/// use dioxus_dnd::prelude::use_dnd_model;
///
/// #[derive(Clone, Copy)]
/// struct Model {
///     cards: Signal<Vec<String>>,
/// }
///
/// fn app() -> Element {
///     let model = use_dnd_model(|| Model {
///         cards: Signal::new(Vec::new()),
///     });
///     rsx! { "{model.cards.read().len()} cards" }
/// }
/// ```
pub fn use_dnd_model<M: Clone + 'static>(init: impl FnOnce() -> M) -> M {
    use_hook(move || {
        let scope = DndScope::new();
        let model = scope.with(init);
        MODEL_OWNERS.with_borrow_mut(|owners| owners.push(ManuallyDrop::new(scope)));
        provide_context(model)
    })
}

/// Apply a drop to a `HashMap<ZoneId, Vec<T>>` model.
///
/// `Move` removes the matching item from `outcome.from` before appending it
/// to `outcome.to`. `Copy` leaves the source alone and passes the payload
/// through `clone_item` first, which is where you should assign a fresh id.
///
/// Semantics worth knowing:
///
/// - Removal matches **every** item in the source whose key equals the
///   payload's key. Keys are expected to be unique within a zone; if they
///   are not, a single `Move` prunes all of them.
/// - A `Move` where `from == Some(to)` removes and re-appends, so dropping
///   an item back onto its own zone sends it to the **end of that list**.
/// - A `Move` with `from: None` (payload from outside any zone, e.g. a
///   palette) skips removal and just appends.
/// - An unknown `to` zone is created on the fly rather than dropping the
///   item on the floor.
pub fn apply_clone_or_move<T, K>(
    zones: &mut HashMap<ZoneId, Vec<T>>,
    outcome: DropOutcome<T>,
    key: impl Fn(&T) -> K,
    mut clone_item: impl FnMut(T) -> T,
) where
    K: PartialEq,
{
    let DropOutcome {
        payload,
        from,
        to,
        effect,
        ..
    } = outcome;
    let item = if effect == DropEffect::Copy {
        clone_item(payload)
    } else {
        if let Some(from) = from {
            let payload_key = key(&payload);
            if let Some(source) = zones.get_mut(&from) {
                source.retain(|item| key(item) != payload_key);
            }
        }
        payload
    };

    zones.entry(to).or_default().push(item);
}

/// Apply a drop between two plain `Vec<T>` lists.
///
/// `Move` removes the matching item from `source` before appending it to
/// `target`. `Copy` leaves the source alone and passes the payload through
/// `clone_item` first, which is where you should assign a fresh id.
///
/// You choose which lists to pass, so the outcome's `from` and `to` fields
/// are **ignored** here; only `payload` and `effect` are consulted. Pass
/// `None` for `source` when the payload came from outside any list. As with
/// [`apply_clone_or_move`], removal matches every item whose key equals the
/// payload's key.
pub fn apply_list_clone_or_move<T, K>(
    source: Option<&mut Vec<T>>,
    target: &mut Vec<T>,
    outcome: DropOutcome<T>,
    key: impl Fn(&T) -> K,
    mut clone_item: impl FnMut(T) -> T,
) where
    K: PartialEq,
{
    let DropOutcome {
        payload, effect, ..
    } = outcome;
    let item = if effect == DropEffect::Copy {
        clone_item(payload)
    } else {
        if let Some(source) = source {
            let payload_key = key(&payload);
            source.retain(|item| key(item) != payload_key);
        }
        payload
    };

    target.push(item);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{DragMode, Point};
    use dioxus::prelude::*;
    use dioxus::signals::SyncSignal;
    use std::cell::Cell;
    use std::sync::mpsc::Sender;

    #[derive(Debug, Clone, PartialEq)]
    struct Card {
        id: u32,
        title: &'static str,
    }

    #[derive(Clone, Copy, PartialEq)]
    struct SignalModel {
        value: Signal<i32>,
    }

    type SignalModelSlot = Rc<RefCell<Option<SignalModel>>>;

    #[derive(Store, Clone, PartialEq)]
    struct StoreState {
        value: i32,
    }

    #[derive(Clone, Copy, PartialEq)]
    struct StoreModel {
        state: Store<StoreState>,
    }

    type StoreModelSlot = Rc<RefCell<Option<StoreModel>>>;

    type ScopeSlot = Rc<RefCell<Option<(DndScope, Signal<i32>, Store<i32>, SyncSignal<i32>)>>>;

    type PanicScopeSlot = Rc<RefCell<Option<(DndScope, DndScope, Signal<i32>)>>>;

    type ScopePairSlot = Rc<RefCell<Option<(DndScope, Signal<i32>, DndScope, Signal<i32>)>>>;

    #[derive(Clone, Copy, PartialEq)]
    struct ThreadModel {
        value: SyncSignal<i32>,
    }

    #[derive(Default)]
    struct SurvivorProbe {
        renders: Cell<usize>,
        value: Cell<i32>,
    }

    fn signal_model_creator() -> Element {
        let slot = use_context::<SignalModelSlot>();
        let model = use_dnd_model(|| SignalModel {
            value: Signal::new(0),
        });
        *slot.borrow_mut() = Some(model);
        rsx! {}
    }

    fn signal_model_survivor() -> Element {
        let model = use_context::<SignalModel>();
        let probe = use_context::<Rc<SurvivorProbe>>();
        let value = *model.value.read();
        probe.value.set(value);
        probe.renders.set(probe.renders.get() + 1);
        rsx! { "{value}" }
    }

    fn store_model_creator() -> Element {
        let slot = use_context::<StoreModelSlot>();
        let model = use_dnd_model(|| StoreModel {
            state: Store::new(StoreState { value: 0 }),
        });
        *slot.borrow_mut() = Some(model);
        rsx! {}
    }

    fn store_model_survivor() -> Element {
        let model = use_context::<StoreModel>();
        let probe = use_context::<Rc<SurvivorProbe>>();
        let value = *model.state.value().read();
        probe.value.set(value);
        probe.renders.set(probe.renders.get() + 1);
        rsx! { "{value}" }
    }

    fn scoped_state_creator() -> Element {
        let slot = use_context::<ScopeSlot>();
        let state = use_hook(|| {
            let scope = DndScope::new();
            let (signal, store, sync_signal) =
                scope.with(|| (Signal::new(1), Store::new(2), SyncSignal::new_maybe_sync(3)));
            (scope, signal, store, sync_signal)
        });
        *slot.borrow_mut() = Some(state);
        rsx! {}
    }

    fn panic_scope_creator() -> Element {
        let slot = use_context::<PanicScopeSlot>();
        let state = use_hook(|| {
            let outer = DndScope::new();
            let inner = DndScope::new();
            let signal = outer.with(|| {
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    inner.with(|| panic!("expected owner-restoration probe"));
                }));
                assert!(panic.is_err());
                Signal::new(7)
            });
            (outer, inner, signal)
        });
        *slot.borrow_mut() = Some(state);
        rsx! {}
    }

    fn scope_pair_creator() -> Element {
        let slot = use_context::<ScopePairSlot>();
        let state = use_hook(|| {
            let first = DndScope::new();
            let first_signal = first.with(|| Signal::new(1));
            let second = DndScope::new();
            let second_signal = second.with(|| Signal::new(2));
            (first, first_signal, second, second_signal)
        });
        *slot.borrow_mut() = Some(state);
        rsx! {}
    }

    fn scoped_survivor() -> Element {
        let value = *use_context::<Signal<i32>>().read();
        let probe = use_context::<Rc<SurvivorProbe>>();
        probe.value.set(value);
        probe.renders.set(probe.renders.get() + 1);
        rsx! { "{value}" }
    }

    fn thread_model_creator() -> Element {
        let sender = use_context::<Sender<ThreadModel>>();
        let model = use_dnd_model(|| ThreadModel {
            value: SyncSignal::new_maybe_sync(21),
        });
        sender.send(model).expect("receiver remains alive");
        rsx! {}
    }

    #[test]
    fn dnd_scope_reclaims_state_only_after_its_last_clone_drops() {
        let slot = ScopeSlot::default();
        let mut creator = VirtualDom::new(scoped_state_creator).with_root_context(slot.clone());
        creator.rebuild_in_place();
        let (scope, signal, mut store, sync_signal) = slot
            .borrow_mut()
            .take()
            .expect("creator provided its scoped state");

        // The hook-owned clone drops with the creator; this retained clone is
        // now the sole lifetime guard.
        drop(creator);
        store.set(3);
        assert_eq!(*signal.peek(), 1);
        assert_eq!(*store.peek(), 3);

        drop(scope);
        assert!(signal.try_read().is_err());
        assert!(sync_signal.try_read().is_err());
    }

    #[test]
    fn dnd_scope_restores_outer_owners_before_resuming_a_panic() {
        let slot = PanicScopeSlot::default();
        let mut creator = VirtualDom::new(panic_scope_creator).with_root_context(slot.clone());
        creator.rebuild_in_place();
        let (outer, inner, signal) = slot
            .borrow_mut()
            .take()
            .expect("creator provided its scopes");

        drop(creator);
        drop(inner);
        assert_eq!(*signal.peek(), 7, "signal must remain owned by outer");
        drop(outer);
        assert!(signal.try_read().is_err());
    }

    #[test]
    fn retiring_one_dynamic_scope_does_not_break_a_surviving_sibling() {
        let slot = ScopePairSlot::default();
        let mut creator = VirtualDom::new(scope_pair_creator).with_root_context(slot.clone());
        creator.rebuild_in_place();
        let (first, first_signal, second, second_signal) = slot
            .borrow_mut()
            .take()
            .expect("creator provided both scopes");
        let probe = Rc::new(SurvivorProbe::default());
        let mut survivor = VirtualDom::new(scoped_survivor)
            .with_root_context(second_signal)
            .with_root_context(probe.clone());
        survivor.rebuild_in_place();
        let renders_before_close = probe.renders.get();

        drop(creator);
        drop(first);
        assert!(first_signal.try_read().is_err());
        survivor.in_runtime(|| {
            let mut value = second_signal;
            value.set(9);
        });
        survivor.render_immediate(&mut dioxus::core::NoOpMutations);
        assert_eq!(probe.value.get(), 9);
        assert!(probe.renders.get() > renders_before_close);

        drop(survivor);
        drop(second);
        assert!(second_signal.try_read().is_err());
    }

    #[test]
    fn model_sync_storage_survives_its_creator_thread() {
        let (sender, receiver) = std::sync::mpsc::channel::<ThreadModel>();
        std::thread::spawn(move || {
            let mut creator =
                VirtualDom::new(thread_model_creator).with_root_context(sender.clone());
            creator.rebuild_in_place();
        })
        .join()
        .expect("creator thread completed");

        let model = receiver.recv().expect("creator published its model");
        assert_eq!(*model.value.peek(), 21);
        let mut value = model.value;
        value.set(22);
        assert_eq!(*model.value.peek(), 22);
    }

    #[test]
    fn model_survives_its_creator_window() {
        let slot = SignalModelSlot::default();
        let mut creator = VirtualDom::new(signal_model_creator).with_root_context(slot.clone());
        creator.rebuild_in_place();
        let model = slot
            .borrow_mut()
            .take()
            .expect("creator provided its model");
        let probe = Rc::new(SurvivorProbe::default());
        let mut survivor = VirtualDom::new(signal_model_survivor)
            .with_root_context(model)
            .with_root_context(probe.clone());
        survivor.rebuild_in_place();
        let renders_before_close = probe.renders.get();

        drop(creator);
        survivor.in_runtime(|| {
            let mut value = model.value;
            value.set(7);
        });
        survivor.render_immediate(&mut dioxus::core::NoOpMutations);

        assert_eq!(probe.value.get(), 7);
        assert!(probe.renders.get() > renders_before_close);
    }

    #[test]
    fn store_model_keeps_its_sync_subscription_storage_after_creator_close() {
        let slot = StoreModelSlot::default();
        let mut creator = VirtualDom::new(store_model_creator).with_root_context(slot.clone());
        creator.rebuild_in_place();
        let model = slot
            .borrow_mut()
            .take()
            .expect("creator provided its store model");
        let probe = Rc::new(SurvivorProbe::default());
        let mut survivor = VirtualDom::new(store_model_survivor)
            .with_root_context(model)
            .with_root_context(probe.clone());
        survivor.rebuild_in_place();
        let renders_before_close = probe.renders.get();

        drop(creator);
        survivor.in_runtime(|| model.state.value().set(11));
        survivor.render_immediate(&mut dioxus::core::NoOpMutations);

        assert_eq!(probe.value.get(), 11);
        assert!(probe.renders.get() > renders_before_close);
    }

    fn outcome(
        payload: Card,
        from: Option<ZoneId>,
        to: ZoneId,
        effect: DropEffect,
    ) -> DropOutcome<Card> {
        DropOutcome {
            payload,
            from,
            to,
            effect,
            mode: DragMode::Pointer,
            client: Point::default(),
            element: Point::default(),
            grab: Point::default(),
            edge: None,
        }
    }

    #[test]
    fn move_removes_from_source_and_appends_to_target() {
        let a = ZoneId(1);
        let b = ZoneId(2);
        let mut zones = HashMap::from([
            (
                a,
                vec![
                    Card {
                        id: 1,
                        title: "one",
                    },
                    Card {
                        id: 2,
                        title: "two",
                    },
                ],
            ),
            (
                b,
                vec![Card {
                    id: 3,
                    title: "three",
                }],
            ),
        ]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 2,
                    title: "two",
                },
                Some(a),
                b,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            zones[&a],
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            zones[&b],
            vec![
                Card {
                    id: 3,
                    title: "three"
                },
                Card {
                    id: 2,
                    title: "two"
                }
            ]
        );
    }

    #[test]
    fn copy_leaves_source_and_allows_new_identity() {
        let a = ZoneId(1);
        let b = ZoneId(2);
        let mut zones = HashMap::from([
            (
                a,
                vec![Card {
                    id: 1,
                    title: "one",
                }],
            ),
            (b, Vec::new()),
        ]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(a),
                b,
                DropEffect::Copy,
            ),
            |card| card.id,
            |mut card| {
                card.id = 10;
                card
            },
        );

        assert_eq!(
            zones[&a],
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            zones[&b],
            vec![Card {
                id: 10,
                title: "one"
            }]
        );
    }

    /// Pins the self-drop semantics documented on `apply_clone_or_move`: a
    /// `Move` back onto the source zone reorders the item to the end.
    #[test]
    fn move_onto_own_zone_reorders_to_end() {
        let a = ZoneId(1);
        let mut zones = HashMap::from([(
            a,
            vec![
                Card {
                    id: 1,
                    title: "one",
                },
                Card {
                    id: 2,
                    title: "two",
                },
            ],
        )]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(a),
                a,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            zones[&a],
            vec![
                Card {
                    id: 2,
                    title: "two"
                },
                Card {
                    id: 1,
                    title: "one"
                }
            ]
        );
    }

    /// A payload from outside any zone (palette, external drop) has no
    /// source to prune; `Move` just appends.
    #[test]
    fn move_without_source_zone_just_appends() {
        let b = ZoneId(2);
        let mut zones = HashMap::from([(b, Vec::new())]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 7,
                    title: "seven",
                },
                None,
                b,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            zones[&b],
            vec![Card {
                id: 7,
                title: "seven"
            }]
        );
    }

    /// An unknown target zone is created rather than losing the item.
    #[test]
    fn unknown_target_zone_is_created() {
        let a = ZoneId(1);
        let ghost = ZoneId(99);
        let mut zones = HashMap::from([(
            a,
            vec![Card {
                id: 1,
                title: "one",
            }],
        )]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(a),
                ghost,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert!(zones[&a].is_empty());
        assert_eq!(
            zones[&ghost],
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
    }

    #[test]
    fn list_move_removes_from_source_and_appends_to_target() {
        let mut source = vec![
            Card {
                id: 1,
                title: "one",
            },
            Card {
                id: 2,
                title: "two",
            },
        ];
        let mut target = vec![Card {
            id: 3,
            title: "three",
        }];

        apply_list_clone_or_move(
            Some(&mut source),
            &mut target,
            outcome(
                Card {
                    id: 2,
                    title: "two",
                },
                Some(ZoneId(1)),
                ZoneId(2),
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            source,
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            target,
            vec![
                Card {
                    id: 3,
                    title: "three"
                },
                Card {
                    id: 2,
                    title: "two"
                }
            ]
        );
    }

    #[test]
    fn list_copy_leaves_source_and_allows_new_identity() {
        let mut source = vec![Card {
            id: 1,
            title: "one",
        }];
        let mut target = Vec::new();

        apply_list_clone_or_move(
            Some(&mut source),
            &mut target,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(ZoneId(1)),
                ZoneId(2),
                DropEffect::Copy,
            ),
            |card| card.id,
            |mut card| {
                card.id = 10;
                card
            },
        );

        assert_eq!(
            source,
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            target,
            vec![Card {
                id: 10,
                title: "one"
            }]
        );
    }

    /// `Move` into a list without a source (`None`) skips removal.
    #[test]
    fn list_move_without_source_just_appends() {
        let mut target = Vec::new();

        apply_list_clone_or_move(
            None,
            &mut target,
            outcome(
                Card {
                    id: 7,
                    title: "seven",
                },
                None,
                ZoneId(2),
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            target,
            vec![Card {
                id: 7,
                title: "seven"
            }]
        );
    }
}

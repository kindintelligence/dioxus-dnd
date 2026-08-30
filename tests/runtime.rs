//! Runtime tests: exercise the store/signal-backed state machines inside a
//! headless `VirtualDom`, and assert rendered accessibility attributes via
//! SSR. Assertions live inside test components - panics propagate through
//! `rebuild_in_place`, failing the test.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;
use dioxus_dnd::test::{drag_sim, rerender, simulate_drag, DragSimProbe};
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

type Shared<T> = Arc<Mutex<T>>;

/// Build a one-shot headless app and return its SSR output.
fn run(app: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

// --- DndContext state machine ------------------------------------------

thread_local! {
    static FROM_PARTS_STATE: RefCell<Option<Store<DragState<u8>>>> = const { RefCell::new(None) };
    static FROM_PARTS_CONTEXTS: RefCell<Option<(DndContext<u8>, DndContext<u8>)>> =
        const { RefCell::new(None) };
}

fn from_parts_app() -> Element {
    let state = use_store(DragState::<u8>::default);
    let announcement = use_signal(String::new);
    let first = use_hook(|| DndContext::from_parts(state, announcement));
    let second = use_hook(|| DndContext::from_parts(state, announcement));
    use_hook(move || {
        FROM_PARTS_STATE.with_borrow_mut(|slot| *slot = Some(state));
        FROM_PARTS_CONTEXTS.with_borrow_mut(|slot| *slot = Some((first, second)));
    });
    rsx! {}
}

#[test]
fn from_parts_preserves_shared_store_behavior() {
    let mut dom = VirtualDom::new(from_parts_app);
    dom.rebuild_in_place();

    dom.in_runtime(|| {
        let mut state = FROM_PARTS_STATE.with_borrow(|slot| slot.expect("state mounted"));
        let (mut first, mut second) =
            FROM_PARTS_CONTEXTS.with_borrow(|slot| slot.expect("contexts mounted"));

        assert!(first == second, "legacy wrappers share handle identity");
        state.set(DragState {
            payload: Some(7),
            effect: DropEffect::Copy,
            ..DragState::default()
        });
        assert!(first.dragging());
        assert!(second.dragging());
        assert_eq!(first.payload(), Some(7));

        second.cancel();
        assert!(!first.dragging());
        assert_eq!(state.read().payload, None);

        first.start(
            9,
            Some(ZoneId(4)),
            Point::new(2.0, 3.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        assert!(second.dragging(), "the sibling wrapper observes the start");
        second.cancel();
        assert!(
            !first.dragging(),
            "the sibling wrapper can cancel shared state"
        );

        let destination = Rect::new(10.0, 20.0, 30.0, 40.0);
        state.set(DragState {
            payload: Some(11),
            settle: Some(destination),
            ..DragState::default()
        });
        assert!(!first.dragging());
        assert_eq!(second.settling(), Some(destination));
        first.finish_settle();
        assert_eq!(state.read().payload, None);
    });
}

#[test]
fn dnd_context_lifecycle() {
    fn app() -> Element {
        let mut dnd = use_dnd_provider::<String>();
        assert!(!dnd.dragging());

        dnd.start(
            "cargo".to_string(),
            Some(ZoneId(7)),
            Point::new(3.0, 4.0),
            Point::new(1.0, 1.0),
            DropEffect::Move,
            DragMode::Pointer,
        );
        assert!(dnd.dragging());
        assert_eq!(dnd.payload().as_deref(), Some("cargo"));
        assert_eq!(dnd.source(), Some(ZoneId(7)));
        assert_eq!(dnd.pointer(), Point::new(3.0, 4.0));
        assert_eq!(dnd.grab(), Point::new(1.0, 1.0));
        assert_eq!(dnd.mode(), DragMode::Pointer);

        // (0,0) pointer samples are noise from some webviews - filtered.
        dnd.update_pointer(Point::new(0.0, 0.0));
        assert_eq!(dnd.pointer(), Point::new(3.0, 4.0));
        dnd.update_pointer(Point::new(9.0, 9.0));
        assert_eq!(dnd.pointer(), Point::new(9.0, 9.0));

        // leave() only clears the hover if that zone is still hovered.
        dnd.enter(ZoneId(1));
        dnd.enter(ZoneId(2)); // moved to an adjacent zone…
        dnd.leave(ZoneId(1)); // …then the stale leave for zone 1 arrives
        assert_eq!(dnd.over(), Some(ZoneId(2)));
        dnd.leave(ZoneId(2));
        assert_eq!(dnd.over(), None);

        // take() hands back payload+source and resets everything.
        dnd.enter(ZoneId(2));
        let (payload, source) = dnd.take().expect("payload present");
        assert_eq!(payload, "cargo");
        assert_eq!(source, Some(ZoneId(7)));
        assert!(!dnd.dragging());
        assert_eq!(dnd.over(), None);
        assert!(dnd.take().is_none(), "second take yields nothing");

        // cancel() from mid-drag also resets.
        dnd.start(
            "x".into(),
            None,
            Point::default(),
            Point::default(),
            DropEffect::Copy,
            DragMode::Keyboard,
        );
        dnd.cancel();
        assert!(!dnd.dragging());

        // announcements flow through their own channel
        dnd.announce("hello");
        assert_eq!(dnd.announcement(), "hello");

        rsx! { div {} }
    }
    run(app);
}

// --- ZoneRegistry --------------------------------------------------------

#[test]
fn registry_register_replace_unregister_and_labels() {
    fn app() -> Element {
        use_dnd_provider::<u32>();
        let mut reg = use_zone_registry::<u32>();

        let record = |id: u64, label: &str| {
            let mut record = ZoneRecord::<u32>::new(ZoneId(id), Callback::new(|_| {}));
            record.label = Some(label.to_string());
            record
        };

        let first_registration = reg.register(record(1, "one"));
        reg.register(record(2, "two"));
        assert_eq!(reg.get(ZoneId(1)).unwrap().label.as_deref(), Some("one"));

        reg.set_rect_if_present(first_registration, Rect::new(0.0, 0.0, 100.0, 40.0));
        assert_eq!(
            reg.cached_rect(ZoneId(1)),
            Some(Rect::new(0.0, 0.0, 100.0, 40.0))
        );

        // re-registering the same id replaces, not duplicates
        let replacement = reg.register(record(1, "uno"));
        assert_eq!(
            reg.acceptable(&0)
                .iter()
                .map(|zone| zone.id)
                .collect::<Vec<_>>(),
            vec![ZoneId(1), ZoneId(2)],
            "same-id replacement retains its registry slot"
        );
        assert_eq!(reg.get(ZoneId(1)).unwrap().label.as_deref(), Some("uno"));

        // A measurement captured by the first registration cannot land in
        // its same-id replacement. The current generation still can.
        reg.set_rect_if_present(first_registration, Rect::new(0.0, 0.0, 999.0, 999.0));
        assert_eq!(reg.cached_rect(ZoneId(1)), None);
        reg.set_rect_if_present(replacement, Rect::new(5.0, 6.0, 70.0, 30.0));
        assert_eq!(
            reg.cached_rect(ZoneId(1)),
            Some(Rect::new(5.0, 6.0, 70.0, 30.0))
        );

        // sync_label updates in place, and is a no-op for unknown ids
        reg.sync_label(ZoneId(2), Some("zwei".into()));
        assert_eq!(reg.get(ZoneId(2)).unwrap().label.as_deref(), Some("zwei"));
        reg.sync_label(ZoneId(99), Some("ghost".into()));
        assert!(reg.get(ZoneId(99)).is_none());

        reg.unregister(ZoneId(1));
        assert!(reg.get(ZoneId(1)).is_none());
        assert_eq!(reg.acceptable(&0).len(), 1);
        reg.set_rect_if_present(replacement, Rect::new(1.0, 2.0, 3.0, 4.0));
        assert!(
            reg.get(ZoneId(1)).is_none(),
            "a stale write must not resurrect"
        );

        rsx! { div {} }
    }
    run(app);
}

#[test]
fn registry_spatial_step_accepts_and_hit_test() {
    fn app() -> Element {
        use_dnd_provider::<u32>();
        let mut reg = use_zone_registry::<u32>();

        let record = |id: u64, rect: Option<Rect>, accepts: Option<Callback<u32, bool>>| {
            let mut record = ZoneRecord::<u32>::new(ZoneId(id), Callback::new(|_| {}));
            record.accepts = accepts;
            record.rect = rect;
            record
        };

        // Registered in one order, laid out in another:
        //   A(id 1) at y=100        (visually last)
        //   B(id 2) at y=0, x=50    (visually second)
        //   C(id 3) at y=0, x=0     (visually first)
        reg.register(record(1, Some(Rect::new(0.0, 100.0, 40.0, 40.0)), None));
        reg.register(record(2, Some(Rect::new(50.0, 0.0, 40.0, 40.0)), None));
        reg.register(record(3, Some(Rect::new(0.0, 0.0, 40.0, 40.0)), None));

        // step_zone follows visual order: C → B → A → wraps to C
        assert_eq!(reg.step_zone(None, &0, 1), Some(ZoneId(3)));
        assert_eq!(reg.step_zone(Some(ZoneId(3)), &0, 1), Some(ZoneId(2)));
        assert_eq!(reg.step_zone(Some(ZoneId(2)), &0, 1), Some(ZoneId(1)));
        assert_eq!(reg.step_zone(Some(ZoneId(1)), &0, 1), Some(ZoneId(3)));
        // and backwards
        assert_eq!(reg.step_zone(Some(ZoneId(3)), &0, -1), Some(ZoneId(1)));

        // acceptance filtering removes zones from the cycle
        reg.register(record(4, None, Some(Callback::new(|v: u32| v >= 10))));
        assert_eq!(reg.acceptable(&5).len(), 3, "zone 4 rejects small payloads");
        assert_eq!(reg.acceptable(&10).len(), 4);

        // hit_test: point inside C only
        assert_eq!(reg.hit_test(Point::new(10.0, 10.0)), Some(ZoneId(3)));
        // overlapping zones: the later registry record wins
        reg.register(record(5, Some(Rect::new(0.0, 0.0, 40.0, 40.0)), None));
        assert_eq!(reg.hit_test(Point::new(10.0, 10.0)), Some(ZoneId(5)));
        // outside everything
        assert_eq!(reg.hit_test(Point::new(500.0, 500.0)), None);

        // hit_test_closest is acceptance-aware: a rejecting zone (id 6) on top
        // of the point is skipped in favor of the accepting zone under it.
        reg.register(record(
            6,
            Some(Rect::new(0.0, 0.0, 40.0, 40.0)),
            Some(Callback::new(|v: u32| v >= 10)),
        ));
        // Topmost by geometry is the rejecting zone 6...
        assert_eq!(reg.hit_test(Point::new(10.0, 10.0)), Some(ZoneId(6)));
        // ...but a small payload falls through to accepting zone 5 beneath it.
        assert_eq!(
            reg.hit_test_closest(Point::new(10.0, 10.0), &5, 48.0),
            Some(ZoneId(5))
        );
        // A large payload is accepted by zone 6 directly.
        assert_eq!(
            reg.hit_test_closest(Point::new(10.0, 10.0), &10, 48.0),
            Some(ZoneId(6))
        );
        // Gutter drop just above zone 1 (outside every rect): the nearest
        // acceptable zone within max_distance wins.
        assert_eq!(
            reg.hit_test_closest(Point::new(20.0, 95.0), &5, 48.0),
            Some(ZoneId(1))
        );
        // The snap measures to the rect, not its center: a large zone whose
        // center sits 500px away still catches a release 10px from its edge.
        reg.register(record(
            7,
            Some(Rect::new(200.0, 200.0, 1000.0, 600.0)),
            None,
        ));
        assert_eq!(
            reg.hit_test_closest(Point::new(190.0, 500.0), &5, 48.0),
            Some(ZoneId(7))
        );

        rsx! { div {} }
    }
    run(app);
}

/// A near-miss evaluates each acceptance predicate once and allocates no
/// cloned registry snapshot. Equal-distance fallback keeps the historical
/// tie-break: the earlier record in registry order wins.
#[test]
fn registry_closest_miss_evaluates_once_and_preserves_ties() {
    fn app() -> Element {
        use std::cell::Cell;
        use std::rc::Rc;

        use_dnd_provider::<u32>();
        let mut reg = use_zone_registry::<u32>();
        let calls = Rc::new(Cell::new(0));
        let record = |id: u64, x: f64, calls: Rc<Cell<usize>>| {
            let mut record = ZoneRecord::<u32>::new(ZoneId(id), Callback::new(|_| {}));
            record.label = Some(format!("zone-{id}"));
            record.accepts = Some(Callback::new(move |_: u32| {
                calls.set(calls.get() + 1);
                true
            }));
            record.rect = Some(Rect::new(x, 0.0, 40.0, 40.0));
            record
        };
        reg.register(record(81, 0.0, calls.clone()));
        reg.register(record(82, 60.0, calls.clone()));

        // Ten pixels from both rect edges. Reverse direct-hit order must not
        // change the fallback's earlier-record-in-registry-order tie-break.
        assert_eq!(
            reg.hit_test_closest(Point::new(50.0, 20.0), &7, 20.0),
            Some(ZoneId(81))
        );
        assert_eq!(calls.get(), 2, "each accepts callback runs exactly once");

        rsx! { div {} }
    }
    run(app);
}

/// Fractional layout tops within one CSS pixel are one visual row, so their
/// x positions decide reading order instead of sub-pixel y jitter.
#[test]
fn registry_spatial_rows_tolerate_subpixel_jitter() {
    fn app() -> Element {
        use_dnd_provider::<u32>();
        let mut reg = use_zone_registry::<u32>();
        let record = |id: u64, x: f64, y: f64| {
            let mut record = ZoneRecord::<u32>::new(ZoneId(id), Callback::new(|_| {}));
            record.rect = Some(Rect::new(x, y, 40.0, 40.0));
            record
        };

        // Raw (y, x) sorting would visit 2 before 1 and 3 before 4.
        reg.register(record(1, 0.0, 0.3));
        reg.register(record(2, 50.0, 0.0));
        reg.register(record(3, 50.0, 50.0));
        reg.register(record(4, 0.0, 50.3));

        assert_eq!(reg.step_zone(None, &0, 1), Some(ZoneId(1)));
        assert_eq!(reg.step_zone(Some(ZoneId(1)), &0, 1), Some(ZoneId(2)));
        assert_eq!(reg.step_zone(Some(ZoneId(2)), &0, 1), Some(ZoneId(4)));
        assert_eq!(reg.step_zone(Some(ZoneId(4)), &0, 1), Some(ZoneId(3)));

        reg.set_direction(Direction::Rtl);
        assert_eq!(reg.step_zone(None, &0, 1), Some(ZoneId(2)));
        assert_eq!(reg.step_zone(Some(ZoneId(2)), &0, 1), Some(ZoneId(1)));
        assert_eq!(reg.step_zone(Some(ZoneId(1)), &0, 1), Some(ZoneId(3)));
        assert_eq!(reg.step_zone(Some(ZoneId(3)), &0, 1), Some(ZoneId(4)));

        rsx! { div {} }
    }
    run(app);
}

// --- Selection (multiselect) ---------------------------------------------

thread_local! {
    static EXTERNAL_SELECTION: RefCell<Option<Selection<u32>>> = const { RefCell::new(None) };
}

#[test]
fn selection_click_semantics() {
    fn app() -> Element {
        let mut sel = use_selection::<u32>();
        assert!(sel.is_empty());

        // plain click: exclusive select
        sel.click(1, Modifiers::empty());
        sel.click(2, Modifiers::empty());
        assert_eq!(sel.items(), vec![2]);

        // ctrl/cmd click: toggle in and out
        sel.click(3, Modifiers::CONTROL);
        assert_eq!(sel.items(), vec![2, 3]);
        sel.click(2, Modifiers::META);
        assert_eq!(sel.items(), vec![3]);

        assert!(sel.is_selected(&3));
        assert_eq!(sel.len(), 1);
        sel.clear();
        assert!(sel.is_empty());

        let ordered = [1, 2, 3, 4, 5];
        sel.select_only(2);
        sel.click_in_order(5, Modifiers::SHIFT, &ordered);
        assert_eq!(sel.items(), vec![2, 3, 4, 5]);
        assert_eq!(sel.keyboard_range(&ordered, 4, -2, true), Some(2));
        assert_eq!(sel.items(), vec![2, 3]);

        sel.clear();
        assert_eq!(sel.keyboard_range(&ordered, 1, 2, true), Some(3));
        assert_eq!(sel.items(), vec![2, 3, 4]);

        assert_eq!(sel.keyboard_range(&ordered, 1, isize::MAX, false), Some(4));
        assert_eq!(sel.keyboard_range(&ordered, 3, isize::MIN, false), Some(0));

        rsx! { div {} }
    }
    run(app);
}

#[test]
fn selection_hook_from_signal_retains_its_range_anchor_across_renders() {
    fn app() -> Element {
        let items = use_signal(Vec::<u32>::new);
        let selection = use_selection_from_signal(items);
        EXTERNAL_SELECTION.with_borrow_mut(|slot| *slot = Some(selection));
        let count = selection.len();
        rsx! { span { "{count}" } }
    }

    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dom.in_runtime(|| {
        let mut selection = EXTERNAL_SELECTION.with_borrow(|slot| slot.unwrap());
        selection.select_only(2);
        selection.toggle(2);
        assert!(selection.is_empty());
    });
    rerender(&mut dom);

    dom.in_runtime(|| {
        let mut selection = EXTERNAL_SELECTION.with_borrow(|slot| slot.unwrap());
        selection.click_in_order(5, Modifiers::SHIFT, &[1, 2, 3, 4, 5]);
        assert_eq!(selection.items(), vec![2, 3, 4, 5]);
    });
}

// --- Rendered accessibility attributes -----------------------------------

#[test]
fn draggable_renders_a11y_attributes() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u8> {
                LiveRegion::<u8> {}
                Draggable::<u8> { payload: 1, label: "thing", "grab me" }
                DropZone::<u8> { label: "bin", on_drop: move |_| {}, "drop here" }
            }
        }
    }
    let html = run(app);
    assert!(html.contains("tabindex=0"), "not focusable: {html}");
    assert!(html.contains(r#"role="button""#), "role missing: {html}");
    assert!(
        !html.contains("draggable=true"),
        "in-app drags should not opt into native HTML drag: {html}"
    );
    assert!(
        html.contains("touch-action: pan-y"),
        "pointer drag style missing (Auto default is pan-y): {html}"
    );
    assert!(
        html.contains("aria-roledescription"),
        "roledescription missing: {html}"
    );
    assert!(
        html.contains(r#"aria-live="polite""#),
        "live region missing: {html}"
    );
}

#[test]
fn disabled_draggable_leaves_tab_order() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u8> {
                Draggable::<u8> { payload: 1, disabled: true, "frozen" }
            }
        }
    }
    let html = run(app);
    assert!(
        html.contains("tabindex=-1"),
        "should leave tab order: {html}"
    );
}

#[test]
fn reorder_buttons_render_labels_and_edge_disabling() {
    fn app() -> Element {
        rsx! {
            ReorderButtons { index: 0, total: 3, label: "Alpha", on_sort: move |_| {} }
        }
    }
    let html = run(app);
    assert!(
        html.contains(r#"aria-label="Move Alpha up""#),
        "up label: {html}"
    );
    assert!(
        html.contains(r#"aria-label="Move Alpha down""#),
        "down label: {html}"
    );
    // index 0: up disabled, down enabled
    assert!(html.contains("disabled"), "edge disabling missing: {html}");
}

#[test]
fn sortable_does_not_render_native_draggable_attrs() {
    fn app() -> Element {
        rsx! {
            SortableList {
                len: 1,
                on_sort: move |_| {},
                render: move |_| rsx! { "row" },
            }
        }
    }
    let html = run(app);
    assert!(
        !html.contains("draggable=true") && !html.contains("draggable=false"),
        "sortable should not render native drag attrs: {html}"
    );
}

thread_local! {
    static STABLE_SORTABLE_REGISTRY: RefCell<Option<ZoneRegistry<SortablePayload<u32>>>> =
        const { RefCell::new(None) };
    static STABLE_SORTABLE_EVENTS: RefCell<Vec<ReorderEvent<u32>>> = const { RefCell::new(Vec::new()) };
}

fn stable_sortable_app() -> Element {
    let record = move |event: ReorderEvent<u32>| {
        STABLE_SORTABLE_EVENTS.with_borrow_mut(|events| events.push(event));
    };
    rsx! {
        SortableProvider::<u32> {
            DragSimProbe::<SortablePayload<u32>> {}
            SortableGroup::<u32> {
                id: SortableGroupId::new(40),
                on_reorder: record,
                SortableItem::<u32> { id: 1, position: 0, "one" }
                SortableItem::<u32> { id: 2, position: 1, "two" }
                SortableItem::<u32> { id: 3, position: 2, "three" }
            }
            SortableGroup::<u32> {
                id: SortableGroupId::new(41),
                on_reorder: record,
            }
            StableSortableRegistryProbe {}
        }
    }
}

fn stable_sortable_swap_app() -> Element {
    rsx! {
        SortableProvider::<u32> {
            SortableGroup::<u32> {
                id: SortableGroupId::new(50),
                strategy: SortStrategy::GridSwap,
                on_reorder: move |event| {
                    STABLE_SORTABLE_EVENTS.with_borrow_mut(|events| events.push(event));
                },
                SortableItem::<u32> { id: 10, position: 0, "ten" }
                SortableItem::<u32> { id: 20, position: 1, "twenty" }
            }
            StableSortableRegistryProbe {}
        }
    }
}

fn stable_sortable_populated_groups_app() -> Element {
    let record = move |event: ReorderEvent<u32>| {
        STABLE_SORTABLE_EVENTS.with_borrow_mut(|events| events.push(event));
    };
    rsx! {
        SortableProvider::<u32> {
            SortableGroup::<u32> {
                id: SortableGroupId::new(60),
                on_reorder: record,
                SortableItem::<u32> { id: 1, position: 0, "one" }
            }
            SortableGroup::<u32> {
                id: SortableGroupId::new(61),
                on_reorder: record,
                SortableItem::<u32> { id: 2, position: 0, "two" }
                SortableItem::<u32> { id: 3, position: 1, "three" }
            }
            StableSortableRegistryProbe {}
        }
    }
}

#[component]
fn StableSortableRegistryProbe() -> Element {
    let registry = use_zone_registry::<SortablePayload<u32>>();
    use_hook(move || {
        STABLE_SORTABLE_REGISTRY.with_borrow_mut(|slot| *slot = Some(registry));
    });
    rsx! {}
}

fn sortable_keyboard_query(payload: SortablePayload<u32>) -> DropQuery<SortablePayload<u32>> {
    let mut query = DropQuery::new(payload);
    query.mode = DragMode::Keyboard;
    query
}

fn sortable_outcome(
    payload: SortablePayload<u32>,
    target: ZoneId,
) -> DropOutcome<SortablePayload<u32>> {
    DropOutcome {
        payload,
        from: None,
        to: target,
        effect: DropEffect::Move,
        mode: DragMode::Keyboard,
        client: Point::default(),
        element: Point::default(),
        grab: Point::default(),
        edge: None,
    }
}

#[test]
fn stable_sortable_components_route_keyboard_positions_and_empty_groups() {
    STABLE_SORTABLE_EVENTS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(stable_sortable_app);
    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    let registry = STABLE_SORTABLE_REGISTRY
        .with_borrow(|slot| slot.expect("stable sortable registry mounted"));

    let first = SortablePayload::new(SortableGroupId::new(40), 1, 0);
    let first_targets =
        dom.in_runtime(|| registry.acceptable_query(&sortable_keyboard_query(first.clone())));
    assert_eq!(
        first_targets.len(),
        4,
        "the source append target, two later items, and empty group should be keyboard targets"
    );
    dom.in_runtime(|| {
        for target in first_targets {
            target
                .on_drop
                .call(sortable_outcome(first.clone(), target.id));
        }
    });
    STABLE_SORTABLE_EVENTS.with_borrow(|events| {
        assert_eq!(
            events.as_slice(),
            [
                ReorderEvent::new(
                    1,
                    None,
                    SortableGroupId::new(40),
                    SortableGroupId::new(40),
                    Placement::After,
                ),
                ReorderEvent::new(
                    1,
                    Some(2),
                    SortableGroupId::new(40),
                    SortableGroupId::new(40),
                    Placement::After,
                ),
                ReorderEvent::new(
                    1,
                    Some(3),
                    SortableGroupId::new(40),
                    SortableGroupId::new(40),
                    Placement::After,
                ),
                ReorderEvent::new(
                    1,
                    None,
                    SortableGroupId::new(40),
                    SortableGroupId::new(41),
                    Placement::After,
                ),
            ]
        )
    });

    STABLE_SORTABLE_EVENTS.with_borrow_mut(Vec::clear);
    let last = SortablePayload::new(SortableGroupId::new(40), 3, 2);
    let last_targets =
        dom.in_runtime(|| registry.acceptable_query(&sortable_keyboard_query(last.clone())));
    dom.in_runtime(|| {
        for target in last_targets {
            target
                .on_drop
                .call(sortable_outcome(last.clone(), target.id));
        }
    });
    STABLE_SORTABLE_EVENTS.with_borrow(|events| {
        assert_eq!(
            events.as_slice(),
            [
                ReorderEvent::new(
                    3,
                    None,
                    SortableGroupId::new(40),
                    SortableGroupId::new(40),
                    Placement::After,
                ),
                ReorderEvent::new(
                    3,
                    Some(1),
                    SortableGroupId::new(40),
                    SortableGroupId::new(40),
                    Placement::Before,
                ),
                ReorderEvent::new(
                    3,
                    Some(2),
                    SortableGroupId::new(40),
                    SortableGroupId::new(40),
                    Placement::Before,
                ),
                ReorderEvent::new(
                    3,
                    None,
                    SortableGroupId::new(40),
                    SortableGroupId::new(41),
                    Placement::After,
                ),
            ]
        )
    });
}

#[test]
fn stable_sortable_keyboard_preserves_swap_intent() {
    STABLE_SORTABLE_EVENTS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(stable_sortable_swap_app);
    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    let registry = STABLE_SORTABLE_REGISTRY
        .with_borrow(|slot| slot.expect("stable sortable registry mounted"));
    let active = SortablePayload::new(SortableGroupId::new(50), 10, 0);
    let targets =
        dom.in_runtime(|| registry.acceptable_query(&sortable_keyboard_query(active.clone())));
    assert_eq!(targets.len(), 2, "group background plus the other tile");

    dom.in_runtime(|| {
        targets[1]
            .on_drop
            .call(sortable_outcome(active, targets[1].id));
    });
    STABLE_SORTABLE_EVENTS.with_borrow(|events| {
        assert_eq!(
            events.as_slice(),
            [ReorderEvent::new(
                10,
                Some(20),
                SortableGroupId::new(50),
                SortableGroupId::new(50),
                Placement::On,
            )]
        );
    });
}

#[test]
fn stable_sortable_keyboard_can_append_to_populated_group() {
    STABLE_SORTABLE_EVENTS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(stable_sortable_populated_groups_app);
    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    let registry = STABLE_SORTABLE_REGISTRY
        .with_borrow(|slot| slot.expect("stable sortable registry mounted"));
    let records = dom.in_runtime(|| registry.records());
    let destination_group_zone = records[2].id;
    let active = SortablePayload::new(SortableGroupId::new(60), 1, 0);
    let append_target = dom.in_runtime(|| {
        registry
            .acceptable_query(&sortable_keyboard_query(active.clone()))
            .into_iter()
            .find(|record| record.id == destination_group_zone)
            .expect("populated destination append target")
    });

    dom.in_runtime(|| {
        append_target
            .on_drop
            .call(sortable_outcome(active, append_target.id));
    });
    STABLE_SORTABLE_EVENTS.with_borrow(|events| {
        assert_eq!(
            events.as_slice(),
            [ReorderEvent::new(
                1,
                None,
                SortableGroupId::new(60),
                SortableGroupId::new(61),
                Placement::After,
            )]
        );
    });
}

#[test]
fn stable_sortable_pointer_delivery_uses_item_edges_and_empty_group_background() {
    STABLE_SORTABLE_EVENTS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(stable_sortable_app);
    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    let registry = STABLE_SORTABLE_REGISTRY
        .with_borrow(|slot| slot.expect("stable sortable registry mounted"));
    let records = dom.in_runtime(|| registry.records());
    assert_eq!(
        records.len(),
        5,
        "two group targets plus three item targets"
    );

    let mut sim = drag_sim::<SortablePayload<u32>>();
    sim.place(&dom, records[0].id, Rect::new(0.0, 0.0, 100.0, 120.0));
    sim.place(&dom, records[1].id, Rect::new(0.0, 0.0, 100.0, 40.0));
    sim.place(&dom, records[2].id, Rect::new(0.0, 40.0, 100.0, 40.0));
    sim.place(&dom, records[3].id, Rect::new(0.0, 80.0, 100.0, 40.0));
    sim.place(&dom, records[4].id, Rect::new(200.0, 0.0, 100.0, 120.0));

    sim.pick_up(&dom, SortablePayload::new(SortableGroupId::new(40), 1, 0));
    sim.move_to(&dom, Point::new(50.0, 110.0));
    assert_eq!(sim.release(&dom), Some(records[3].id));
    STABLE_SORTABLE_EVENTS.with_borrow(|events| {
        assert_eq!(
            events.as_slice(),
            [ReorderEvent::new(
                1,
                Some(3),
                SortableGroupId::new(40),
                SortableGroupId::new(40),
                Placement::After,
            )]
        )
    });

    STABLE_SORTABLE_EVENTS.with_borrow_mut(Vec::clear);
    sim.pick_up(&dom, SortablePayload::new(SortableGroupId::new(40), 2, 1));
    sim.move_to(&dom, Point::new(50.0, 60.0));
    assert_eq!(
        sim.release(&dom),
        Some(records[2].id),
        "a pointer self-hit must not fall through to group append"
    );
    STABLE_SORTABLE_EVENTS.with_borrow(|events| assert!(events.is_empty()));

    STABLE_SORTABLE_EVENTS.with_borrow_mut(Vec::clear);
    sim.pick_up(&dom, SortablePayload::new(SortableGroupId::new(40), 2, 1));
    sim.move_to(&dom, Point::new(250.0, 60.0));
    assert_eq!(sim.release(&dom), Some(records[4].id));
    STABLE_SORTABLE_EVENTS.with_borrow(|events| {
        assert_eq!(
            events.as_slice(),
            [ReorderEvent::new(
                2,
                None,
                SortableGroupId::new(40),
                SortableGroupId::new(41),
                Placement::After,
            )]
        )
    });
}

#[test]
fn sortable_touch_handle_keeps_wrapper_on_one_row() {
    fn app() -> Element {
        rsx! {
            SortableList {
                len: 1,
                touch_handle: true,
                render: move |_| rsx! { div { class: "row", "Alpha" } },
                on_sort: move |_| {},
            }
        }
    }
    let html = run(app);
    assert!(html.contains("data-sort-handle"), "handle missing: {html}");
    assert!(
        html.contains("data-sort-content"),
        "content slot missing: {html}"
    );
    assert!(
        html.contains("display: flex"),
        "handle wrapper must be flex: {html}"
    );
    assert!(
        html.contains("align-items: stretch"),
        "handle wrapper alignment missing: {html}"
    );
    assert!(
        html.contains("width: 100%"),
        "handle wrapper width missing: {html}"
    );
    assert!(
        html.contains("flex: 1 1 auto"),
        "rendered row slot must fill remaining width: {html}"
    );
    assert!(
        html.contains("place-items: center"),
        "handle glyph must be centered: {html}"
    );
}

#[test]
fn nested_zone_traversal() {
    fn app() -> Element {
        use_dnd_provider::<u32>();
        let mut reg = use_zone_registry::<u32>();

        let record = |id: u64, parent: Option<u64>, y: f64| {
            let mut record = ZoneRecord::<u32>::new(ZoneId(id), Callback::new(|_| {}));
            record.parent = parent.map(ZoneId);
            record.rect = Some(Rect::new(0.0, y, 100.0, 40.0));
            record
        };

        // Two root boards; the first contains two columns.
        reg.register(record(1, None, 0.0)); //   board A
        reg.register(record(2, None, 200.0)); // board B
        reg.register(record(10, Some(1), 10.0)); //  A / column 1
        reg.register(record(11, Some(1), 50.0)); //  A / column 2

        // Root siblings cycle among boards only - columns don't leak up.
        assert_eq!(reg.step_sibling(None, &0, 1), Some(ZoneId(1)));
        assert_eq!(reg.step_sibling(Some(ZoneId(1)), &0, 1), Some(ZoneId(2)));
        assert_eq!(reg.step_sibling(Some(ZoneId(2)), &0, 1), Some(ZoneId(1)));

        // Descend into board A → first column spatially; siblings cycle
        // within the level; ascend returns to the board.
        assert_eq!(reg.first_child(ZoneId(1), &0), Some(ZoneId(10)));
        assert_eq!(reg.step_sibling(Some(ZoneId(10)), &0, 1), Some(ZoneId(11)));
        assert_eq!(reg.step_sibling(Some(ZoneId(11)), &0, 1), Some(ZoneId(10)));
        assert_eq!(reg.parent_of(ZoneId(11)), Some(ZoneId(1)));

        // Leaves and roots have no further depth.
        assert_eq!(reg.first_child(ZoneId(10), &0), None);
        assert_eq!(reg.parent_of(ZoneId(1)), None);

        // ascend resolves a registered parent, and refuses one that only
        // exists in another type's registry (the parent context is shared
        // across payload types, so records can carry foreign parent ids).
        assert_eq!(reg.ascend(ZoneId(11)), Some(ZoneId(1)));
        assert_eq!(reg.ascend(ZoneId(1)), None, "roots have nowhere to go");
        reg.register(record(20, Some(99), 300.0)); // parent 99 lives elsewhere
        assert_eq!(reg.parent_of(ZoneId(20)), Some(ZoneId(99)));
        assert!(!reg.contains(ZoneId(99)));
        assert_eq!(reg.ascend(ZoneId(20)), None);
        // Sibling grouping under the foreign parent still works: it only
        // compares parent ids, never resolves the parent record.
        reg.register(record(21, Some(99), 340.0));
        assert_eq!(reg.step_sibling(Some(ZoneId(20)), &0, 1), Some(ZoneId(21)));

        rsx! { div {} }
    }
    run(app);
}

/// A `DropZone<A>` nested inside a `DropZone<B>` records B's id as its
/// parent - `ParentZone` is one context shared across payload types. That
/// foreign id must never be *entered* by keyboard ascent, or Enter would
/// silently no-op on a zone this world can't resolve.
#[test]
fn cross_type_nested_zone_ascend_stays_in_its_own_world() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u8> {
                DndProvider::<u16> {
                    DropZone::<u8> { id: ZoneId(1), on_drop: move |_: DropOutcome<u8>| {},
                        DropZone::<u16> { id: ZoneId(2), on_drop: move |_: DropOutcome<u16>| {},
                            CrossWorldProbe {}
                        }
                    }
                }
            }
        }
    }
    #[component]
    fn CrossWorldProbe() -> Element {
        let reg8 = use_zone_registry::<u8>();
        let reg16 = use_zone_registry::<u16>();
        // The u16 zone discovered the u8 zone as its parent...
        assert_eq!(reg16.parent_of(ZoneId(2)), Some(ZoneId(1)));
        // ...but that parent lives in the other world's registry,
        assert!(reg8.contains(ZoneId(1)));
        assert!(!reg16.contains(ZoneId(1)));
        // ...so ascent refuses it (the Draggable then falls back to a sibling).
        assert_eq!(reg16.ascend(ZoneId(2)), None);
        rsx! { div {} }
    }
    run(app);
}

#[test]
fn nested_dropzones_discover_parents_from_context() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u8> {
                DropZone::<u8> { id: ZoneId(1), on_drop: move |_| {},
                    DropZone::<u8> { id: ZoneId(2), on_drop: move |_| {},
                        NestingProbe {}
                    }
                }
            }
        }
    }
    #[component]
    fn NestingProbe() -> Element {
        let reg = use_zone_registry::<u8>();
        // The inner zone should have registered with the outer as parent.
        assert_eq!(reg.parent_of(ZoneId(2)), Some(ZoneId(1)));
        assert_eq!(reg.parent_of(ZoneId(1)), None);
        rsx! { div {} }
    }
    run(app);
}

#[test]
fn canvas_dropzone_registers_with_label() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u8> {
                CanvasDropZone::<u8> {
                    id: ZoneId(7),
                    label: "canvas",
                    on_drop: move |_| {},
                    CanvasProbe {}
                }
            }
        }
    }
    #[component]
    fn CanvasProbe() -> Element {
        let reg = use_zone_registry::<u8>();
        assert_eq!(reg.get(ZoneId(7)).unwrap().label.as_deref(), Some("canvas"));
        rsx! { div {} }
    }
    run(app);
}

#[derive(Clone, Props)]
struct DynamicCanvasProps {
    phase: Shared<u8>,
    drops: Shared<Vec<Point>>,
}

impl PartialEq for DynamicCanvasProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.phase, &other.phase) && Arc::ptr_eq(&self.drops, &other.drops)
    }
}

fn dynamic_canvas_app(props: DynamicCanvasProps) -> Element {
    let phase = *props.phase.lock().unwrap();
    let drops = props.drops.clone();
    let snap = if phase == 0 {
        SnapGrid(10.0)
    } else {
        SnapGrid(25.0)
    };
    let bounds = if phase == 0 {
        Bounds {
            width: 100.0,
            height: 50.0,
        }
    } else {
        Bounds {
            width: 60.0,
            height: 60.0,
        }
    };

    rsx! {
        DndProvider::<u8> {
            CanvasDropZone::<u8> {
                id: ZoneId(7),
                snap,
                bounds,
                on_drop: move |drop: CanvasDrop<u8>| drops.lock().unwrap().push(drop.position),
                DynamicCanvasProbe { phase }
            }
        }
    }
}

#[component]
fn DynamicCanvasProbe(phase: u8) -> Element {
    let reg = use_zone_registry::<u8>();

    if phase == 0 || phase == 2 {
        reg.get(ZoneId(7))
            .expect("canvas zone registered")
            .on_drop
            .call(DropOutcome {
                payload: 1,
                from: None,
                to: ZoneId(7),
                effect: DropEffect::Move,
                mode: DragMode::Pointer,
                client: Point::new(107.0, 46.0),
                element: Point::new(107.0, 46.0),
                grab: Point::new(9.0, 8.0),
                edge: None,
            });
    }

    rsx! { div {} }
}

#[test]
fn canvas_dropzone_registered_callback_reads_latest_snap_and_bounds() {
    let phase = Arc::new(Mutex::new(0));
    let drops = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        dynamic_canvas_app,
        DynamicCanvasProps {
            phase: phase.clone(),
            drops: drops.clone(),
        },
    );

    dom.rebuild_in_place();
    assert_eq!(*drops.lock().unwrap(), vec![Point::new(100.0, 40.0)]);

    *phase.lock().unwrap() = 1;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    assert_eq!(
        drops.lock().unwrap().len(),
        1,
        "prop update pass should not deliver a drop"
    );

    *phase.lock().unwrap() = 2;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    assert_eq!(
        *drops.lock().unwrap(),
        vec![Point::new(100.0, 40.0), Point::new(60.0, 50.0)]
    );
}

#[derive(Clone, Props)]
struct KeyboardPolicyCanvasProps {
    policy: CanvasKeyboardPlacement,
    mode: DragMode,
    drops: Shared<Vec<Point>>,
}

impl PartialEq for KeyboardPolicyCanvasProps {
    fn eq(&self, other: &Self) -> bool {
        self.policy == other.policy
            && self.mode == other.mode
            && Arc::ptr_eq(&self.drops, &other.drops)
    }
}

fn keyboard_policy_canvas_app(props: KeyboardPolicyCanvasProps) -> Element {
    let drops = props.drops.clone();
    rsx! {
        DndProvider::<u8> {
            CanvasDropZone::<u8> {
                id: ZoneId(77),
                keyboard: props.policy,
                on_drop: move |drop: CanvasDrop<u8>| drops.lock().unwrap().push(drop.position),
                KeyboardPolicyProbe { mode: props.mode }
            }
        }
    }
}

#[component]
fn KeyboardPolicyProbe(mode: DragMode) -> Element {
    let reg = use_zone_registry::<u8>();
    reg.get(ZoneId(77))
        .expect("canvas zone registered")
        .on_drop
        .call(DropOutcome {
            payload: 1,
            from: None,
            to: ZoneId(77),
            effect: DropEffect::Move,
            mode,
            client: Point::new(100.0, 80.0),
            element: Point::new(80.0, 60.0),
            grab: Point::default(),

            edge: None,
        });
    rsx! { div {} }
}

fn run_keyboard_policy(policy: CanvasKeyboardPlacement, mode: DragMode) -> Vec<Point> {
    let drops = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        keyboard_policy_canvas_app,
        KeyboardPolicyCanvasProps {
            policy,
            mode,
            drops: drops.clone(),
        },
    );
    dom.rebuild_in_place();
    let out = drops.lock().unwrap().clone();
    out
}

#[test]
fn canvas_keyboard_policy_defaults_to_core_center_geometry() {
    assert_eq!(
        run_keyboard_policy(CanvasKeyboardPlacement::default(), DragMode::Keyboard),
        vec![Point::new(80.0, 60.0)]
    );
}

#[test]
fn canvas_keyboard_policy_can_use_origin() {
    assert_eq!(
        run_keyboard_policy(CanvasKeyboardPlacement::Origin, DragMode::Keyboard),
        vec![Point::default()]
    );
}

#[test]
fn canvas_keyboard_policy_can_use_fixed_point() {
    assert_eq!(
        run_keyboard_policy(
            CanvasKeyboardPlacement::Fixed(Point::new(24.0, 36.0)),
            DragMode::Keyboard,
        ),
        vec![Point::new(24.0, 36.0)]
    );
}

#[test]
fn canvas_keyboard_policy_does_not_affect_pointer_drops() {
    assert_eq!(
        run_keyboard_policy(CanvasKeyboardPlacement::Origin, DragMode::Pointer),
        vec![Point::new(80.0, 60.0)]
    );
}

#[derive(Clone, Props)]
struct DynamicKeyboardPolicyProps {
    phase: Shared<u8>,
    drops: Shared<Vec<Point>>,
}

impl PartialEq for DynamicKeyboardPolicyProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.phase, &other.phase) && Arc::ptr_eq(&self.drops, &other.drops)
    }
}

fn dynamic_keyboard_policy_app(props: DynamicKeyboardPolicyProps) -> Element {
    let phase = *props.phase.lock().unwrap();
    let drops = props.drops.clone();
    let keyboard = match phase {
        0 => CanvasKeyboardPlacement::Center,
        1 => CanvasKeyboardPlacement::Origin,
        _ => CanvasKeyboardPlacement::Fixed(Point::new(24.0, 36.0)),
    };

    rsx! {
        DndProvider::<u8> {
            CanvasDropZone::<u8> {
                id: ZoneId(78),
                keyboard,
                on_drop: move |drop: CanvasDrop<u8>| drops.lock().unwrap().push(drop.position),
                DynamicKeyboardPolicyProbe { phase }
            }
        }
    }
}

#[component]
fn DynamicKeyboardPolicyProbe(phase: u8) -> Element {
    let reg = use_zone_registry::<u8>();
    if phase == 0 || phase == 2 {
        reg.get(ZoneId(78))
            .expect("canvas zone registered")
            .on_drop
            .call(DropOutcome {
                payload: 1,
                from: None,
                to: ZoneId(78),
                effect: DropEffect::Move,
                mode: DragMode::Keyboard,
                client: Point::new(100.0, 80.0),
                element: Point::new(80.0, 60.0),
                grab: Point::default(),

                edge: None,
            });
    }
    rsx! { div {} }
}

#[test]
fn canvas_dropzone_registered_callback_reads_latest_keyboard_policy() {
    let phase = Arc::new(Mutex::new(0));
    let drops = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        dynamic_keyboard_policy_app,
        DynamicKeyboardPolicyProps {
            phase: phase.clone(),
            drops: drops.clone(),
        },
    );

    dom.rebuild_in_place();
    assert_eq!(*drops.lock().unwrap(), vec![Point::new(80.0, 60.0)]);

    *phase.lock().unwrap() = 1;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    assert_eq!(
        drops.lock().unwrap().len(),
        1,
        "prop update pass should not deliver a drop"
    );

    *phase.lock().unwrap() = 2;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    assert_eq!(
        *drops.lock().unwrap(),
        vec![Point::new(80.0, 60.0), Point::new(24.0, 36.0)]
    );
}

// --- Board slots join the zone registry ----------------------------------

#[derive(Clone, Props)]
struct BoardSlotRegistryProps {
    moves: Shared<Vec<MoveEvent<&'static str>>>,
}

impl PartialEq for BoardSlotRegistryProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.moves, &other.moves)
    }
}

fn board_slot_registry_app(props: BoardSlotRegistryProps) -> Element {
    let moves = props.moves.clone();
    rsx! {
        DndProvider::<BoardPayload<&'static str>> {
            BoardColumn::<&'static str> {
                id: ZoneId(90),
                on_move: move |_| {},
                BoardSlot::<&'static str> {
                    column: ZoneId(90),
                    index: 1,
                    on_move: move |mv| moves.lock().unwrap().push(mv),
                    "slot"
                }
                BoardSlotProbe {}
            }
        }
    }
}

#[component]
fn BoardSlotProbe() -> Element {
    let registry = use_zone_registry::<BoardPayload<&'static str>>();
    let pointer_payload = BoardPayload {
        item: "pointer-card",
        from: ZoneId(10),
        index: 0,
    };
    let keyboard_payload = BoardPayload {
        item: "keyboard-card",
        from: ZoneId(11),
        index: 2,
    };
    let slots = registry.children_of(Some(ZoneId(90)), &pointer_payload);
    assert_eq!(slots.len(), 1, "board slot should register as column child");
    assert_eq!(slots[0].label.as_deref(), Some("Insert at position 1"));
    slots[0].on_drop.call(DropOutcome {
        payload: pointer_payload,
        from: Some(ZoneId(10)),
        to: slots[0].id,
        effect: DropEffect::Move,
        mode: DragMode::Pointer,
        client: Point::new(8.0, 12.0),
        element: Point::new(8.0, 12.0),
        grab: Point::default(),

        edge: None,
    });
    slots[0].on_drop.call(DropOutcome {
        payload: keyboard_payload,
        from: Some(ZoneId(11)),
        to: slots[0].id,
        effect: DropEffect::Move,
        mode: DragMode::Keyboard,
        client: Point::default(),
        element: Point::default(),
        grab: Point::default(),

        edge: None,
    });
    rsx! { div {} }
}

#[test]
fn board_slot_registers_for_pointer_and_keyboard_paths() {
    let moves = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        board_slot_registry_app,
        BoardSlotRegistryProps {
            moves: moves.clone(),
        },
    );
    dom.rebuild_in_place();

    assert_eq!(
        *moves.lock().unwrap(),
        vec![
            MoveEvent::new("pointer-card", (ZoneId(10), 0), (ZoneId(90), Some(1))),
            MoveEvent::new("keyboard-card", (ZoneId(11), 2), (ZoneId(90), Some(1))),
        ]
    );
}

// --- Board slots inherit the column's acceptance filter (#4) --------------

/// A `BoardSlot` must honor the enclosing `BoardColumn`'s `accepts` filter, so
/// a precise-insert respects the same WIP limit as an append. It inherits the
/// filter via context: the slot is filtered out of the column's acceptable
/// children for a rejected payload, and its registered drop is a no-op for one.
#[test]
fn board_slot_inherits_column_accepts() {
    let moves: Shared<Vec<MoveEvent<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

    fn app(props: BoardSlotRegistryProps) -> Element {
        let moves = props.moves.clone();
        rsx! {
            DndProvider::<BoardPayload<&'static str>> {
                BoardColumn::<&'static str> {
                    id: ZoneId(90),
                    // WIP-style filter: reject anything labelled "blocked".
                    accepts: move |p: BoardPayload<&'static str>| p.item != "blocked",
                    on_move: move |_| {},
                    BoardSlot::<&'static str> {
                        column: ZoneId(90),
                        index: 0,
                        on_move: move |mv| moves.lock().unwrap().push(mv),
                        "slot"
                    }
                    BoardAcceptsProbe {}
                }
            }
        }
    }

    #[component]
    fn BoardAcceptsProbe() -> Element {
        let registry = use_zone_registry::<BoardPayload<&'static str>>();
        let ok = BoardPayload {
            item: "ok",
            from: ZoneId(10),
            index: 0,
        };
        let blocked = BoardPayload {
            item: "blocked",
            from: ZoneId(10),
            index: 0,
        };

        // Hover/keyboard filtering: the slot is an acceptable child for an
        // allowed payload, and filtered out for a rejected one.
        let accepted = registry.children_of(Some(ZoneId(90)), &ok);
        assert_eq!(accepted.len(), 1, "slot accepts an allowed payload");
        assert!(
            registry.children_of(Some(ZoneId(90)), &blocked).is_empty(),
            "slot inherits the column's rejection"
        );

        // Drop delivery: a rejected payload is a no-op; an allowed one moves.
        let slot = &accepted[0];
        for (payload, mode) in [(blocked, DragMode::Pointer), (ok, DragMode::Keyboard)] {
            slot.on_drop.call(DropOutcome {
                payload,
                from: Some(ZoneId(10)),
                to: slot.id,
                effect: DropEffect::Move,
                mode,
                client: Point::default(),
                element: Point::default(),
                grab: Point::default(),

                edge: None,
            });
        }
        rsx! { div {} }
    }

    let mut dom = VirtualDom::new_with_props(
        app,
        BoardSlotRegistryProps {
            moves: moves.clone(),
        },
    );
    dom.rebuild_in_place();

    // Only the allowed payload produced a move; the blocked one was dropped.
    assert_eq!(
        *moves.lock().unwrap(),
        vec![MoveEvent::new("ok", (ZoneId(10), 0), (ZoneId(90), Some(0)))]
    );
}

// --- Explicit low column ids never collide with slot auto ids -------------

/// Regression: `use_zone_id` draws from one process-wide counter and the zone
/// registry replaces records by id, so when the counter began at 1 a slot's
/// auto id could land exactly on a *neighboring column's* hand-picked id
/// (say `ZoneId(2)`), and that column registering silently replaced the slot,
/// which then stopped lighting up and receiving drops. Auto ids now start at
/// 2^32, so explicit ids in the `u32` range can never be knocked out.
#[test]
fn slot_auto_ids_never_collide_with_explicit_column_ids() {
    fn app() -> Element {
        rsx! {
            DndProvider::<BoardPayload<&'static str>> {
                for col in 1..=3u64 {
                    BoardColumn::<&'static str> {
                        id: ZoneId(col),
                        on_move: move |_| {},
                        BoardSlot::<&'static str> {
                            column: ZoneId(col),
                            index: 0,
                            on_move: move |_| {},
                        }
                        BoardSlot::<&'static str> {
                            column: ZoneId(col),
                            index: 1,
                            on_move: move |_| {},
                        }
                    }
                }
                CollisionProbe {}
            }
        }
    }

    #[component]
    fn CollisionProbe() -> Element {
        let registry = use_zone_registry::<BoardPayload<&'static str>>();
        let payload = BoardPayload {
            item: "card",
            from: ZoneId(1),
            index: 0,
        };
        let roots = registry.children_of(None, &payload);
        assert_eq!(roots.len(), 3, "every explicit column stays registered");
        // Each column still owns both of its slots: no slot was replaced by a
        // neighboring column registering over its auto id.
        for col in 1..=3u64 {
            assert_eq!(
                registry.children_of(Some(ZoneId(col)), &payload).len(),
                2,
                "column {col} keeps both slots"
            );
        }
        rsx! {
            div {}
        }
    }

    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
}

// --- Board slots deliver the current index after a prop change (#3) -------

#[derive(Clone, Props)]
struct DynamicBoardSlotProps {
    phase: Shared<u8>,
    moves: Shared<Vec<MoveEvent<&'static str>>>,
}

impl PartialEq for DynamicBoardSlotProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.phase, &other.phase) && Arc::ptr_eq(&self.moves, &other.moves)
    }
}

fn dynamic_board_slot_app(props: DynamicBoardSlotProps) -> Element {
    let phase = *props.phase.lock().unwrap();
    let moves = props.moves.clone();
    // The slot's index is positional; it changes as items shift above it.
    let index = if phase >= 2 { 3 } else { 1 };
    rsx! {
        DndProvider::<BoardPayload<&'static str>> {
            BoardColumn::<&'static str> {
                id: ZoneId(90),
                on_move: move |_| {},
                BoardSlot::<&'static str> {
                    column: ZoneId(90),
                    index,
                    on_move: move |mv| moves.lock().unwrap().push(mv),
                    "slot"
                }
                DynamicBoardSlotProbe { phase }
            }
        }
    }
}

#[component]
fn DynamicBoardSlotProbe(phase: u8) -> Element {
    let registry = use_zone_registry::<BoardPayload<&'static str>>();
    if phase == 0 || phase == 2 {
        let payload = BoardPayload {
            item: "card",
            from: ZoneId(10),
            index: 0,
        };
        let slot = registry.children_of(Some(ZoneId(90)), &payload).remove(0);
        slot.on_drop.call(DropOutcome {
            payload,
            from: Some(ZoneId(10)),
            to: slot.id,
            effect: DropEffect::Move,
            mode: DragMode::Keyboard,
            client: Point::default(),
            element: Point::default(),
            grab: Point::default(),

            edge: None,
        });
    }
    rsx! { div {} }
}

/// The registered (pointer/keyboard) drop must target the slot's *current*
/// index, not the one captured when the zone first registered.
#[test]
fn board_slot_registered_drop_reads_latest_index() {
    let phase = Arc::new(Mutex::new(0u8));
    let moves = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        dynamic_board_slot_app,
        DynamicBoardSlotProps {
            phase: phase.clone(),
            moves: moves.clone(),
        },
    );

    dom.rebuild_in_place();
    assert_eq!(
        moves.lock().unwrap().last().unwrap().to,
        (ZoneId(90), Some(1))
    );

    // Prop-update pass: no drop delivered, just re-render with the new index.
    *phase.lock().unwrap() = 1;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    assert_eq!(moves.lock().unwrap().len(), 1);

    // Deliver again: the drop now targets the updated index, not the stale one.
    *phase.lock().unwrap() = 2;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    assert_eq!(
        moves.lock().unwrap().last().unwrap().to,
        (ZoneId(90), Some(3))
    );
}

#[derive(Clone, Props)]
struct DynamicBoardAcceptsProps {
    allowed: Shared<bool>,
    observed: Shared<Vec<bool>>,
}

impl PartialEq for DynamicBoardAcceptsProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.allowed, &other.allowed) && Arc::ptr_eq(&self.observed, &other.observed)
    }
}

fn dynamic_board_accepts_app(props: DynamicBoardAcceptsProps) -> Element {
    let allowed = *props.allowed.lock().unwrap();
    rsx! {
        DndProvider::<BoardPayload<&'static str>> {
            BoardColumn::<&'static str> {
                id: ZoneId(191),
                accepts: move |_: BoardPayload<&'static str>| allowed,
                on_move: move |_| {},
                BoardSlot::<&'static str> {
                    column: ZoneId(191),
                    index: 0,
                    on_move: move |_| {},
                }
                DynamicBoardAcceptsProbe { allowed, observed: props.observed }
            }
        }
    }
}

#[derive(Clone, Props)]
struct DynamicBoardAcceptsProbeProps {
    allowed: bool,
    observed: Shared<Vec<bool>>,
}

impl PartialEq for DynamicBoardAcceptsProbeProps {
    fn eq(&self, other: &Self) -> bool {
        self.allowed == other.allowed && Arc::ptr_eq(&self.observed, &other.observed)
    }
}

#[allow(non_snake_case)]
fn DynamicBoardAcceptsProbe(props: DynamicBoardAcceptsProbeProps) -> Element {
    let registry = use_zone_registry::<BoardPayload<&'static str>>();
    let payload = BoardPayload {
        item: "card",
        from: ZoneId(1),
        index: 0,
    };
    let accepted = !registry.children_of(Some(ZoneId(191)), &payload).is_empty();
    props.observed.lock().unwrap().push(accepted);
    assert_eq!(accepted, props.allowed);
    rsx! {}
}

#[test]
fn board_slots_follow_live_column_acceptance_policy() {
    let allowed = Arc::new(Mutex::new(false));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        dynamic_board_accepts_app,
        DynamicBoardAcceptsProps {
            allowed: allowed.clone(),
            observed: observed.clone(),
        },
    );
    dom.rebuild_in_place();

    *allowed.lock().unwrap() = true;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);

    assert_eq!(*observed.lock().unwrap(), vec![false, true]);
}

// --- Tree targets join the zone registry ---------------------------------

/// TreeNodeTargets register themselves as zones (that's what makes them
/// reachable by touch hit-testing and keyboard navigation), honoring the
/// permissive any-intent filter at the registry level. Registration runs
/// in `use_hook` during first render, so a probe sibling rendered *after*
/// the targets observes them synchronously.
#[test]
fn tree_targets_register_as_zones() {
    fn app() -> Element {
        use_dnd_provider::<u32>();
        rsx! {
            TreeNodeTarget::<u32> {
                node: NodeId(1),
                label: "alpha",
                on_drop: move |_| {},
            }
            TreeNodeTarget::<u32> {
                node: NodeId(2),
                // rejects everything, for every intent: filtered out of
                // acceptable() but still registered
                accepts: move |(_, _): (u32, DropIntent)| false,
                on_drop: move |_| {},
            }
            TreeProbe {}
        }
    }

    #[component]
    fn TreeProbe() -> Element {
        let registry = use_zone_registry::<u32>();
        let acceptable = registry.children_of(None, &7u32);
        assert_eq!(acceptable.len(), 1, "only the permissive target accepts");
        assert_eq!(acceptable[0].label.as_deref(), Some("alpha"));
        rsx! { "ok" }
    }

    run(app);
}

#[derive(Clone, Props)]
struct TreeIntentAcceptsProps {
    drops: Shared<Vec<TreeDropEvent<&'static str>>>,
}

impl PartialEq for TreeIntentAcceptsProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.drops, &other.drops)
    }
}

fn tree_intent_accepts_app(props: TreeIntentAcceptsProps) -> Element {
    let drops = props.drops.clone();
    rsx! {
        DndProvider::<&'static str> {
            TreeNodeTarget::<&'static str> {
                node: NodeId(12),
                row_height: 100.0,
                accepts: move |(_, intent): (&'static str, DropIntent)| intent == DropIntent::Into,
                on_drop: move |ev| drops.lock().unwrap().push(ev),
                "node"
            }
            TreeIntentAcceptsProbe {}
        }
    }
}

#[component]
fn TreeIntentAcceptsProbe() -> Element {
    let registry = use_zone_registry::<&'static str>();
    let zones = registry.children_of(None, &"payload");
    assert_eq!(
        zones.len(),
        1,
        "registry should keep a target reachable when any intent accepts"
    );

    zones[0].on_drop.call(DropOutcome {
        payload: "before",
        from: None,
        to: zones[0].id,
        effect: DropEffect::Move,
        mode: DragMode::Keyboard,
        client: Point::default(),
        element: Point::new(0.0, 10.0),
        grab: Point::default(),

        edge: None,
    });
    zones[0].on_drop.call(DropOutcome {
        payload: "into",
        from: None,
        to: zones[0].id,
        effect: DropEffect::Move,
        mode: DragMode::Keyboard,
        client: Point::default(),
        element: Point::new(0.0, 50.0),
        grab: Point::default(),

        edge: None,
    });

    rsx! { div {} }
}

#[test]
fn tree_target_registry_filter_is_permissive_but_drop_rechecks_exact_intent() {
    let drops = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        tree_intent_accepts_app,
        TreeIntentAcceptsProps {
            drops: drops.clone(),
        },
    );
    dom.rebuild_in_place();

    assert_eq!(
        *drops.lock().unwrap(),
        vec![TreeDropEvent::new("into", NodeId(12), DropIntent::Into)]
    );
}

#[derive(Clone, Props)]
struct DynamicTreeTargetProps {
    phase: Shared<u8>,
    drops: Shared<Vec<(u8, TreeDropEvent<&'static str>)>>,
    runs: Shared<Vec<u8>>,
}

impl PartialEq for DynamicTreeTargetProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.phase, &other.phase)
            && Arc::ptr_eq(&self.drops, &other.drops)
            && Arc::ptr_eq(&self.runs, &other.runs)
    }
}

fn dynamic_tree_target_app(props: DynamicTreeTargetProps) -> Element {
    let phase = *props.phase.lock().unwrap();
    let drops = props.drops.clone();
    let probe_runs = props.runs.clone();
    rsx! {
        DndProvider::<&'static str> {
            TreeNodeTarget::<&'static str> {
                node: if phase == 0 { NodeId(7) } else { NodeId(8) },
                label: if phase == 0 { "alpha" } else { "beta" },
                row_height: if phase == 0 { 100.0 } else { 400.0 },
                accepts: move |(payload, _): (&'static str, DropIntent)| {
                    phase == 0 || payload == "allowed"
                },
                on_drop: move |ev| drops.lock().unwrap().push((phase, ev)),
                "node"
            }
            DynamicTreeTargetProbe {
                phase,
                runs: probe_runs,
            }
        }
    }
}

#[derive(Clone, Props)]
struct DynamicTreeTargetProbeProps {
    phase: u8,
    runs: Shared<Vec<u8>>,
}

impl PartialEq for DynamicTreeTargetProbeProps {
    fn eq(&self, other: &Self) -> bool {
        self.phase == other.phase && Arc::ptr_eq(&self.runs, &other.runs)
    }
}

#[allow(non_snake_case)]
fn DynamicTreeTargetProbe(props: DynamicTreeTargetProbeProps) -> Element {
    let phase = props.phase;
    let registry = use_zone_registry::<&'static str>();
    use_effect(use_reactive!(|(phase)| {
        {
            let mut runs = props.runs.lock().unwrap();
            if runs.contains(&phase) {
                return;
            }
            runs.push(phase);
        }

        match phase {
            0 => {
                let zones = registry.children_of(None, &"blocked");
                assert_eq!(zones.len(), 1);
                assert_eq!(zones[0].label.as_deref(), Some("alpha"));
                zones[0].on_drop.call(DropOutcome {
                    payload: "first",
                    from: None,
                    to: zones[0].id,
                    effect: DropEffect::Move,
                    mode: DragMode::Keyboard,
                    client: Point::default(),
                    element: Point::new(0.0, 120.0),
                    grab: Point::default(),
                    edge: None,
                });
            }
            1 => {
                assert!(
                    registry.children_of(None, &"blocked").is_empty(),
                    "updated accepts callback should reject blocked payloads"
                );
                let zones = registry.children_of(None, &"allowed");
                assert_eq!(zones.len(), 1);
                assert_eq!(zones[0].label.as_deref(), Some("beta"));
                zones[0].on_drop.call(DropOutcome {
                    payload: "blocked",
                    from: None,
                    to: zones[0].id,
                    effect: DropEffect::Move,
                    mode: DragMode::Keyboard,
                    client: Point::default(),
                    element: Point::new(0.0, 120.0),
                    grab: Point::default(),
                    edge: None,
                });
                zones[0].on_drop.call(DropOutcome {
                    payload: "allowed",
                    from: None,
                    to: zones[0].id,
                    effect: DropEffect::Move,
                    mode: DragMode::Keyboard,
                    client: Point::default(),
                    element: Point::new(0.0, 120.0),
                    grab: Point::default(),
                    edge: None,
                });
            }
            other => panic!("unexpected phase {other}"),
        }
    }));

    rsx! { div {} }
}

#[test]
fn tree_target_registered_callback_reads_latest_props() {
    let phase = Arc::new(Mutex::new(0));
    let drops = Arc::new(Mutex::new(Vec::new()));
    let runs = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        dynamic_tree_target_app,
        DynamicTreeTargetProps {
            phase: phase.clone(),
            drops: drops.clone(),
            runs: runs.clone(),
        },
    );

    dom.rebuild_in_place();
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
    assert_eq!(
        *drops.lock().unwrap(),
        vec![(0, TreeDropEvent::new("first", NodeId(7), DropIntent::After))]
    );

    *phase.lock().unwrap() = 1;
    dom.mark_dirty(ScopeId::APP);
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }

    assert_eq!(
        *drops.lock().unwrap(),
        vec![
            (0, TreeDropEvent::new("first", NodeId(7), DropIntent::After)),
            (
                1,
                TreeDropEvent::new("allowed", NodeId(8), DropIntent::Into)
            ),
        ]
    );
}

// --- state data-attributes (the Tailwind contract) -----------------------
//
// State attributes must be *absent* when inactive - not `="false"` - so
// presence-based selectors (CSS `[data-dragging]`, Tailwind
// `data-dragging:opacity-50`) never match idle elements.

#[test]
fn state_attributes_absent_when_idle() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            Draggable::<String> { payload: "a".to_string(), "item" }
            DropZone::<String> { on_drop: move |_: DropOutcome<String>| {}, "zone" }
            FileDropZone { on_files: move |_| {}, "files" }
            SortableList {
                len: 1,
                on_sort: move |_| {},
                render: move |_| rsx! { "sortable" },
            }
        }
    }
    let html = run(app);
    for attr in [
        "data-dragging",
        "data-disabled",
        "data-over",
        "data-active",
        "data-drop-target",
    ] {
        assert!(
            !html.contains(attr),
            "{attr} must be absent when idle: {html}"
        );
    }
}

#[test]
fn file_drop_zone_renders_a_headless_native_picker() {
    fn app() -> Element {
        rsx! {
            FileDropZone {
                filter: FileFilter::new()
                    .extensions(["png"])
                    .content_types(["image/*"]),
                on_files: move |_| {},
                class: "picker-shell",
                "Click or drop"
            }
        }
    }

    let html = run(app);
    assert!(html.contains(r#"class="picker-shell""#), "missing: {html}");
    assert!(html.contains(r#"type="file""#), "missing: {html}");
    assert!(html.contains(r#"accept=".png,image/*""#), "missing: {html}");
    assert!(html.contains("multiple"), "missing: {html}");
    assert!(html.contains("hidden"), "missing: {html}");
    assert!(html.contains(r#"role="button""#), "missing: {html}");
    assert!(html.contains("tabindex=0"), "missing: {html}");
    assert!(
        !html.contains("style="),
        "FileDropZone must not ship visual styles: {html}"
    );
}

#[test]
fn handle_activation_moves_semantics_to_the_handle() {
    fn app() -> Element {
        rsx! {
            DndProvider::<String> {
                Draggable::<String> {
                    payload: "card".to_string(),
                    activation: ActivationPolicy::handle(ActivationConstraint::Distance(6.0)),
                    input { value: "editable" }
                    DragHandle { label: "Move card", "Move" }
                }
            }
        }
    }

    let html = run(app);
    assert!(html.contains("data-dnd-handle"), "missing handle: {html}");
    assert!(
        html.contains(r#"aria-label="Move card""#),
        "missing: {html}"
    );
    assert!(
        html.contains("tabindex=-1"),
        "wrapper should leave tab order: {html}"
    );
}

thread_local! {
    static MONITOR_EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    static MONITOR_EDGES: RefCell<Vec<Option<Edge>>> = const { RefCell::new(Vec::new()) };
    static MONITOR_CANCEL_REASONS: RefCell<Vec<CancelReason>> = const { RefCell::new(Vec::new()) };
    static SETTLE_CONTEXT: RefCell<Option<DndContext<String>>> = const { RefCell::new(None) };
    static DISABLED_EFFECT_CONTEXT: RefCell<Option<DndContext<String>>> = const { RefCell::new(None) };
    static REENTRANT_MONITOR_EVENTS: RefCell<Vec<(&'static str, Option<CancelReason>)>> =
        const { RefCell::new(Vec::new()) };
}

#[component]
fn SettleContextProbe() -> Element {
    let dnd = use_dnd::<String>();
    use_hook(move || SETTLE_CONTEXT.with_borrow_mut(|slot| *slot = Some(dnd)));
    rsx! {}
}

#[component]
fn DisabledEffectContextProbe() -> Element {
    let dnd = use_dnd::<String>();
    use_hook(move || DISABLED_EFFECT_CONTEXT.with_borrow_mut(|slot| *slot = Some(dnd)));
    rsx! {}
}

#[component]
fn MonitorProbe() -> Element {
    use_dnd_monitor::<String>(|event| {
        let name = match event {
            DndEvent::Started(_) => "started",
            DndEvent::Moved(_) => "moved",
            DndEvent::TargetChanged { .. } => "target",
            DndEvent::Dropped(receipt) => {
                MONITOR_EDGES.with_borrow_mut(|edges| edges.push(receipt.outcome.edge));
                "dropped"
            }
            DndEvent::Cancelled { reason, .. } => {
                MONITOR_CANCEL_REASONS.with_borrow_mut(|reasons| reasons.push(reason));
                "cancelled"
            }
            _ => "other",
        };
        MONITOR_EVENTS.with_borrow_mut(|events| events.push(name));
    });
    rsx! {}
}

#[component]
fn ReentrantMonitorProbe() -> Element {
    let mut dnd = use_dnd::<String>();
    use_dnd_monitor::<String>(move |event| {
        if matches!(event, DndEvent::Started(_)) {
            dnd.cancel_with_reason(CancelReason::PointerCancelled);
        }
    });
    use_dnd_monitor::<String>(|event| {
        let observed = match event {
            DndEvent::Started(_) => Some(("started", None)),
            DndEvent::Cancelled { reason, .. } => Some(("cancelled", Some(reason))),
            _ => None,
        };
        if let Some(observed) = observed {
            REENTRANT_MONITOR_EVENTS.with_borrow_mut(|events| events.push(observed));
        }
    });
    rsx! {}
}

#[test]
fn monitor_observes_shared_delivery_lifecycle() {
    const TARGET: ZoneId = ZoneId(9_001);
    fn app() -> Element {
        rsx! {
            DndProvider::<String> {
                MonitorProbe {}
                DragSimProbe::<String> {}
                DropZone::<String> {
                    id: TARGET,
                    edge: EdgeSet::Vertical,
                    on_drop: move |_| {},
                    "target"
                }
            }
        }
    }

    MONITOR_EVENTS.with_borrow_mut(Vec::clear);
    MONITOR_EDGES.with_borrow_mut(Vec::clear);
    MONITOR_CANCEL_REASONS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<String>();
    sim.place(&dom, TARGET, Rect::new(0.0, 0.0, 100.0, 100.0));
    sim.pick_up(&dom, "card".to_string());
    sim.move_to(&dom, Point::new(20.0, 20.0));
    assert_eq!(sim.release(&dom), Some(TARGET));

    MONITOR_EVENTS
        .with_borrow(|events| assert_eq!(events, &["started", "moved", "target", "dropped"]));
    MONITOR_EDGES.with_borrow(|edges| assert_eq!(edges, &[Some(Edge::Top)]));
}

#[test]
fn disabled_drop_effect_skips_acceptance_hover_and_delivery() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    const TARGET: ZoneId = ZoneId(9_000);
    static LEGACY_ACCEPTS: AtomicUsize = AtomicUsize::new(0);
    static RICH_ACCEPTS: AtomicUsize = AtomicUsize::new(0);
    static DELIVERIES: AtomicUsize = AtomicUsize::new(0);

    fn app() -> Element {
        rsx! {
            DndProvider::<String> {
                DragSimProbe::<String> {}
                DisabledEffectContextProbe {}
                DropZone::<String> {
                    id: TARGET,
                    accepts: move |_| {
                        LEGACY_ACCEPTS.fetch_add(1, Ordering::Relaxed);
                        true
                    },
                    accepts_query: move |_| {
                        RICH_ACCEPTS.fetch_add(1, Ordering::Relaxed);
                        true
                    },
                    on_drop: move |_| {
                        DELIVERIES.fetch_add(1, Ordering::Relaxed);
                    },
                    "target"
                }
            }
        }
    }

    LEGACY_ACCEPTS.store(0, Ordering::Relaxed);
    RICH_ACCEPTS.store(0, Ordering::Relaxed);
    DELIVERIES.store(0, Ordering::Relaxed);
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<String>();
    sim.place(&dom, TARGET, Rect::new(-10.0, -10.0, 100.0, 100.0));
    let mut dnd =
        DISABLED_EFFECT_CONTEXT.with_borrow(|slot| slot.expect("disabled-effect context mounted"));
    dom.in_runtime(|| {
        dnd.start(
            "card".to_string(),
            None,
            Point::new(10.0, 10.0),
            Point::default(),
            DropEffect::None,
            DragMode::Pointer,
        )
    });
    rerender(&mut dom);
    let html = dioxus_ssr::render(&dom);
    assert!(!html.contains("data-active"));
    assert_eq!(LEGACY_ACCEPTS.load(Ordering::Relaxed), 0);
    assert_eq!(RICH_ACCEPTS.load(Ordering::Relaxed), 0);

    assert_eq!(sim.release_as(&dom, DropEffect::None), None);
    assert_eq!(LEGACY_ACCEPTS.load(Ordering::Relaxed), 0);
    assert_eq!(RICH_ACCEPTS.load(Ordering::Relaxed), 0);
    assert_eq!(DELIVERIES.load(Ordering::Relaxed), 0);
    assert!(dom.in_runtime(|| !dnd.dragging()));
    assert!(sim.completions(&dom).is_empty());
}

#[test]
fn settled_drop_has_exactly_one_terminal_monitor_event() {
    const TARGET: ZoneId = ZoneId(9_002);
    fn app() -> Element {
        rsx! {
            DndProvider::<String> {
                MonitorProbe {}
                SettleContextProbe {}
                DragSimProbe::<String> {}
                DropZone::<String> {
                    id: TARGET,
                    on_drop: move |_| {},
                    "target"
                }
                DragOverlay::<String> { settle: true, "ghost" }
            }
        }
    }

    MONITOR_EVENTS.with_borrow_mut(Vec::clear);
    MONITOR_CANCEL_REASONS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<String>();
    sim.place(&dom, TARGET, Rect::new(0.0, 0.0, 100.0, 100.0));
    sim.pick_up(&dom, "card".to_string());
    sim.move_to(&dom, Point::new(20.0, 20.0));
    assert_eq!(sim.release(&dom), Some(TARGET));

    let mut dnd = SETTLE_CONTEXT.with_borrow(|slot| slot.expect("settle context mounted"));
    assert!(dom.in_runtime(|| dnd.settling().is_some()));
    dom.in_runtime(|| dnd.cancel());

    MONITOR_EVENTS.with_borrow(|events| {
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == "dropped" || **event == "cancelled")
                .count(),
            1,
            "a visual-settle interruption cannot create a second terminal event: {events:?}"
        );
        assert_eq!(events.last(), Some(&"dropped"));
    });
    MONITOR_CANCEL_REASONS.with_borrow(|reasons| assert!(reasons.is_empty()));
    assert_eq!(sim.completions(&dom), vec![true]);
    assert!(dom.in_runtime(|| dnd.payload().is_none()));
}

#[test]
fn monitor_reports_no_target_and_replaced_cancellation_reasons() {
    fn app() -> Element {
        rsx! {
            DndProvider::<String> {
                MonitorProbe {}
                DragSimProbe::<String> {}
            }
        }
    }

    MONITOR_EVENTS.with_borrow_mut(Vec::clear);
    MONITOR_CANCEL_REASONS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<String>();

    sim.pick_up(&dom, "first".to_string());
    sim.pick_up(&dom, "replacement".to_string());
    assert_eq!(sim.completions(&dom), vec![false]);
    sim.move_to(&dom, Point::new(100.0, 100.0));
    assert_eq!(sim.release(&dom), None);
    assert_eq!(sim.completions(&dom), vec![false, false]);
    MONITOR_CANCEL_REASONS.with_borrow(|reasons| {
        assert_eq!(
            reasons.as_slice(),
            &[CancelReason::Replaced, CancelReason::NoTarget]
        );
    });
}

#[test]
fn nested_monitor_events_are_delivered_fifo_to_every_listener() {
    fn app() -> Element {
        rsx! {
            DndProvider::<String> {
                ReentrantMonitorProbe {}
                DragSimProbe::<String> {}
            }
        }
    }

    REENTRANT_MONITOR_EVENTS.with_borrow_mut(Vec::clear);
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<String>();
    sim.pick_up(&dom, "card".to_string());

    assert!(!sim.dragging(&dom));
    assert_eq!(sim.completions(&dom), vec![false]);
    REENTRANT_MONITOR_EVENTS.with_borrow(|events| {
        assert_eq!(
            events.as_slice(),
            &[
                ("started", None),
                ("cancelled", Some(CancelReason::PointerCancelled)),
            ]
        );
    });
}

#[test]
fn disabled_draggable_carries_data_disabled() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            Draggable::<String> { payload: "a".to_string(), disabled: true, "item" }
        }
    }
    let html = run(app);
    assert!(html.contains(r#"data-disabled="true""#), "missing: {html}");
}

#[test]
fn state_attributes_present_mid_drag() {
    fn app() -> Element {
        let mut dnd = use_dnd_provider::<String>();
        dnd.start(
            "x".to_string(),
            None,
            Point::new(1.0, 1.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        dnd.enter(ZoneId(9));
        rsx! {
            // the dragged payload lights up; the other one doesn't
            Draggable::<String> { payload: "x".to_string(), "dragged" }
            Draggable::<String> { payload: "y".to_string(), "bystander" }
            // hovered zone: data-active + data-over; other zone: active only
            DropZone::<String> { id: ZoneId(9), on_drop: move |_: DropOutcome<String>| {}, "over" }
            DropZone::<String> { id: ZoneId(10), on_drop: move |_: DropOutcome<String>| {}, "idle" }
            // a zone that rejects the payload stays dark entirely
            DropZone::<String> {
                id: ZoneId(11),
                accepts: move |_: String| false,
                on_drop: move |_: DropOutcome<String>| {},
                "reject"
            }
        }
    }
    let html = run(app);
    assert_eq!(
        html.matches(r#"data-dragging="true""#).count(),
        1,
        "exactly the dragged payload's wrapper lights up: {html}"
    );
    assert_eq!(
        html.matches(r#"data-over="true""#).count(),
        1,
        "exactly the hovered zone is over: {html}"
    );
    assert_eq!(
        html.matches(r#"data-active="true""#).count(),
        2,
        "both accepting zones are active, the rejecting one is not: {html}"
    );
}

// --- class forwarding & style merging ------------------------------------

#[test]
fn forwarded_attributes_cannot_replace_component_invariants() {
    fn app() -> Element {
        let mut dnd = use_dnd_provider::<u8>();
        dnd.start(
            1,
            None,
            Point::new(10.0, 10.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        rsx! {
            Draggable::<u8> {
                payload: 1,
                role: "link",
                tabindex: -1_i64,
                "drag"
            }
            DropZone::<u8> {
                id: ZoneId(42),
                "data-active": "caller-value",
                on_drop: move |_| {},
                "drop"
            }
        }
    }

    let html = run(app);
    assert!(
        html.contains(r#"role="button""#),
        "draggable role replaced: {html}"
    );
    assert!(
        html.contains("tabindex=0"),
        "draggable tabindex replaced: {html}"
    );
    assert!(
        html.contains(r#"data-active="true""#) && !html.contains("caller-value"),
        "drop-zone state marker replaced: {html}"
    );
}

#[test]
fn drag_overlay_forwards_class_and_merges_style() {
    fn app() -> Element {
        let mut dnd = use_dnd_provider::<String>();
        dnd.start(
            "x".to_string(),
            None,
            Point::new(10.0, 20.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        rsx! {
            DragOverlay::<String> {
                class: "rotate-3 shadow-xl",
                style: "opacity: 0.9;",
                "ghost"
            }
        }
    }
    let html = run(app);
    assert!(
        html.contains(r#"class="rotate-3 shadow-xl""#),
        "class missing: {html}"
    );
    // Functional positioning survives the user style, which is appended.
    assert!(html.contains("position: fixed"), "positioning lost: {html}");
    assert!(html.contains("opacity: 0.9"), "user style lost: {html}");
}

#[test]
fn draggable_merges_user_style_with_touch_action() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            Draggable::<String> {
                payload: "x".to_string(),
                style: "background: red;",
                "item"
            }
        }
    }
    let html = run(app);
    // User styling remains present, while the drag sensor's behavior-critical
    // touch declarations are emitted last.
    assert!(
        html.contains("touch-action: pan-y"),
        "touch-action must survive a user style: {html}"
    );
    assert!(
        html.contains("background: red; touch-action: pan-y"),
        "touch behavior must follow the user style: {html}"
    );
}

#[test]
fn dioxus_style_properties_cannot_replace_drag_or_flip_invariants() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            Draggable::<String> {
                payload: "x".to_string(),
                touch_action: "auto",
                user_select: "text",
                background_color: "red",
                "drag"
            }
            FlipItem {
                epoch: 0,
                transform: "scale(9)",
                transition: "none",
                opacity: "0.5",
                "flip"
            }
        }
    }

    let html = run(app);
    assert!(
        html.contains("touch-action: pan-y pinch-zoom"),
        "drag touch ownership missing: {html}"
    );
    assert!(!html.contains("touch-action:auto"), "caller won: {html}");
    assert!(!html.contains("user-select:text"), "caller won: {html}");
    assert!(
        html.contains("background-color:red"),
        "user style lost: {html}"
    );
    assert!(
        html.contains("transform: none"),
        "FLIP transform lost: {html}"
    );
    assert!(!html.contains("scale(9)"), "caller replaced FLIP: {html}");
    assert!(html.contains("opacity:0.5"), "unrelated style lost: {html}");
}

#[test]
fn draggable_touch_immediate_restores_touch_action_none() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            Draggable::<String> {
                payload: "x".to_string(),
                touch: TouchSense::Immediate,
                "item"
            }
        }
    }
    let html = run(app);
    assert!(
        html.contains("touch-action: none"),
        "Immediate must own the touch surface outright: {html}"
    );
}

#[test]
fn draggable_does_not_render_native_attrs() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            Draggable::<String> {
                payload: "x".to_string(),
                "item"
            }
        }
    }
    let html = run(app);
    assert!(
        !html.contains("draggable=true") && !html.contains("draggable=false"),
        "in-app draggables should not render native drag attrs: {html}"
    );
}

#[test]
fn board_and_selectable_draggables_do_not_render_native_attrs() {
    fn app() -> Element {
        use_dnd_provider::<BoardPayload<String>>();
        rsx! {
            BoardItem::<String> {
                item: "board-default".to_string(),
                column: ZoneId(1),
                index: 0,
                "board-default"
            }
        }
    }
    let html = run(app);
    assert!(
        !html.contains("draggable=true") && !html.contains("draggable=false"),
        "BoardItem should not render native drag attrs: {html}"
    );

    fn selectable_app() -> Element {
        let selection = use_selection::<u32>();
        use_dnd_provider::<Vec<u32>>();
        rsx! {
            SelectableDraggable::<u32> {
                item: 1,
                selection,
                "select-default"
            }
        }
    }
    let html = run(selectable_app);
    assert!(
        !html.contains("draggable=true") && !html.contains("draggable=false"),
        "SelectableDraggable should not render native drag attrs: {html}"
    );
}

// --- Rect refresh channel --------------------------------------------------
//
// One type-erased "re-measure your zones" channel per provider *tree*:
// nested providers inherit the outermost channel instead of creating their
// own, so a scroll surface anywhere below reaches every registry with one
// handle - and a provider unmounting takes its thunk with it.

#[derive(Clone, Props)]
struct RefreshChannelProps {
    phase: Shared<u8>,
    observed: Shared<Vec<(u8, usize)>>,
}

impl PartialEq for RefreshChannelProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.phase, &other.phase) && Arc::ptr_eq(&self.observed, &other.observed)
    }
}

fn refresh_channel_app(props: RefreshChannelProps) -> Element {
    let phase = *props.phase.lock().unwrap();
    let observed = props.observed.clone();
    rsx! {
        DndProvider::<u8> {
            if phase == 0 {
                DndProvider::<u16> {
                    InnerRefreshProbe {}
                }
            }
            OuterRefreshProbe { phase, observed }
        }
    }
}

#[component]
fn InnerRefreshProbe() -> Element {
    // Seen from inside the nested provider: the same shared channel, with
    // one thunk per provider. A per-provider channel would read 1 here.
    let bus = use_rect_refresh();
    assert_eq!(bus.len(), 2, "nested providers must share one channel");
    rsx! { div {} }
}

#[derive(Clone, Props)]
struct OuterRefreshProbeProps {
    phase: u8,
    observed: Shared<Vec<(u8, usize)>>,
}

impl PartialEq for OuterRefreshProbeProps {
    fn eq(&self, other: &Self) -> bool {
        self.phase == other.phase && Arc::ptr_eq(&self.observed, &other.observed)
    }
}

#[allow(non_snake_case)]
fn OuterRefreshProbe(props: OuterRefreshProbeProps) -> Element {
    let bus = use_rect_refresh();
    props
        .observed
        .lock()
        .unwrap()
        .push((props.phase, bus.len()));
    // Pinging is always safe, dragging or not - idle thunks are no-ops.
    bus.refresh_all();
    rsx! { div {} }
}

#[test]
fn rect_refresh_channel_is_shared_and_unregisters_on_unmount() {
    let phase = Arc::new(Mutex::new(0u8));
    let observed: Shared<Vec<(u8, usize)>> = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        refresh_channel_app,
        RefreshChannelProps {
            phase: phase.clone(),
            observed: observed.clone(),
        },
    );

    dom.rebuild_in_place();
    assert_eq!(
        observed.lock().unwrap().last(),
        Some(&(0, 2)),
        "both providers registered on one channel"
    );

    // Unmount the inner provider; its thunk must leave the channel.
    *phase.lock().unwrap() = 1;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    // One more settled pass so the probe observes the post-unmount state.
    *phase.lock().unwrap() = 2;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    assert_eq!(
        observed.lock().unwrap().last(),
        Some(&(2, 1)),
        "the unmounted provider's thunk is gone"
    );
}

// --- Reduced motion: animated components honor the OS preference -----------

/// Every component that animates via inline transitions marks its moving
/// element with `data-dnd-motion` and renders (once per subtree) a
/// stylesheet that collapses those transitions under
/// `prefers-reduced-motion: reduce`.
#[test]
fn animated_components_ship_the_reduced_motion_override() {
    fn app() -> Element {
        rsx! {
            SortableList {
                len: 2,
                on_sort: move |_| {},
                render: move |_| rsx! { "row" },
            }
        }
    }
    let html = run(app);
    assert_eq!(
        html.matches("prefers-reduced-motion").count(),
        1,
        "one override stylesheet: {html}"
    );
    assert_eq!(
        html.matches("data-dnd-motion=true").count(),
        2,
        "every row is marked: {html}"
    );
    // The sheet must hide itself inline: the UA's `style { display: none }`
    // has zero specificity, so an app rule like `.list > * { display: flex }`
    // would otherwise paint the CSS source as visible text (seen in the
    // gallery). An inline declaration outranks any selector.
    assert!(
        html.contains(r#"<style style="display: none;">"#),
        "stylesheet must be inline-hidden: {html}"
    );

    // FlipItems nested under a grid inherit the grid's stylesheet instead
    // of rendering their own.
    fn grid_app() -> Element {
        rsx! {
            SortableGrid {
                len: 2,
                cols: 2,
                on_sort: move |_| {},
                render: move |ix: usize| rsx! {
                    FlipItem { epoch: 0, "tile {ix}" }
                },
            }
        }
    }
    let html = run(grid_app);
    assert_eq!(
        html.matches("prefers-reduced-motion").count(),
        1,
        "grid anchors one stylesheet for its FlipItems: {html}"
    );
    assert_eq!(
        html.matches("data-dnd-motion=true").count(),
        2,
        "each FlipItem is marked: {html}"
    );
}

// --- RTL: keyboard order follows the visual right-to-left flow -------------

/// With `Direction::Rtl`, spatial ordering within a row runs right-to-left,
/// so keyboard stepping visits zones in the order an RTL reader sees them.
/// The vertical order (top rows first) never changes.
#[test]
fn rtl_spatial_order_follows_reading_direction() {
    fn app() -> Element {
        use_dnd_provider::<u32>();
        let mut reg = use_zone_registry::<u32>();

        let record = |id: u64, x: f64, y: f64| {
            let mut record = ZoneRecord::<u32>::new(ZoneId(id), Callback::new(|_| {}));
            record.rect = Some(Rect::new(x, y, 40.0, 40.0));
            record
        };
        // One row of three zones, then one zone on a second row.
        reg.register(record(1, 0.0, 0.0));
        reg.register(record(2, 50.0, 0.0));
        reg.register(record(3, 100.0, 0.0));
        reg.register(record(4, 0.0, 100.0));

        // LTR: left-to-right within the row, then the next row.
        assert_eq!(reg.direction(), Direction::Ltr);
        assert_eq!(reg.step_zone(None, &0, 1), Some(ZoneId(1)));
        assert_eq!(reg.step_zone(Some(ZoneId(1)), &0, 1), Some(ZoneId(2)));
        assert_eq!(reg.step_zone(Some(ZoneId(3)), &0, 1), Some(ZoneId(4)));

        // RTL: the rightmost zone comes first; rows still top-to-bottom.
        reg.set_direction(Direction::Rtl);
        assert_eq!(reg.step_zone(None, &0, 1), Some(ZoneId(3)));
        assert_eq!(reg.step_zone(Some(ZoneId(3)), &0, 1), Some(ZoneId(2)));
        assert_eq!(reg.step_zone(Some(ZoneId(2)), &0, 1), Some(ZoneId(1)));
        assert_eq!(reg.step_zone(Some(ZoneId(1)), &0, 1), Some(ZoneId(4)));
        // Sibling stepping (the arrow-key path) mirrors the same way.
        assert_eq!(reg.step_sibling(None, &0, 1), Some(ZoneId(3)));

        rsx! { div {} }
    }
    run(app);
}

/// The `dir` prop on `DndProvider` reaches the registry.
#[test]
fn provider_dir_prop_sets_registry_direction() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u8> {
                dir: Direction::Rtl,
                DirProbe {}
            }
        }
    }
    #[component]
    fn DirProbe() -> Element {
        assert_eq!(use_zone_registry::<u8>().direction(), Direction::Rtl);
        rsx! { div {} }
    }
    run(app);
}

/// Self-contained components need no DndProvider, so `AutoScroll` anchors
/// the rect-refresh channel itself when it's the outermost participant -
/// a `SortableList` (and `SortableGrid`) inside joins it, which is what
/// lets autoscrolled sortables re-measure mid-drag.
#[test]
fn autoscroll_anchors_the_refresh_channel_for_sortables() {
    fn app() -> Element {
        rsx! {
            AutoScroll {
                SortableList {
                    len: 3,
                    on_sort: move |_| {},
                    render: move |ix: usize| rsx! { "row {ix}" },
                }
                SortableGrid {
                    len: 4,
                    cols: 2,
                    on_sort: move |_| {},
                    render: move |ix: usize| rsx! { "tile {ix}" },
                }
                ChannelProbe {}
            }
        }
    }
    #[component]
    fn ChannelProbe() -> Element {
        let bus = try_use_context::<RectRefresh>().expect("AutoScroll anchors a channel");
        assert_eq!(bus.len(), 2, "the list and the grid each joined");
        // Pinging with no drag in flight is a no-op, not a panic.
        bus.refresh_all();
        rsx! { div {} }
    }
    run(app);
}

// --- Bridge zones: one box registered in two type-worlds ------------------
//
// The documented cross-type pattern (README "Mixing payload types", the
// gallery's Standup page): zone ids are process-global while registries are
// per-type, so one element registers the *same* ZoneId in two registries
// and fans one DOM measurement into both provider-owned geometry records.
// These tests pin the crate invariants that pattern depends on.

#[derive(Clone, Props)]
struct BridgeProps {
    ticket_drops: Shared<Vec<&'static str>>,
    person_drops: Shared<Vec<u32>>,
}

impl PartialEq for BridgeProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ticket_drops, &other.ticket_drops)
            && Arc::ptr_eq(&self.person_drops, &other.person_drops)
    }
}

fn bridge_app(props: BridgeProps) -> Element {
    use_dnd_provider::<&'static str>();
    use_dnd_provider::<u32>();
    let mut reg_a = use_zone_registry::<&'static str>();
    let mut reg_b = use_zone_registry::<u32>();

    let id = ZoneId(500);
    let ticket_drops = props.ticket_drops.clone();
    let person_drops = props.person_drops.clone();
    use_hook(move || {
        let mut tickets = ZoneRecord::new(
            id,
            Callback::new(move |o: DropOutcome<&'static str>| {
                ticket_drops.lock().unwrap().push(o.payload)
            }),
        );
        tickets.label = Some("agenda".into());
        reg_a.register(tickets);
        let mut people = ZoneRecord::new(
            id,
            Callback::new(move |o: DropOutcome<u32>| person_drops.lock().unwrap().push(o.payload)),
        );
        people.label = Some("agenda".into());
        reg_b.register(people);
    });

    rsx! {
        BridgeProbe {}
    }
}

#[component]
fn BridgeProbe() -> Element {
    let mut reg_a = use_zone_registry::<&'static str>();
    let mut reg_b = use_zone_registry::<u32>();
    let id = ZoneId(500);

    // The same id resolves in both worlds.
    assert!(reg_a.contains(id) && reg_b.contains(id));

    // The element's mount callback fans one measurement into both worlds;
    // each registry owns its copy and both hit-test the same box.
    let rect = Rect::new(0.0, 0.0, 100.0, 50.0);
    reg_a.set_rect(id, rect);
    reg_b.set_rect(id, rect);
    assert_eq!(
        reg_b.cached_rect(id),
        Some(Rect::new(0.0, 0.0, 100.0, 50.0)),
    );
    let p = Point::new(10.0, 10.0);
    assert_eq!(reg_a.hit_test(p), Some(id));
    assert_eq!(reg_b.hit_test(p), Some(id));
    assert_eq!(reg_a.hit_test_closest(p, &"ticket", 48.0), Some(id));
    assert_eq!(reg_b.hit_test_closest(p, &7, 48.0), Some(id));

    // Keyboard navigation lists the bridge among each world's own zones.
    assert_eq!(reg_a.step_zone(None, &"ticket", 1), Some(id));
    assert_eq!(reg_b.step_zone(None, &7, 1), Some(id));

    // Each drop is delivered through its own typed callback.
    let outcome_a = DropOutcome {
        payload: "ship it",
        from: None,
        to: id,
        effect: DropEffect::Move,
        mode: DragMode::Pointer,
        client: p,
        element: p,
        grab: Point::default(),

        edge: None,
    };
    reg_a.get(id).unwrap().on_drop.call(outcome_a);
    let outcome_b = DropOutcome {
        payload: 7u32,
        from: None,
        to: id,
        effect: DropEffect::Move,
        mode: DragMode::Keyboard,
        client: p,
        element: p,
        grab: Point::default(),

        edge: None,
    };
    reg_b.get(id).unwrap().on_drop.call(outcome_b);

    // The registrations are independent: leaving one world doesn't touch
    // the other.
    reg_a.unregister(id);
    assert!(!reg_a.contains(id));
    assert!(reg_b.contains(id));

    rsx! { div {} }
}

#[test]
fn bridge_zone_same_id_in_two_registries_stays_typed() {
    let ticket_drops = Arc::new(Mutex::new(Vec::new()));
    let person_drops = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        bridge_app,
        BridgeProps {
            ticket_drops: ticket_drops.clone(),
            person_drops: person_drops.clone(),
        },
    );
    dom.rebuild_in_place();

    // One drop per world, each through its own callback - no crossover.
    assert_eq!(*ticket_drops.lock().unwrap(), vec!["ship it"]);
    assert_eq!(*person_drops.lock().unwrap(), vec![7]);
}

// The packaged form of the same pattern: `BridgeDropZone<A, B>` registers
// itself in both worlds with per-world acceptance and typed callbacks.

fn bridge_component_app(props: BridgeProps) -> Element {
    let ticket_drops = props.ticket_drops.clone();
    let person_drops = props.person_drops.clone();
    rsx! {
        DndProvider::<&'static str> {
            DndProvider::<u32> {
                BridgeDropZone::<&'static str, u32> {
                    id: ZoneId(600),
                    label: "agenda",
                    accepts_a: move |t: &'static str| t != "done",
                    on_drop_a: move |o: DropOutcome<&'static str>| {
                        ticket_drops.lock().unwrap().push(o.payload)
                    },
                    on_drop_b: move |o: DropOutcome<u32>| {
                        person_drops.lock().unwrap().push(o.payload)
                    },
                    "agenda"
                }
                BridgeComponentProbe {}
            }
        }
    }
}

#[component]
fn BridgeComponentProbe() -> Element {
    let reg_a = use_zone_registry::<&'static str>();
    let reg_b = use_zone_registry::<u32>();
    let id = ZoneId(600);

    // One component, registered in both worlds, label synced to each.
    assert!(reg_a.contains(id) && reg_b.contains(id));
    let rec_a = reg_a.get(id).expect("registered in world A");
    let rec_b = reg_b.get(id).expect("registered in world B");
    assert_eq!(rec_a.label.as_deref(), Some("agenda"));
    assert_eq!(rec_b.label.as_deref(), Some("agenda"));

    // Per-world acceptance: world A filters, world B takes everything.
    assert!(rec_a.accepts_payload(&"fix the ghost"));
    assert!(!rec_a.accepts_payload(&"done"));
    assert!(rec_b.accepts_payload(&7));
    // Keyboard navigation honors it too: a rejected payload finds no zone.
    assert_eq!(reg_a.step_zone(None, &"fix the ghost", 1), Some(id));
    assert_eq!(reg_a.step_zone(None, &"done", 1), None);

    // Each drop is delivered through its own typed callback.
    let p = Point::new(5.0, 5.0);
    rec_a.on_drop.call(DropOutcome {
        payload: "fix the ghost",
        from: None,
        to: id,
        effect: DropEffect::Move,
        mode: DragMode::Pointer,
        client: p,
        element: p,
        grab: Point::default(),

        edge: None,
    });
    rec_b.on_drop.call(DropOutcome {
        payload: 7u32,
        from: None,
        to: id,
        effect: DropEffect::Move,
        mode: DragMode::Keyboard,
        client: p,
        element: p,
        grab: Point::default(),

        edge: None,
    });

    rsx! { div {} }
}

#[test]
fn bridge_drop_zone_component_registers_both_worlds_with_per_world_accepts() {
    let ticket_drops = Arc::new(Mutex::new(Vec::new()));
    let person_drops = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        bridge_component_app,
        BridgeProps {
            ticket_drops: ticket_drops.clone(),
            person_drops: person_drops.clone(),
        },
    );
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);

    assert_eq!(*ticket_drops.lock().unwrap(), vec!["fix the ghost"]);
    assert_eq!(*person_drops.lock().unwrap(), vec![7]);
    // Idle: neither styling hook is present.
    assert!(
        !html.contains("data-active"),
        "idle zone must not be active: {html}"
    );
    assert!(
        !html.contains("data-over"),
        "idle zone must not be over: {html}"
    );
}

#[derive(Clone, Props)]
struct DynamicBridgeAcceptsProps {
    allowed: Shared<bool>,
    observed: Shared<Vec<bool>>,
}

impl PartialEq for DynamicBridgeAcceptsProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.allowed, &other.allowed) && Arc::ptr_eq(&self.observed, &other.observed)
    }
}

fn dynamic_bridge_accepts_app(props: DynamicBridgeAcceptsProps) -> Element {
    let allowed = *props.allowed.lock().unwrap();
    rsx! {
        DndProvider::<u8> {
            DndProvider::<u16> {
                BridgeDropZone::<u8, u16> {
                    id: ZoneId(601),
                    accepts_a: move |_: u8| allowed,
                    on_drop_a: move |_: DropOutcome<u8>| {},
                    on_drop_b: move |_: DropOutcome<u16>| {},
                }
                DynamicBridgeAcceptsProbe { allowed, observed: props.observed }
            }
        }
    }
}

#[derive(Clone, Props)]
struct DynamicBridgeAcceptsProbeProps {
    allowed: bool,
    observed: Shared<Vec<bool>>,
}

impl PartialEq for DynamicBridgeAcceptsProbeProps {
    fn eq(&self, other: &Self) -> bool {
        self.allowed == other.allowed && Arc::ptr_eq(&self.observed, &other.observed)
    }
}

#[allow(non_snake_case)]
fn DynamicBridgeAcceptsProbe(props: DynamicBridgeAcceptsProbeProps) -> Element {
    let registry = use_zone_registry::<u8>();
    let accepted = registry
        .acceptable(&7)
        .iter()
        .any(|zone| zone.id == ZoneId(601));
    props.observed.lock().unwrap().push(accepted);
    assert_eq!(accepted, props.allowed);
    rsx! {}
}

#[test]
fn bridge_registry_delivery_follows_live_acceptance_policy() {
    let allowed = Arc::new(Mutex::new(false));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut dom = VirtualDom::new_with_props(
        dynamic_bridge_accepts_app,
        DynamicBridgeAcceptsProps {
            allowed: allowed.clone(),
            observed: observed.clone(),
        },
    );
    dom.rebuild_in_place();

    *allowed.lock().unwrap() = true;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);

    assert_eq!(*observed.lock().unwrap(), vec![false, true]);
}

// The generated form for N > 2: `bridge_drop_zone!` expands the same recipe
// to any list of payload worlds - here three - with per-world acceptance
// and typed callbacks, no `dyn Any` anywhere.

dioxus_dnd::bridge_drop_zone!(TriBridgeZone {
    (&'static str, accepts_ticket, on_drop_ticket),
    (u32, accepts_person, on_drop_person),
    (i8, accepts_alert, on_drop_alert),
});

static TRI_TICKETS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
static TRI_PEOPLE: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static TRI_ALERTS: Mutex<Vec<i8>> = Mutex::new(Vec::new());

thread_local! {
    static DYNAMIC_TRI_ID: RefCell<Option<Signal<ZoneId>>> = const { RefCell::new(None) };
    static DYNAMIC_TRI_REGISTRY: RefCell<Option<ZoneRegistry<&'static str>>> = const { RefCell::new(None) };
    static DYNAMIC_BRIDGE_PARENT: RefCell<Option<ZoneId>> = const { RefCell::new(None) };
}

const DYNAMIC_TRI_CHILD: ZoneId = ZoneId(704);

fn tri_bridge_app() -> Element {
    rsx! {
        DndProvider::<&'static str> {
            DndProvider::<u32> {
                DndProvider::<i8> {
                    TriBridgeZone {
                        id: ZoneId(700),
                        label: "agenda",
                        accepts_ticket: move |t: &'static str| t != "done",
                        on_drop_ticket: move |o: DropOutcome<&'static str>| {
                            TRI_TICKETS.lock().unwrap().push(o.payload)
                        },
                        on_drop_person: move |o: DropOutcome<u32>| {
                            TRI_PEOPLE.lock().unwrap().push(o.payload)
                        },
                        on_drop_alert: move |o: DropOutcome<i8>| {
                            TRI_ALERTS.lock().unwrap().push(o.payload)
                        },
                        "agenda"
                    }
                    TriBridgeProbe {}
                }
            }
        }
    }
}

#[component]
fn TriBridgeProbe() -> Element {
    let mut reg_a = use_zone_registry::<&'static str>();
    let mut reg_b = use_zone_registry::<u32>();
    let mut reg_c = use_zone_registry::<i8>();
    let id = ZoneId(700);

    // One component, registered in all three worlds, label synced to each.
    assert!(reg_a.contains(id) && reg_b.contains(id) && reg_c.contains(id));
    for label in [
        reg_a.get(id).unwrap().label.clone(),
        reg_b.get(id).unwrap().label.clone(),
        reg_c.get(id).unwrap().label.clone(),
    ] {
        assert_eq!(label.as_deref(), Some("agenda"));
    }

    // Per-world acceptance: the ticket world filters, the others take all.
    let rec_a = reg_a.get(id).unwrap();
    assert!(rec_a.accepts_payload(&"fix the ghost"));
    assert!(!rec_a.accepts_payload(&"done"));
    assert!(reg_c.get(id).unwrap().accepts_payload(&-3i8));

    // The element's mount callback fans one measurement into all three
    // provider-owned records.
    let rect = Rect::new(0.0, 0.0, 80.0, 40.0);
    reg_a.set_rect(id, rect);
    reg_b.set_rect(id, rect);
    reg_c.set_rect(id, rect);
    let p = Point::new(5.0, 5.0);
    assert_eq!(reg_a.hit_test(p), Some(id));
    assert_eq!(reg_b.hit_test(p), Some(id));
    assert_eq!(reg_c.hit_test(p), Some(id));

    // Each drop is delivered through its own typed callback.
    rec_a.on_drop.call(DropOutcome {
        payload: "fix the ghost",
        from: None,
        to: id,
        effect: DropEffect::Move,
        mode: DragMode::Pointer,
        client: p,
        element: p,
        grab: Point::default(),
        edge: None,
    });
    reg_b.get(id).unwrap().on_drop.call(DropOutcome {
        payload: 7u32,
        from: None,
        to: id,
        effect: DropEffect::Move,
        mode: DragMode::Keyboard,
        client: p,
        element: p,
        grab: Point::default(),
        edge: None,
    });
    reg_c.get(id).unwrap().on_drop.call(DropOutcome {
        payload: -3i8,
        from: None,
        to: id,
        effect: DropEffect::Move,
        mode: DragMode::Pointer,
        client: p,
        element: p,
        grab: Point::default(),
        edge: None,
    });

    rsx! { div {} }
}

#[test]
fn bridge_drop_zone_macro_registers_three_worlds_with_typed_callbacks() {
    TRI_TICKETS.lock().unwrap().clear();
    TRI_PEOPLE.lock().unwrap().clear();
    TRI_ALERTS.lock().unwrap().clear();
    let mut dom = VirtualDom::new(tri_bridge_app);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);

    assert_eq!(*TRI_TICKETS.lock().unwrap(), vec!["fix the ghost"]);
    assert_eq!(*TRI_PEOPLE.lock().unwrap(), vec![7]);
    assert_eq!(*TRI_ALERTS.lock().unwrap(), vec![-3]);
    // Idle: neither styling hook is present.
    assert!(
        !html.contains("data-active"),
        "idle zone must not be active: {html}"
    );
    assert!(
        !html.contains("data-over"),
        "idle zone must not be over: {html}"
    );
}

fn dynamic_tri_bridge_app() -> Element {
    let id = use_signal(|| ZoneId(701));
    use_hook(move || DYNAMIC_TRI_ID.with_borrow_mut(|slot| *slot = Some(id)));
    rsx! {
        DndProvider::<&'static str> {
            DndProvider::<u32> {
                DndProvider::<i8> {
                    TriBridgeZone {
                        id: id(),
                        on_drop_ticket: move |_: DropOutcome<&'static str>| {},
                        on_drop_person: move |_: DropOutcome<u32>| {},
                        on_drop_alert: move |_: DropOutcome<i8>| {},
                        DynamicBridgeParentProbe {}
                        DropZone::<&'static str> {
                            id: DYNAMIC_TRI_CHILD,
                            on_drop: move |_: DropOutcome<&'static str>| {},
                            DynamicTriProbe {}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DynamicBridgeParentProbe() -> Element {
    let parent = dioxus_dnd::core::use_parent_zone();
    DYNAMIC_BRIDGE_PARENT.with_borrow_mut(|slot| *slot = parent);
    rsx! {}
}

#[component]
fn DynamicTriProbe() -> Element {
    let registry = use_zone_registry::<&'static str>();
    use_hook(move || DYNAMIC_TRI_REGISTRY.with_borrow_mut(|slot| *slot = Some(registry)));
    rsx! {}
}

#[test]
fn bridge_macro_remounts_nested_context_when_its_id_changes() {
    let mut dom = VirtualDom::new(dynamic_tri_bridge_app);
    dom.rebuild_in_place();
    let registry = DYNAMIC_TRI_REGISTRY.with_borrow(|slot| slot.expect("nested probe mounted"));
    assert_eq!(
        dom.in_runtime(|| registry.get(DYNAMIC_TRI_CHILD).and_then(|zone| zone.parent)),
        Some(ZoneId(701))
    );

    let mut id = DYNAMIC_TRI_ID.with_borrow(|slot| slot.expect("dynamic id captured"));
    dom.in_runtime(|| id.set(ZoneId(702)));
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }

    assert!(!dom.in_runtime(|| registry.contains(ZoneId(701))));
    assert!(dom.in_runtime(|| registry.contains(ZoneId(702))));
    assert_eq!(
        DYNAMIC_BRIDGE_PARENT.with_borrow(|parent| *parent),
        Some(ZoneId(702)),
        "the bridge boundary must publish its current id"
    );
    assert_eq!(
        dom.in_runtime(|| registry.get(DYNAMIC_TRI_CHILD).and_then(|zone| zone.parent)),
        Some(ZoneId(702))
    );
}

thread_local! {
    static DYNAMIC_ID_MOUNTS: Cell<usize> = const { Cell::new(0) };
    static DYNAMIC_ID_PHASE: RefCell<Option<Signal<bool>>> = const { RefCell::new(None) };
    static DYNAMIC_CANVAS_REGISTRY: RefCell<Option<ZoneRegistry<u16>>> = const { RefCell::new(None) };
    static DYNAMIC_BOARD_REGISTRY: RefCell<Option<ZoneRegistry<BoardPayload<u32>>>> = const { RefCell::new(None) };
    static DYNAMIC_SORTABLE_ID_REGISTRY: RefCell<Option<ZoneRegistry<SortablePayload<u32>>>> = const { RefCell::new(None) };
}

fn dynamic_identity_app() -> Element {
    let phase = use_signal(|| false);
    use_hook(move || DYNAMIC_ID_PHASE.with_borrow_mut(|slot| *slot = Some(phase)));
    let second = phase();
    let drag_id = if second { DragId(802) } else { DragId(801) };
    let canvas_id = if second { ZoneId(812) } else { ZoneId(811) };
    let board_id = if second { ZoneId(822) } else { ZoneId(821) };
    let group_id = SortableGroupId::new(if second { 32 } else { 31 });
    rsx! {
        DndProvider::<u8> {
            Draggable::<u8> { payload: 1, drag_id, DynamicIdentityMountProbe {} }
        }
        DndProvider::<u16> {
            CanvasDropZone::<u16> {
                id: canvas_id,
                on_drop: move |_| {},
                DynamicCanvasIdentityProbe {}
            }
        }
        DndProvider::<BoardPayload<u32>> {
            BoardColumn::<u32> {
                id: board_id,
                on_move: move |_| {},
                DynamicBoardIdentityProbe {}
            }
        }
        SortableProvider::<u32> {
            SortableGroup::<u32> {
                id: group_id,
                on_reorder: move |_| {},
                DynamicSortableIdentityProbe {}
            }
        }
    }
}

#[component]
fn DynamicIdentityMountProbe() -> Element {
    use_hook(|| DYNAMIC_ID_MOUNTS.with(|mounts| mounts.set(mounts.get() + 1)));
    rsx! {}
}

#[component]
fn DynamicCanvasIdentityProbe() -> Element {
    let registry = use_zone_registry::<u16>();
    use_hook(move || DYNAMIC_CANVAS_REGISTRY.with_borrow_mut(|slot| *slot = Some(registry)));
    rsx! {}
}

#[component]
fn DynamicBoardIdentityProbe() -> Element {
    let registry = use_zone_registry::<BoardPayload<u32>>();
    use_hook(move || DYNAMIC_BOARD_REGISTRY.with_borrow_mut(|slot| *slot = Some(registry)));
    rsx! {}
}

#[component]
fn DynamicSortableIdentityProbe() -> Element {
    let registry = use_zone_registry::<SortablePayload<u32>>();
    use_hook(move || {
        DYNAMIC_SORTABLE_ID_REGISTRY.with_borrow_mut(|slot| *slot = Some(registry));
    });
    rsx! {}
}

#[test]
fn public_identity_props_replace_their_keyed_dioxus_instances() {
    DYNAMIC_ID_MOUNTS.with(|mounts| mounts.set(0));
    let mut dom = VirtualDom::new(dynamic_identity_app);
    dom.rebuild_in_place();

    let canvas = DYNAMIC_CANVAS_REGISTRY.with_borrow(|slot| slot.expect("canvas registry"));
    let board = DYNAMIC_BOARD_REGISTRY.with_borrow(|slot| slot.expect("board registry"));
    let sortable =
        DYNAMIC_SORTABLE_ID_REGISTRY.with_borrow(|slot| slot.expect("sortable registry"));
    assert!(dom.in_runtime(|| canvas.contains(ZoneId(811))));
    assert!(dom.in_runtime(|| board.contains(ZoneId(821))));
    let first_group_zone = dom.in_runtime(|| sortable.records()[0].id);

    let mut phase = DYNAMIC_ID_PHASE.with_borrow(|slot| slot.expect("identity phase signal"));
    dom.in_runtime(|| phase.set(true));
    for _ in 0..3 {
        dom.process_events();
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }

    let canvas_ids = dom.in_runtime(|| {
        canvas
            .records()
            .into_iter()
            .map(|record| record.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(canvas_ids, [ZoneId(812)]);
    let board_ids = dom.in_runtime(|| {
        board
            .records()
            .into_iter()
            .map(|record| record.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(board_ids, [ZoneId(822)]);
    let group_records = dom.in_runtime(|| sortable.records());
    assert_eq!(group_records.len(), 1);
    assert_ne!(group_records[0].id, first_group_zone);
    assert_eq!(DYNAMIC_ID_MOUNTS.with(Cell::get), 2);
}

// --- Headless test driver: dioxus_dnd::test ---------------------------------

/// The full pointer arc through the driver: pick up, hover (with the
/// data-attributes users style reacting mid-flight), release, handler runs
/// with the production outcome.
#[test]
fn drag_sim_drives_a_full_pointer_arc() {
    static LANDED: Mutex<Vec<(String, Option<ZoneId>)>> = Mutex::new(Vec::new());

    fn app() -> Element {
        rsx! {
            DndProvider::<String> {
                DragSimProbe::<String> {}
                DropZone::<String> {
                    id: ZoneId(71),
                    label: "Reading",
                    on_drop: move |o: DropOutcome<String>| {
                        LANDED.lock().unwrap().push((o.payload, o.from))
                    },
                    "reading"
                }
                DropZone::<String> {
                    id: ZoneId(72),
                    label: "Finished",
                    on_drop: move |o: DropOutcome<String>| {
                        LANDED.lock().unwrap().push((o.payload, o.from))
                    },
                    "finished"
                }
            }
        }
    }

    LANDED.lock().unwrap().clear();
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<String>();

    // Headless layout: the test owns the geometry.
    sim.place(&dom, ZoneId(71), Rect::new(0.0, 0.0, 100.0, 40.0));
    sim.place(&dom, ZoneId(72), Rect::new(0.0, 60.0, 100.0, 40.0));

    sim.pick_up_from(&dom, "book".to_string(), Some(ZoneId(71)));
    assert!(sim.dragging(&dom));
    rerender(&mut dom);
    assert!(
        dioxus_ssr::render(&dom).contains("data-active"),
        "zones lit on pickup"
    );

    sim.move_to(&dom, Point::new(50.0, 80.0));
    assert_eq!(sim.over(&dom), Some(ZoneId(72)));
    rerender(&mut dom);
    assert!(
        dioxus_ssr::render(&dom).contains("data-over"),
        "hovered zone highlighted"
    );

    assert_eq!(sim.release(&dom), Some(ZoneId(72)));
    assert!(!sim.dragging(&dom));
    rerender(&mut dom);
    assert!(
        !dioxus_ssr::render(&dom).contains("data-active"),
        "zones unlit after the drop"
    );
    assert_eq!(
        *LANDED.lock().unwrap(),
        vec![("book".to_string(), Some(ZoneId(71)))]
    );
}

/// Releases mirror the pointer gesture's forgiveness: a rejecting zone
/// under the pointer doesn't take the drop, a near miss within 48px snaps
/// to the closest acceptable zone, and a far miss cancels.
#[test]
fn drag_sim_release_respects_acceptance_and_snap() {
    static TAKEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                DragSimProbe::<u32> {}
                DropZone::<u32> {
                    id: ZoneId(73),
                    accepts: move |_p: u32| false,
                    on_drop: move |o: DropOutcome<u32>| TAKEN.lock().unwrap().push(o.payload),
                    "rejects"
                }
                DropZone::<u32> {
                    id: ZoneId(74),
                    on_drop: move |o: DropOutcome<u32>| TAKEN.lock().unwrap().push(o.payload),
                    "accepts"
                }
            }
        }
    }

    TAKEN.lock().unwrap().clear();
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<u32>();
    sim.place(&dom, ZoneId(73), Rect::new(0.0, 0.0, 100.0, 40.0));
    sim.place(&dom, ZoneId(74), Rect::new(0.0, 100.0, 100.0, 40.0));

    // Over the rejecting zone, 80px from the accepting one: cancels.
    sim.pick_up(&dom, 5);
    sim.move_to(&dom, Point::new(50.0, 20.0));
    assert_eq!(sim.release(&dom), None);
    assert!(!sim.dragging(&dom), "cancel reset the drag");
    assert!(TAKEN.lock().unwrap().is_empty());

    // In the gap, 20px from the accepting zone's edge: snaps to it.
    sim.pick_up(&dom, 5);
    sim.move_to(&dom, Point::new(50.0, 80.0));
    assert_eq!(sim.over(&dom), None, "gap hovers nothing");
    assert_eq!(sim.release(&dom), Some(ZoneId(74)), "48px snap");
    assert_eq!(*TAKEN.lock().unwrap(), vec![5]);
}

/// Release selection is acceptance-aware before delivery: a rejecting zone
/// later in registry order cannot mask an accepting overlapping target.
#[test]
fn drag_sim_release_falls_through_overlapping_rejector() {
    static TAKEN: Mutex<Vec<ZoneId>> = Mutex::new(Vec::new());

    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                DragSimProbe::<u32> {}
                DropZone::<u32> {
                    id: ZoneId(83),
                    on_drop: move |_: DropOutcome<u32>| TAKEN.lock().unwrap().push(ZoneId(83)),
                    "accepts"
                }
                DropZone::<u32> {
                    id: ZoneId(84),
                    accepts: move |_: u32| false,
                    on_drop: move |_: DropOutcome<u32>| -> () {
                        panic!("rejecting zone received drop");
                    },
                    "rejects"
                }
            }
        }
    }

    TAKEN.lock().unwrap().clear();
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<u32>();
    let overlap = Rect::new(0.0, 0.0, 100.0, 40.0);
    sim.place(&dom, ZoneId(83), overlap);
    sim.place(&dom, ZoneId(84), overlap);

    sim.pick_up(&dom, 5);
    sim.move_to(&dom, Point::new(50.0, 20.0));
    assert_eq!(sim.release(&dom), Some(ZoneId(83)));
    assert_eq!(*TAKEN.lock().unwrap(), vec![ZoneId(83)]);
}

/// The one-line arc, and proof the driver ends in the production drop
/// path: the receiving zone's closest-edge enrichment and an explicit
/// copy effect both arrive in the outcome.
#[test]
fn simulate_drag_delivers_production_outcomes() {
    static GOT: Mutex<Vec<(u32, Option<Edge>, DropEffect)>> = Mutex::new(Vec::new());

    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                DragSimProbe::<u32> {}
                DropZone::<u32> {
                    id: ZoneId(75),
                    edge: EdgeSet::Vertical,
                    on_drop: move |o: DropOutcome<u32>| {
                        GOT.lock().unwrap().push((o.payload, o.edge, o.effect))
                    },
                    "tray"
                }
            }
        }
    }

    GOT.lock().unwrap().clear();
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<u32>();
    sim.place(&dom, ZoneId(75), Rect::new(0.0, 0.0, 200.0, 50.0));

    // One line, low in the zone: lands with the edge the zone computed.
    let landed = simulate_drag(&mut dom, 9u32, None, &[Point::new(100.0, 45.0)]);
    assert_eq!(landed, Some(ZoneId(75)));

    // Granular arc with a forced copy effect.
    sim.pick_up(&dom, 10);
    sim.move_to(&dom, Point::new(100.0, 5.0));
    assert_eq!(sim.release_as(&dom, DropEffect::Copy), Some(ZoneId(75)));

    assert_eq!(
        *GOT.lock().unwrap(),
        vec![
            (9, Some(Edge::Bottom), DropEffect::Move),
            (10, Some(Edge::Top), DropEffect::Copy),
        ]
    );
}

// --- Debug overlay: the registry, drawn -------------------------------------

/// The overlay draws one outline per *measured* zone with its label and id,
/// marks the hovered zone and per-zone acceptance live during a drag, and
/// counts unmeasured zones in the status chip so absence is visible too.
#[test]
fn debug_overlay_draws_measured_zones_with_live_state() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                DropZone::<u32> {
                    id: ZoneId(61),
                    label: "Inbox",
                    on_drop: move |_: DropOutcome<u32>| {},
                    "a"
                }
                DropZone::<u32> {
                    id: ZoneId(62),
                    label: "Archive",
                    accepts: move |_p: u32| false,
                    on_drop: move |_: DropOutcome<u32>| {},
                    "b"
                }
                DropZone::<u32> {
                    id: ZoneId(63),
                    label: "Unmeasured",
                    on_drop: move |_: DropOutcome<u32>| {},
                    "c"
                }
                DebugProbe {}
                DndDebugOverlay::<u32> {}
            }
        }
    }

    #[component]
    fn DebugProbe() -> Element {
        let mut reg = use_zone_registry::<u32>();
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            // Measure two zones; leave 63 rect-less.
            reg.set_rect(ZoneId(61), Rect::new(0.0, 0.0, 100.0, 40.0));
            reg.set_rect(ZoneId(62), Rect::new(0.0, 60.0, 100.0, 40.0));
            // A drag in flight, hovering Inbox.
            dnd.start(
                5,
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                DragMode::Pointer,
            );
            dnd.enter(ZoneId(61));
        });
        rsx! {
            div {}
        }
    }

    let html = run(app);
    // Hovered, accepting zone: outlined, marked over.
    assert!(
        html.contains(r#"data-debug-zone="61" data-over="true" data-accepts="true""#),
        "zone 61 over + accepting: {html}"
    );
    assert!(html.contains("Inbox #61 - over"), "labeled tag: {html}");
    // Rejecting zone: outlined, no over, acceptance false.
    assert!(
        html.contains(r#"data-debug-zone="62" data-accepts="false""#),
        "zone 62 rejects: {html}"
    );
    assert!(
        html.contains("Archive #62 - rejects"),
        "rejects tag: {html}"
    );
    // Unmeasured zone draws no outline but is counted.
    assert!(
        !html.contains(r#"data-debug-zone="63""#),
        "no rect, no outline: {html}"
    );
    assert!(
        html.contains("dragging - over zone 61"),
        "status chip: {html}"
    );
}

/// Idle: no acceptance markers, and the status chip reports the census.
#[test]
fn debug_overlay_idle_reports_census() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                DropZone::<u32> {
                    id: ZoneId(64),
                    label: "Inbox",
                    on_drop: move |_: DropOutcome<u32>| {},
                    "a"
                }
                DndDebugOverlay::<u32> {}
            }
        }
    }
    let html = run(app);
    assert!(
        !html.contains("data-accepts"),
        "no drag, no acceptance: {html}"
    );
    assert!(!html.contains("data-over"), "no drag, no hover: {html}");
    assert!(
        html.contains("1 zones (1 unmeasured) - idle"),
        "census: {html}"
    );
}

// --- Localizable strings: the DndStrings context ---------------------------

/// Components read every phrase from the `DndStrings` context; providing
/// one whose closures read a locale swaps the whole voice. English stays
/// the built-in fallback when nothing is provided (pinned by the existing
/// `reorder_buttons_render_labels_and_edge_disabling` test).
#[test]
fn dnd_strings_context_swaps_the_locale() {
    use std::rc::Rc;

    #[derive(Clone, PartialEq, Props)]
    struct LocaleProps {
        locale: &'static str,
    }

    fn app(props: LocaleProps) -> Element {
        // A real app reads a signal (or an i18n crate) inside the closures;
        // the lookup happens per call, so a live switch re-renders readers.
        let locale = use_signal(|| props.locale);
        use_context_provider(|| DndStrings {
            move_up: Rc::new(move |name| match *locale.peek() {
                "es" => format!("Subir {name}"),
                _ => format!("Move {name} up"),
            }),
            move_down: Rc::new(move |name| match *locale.peek() {
                "es" => format!("Bajar {name}"),
                _ => format!("Move {name} down"),
            }),
            row: Rc::new(move |n| match *locale.peek() {
                "es" => format!("elemento {n}"),
                _ => format!("item {n}"),
            }),
            selection_count: Rc::new(move |n| match *locale.peek() {
                "es" => format!("{n} elementos"),
                _ => format!("{n} item(s)"),
            }),
            ..Default::default()
        });
        rsx! {
            ReorderButtons { index: 1, total: 3, on_sort: move |_| {} }
            DndProvider::<Vec<u32>> {
                SelectionBadgeProbe {}
                SelectionCount::<u32> {}
            }
        }
    }

    #[component]
    fn SelectionBadgeProbe() -> Element {
        let mut dnd = use_dnd::<Vec<u32>>();
        use_hook(move || {
            dnd.start(
                vec![7, 8, 9],
                None,
                Point::new(1.0, 1.0),
                Point::default(),
                DropEffect::Move,
                DragMode::Pointer,
            )
        });
        rsx! {
            div {}
        }
    }

    for (locale, up, down, badge) in [
        ("en", "Move item 2 up", "Move item 2 down", "3 item(s)"),
        ("es", "Subir elemento 2", "Bajar elemento 2", "3 elementos"),
    ] {
        let mut dom = VirtualDom::new_with_props(app, LocaleProps { locale });
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(
            html.contains(&format!("aria-label=\"{up}\"")),
            "{locale}: {html}"
        );
        assert!(
            html.contains(&format!("aria-label=\"{down}\"")),
            "{locale}: {html}"
        );
        assert!(html.contains(badge), "{locale}: {html}");
    }
}

// --- Closest edge: data-edge + DropOutcome::edge ---------------------------

/// Production delivery applies a target's edge policy before both the
/// monitor receipt and the target callback observe the outcome.
#[test]
fn drop_zone_edge_prop_enriches_pointer_outcomes() {
    static RECEIVED: Mutex<Vec<Option<Edge>>> = Mutex::new(Vec::new());

    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                DragSimProbe::<u32> {}
                DropZone::<u32> {
                    id: ZoneId(41),
                    edge: EdgeSet::Vertical,
                    on_drop: move |o: DropOutcome<u32>| RECEIVED.lock().unwrap().push(o.edge),
                    "tracked"
                }
                DropZone::<u32> {
                    id: ZoneId(42),
                    on_drop: move |o: DropOutcome<u32>| RECEIVED.lock().unwrap().push(o.edge),
                    "plain"
                }
            }
        }
    }

    RECEIVED.lock().unwrap().clear();
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let mut sim = drag_sim::<u32>();
    sim.place(&dom, ZoneId(41), Rect::new(0.0, 0.0, 300.0, 40.0));
    sim.place(&dom, ZoneId(42), Rect::new(0.0, 100.0, 300.0, 40.0));

    sim.pick_up(&dom, 1);
    sim.move_to(&dom, Point::new(150.0, 31.0));
    assert_eq!(sim.release(&dom), Some(ZoneId(41)));

    sim.pick_up(&dom, 2);
    sim.move_to(&dom, Point::new(150.0, 131.0));
    assert_eq!(sim.release(&dom), Some(ZoneId(42)));

    assert_eq!(*RECEIVED.lock().unwrap(), vec![Some(Edge::Bottom), None]);
}

/// `data-edge` never renders while idle, even with the prop set.
#[test]
fn edge_attribute_absent_when_idle() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                DropZone::<u32> {
                    edge: EdgeSet::All,
                    on_drop: move |_: DropOutcome<u32>| {},
                    "zone"
                }
            }
        }
    }
    let html = run(app);
    assert!(
        !html.contains("data-edge"),
        "idle zone shows no edge: {html}"
    );
}

// --- Drop-settle: the overlay glides home after a successful drop ---------

/// The settling phase between "drop delivered" and "ghost gone": the payload
/// stays readable for the ghost's content, but the drag is over for every
/// zone and draggable.
#[test]
fn drop_settle_state_machine() {
    fn app() -> Element {
        let mut dnd = use_dnd_provider::<String>();
        dnd.start(
            "book".to_string(),
            Some(ZoneId(1)),
            Point::new(50.0, 60.0),
            Point::new(5.0, 5.0),
            DropEffect::Move,
            DragMode::Pointer,
        );
        dnd.enter(ZoneId(2));

        // A settling take hands the payload to the drop handler but keeps
        // it readable, records the destination, and ends the drag.
        let to = Rect::new(100.0, 100.0, 80.0, 40.0);
        let (p, from) = dnd.take_settling(to).expect("payload present");
        assert_eq!(p, "book");
        assert_eq!(from, Some(ZoneId(1)));
        assert!(!dnd.dragging(), "settling is not dragging");
        assert_eq!(
            dnd.payload().as_deref(),
            Some("book"),
            "ghost keeps content"
        );
        assert_eq!(dnd.settling(), Some(to));
        assert_eq!(dnd.over(), None, "hover cleared at drop");
        // The release position survives, so the overlay holds it while the
        // glide arms.
        assert_eq!(dnd.pointer(), Point::new(50.0, 60.0));
        assert_eq!(dnd.grab(), Point::new(5.0, 5.0));

        // finish_settle resets everything...
        dnd.finish_settle();
        assert_eq!(dnd.payload(), None);
        assert_eq!(dnd.settling(), None);

        // ...and is a guarded no-op otherwise: a late transitionend can't
        // clobber a drag that started after the glide.
        dnd.start(
            "next".to_string(),
            None,
            Point::new(1.0, 1.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        dnd.finish_settle();
        assert!(dnd.dragging());
        assert_eq!(dnd.payload().as_deref(), Some("next"));

        // Starting a new drag mid-settle interrupts the glide.
        dnd.take_settling(to).expect("second drop");
        assert!(dnd.settling().is_some());
        dnd.start(
            "third".to_string(),
            None,
            Point::new(1.0, 1.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        assert_eq!(dnd.settling(), None);
        assert!(dnd.dragging());

        // With no payload in flight there is nothing to settle.
        dnd.cancel();
        assert!(dnd.take_settling(to).is_none());

        rsx! {
            div {}
        }
    }
    run(app);
}

/// Mid-settle markup: the ghost renders at the release point with the
/// transition armed, marked for the reduced-motion override, with the
/// override sheet in the subtree.
#[test]
fn settle_overlay_renders_armed_ghost_with_motion_marker() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                SettleScene {}
            }
        }
    }

    #[component]
    fn SettleScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.start(
                7,
                None,
                Point::new(300.0, 200.0),
                Point::new(8.0, 8.0),
                DropEffect::Move,
                DragMode::Pointer,
            );
            dnd.take_settling(Rect::new(0.0, 0.0, 100.0, 100.0));
        });
        rsx! {
            DragOverlay::<u32> { settle: true, class: "ghost", "g" }
        }
    }

    let html = run(app);
    assert!(html.contains("data-dnd-motion"), "marked for 1.3: {html}");
    assert!(
        html.contains("transition: transform 200ms ease;"),
        "transition armed on the hold frame: {html}"
    );
    assert!(
        html.contains("left: 292px; top: 192px;"),
        "held at the release point (pointer - grab): {html}"
    );
    assert!(
        html.contains("prefers-reduced-motion"),
        "override sheet rendered: {html}"
    );
}

/// Without `settle`, a completed drop unmounts the overlay immediately -
/// the pre-settle behavior stays the default.
#[test]
fn overlay_without_settle_vanishes_on_drop() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                PlainScene {}
            }
        }
    }

    #[component]
    fn PlainScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.start(
                7,
                None,
                Point::new(300.0, 200.0),
                Point::new(8.0, 8.0),
                DropEffect::Move,
                DragMode::Pointer,
            );
            dnd.take();
        });
        rsx! {
            DragOverlay::<u32> { class: "ghost", "g" }
        }
    }

    let html = run(app);
    assert!(
        !html.contains("ghost"),
        "no ghost after a plain take: {html}"
    );
}

/// `match_source: true` dresses the ghost in the grabbed element's measured
/// rect (border-box), and stays content-sized while the measurement is
/// still pending.
#[test]
fn overlay_match_source_sizes_ghost_to_the_measured_rect() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                MatchSourceScene {}
            }
        }
    }

    #[component]
    fn MatchSourceScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.start(
                7,
                None,
                Point::new(300.0, 200.0),
                Point::new(8.0, 8.0),
                DropEffect::Move,
                DragMode::Pointer,
            );
            // What Draggable's post-pickup measurement would deliver.
            dnd.set_source_rect(Some(Rect::new(100.0, 50.0, 240.0, 44.0)));
        });
        rsx! {
            DragOverlay::<u32> { match_source: true, class: "ghost", "g" }
        }
    }

    let html = run(app);
    assert!(
        html.contains("width: 240px; height: 44px; box-sizing: border-box;"),
        "ghost must wear the source rect: {html}"
    );
    assert!(
        html.contains("left: 292px; top: 192px;"),
        "pointer - grab anchoring unchanged: {html}"
    );
}

/// Without a measured rect (measurement pending, or a custom source that
/// never set one), a `match_source` ghost renders nothing - showing it
/// content-sized would pop to the matched size a frame later.
#[test]
fn overlay_match_source_without_rect_renders_nothing() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                UnmeasuredScene {}
            }
        }
    }

    #[component]
    fn UnmeasuredScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.start(
                7,
                None,
                Point::new(300.0, 200.0),
                Point::new(8.0, 8.0),
                DropEffect::Move,
                DragMode::Pointer,
            );
        });
        rsx! {
            DragOverlay::<u32> { match_source: true, class: "ghost", "g" }
        }
    }

    let html = run(app);
    assert!(
        !html.contains("ghost"),
        "no unsized ghost before the measurement lands: {html}"
    );
}

/// While a drop settles, a `SettleSlot` marked active holds its space
/// invisibly (no second copy beside the gliding ghost) and reveals once the
/// settle finishes; `retarget_settle` re-aims the stored rect for the
/// overlay to pick up. Inactive slots and idle providers are untouched.
#[test]
fn settle_slot_hides_while_settling_and_retargets() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                SlotScene {}
            }
        }
    }

    #[component]
    fn SlotScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.start(
                7,
                None,
                Point::new(300.0, 200.0),
                Point::new(8.0, 8.0),
                DropEffect::Move,
                DragMode::Pointer,
            );
            dnd.take_settling(Rect::new(0.0, 0.0, 100.0, 100.0));
            // The landed element announcing its real position.
            dnd.retarget_settle(Rect::new(40.0, 60.0, 80.0, 20.0));
        });
        assert_eq!(
            dnd.settling(),
            Some(Rect::new(40.0, 60.0, 80.0, 20.0)),
            "retarget must replace the settle rect"
        );
        rsx! {
            SettleSlot::<u32> { active: true, class: "landed", "book" }
            SettleSlot::<u32> { active: false, class: "bystander", "other" }
        }
    }

    let html = run(app);
    assert!(
        html.contains(r#"data-settling="true""#),
        "active slot marked: {html}"
    );
    assert_eq!(
        html.matches("visibility: hidden;").count(),
        1,
        "exactly the active slot hides: {html}"
    );
    assert_eq!(
        html.matches("data-settling").count(),
        1,
        "the bystander slot is untouched: {html}"
    );
}

/// Keyboard-drop focus continuity: the drop records the payload as a
/// refocus request; the matching mount claims it exactly once, a foreign
/// payload never does, and a new drag clears any unclaimed request.
#[test]
fn refocus_request_is_claimed_once_by_the_matching_payload() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                RefocusScene {}
            }
        }
    }

    #[component]
    fn RefocusScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.request_refocus(7);
            // A foreign payload doesn't claim (or consume) the request.
            assert!(!dnd.claim_refocus(&9));
            // The landing element does - exactly once.
            assert!(dnd.claim_refocus(&7));
            assert!(!dnd.claim_refocus(&7));

            // An unclaimed request dies with the next drag.
            dnd.request_refocus(7);
            dnd.start(
                8,
                None,
                Point::default(),
                Point::default(),
                DropEffect::Move,
                DragMode::Pointer,
            );
            assert!(!dnd.claim_refocus(&7));
            dnd.cancel();
        });
        rsx! { div {} }
    }

    let _ = run(app);
}

/// `retarget_settle` outside a settle is a no-op, so a stale landing
/// element can never plant a rect into a fresh drag.
#[test]
fn retarget_settle_is_inert_when_not_settling() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                InertScene {}
            }
        }
    }

    #[component]
    fn InertScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.retarget_settle(Rect::new(1.0, 2.0, 3.0, 4.0));
        });
        assert_eq!(dnd.settling(), None);
        rsx! { div {} }
    }

    let _ = run(app);
}

/// A keyboard drag has no meaningful pointer, so the overlay renders
/// nothing (it used to pin the ghost to the viewport corner). Zones
/// highlight and the LiveRegion narrates instead.
#[test]
fn overlay_skips_keyboard_drags() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u32> {
                KeyboardScene {}
            }
        }
    }

    #[component]
    fn KeyboardScene() -> Element {
        let mut dnd = use_dnd::<u32>();
        use_hook(move || {
            dnd.start(
                7,
                None,
                Point::default(),
                Point::default(),
                DropEffect::Move,
                DragMode::Keyboard,
            );
        });
        rsx! {
            DragOverlay::<u32> { class: "ghost", "g" }
        }
    }

    let html = run(app);
    assert!(
        !html.contains("ghost"),
        "no corner-pinned ghost for keyboard drags: {html}"
    );
}

#[test]
fn grid_merges_user_style_after_layout_default() {
    fn app() -> Element {
        rsx! {
            SortableGrid {
                len: 2,
                cols: 2,
                style: "grid-template-columns: 2fr 1fr;",
                render: move |ix: usize| rsx! { "t{ix}" },
                on_sort: move |_| {},
            }
        }
    }
    let html = run(app);
    // One merged style attribute: default first, user override after.
    // (+1 for the hidden reduced-motion stylesheet the grid anchors.)
    assert_eq!(
        html.matches("style=").count(),
        4,
        "wrapper + 2 tiles + hidden stylesheet: {html}"
    );
    assert!(
        html.contains(
            "display: grid; grid-template-columns: repeat(2, 1fr); grid-template-columns: 2fr 1fr;"
        ),
        "user tracks must land after the default: {html}"
    );
}

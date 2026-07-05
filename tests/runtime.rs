//! Runtime tests: exercise the store/signal-backed state machines inside a
//! headless `VirtualDom`, and assert rendered accessibility attributes via
//! SSR. Assertions live inside test components — panics propagate through
//! `rebuild_in_place`, failing the test.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

/// Build a one-shot headless app and return its SSR output.
fn run(app: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

// --- DndContext state machine ------------------------------------------

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

        // (0,0) pointer samples are noise from some webviews — filtered.
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

        let record = |id: u64, label: &str| ZoneRecord::<u32> {
            id: ZoneId(id),
            parent: None,
            label: Some(label.to_string()),
            on_drop: Callback::new(|_| {}),
            accepts: None,
            mounted: Signal::new(None),
            rect: Signal::new(None),
        };

        reg.register(record(1, "one"));
        reg.register(record(2, "two"));
        assert_eq!(reg.get(ZoneId(1)).unwrap().label.as_deref(), Some("one"));

        // re-registering the same id replaces, not duplicates
        reg.register(record(1, "uno"));
        assert_eq!(reg.acceptable(&0).len(), 2);
        assert_eq!(reg.get(ZoneId(1)).unwrap().label.as_deref(), Some("uno"));

        // sync_label updates in place, and is a no-op for unknown ids
        reg.sync_label(ZoneId(2), Some("zwei".into()));
        assert_eq!(reg.get(ZoneId(2)).unwrap().label.as_deref(), Some("zwei"));
        reg.sync_label(ZoneId(99), Some("ghost".into()));
        assert!(reg.get(ZoneId(99)).is_none());

        reg.unregister(ZoneId(1));
        assert!(reg.get(ZoneId(1)).is_none());
        assert_eq!(reg.acceptable(&0).len(), 1);

        rsx! { div {} }
    }
    run(app);
}

#[test]
fn registry_spatial_step_accepts_and_hit_test() {
    fn app() -> Element {
        use_dnd_provider::<u32>();
        let mut reg = use_zone_registry::<u32>();

        let record =
            |id: u64, rect: Option<Rect>, accepts: Option<Callback<u32, bool>>| ZoneRecord::<u32> {
                id: ZoneId(id),
                parent: None,
                label: None,
                on_drop: Callback::new(|_| {}),
                accepts,
                mounted: Signal::new(None),
                rect: Signal::new(rect),
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
        // overlapping zones: the later-registered one wins (topmost)
        reg.register(record(5, Some(Rect::new(0.0, 0.0, 40.0, 40.0)), None));
        assert_eq!(reg.hit_test(Point::new(10.0, 10.0)), Some(ZoneId(5)));
        // outside everything
        assert_eq!(reg.hit_test(Point::new(500.0, 500.0)), None);

        rsx! { div {} }
    }
    run(app);
}

// --- Selection (multiselect) ---------------------------------------------

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

        rsx! { div {} }
    }
    run(app);
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
    assert!(html.contains("draggable"), "draggable attr missing: {html}");
    assert!(html.contains("tabindex=0"), "not focusable: {html}");
    assert!(html.contains(r#"role="button""#), "role missing: {html}");
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

        let record = |id: u64, parent: Option<u64>, y: f64| ZoneRecord::<u32> {
            id: ZoneId(id),
            parent: parent.map(ZoneId),
            label: None,
            on_drop: Callback::new(|_| {}),
            accepts: None,
            mounted: Signal::new(None),
            rect: Signal::new(Some(Rect::new(0.0, y, 100.0, 40.0))),
        };

        // Two root boards; the first contains two columns.
        reg.register(record(1, None, 0.0)); //   board A
        reg.register(record(2, None, 200.0)); // board B
        reg.register(record(10, Some(1), 10.0)); //  A / column 1
        reg.register(record(11, Some(1), 50.0)); //  A / column 2

        // Root siblings cycle among boards only — columns don't leak up.
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

//! Runtime tests: exercise the store/signal-backed state machines inside a
//! headless `VirtualDom`, and assert rendered accessibility attributes via
//! SSR. Assertions live inside test components - panics propagate through
//! `rebuild_in_place`, failing the test.

use dioxus::prelude::*;
use dioxus_dnd::prelude::*;
use std::sync::{Arc, Mutex};

type Shared<T> = Arc<Mutex<T>>;

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
        // acceptable center within max_distance wins.
        assert_eq!(
            reg.hit_test_closest(Point::new(20.0, 95.0), &5, 48.0),
            Some(ZoneId(1))
        );

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
    assert!(html.contains("tabindex=0"), "not focusable: {html}");
    assert!(html.contains(r#"role="button""#), "role missing: {html}");
    assert!(
        !html.contains("draggable=true"),
        "in-app drags should not opt into native HTML drag: {html}"
    );
    assert!(
        html.contains("touch-action: none"),
        "pointer drag style missing: {html}"
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
            MoveEvent {
                item: "pointer-card",
                from: (ZoneId(10), 0),
                to: (ZoneId(90), Some(1)),
            },
            MoveEvent {
                item: "keyboard-card",
                from: (ZoneId(11), 2),
                to: (ZoneId(90), Some(1)),
            },
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
        vec![MoveEvent {
            item: "ok",
            from: (ZoneId(10), 0),
            to: (ZoneId(90), Some(0)),
        }]
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
        vec![TreeDropEvent {
            payload: "into",
            target: NodeId(12),
            intent: DropIntent::Into,
        }]
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
    {
        let mut runs = props.runs.lock().unwrap();
        if runs.contains(&phase) {
            return rsx! { div {} };
        }
        runs.push(phase);
    }

    let registry = use_zone_registry::<&'static str>();
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
            });
        }
        other => panic!("unexpected phase {other}"),
    }

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
    assert_eq!(
        *drops.lock().unwrap(),
        vec![(
            0,
            TreeDropEvent {
                payload: "first",
                target: NodeId(7),
                intent: DropIntent::After,
            },
        )]
    );

    *phase.lock().unwrap() = 1;
    dom.mark_dirty(ScopeId::APP);
    dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);

    assert_eq!(
        *drops.lock().unwrap(),
        vec![
            (
                0,
                TreeDropEvent {
                    payload: "first",
                    target: NodeId(7),
                    intent: DropIntent::After,
                },
            ),
            (
                1,
                TreeDropEvent {
                    payload: "allowed",
                    target: NodeId(8),
                    intent: DropIntent::Into,
                },
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
    assert!(
        html.contains("touch-action: none; background: red;"),
        "touch-action must survive a user style: {html}"
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

// --- Bridge zones: one box registered in two type-worlds ------------------
//
// The documented cross-type pattern (README "Mixing payload types", the
// gallery's Standup page): zone ids are process-global while registries are
// per-type, so one element registers the *same* ZoneId in two registries,
// sharing its mounted/rect signals. These tests pin the crate invariants
// that pattern depends on.

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
    let mounted = use_signal(|| None::<std::rc::Rc<dioxus::html::MountedData>>);
    let rect = use_signal(|| None::<Rect>);
    let ticket_drops = props.ticket_drops.clone();
    let person_drops = props.person_drops.clone();
    use_hook(move || {
        reg_a.register(ZoneRecord {
            id,
            parent: None,
            label: Some("agenda".into()),
            on_drop: Callback::new(move |o: DropOutcome<&'static str>| {
                ticket_drops.lock().unwrap().push(o.payload)
            }),
            accepts: None,
            mounted,
            rect,
        });
        reg_b.register(ZoneRecord {
            id,
            parent: None,
            label: Some("agenda".into()),
            on_drop: Callback::new(move |o: DropOutcome<u32>| {
                person_drops.lock().unwrap().push(o.payload)
            }),
            accepts: None,
            mounted,
            rect,
        });
    });

    rsx! {
        BridgeProbe {}
    }
}

#[component]
fn BridgeProbe() -> Element {
    let mut reg_a = use_zone_registry::<&'static str>();
    let reg_b = use_zone_registry::<u32>();
    let id = ZoneId(500);

    // The same id resolves in both worlds.
    assert!(reg_a.contains(id) && reg_b.contains(id));

    // The rect signal is one shared handle: measuring through one world's
    // record is immediately visible through the other's, and both worlds'
    // hit-testing find the same box.
    let mut rect = reg_a.get(id).expect("registered in world A").rect;
    rect.set(Some(Rect::new(0.0, 0.0, 100.0, 50.0)));
    assert_eq!(
        *reg_b.get(id).expect("registered in world B").rect.peek(),
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
    assert_eq!(
        html.matches("style=").count(),
        3,
        "wrapper + 2 tiles: {html}"
    );
    assert!(
        html.contains(
            "display: grid; grid-template-columns: repeat(2, 1fr); grid-template-columns: 2fr 1fr;"
        ),
        "user tracks must land after the default: {html}"
    );
}

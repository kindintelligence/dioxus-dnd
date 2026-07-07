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
fn draggable_native_can_be_disabled_without_losing_keyboard_access() {
    fn app() -> Element {
        rsx! {
            DndProvider::<u8> {
                Draggable::<u8> { payload: 1, native: false, "keyboard only" }
            }
        }
    }
    let html = run(app);
    assert!(
        html.contains("draggable=false"),
        "native drag should be explicitly disabled: {html}"
    );
    assert!(html.contains("tabindex=0"), "keyboard access lost: {html}");
    assert!(html.contains(r#"role="button""#), "role missing: {html}");
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
fn sortable_native_drag_is_opt_in() {
    fn app() -> Element {
        rsx! {
            div {
                SortableList {
                    len: 1,
                    on_sort: move |_| {},
                    render: move |_| rsx! { "pointer" },
                }
                SortableList {
                    len: 1,
                    input: DragInputMode::Native,
                    on_sort: move |_| {},
                    render: move |_| rsx! { "native" },
                }
            }
        }
    }
    let html = run(app);
    assert_eq!(
        html.matches("draggable=false").count(),
        1,
        "default list should opt out of native drag: {html}"
    );
    assert_eq!(
        html.matches("draggable=true").count(),
        1,
        "native opt-in list should enable native drag: {html}"
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

#[test]
fn canvas_keyboard_policy_does_not_affect_native_outcomes() {
    assert_eq!(
        run_keyboard_policy(CanvasKeyboardPlacement::Origin, DragMode::Native),
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
fn pointer_draggable_merges_user_style_with_touch_action() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            PointerDraggable::<String> {
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
fn pointer_draggable_input_mode_controls_native_attr() {
    fn app() -> Element {
        use_dnd_provider::<String>();
        rsx! {
            PointerDraggable::<String> {
                payload: "default".to_string(),
                "default"
            }
            PointerDraggable::<String> {
                payload: "pointer".to_string(),
                input: DragInputMode::Pointer,
                "pointer"
            }
            PointerDraggable::<String> {
                payload: "native".to_string(),
                input: DragInputMode::Native,
                "native"
            }
            PointerDraggable::<String> {
                payload: "hybrid".to_string(),
                input: DragInputMode::Hybrid,
                "hybrid"
            }
        }
    }
    let html = run(app);
    assert_eq!(
        html.matches("draggable=false").count(),
        2,
        "default and pointer mode should disable native drag: {html}"
    );
    assert_eq!(
        html.matches("draggable=true").count(),
        2,
        "native and hybrid mode should enable native drag: {html}"
    );
}

#[test]
fn pointer_wrappers_default_to_pointer_drag() {
    fn app() -> Element {
        use_dnd_provider::<BoardPayload<String>>();
        rsx! {
            BoardItem::<String> {
                item: "board-default".to_string(),
                column: ZoneId(1),
                index: 0,
                "board-default"
            }
            BoardItem::<String> {
                item: "board-native".to_string(),
                column: ZoneId(1),
                index: 1,
                input: DragInputMode::Native,
                "board-native"
            }
        }
    }
    let html = run(app);
    assert_eq!(
        html.matches("draggable=false").count(),
        1,
        "BoardItem default should disable native drag: {html}"
    );
    assert_eq!(
        html.matches("draggable=true").count(),
        1,
        "BoardItem native opt-in should enable native drag: {html}"
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
            SelectableDraggable::<u32> {
                item: 2,
                selection,
                input: DragInputMode::Hybrid,
                "select-hybrid"
            }
        }
    }
    let html = run(selectable_app);
    assert_eq!(
        html.matches("draggable=false").count(),
        1,
        "SelectableDraggable default should disable native drag: {html}"
    );
    assert_eq!(
        html.matches("draggable=true").count(),
        1,
        "SelectableDraggable hybrid opt-in should enable native drag: {html}"
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

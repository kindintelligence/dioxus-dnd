//! Cross-window drag tests (TODO 3.5): two headless `VirtualDom`s joined
//! to one `DndWorld`, standing in for two desktop windows. Geometry is fed
//! by hand (the headless stand-in for the desktop glue), zone rects are
//! placed per window in that window's client px, and drags run through the
//! world-aware `DragSim` - the same resolution and delivery paths as the
//! live pointer gesture.
//!
//! Layout used throughout (global desktop physical px):
//!   window A: origin (0, 0),    client 800x600, scale 1.0
//!   window B: origin (1000, 0), client 800x600 physical, scale 2.0
//!            (so B's client space is 400x300 CSS px)
//!   zone A1 in A at client (0, 100, 200, 80)
//!   zone B1 in B at client (50, 50, 100, 100)
//!
//! A-client coords double as global coords (origin 0, scale 1), which keeps
//! the arithmetic in tests readable.

use std::cell::RefCell;

use dioxus::dioxus_core::NoOpMutations;
use dioxus::prelude::*;
use dioxus_dnd::core::ZoneLocation;
use dioxus_dnd::prelude::*;
use dioxus_dnd::test::{drag_sim, rerender, DragSim, DragSimProbe};

const ZONE_A1: ZoneId = ZoneId(1);
const ZONE_B1: ZoneId = ZoneId(2);
const ZONE_B2: ZoneId = ZoneId(3);

thread_local! {
    static WORLD: RefCell<Option<DndWorld<String>>> = const { RefCell::new(None) };
    /// (window tag, payload, to, client point) per delivered drop.
    static DROPS: RefCell<Vec<(&'static str, String, ZoneId, Point)>> =
        const { RefCell::new(Vec::new()) };
    static EFFECTS: RefCell<Vec<DropEffect>> = const { RefCell::new(Vec::new()) };
}

fn log_drop(tag: &'static str, o: &DropOutcome<String>) {
    DROPS.with_borrow_mut(|d| d.push((tag, o.payload.clone(), o.to, o.client)));
    EFFECTS.with_borrow_mut(|effects| effects.push(o.effect));
}

fn window_a() -> Element {
    let world = use_dnd_world::<String>();
    WORLD.with_borrow_mut(|w| *w = Some(world));
    rsx! {
        DndProvider::<String> {
            DragSimProbe::<String> {}
            DropZone::<String> {
                id: ZONE_A1,
                label: "shelf-a",
                on_drop: move |o: DropOutcome<String>| log_drop("A", &o),
                "zone-a"
            }
            DragOverlay::<String> { "GHOST" }
        }
    }
}

fn window_a_settling() -> Element {
    let world = use_dnd_world::<String>();
    WORLD.with_borrow_mut(|slot| *slot = Some(world));
    rsx! {
        DndProvider::<String> {
            DragSimProbe::<String> {}
            DropZone::<String> {
                id: ZONE_A1,
                on_drop: move |o: DropOutcome<String>| log_drop("A", &o),
                "zone-a"
            }
            DragOverlay::<String> { settle: true, "GHOST" }
        }
    }
}

fn window_b() -> Element {
    rsx! {
        DndProvider::<String> {
            DropZone::<String> {
                id: ZONE_B1,
                label: "shelf-b",
                on_drop: move |o: DropOutcome<String>| log_drop("B", &o),
                "zone-b"
            }
            DragOverlay::<String> { "GHOST" }
        }
    }
}

fn window_b_with_overlapping_rejector() -> Element {
    rsx! {
        DndProvider::<String> {
            DropZone::<String> {
                id: ZONE_B1,
                label: "accepting-b",
                on_drop: move |o: DropOutcome<String>| log_drop("B", &o),
                "accepting-zone-b"
            }
            DropZone::<String> {
                id: ZONE_B2,
                label: "rejecting-b",
                accepts: move |_: String| false,
                on_drop: move |_: DropOutcome<String>| -> () {
                    panic!("rejecting zone received drop");
                },
                "rejecting-zone-b"
            }
        }
    }
}

/// Window B with its own sim probe, for tests that keep driving after
/// window A is gone (mounted second, so `drag_sim` returns ITS handle).
fn window_b_probed() -> Element {
    rsx! {
        DndProvider::<String> {
            DragSimProbe::<String> {}
            DropZone::<String> {
                id: ZONE_B1,
                label: "shelf-b",
                on_drop: move |o: DropOutcome<String>| log_drop("B", &o),
                "zone-b"
            }
            DragOverlay::<String> { "GHOST" }
        }
    }
}

/// Window B with a settle-enabled overlay, for the cross-window settle test.
fn window_b_settling() -> Element {
    rsx! {
        DndProvider::<String> {
            DropZone::<String> {
                id: ZONE_B1,
                label: "shelf-b",
                on_drop: move |o: DropOutcome<String>| log_drop("B", &o),
                "zone-b"
            }
            DragOverlay::<String> { settle: true, "GHOST" }
        }
    }
}

fn window_b_edge() -> Element {
    rsx! {
        DndProvider::<String> {
            DropZone::<String> {
                id: ZONE_B1,
                edge: EdgeSet::Vertical,
                on_drop: move |o: DropOutcome<String>| log_drop("B", &o),
                "zone-b"
            }
            DragOverlay::<String> { "GHOST" }
        }
    }
}

fn window_b_tree() -> Element {
    rsx! {
        DndProvider::<String> {
            TreeNodeTarget::<String> {
                node: NodeId(1),
                row_height: 100.0,
                on_drop: move |_: TreeDropEvent<String>| {},
                "tree-b"
            }
            DragOverlay::<String> { "GHOST" }
        }
    }
}

fn window_b_duplicate_id() -> Element {
    rsx! {
        DndProvider::<String> {
            DropZone::<String> {
                id: ZONE_A1,
                on_drop: move |o: DropOutcome<String>| log_drop("B", &o),
                "zone-b-duplicate-id"
            }
        }
    }
}

fn window_c_settling() -> Element {
    rsx! {
        DndProvider::<String> {
            DragOverlay::<String> { settle: true, "GHOST" }
        }
    }
}

fn window_b_observes_committed_source() -> Element {
    let world = use_context::<DndWorld<String>>();
    rsx! {
        DndProvider::<String> {
            DropZone::<String> {
                id: ZONE_B1,
                on_drop: move |o: DropOutcome<String>| {
                    assert!(world.drag_session().is_some(), "source finalized before receiver");
                    assert!(
                        world.source_location().is_some(),
                        "source window metadata cleared before receiver"
                    );
                    // Simulate source cleanup during receiver user code. A
                    // committed success must remain true, not become a
                    // cancellation.
                    world.cancel_drag();
                    log_drop("B", &o);
                },
                "zone-b"
            }
        }
    }
}

fn window_b_starts_replacement_drag() -> Element {
    rsx! {
        DndProvider::<String> {
            ReplacementDragZone {}
        }
    }
}

#[component]
fn ReplacementDragZone() -> Element {
    let mut dnd = use_dnd::<String>();
    let joined = use_joined_window::<String>().expect("joined receiver");
    rsx! {
        DropZone::<String> {
            id: ZONE_B1,
            on_drop: move |_: DropOutcome<String>| {
                dnd.start(
                    "replacement".to_string(),
                    None,
                    Point::new(10.0, 10.0),
                    Point::default(),
                    DropEffect::Move,
                    DragMode::Pointer,
                );
                joined.world.begin_from(joined.key);
            },
            "zone-b"
        }
    }
}

fn geometry_status() -> Element {
    let geometry = use_context::<WindowGeometry>();
    rsx! { span { if geometry.live() { "live" } else { "inert" } } }
}

struct TwoWindows {
    dom_a: VirtualDom,
    dom_b: VirtualDom,
    world: DndWorld<String>,
    sim: DragSim<String>,
    key_a: WindowKey,
    key_b: WindowKey,
}

/// Build both windows, feed the standard geometry, place both zones.
fn two_windows(b_app: fn() -> Element) -> TwoWindows {
    DROPS.with_borrow_mut(|d| d.clear());
    EFFECTS.with_borrow_mut(|effects| effects.clear());
    let mut dom_a = VirtualDom::new(window_a);
    dom_a.rebuild_in_place();
    let world = WORLD.with_borrow(|w| *w).expect("window A created a world");

    let mut dom_b = VirtualDom::new(b_app).with_root_context(world);
    dom_b.rebuild_in_place();

    let windows = dom_a.in_runtime(|| world.windows());
    assert_eq!(windows.len(), 2, "both providers joined");
    let (rec_a, rec_b) = (windows[0], windows[1]);
    dom_a.in_runtime(|| {
        rec_a
            .geometry
            .set(Point::new(0.0, 0.0), (800.0, 600.0), 1.0);
    });
    dom_b.in_runtime(|| {
        rec_b
            .geometry
            .set(Point::new(1000.0, 0.0), (800.0, 600.0), 2.0);
    });

    let sim = drag_sim::<String>();
    sim.place(&dom_a, ZONE_A1, Rect::new(0.0, 100.0, 200.0, 80.0));
    sim.place_in(
        &dom_a,
        rec_b.key,
        ZONE_B1,
        Rect::new(50.0, 50.0, 100.0, 100.0),
    );

    TwoWindows {
        dom_a,
        dom_b,
        world,
        sim,
        key_a: rec_a.key,
        key_b: rec_b.key,
    }
}

fn rerender_both(tw: &mut TwoWindows) {
    rerender(&mut tw.dom_a);
    rerender(&mut tw.dom_b);
}

#[test]
fn drag_crosses_from_a_into_b_and_drops_in_b_local_coords() {
    let mut tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());
    assert_eq!(tw.sim.window_key(), Some(tw.key_a));

    // Over A's own zone first: the classic in-window arc still works.
    sim.move_to(&tw.dom_a, Point::new(100.0, 140.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_A1));
    rerender_both(&mut tw);
    assert!(dioxus_ssr::render(&tw.dom_a).contains("data-over"));

    // Global (1200, 200) = B-local (100, 100): inside zone B1. The zone in
    // the OTHER window lights up through the shared context.
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));
    rerender_both(&mut tw);
    assert!(!dioxus_ssr::render(&tw.dom_a).contains("data-over"));
    assert!(dioxus_ssr::render(&tw.dom_b).contains("data-over"));

    // Release delivers in B, with client coords in B's OWN space.
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_B1));
    assert!(!sim.dragging(&tw.dom_a));
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);
    DROPS.with_borrow(|d| {
        assert_eq!(d.len(), 1);
        let (tag, payload, to, client) = &d[0];
        assert_eq!(*tag, "B");
        assert_eq!(payload, "card");
        assert_eq!(*to, ZONE_B1);
        assert_eq!(*client, Point::new(100.0, 100.0));
    });
}

#[test]
fn foreign_release_snaps_within_the_target_windows_css_px() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());
    // Global (1320, 200) = B-local (160, 100): 10 CSS px right of zone B1
    // (which ends at x=150) - inside the 48px snap, in B's OWN css px
    // (that's 20 physical px, so a global-space snap would compute 20).
    sim.move_to(&tw.dom_a, Point::new(1320.0, 200.0));
    assert_eq!(sim.over(&tw.dom_a), None, "beside the zone, not over it");
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_B1));
    DROPS.with_borrow(|d| {
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, "B");
    });
}

#[test]
fn release_outside_every_window_cancels() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());
    sim.move_to(&tw.dom_a, Point::new(5000.0, 5000.0));
    assert_eq!(sim.release(&tw.dom_a), None);
    assert!(!sim.dragging(&tw.dom_a));
    DROPS.with_borrow(|d| assert!(d.is_empty()));
}

#[test]
fn zoneless_spot_in_a_foreign_window_clears_the_hover() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));
    // Still inside window B, but outside its only zone.
    sim.move_to(&tw.dom_a, Point::new(1700.0, 500.0));
    assert_eq!(sim.over(&tw.dom_a), None);
}

#[test]
fn overlapping_windows_resolve_to_the_most_recently_focused() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;

    // Overlap B onto A: both now contain global (400, 200). A zone in each
    // window sits under that point (B-local (150, 50) with B at scale 2.0
    // moved to (100, 100)... keep it simple: same scale for this test).
    let rec_b = tw.dom_a.in_runtime(|| tw.world.record(tw.key_b)).unwrap();
    tw.dom_b.in_runtime(|| {
        rec_b
            .geometry
            .set(Point::new(100.0, 100.0), (800.0, 600.0), 1.0);
    });
    // Re-place B's zone so it contains B-local (300, 100) = global (400, 200).
    sim.place_in(
        &tw.dom_a,
        tw.key_b,
        ZONE_B1,
        Rect::new(250.0, 50.0, 100.0, 100.0),
    );
    // A's zone also contains global/A-local (400, 200)... it doesn't (A1 is
    // at (0,100,200,80)) - re-place it so the point is genuinely contested.
    sim.place(&tw.dom_a, ZONE_A1, Rect::new(350.0, 150.0, 100.0, 100.0));

    sim.pick_up(&tw.dom_a, "card".to_string());

    // B focused last: B's zone wins the contested point.
    tw.dom_b.in_runtime(|| rec_b.geometry.mark_focused());
    sim.move_to(&tw.dom_a, Point::new(400.0, 200.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));

    // A focused last: same point now belongs to A's zone.
    let rec_a = tw.dom_a.in_runtime(|| tw.world.record(tw.key_a)).unwrap();
    tw.dom_a.in_runtime(|| rec_a.geometry.mark_focused());
    sim.move_to(&tw.dom_a, Point::new(401.0, 200.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_A1));
}

#[test]
fn hovered_window_closing_clears_hover_and_the_drag_survives() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;
    let closed_geometry = tw.world.record(tw.key_b).unwrap().geometry;

    sim.pick_up(&tw.dom_a, "card".to_string());
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));

    // Window B closes mid-drag: dropping its VirtualDom unmounts the
    // provider, which leaves the world.
    drop(tw.dom_b);
    assert_eq!(tw.dom_a.in_runtime(|| tw.world.windows().len()), 1);
    assert_eq!(
        sim.over(&tw.dom_a),
        None,
        "hover into the dead window cleared"
    );
    assert!(sim.dragging(&tw.dom_a), "the drag itself survives");
    assert!(!closed_geometry.live(), "dead geometry degrades to inert");
    assert!(
        !closed_geometry.eligible(),
        "dead eligibility degrades to false"
    );
    closed_geometry.set_eligible(false);

    // And the drag can still land at home.
    sim.move_to(&tw.dom_a, Point::new(100.0, 140.0));
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_A1));
    DROPS.with_borrow(|d| assert_eq!(d[0].0, "A"));
}

#[test]
fn origin_window_closing_aborts_its_drag() {
    let tw = two_windows(window_b);

    // A drag that ORIGINATES in window B, driven through the world's
    // custom-source API (begin_from is what Draggable calls at pickup).
    let world = tw.world;
    let key_b = tw.key_b;
    tw.dom_b.in_runtime(|| {
        let mut ctx = world.context();
        ctx.start(
            "card".to_string(),
            None,
            Point::new(10.0, 10.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        world.begin_from(key_b);
    });
    assert!(tw.dom_a.in_runtime(|| world.context().dragging()));

    // Its window closes mid-drag: the coordinate anchor is gone, abort.
    drop(tw.dom_b);
    assert!(!tw.dom_a.in_runtime(|| world.context().dragging()));
    assert!(tw.dom_a.in_runtime(|| world.context().payload().is_none()));
}

#[test]
fn exactly_one_window_presents_the_ghost() {
    let mut tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());

    // Pointer inside A: A presents, B doesn't.
    sim.move_to(&tw.dom_a, Point::new(100.0, 140.0));
    rerender_both(&mut tw);
    assert!(dioxus_ssr::render(&tw.dom_a).contains("GHOST"));
    assert!(!dioxus_ssr::render(&tw.dom_b).contains("GHOST"));

    // Pointer inside B: the ghost hands off, anchored in B's client px
    // (global (1200, 200) -> B-local (100, 100)).
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    rerender_both(&mut tw);
    let b_html = dioxus_ssr::render(&tw.dom_b);
    assert!(!dioxus_ssr::render(&tw.dom_a).contains("GHOST"));
    assert!(b_html.contains("GHOST"));
    assert!(
        b_html.contains("left: 100px; top: 100px;"),
        "ghost anchored in B's own client px, got: {b_html}"
    );

    // Pointer outside every window: the origin window keeps the ghost.
    sim.move_to(&tw.dom_a, Point::new(5000.0, 5000.0));
    rerender_both(&mut tw);
    assert!(dioxus_ssr::render(&tw.dom_a).contains("GHOST"));
    assert!(!dioxus_ssr::render(&tw.dom_b).contains("GHOST"));
}

#[test]
fn cross_window_settle_presents_in_the_receiving_window() {
    let mut tw = two_windows(window_b_settling);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_B1));

    // B's overlay is settle-enabled, so the drop entered the settling
    // phase: no longer dragging, payload still readable for the ghost.
    let ctx = tw.world.context();
    assert!(!tw.dom_a.in_runtime(|| ctx.dragging()));
    assert!(tw.dom_a.in_runtime(|| ctx.settling().is_some()));
    assert_eq!(tw.world.settling_in(), Some(tw.key_b));
    assert!(tw.dom_a.in_runtime(|| ctx.payload().is_some()));
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);

    // The RECEIVING window presents the glide.
    rerender_both(&mut tw);
    assert!(!dioxus_ssr::render(&tw.dom_a).contains("GHOST"));
    assert!(dioxus_ssr::render(&tw.dom_b).contains("GHOST"));

    // The glide completing resets everything (headless stand-in for the
    // overlay's transitionend).
    tw.dom_b.in_runtime(|| {
        assert!(!tw.world.finish_settle_from(tw.key_a));
        assert!(tw.world.finish_settle_from(tw.key_b));
    });
    tw.dom_b.render_immediate(&mut NoOpMutations);
    assert!(tw.dom_a.in_runtime(|| ctx.payload().is_none()));
    assert_eq!(tw.world.settling_in(), None);
}

#[test]
fn only_the_elected_window_can_finish_a_world_settle() {
    DROPS.with_borrow_mut(|drops| drops.clear());
    let mut dom_a = VirtualDom::new(window_a_settling);
    dom_a.rebuild_in_place();
    let world = WORLD.with_borrow(|slot| *slot).unwrap();
    let mut dom_b = VirtualDom::new(window_b_settling).with_root_context(world);
    dom_b.rebuild_in_place();
    let mut dom_c = VirtualDom::new(window_c_settling).with_root_context(world);
    dom_c.rebuild_in_place();
    let windows = world.windows();
    assert_eq!(windows.len(), 3);
    let (rec_a, rec_b, rec_c) = (windows[0], windows[1], windows[2]);
    dom_a.in_runtime(|| {
        rec_a
            .geometry
            .set(Point::new(0.0, 0.0), (800.0, 600.0), 1.0)
    });
    dom_b.in_runtime(|| {
        rec_b
            .geometry
            .set(Point::new(1000.0, 0.0), (800.0, 600.0), 2.0)
    });
    dom_c.in_runtime(|| {
        rec_c
            .geometry
            .set(Point::new(2000.0, 0.0), (400.0, 400.0), 1.0)
    });
    let mut sim = drag_sim::<String>();
    sim.place(&dom_a, ZONE_A1, Rect::new(0.0, 100.0, 200.0, 80.0));
    sim.place_in(
        &dom_a,
        rec_b.key,
        ZONE_B1,
        Rect::new(50.0, 50.0, 100.0, 100.0),
    );
    sim.pick_up(&dom_a, "card".to_string());
    sim.move_to(&dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.release(&dom_a), Some(ZONE_B1));
    assert_eq!(world.settling_in(), Some(rec_b.key));
    assert!(!world.finish_settle_from(rec_a.key));
    assert!(!world.finish_settle_from(rec_c.key));

    // A non-presenter may render and close without resetting B's glide.
    rerender(&mut dom_a);
    rerender(&mut dom_c);
    assert!(world.context().settling().is_some());
    drop(dom_c);
    assert!(world.context().settling().is_some());
    assert_eq!(world.settling_in(), Some(rec_b.key));

    // Closing the elected presenter leaves no transition listener, so its
    // owner cleanup finishes the settle.
    drop(dom_b);
    assert!(world.context().settling().is_none());
    assert!(world.context().payload().is_none());
    assert_eq!(world.settling_in(), None);
}

#[test]
fn receiver_settle_survives_origin_window_closing() {
    let tw = two_windows(window_b_settling);
    let mut sim = tw.sim;
    sim.pick_up(&tw.dom_a, "card".to_string());
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_B1));
    assert_eq!(tw.world.settling_in(), Some(tw.key_b));

    drop(tw.dom_a);
    assert!(tw.world.context().settling().is_some());
    let mut dom_b = tw.dom_b;
    rerender(&mut dom_b);
    let html = dioxus_ssr::render(&dom_b);
    assert!(html.contains("GHOST"));
    assert!(
        html.contains("left: 100px; top: 100px"),
        "receiver lost the origin-independent settle anchor: {html}"
    );
    assert!(tw.world.finish_settle_from(tw.key_b));
    assert!(tw.world.context().payload().is_none());
    assert_eq!(tw.world.origin_window(), None);
}

#[test]
fn custom_world_settle_claim_and_finish_keep_metadata_coherent() {
    let tw = two_windows(window_b);
    let mut ctx = tw.world.context();
    tw.dom_a.in_runtime(|| {
        ctx.start(
            "custom".to_string(),
            Some(ZONE_A1),
            Point::new(100.0, 140.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        tw.world.begin_from(tw.key_a);
        tw.world.claim_settle(tw.key_b);
        assert!(ctx
            .take_settling(Rect::new(50.0, 50.0, 100.0, 100.0))
            .is_some());
    });
    assert_eq!(tw.world.settling_in(), Some(tw.key_b));
    assert!(!tw.world.finish_settle_from(tw.key_a));
    assert!(tw.world.finish_settle_from(tw.key_b));
    assert_eq!(tw.world.settling_in(), None);
    assert_eq!(tw.world.origin_window(), None);
    assert!(ctx.payload().is_none());
}

#[test]
fn claimless_legacy_settle_cannot_outlive_its_origin_window() {
    let tw = two_windows(window_b);
    let mut ctx = tw.world.context();
    tw.dom_a.in_runtime(|| {
        ctx.start(
            "legacy".to_string(),
            Some(ZONE_A1),
            Point::new(100.0, 140.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Pointer,
        );
        tw.world.begin_from(tw.key_a);
        assert!(ctx
            .take_settling(Rect::new(50.0, 50.0, 100.0, 100.0))
            .is_some());
    });
    assert_eq!(tw.world.settling_in(), None, "legacy settle has no claim");

    drop(tw.dom_a);
    assert!(ctx.settling().is_none());
    assert!(ctx.payload().is_none());
    assert_eq!(tw.world.origin_window(), None);
}

#[test]
fn world_creator_closing_first_leaves_survivors_fully_functional() {
    // Bespoke setup: B carries the probe here, so the shared helper (whose
    // sim must belong to A) doesn't fit.
    DROPS.with_borrow_mut(|d| d.clear());
    let mut dom_a = VirtualDom::new(window_a);
    dom_a.rebuild_in_place();
    let world = WORLD.with_borrow(|w| *w).expect("window A created a world");
    let mut dom_b = VirtualDom::new(window_b_probed).with_root_context(world);
    dom_b.rebuild_in_place();
    let rec_b = dom_b.in_runtime(|| world.windows())[1];
    dom_b.in_runtime(|| {
        rec_b
            .geometry
            .set(Point::new(1000.0, 0.0), (800.0, 600.0), 2.0);
    });
    let sim = drag_sim::<String>(); // B's probe mounted last
    sim.place(&dom_b, ZONE_B1, Rect::new(50.0, 50.0, 100.0, 100.0));

    // The window that CREATED the world closes first. The world's state is
    // process-lived, so nothing the survivor renders from dies with it.
    drop(dom_a);
    assert_eq!(dom_b.in_runtime(|| world.windows().len()), 1);

    // B still renders, and a full drag arc inside B still works.
    rerender(&mut dom_b);
    let mut sim = sim;
    sim.pick_up(&dom_b, "card".to_string());
    sim.move_to(&dom_b, Point::new(100.0, 100.0));
    assert_eq!(sim.over(&dom_b), Some(ZONE_B1));
    rerender(&mut dom_b);
    let html = dioxus_ssr::render(&dom_b);
    assert!(html.contains("data-over"));
    assert!(html.contains("GHOST"));
    assert_eq!(sim.release(&dom_b), Some(ZONE_B1));
    DROPS.with_borrow(|d| assert_eq!(d[0].0, "B"));
}

#[test]
fn host_side_tracking_drives_the_drag_where_webviews_are_blind() {
    let mut tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());

    // The glue's poller feeds global cursor positions: over window B's
    // zone (global (1200,200) = B-local (100,100)) the shared state
    // updates exactly as webview moves would have.
    tw.dom_a
        .in_runtime(|| tw.world.track_global(Point::new(1200.0, 200.0)));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));
    rerender_both(&mut tw);
    assert!(dioxus_ssr::render(&tw.dom_b).contains("data-over"));
    // The pointer is stored in ORIGIN-window client px (the anchor space).
    assert_eq!(
        tw.dom_a.in_runtime(|| tw.world.context().pointer()),
        Point::new(1200.0, 200.0),
    );

    // Over dead space: hover clears, drag continues.
    tw.dom_a
        .in_runtime(|| tw.world.track_global(Point::new(5000.0, 5000.0)));
    assert_eq!(sim.over(&tw.dom_a), None);
    assert!(sim.dragging(&tw.dom_a));

    // A non-origin window's first event mid-drag means the button is up:
    // the glue completes the drop at that global position.
    let dropped = tw
        .dom_b
        .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0)));
    assert_eq!(dropped, Some(ZONE_B1));
    assert!(!sim.dragging(&tw.dom_a));
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);
    DROPS.with_borrow(|d| {
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, "B");
        assert_eq!(d[0].3, Point::new(100.0, 100.0), "B-local drop coords");
    });

    // Idempotence in the real ordering: a local webview pointerup arriving
    // after the host already delivered is a harmless no-op.
    assert_eq!(sim.release(&tw.dom_a), None);
    DROPS.with_borrow(|d| assert_eq!(d.len(), 1));
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);

    // A repeated host echo is inert too.
    let host_echo = tw
        .dom_a
        .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0)));
    assert_eq!(host_echo, None);
    DROPS.with_borrow(|d| assert_eq!(d.len(), 1));
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);

    // Host completion retired the source generation, so the same source can
    // immediately begin another gesture.
    sim.pick_up(&tw.dom_a, "card-again".to_string());
    assert!(sim.dragging(&tw.dom_a));
    tw.dom_a.in_runtime(|| tw.world.cancel_drag());
    assert_eq!(sim.completions(&tw.dom_a), vec![true, false]);
}

#[test]
fn kill_switch_makes_host_drive_inert_and_reversible() {
    let mut tw = two_windows(window_b);
    let mut sim = tw.sim;

    assert!(
        tw.dom_a.in_runtime(|| tw.world.bridging_enabled()),
        "worlds default to bridging on"
    );
    tw.dom_a.in_runtime(|| tw.world.set_bridging(false));

    sim.pick_up(&tw.dom_a, "card".to_string());

    // Host drive is inert at the world boundary, not merely unpolled: even
    // a custom host calling `track_global` directly moves nothing.
    tw.dom_a
        .in_runtime(|| tw.world.track_global(Point::new(1200.0, 200.0)));
    assert_eq!(sim.over(&tw.dom_a), None);
    rerender_both(&mut tw);
    assert!(!dioxus_ssr::render(&tw.dom_b).contains("data-over"));

    // A host-reported release cannot deliver either; the drag survives for
    // the origin webview's own event paths.
    let dropped = tw
        .dom_b
        .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0)));
    assert_eq!(dropped, None);
    assert!(sim.dragging(&tw.dom_a));
    DROPS.with_borrow(|d| assert!(d.is_empty()));

    // Per-window drags stay fully alive - the modeled Wayland degradation,
    // not a dead app: the origin's local gesture hovers and drops normally.
    sim.move_to(&tw.dom_a, Point::new(100.0, 140.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_A1));
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_A1));
    DROPS.with_borrow(|d| assert_eq!(d.len(), 1));

    // Re-enabling restores cross-window drive for the next gesture.
    tw.dom_a.in_runtime(|| tw.world.set_bridging(true));
    sim.pick_up(&tw.dom_a, "card-2".to_string());
    tw.dom_a
        .in_runtime(|| tw.world.track_global(Point::new(1200.0, 200.0)));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));
    let dropped = tw
        .dom_b
        .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0)));
    assert_eq!(dropped, Some(ZONE_B1));
    DROPS.with_borrow(|d| assert_eq!(d.len(), 2));
}

#[test]
fn local_completion_is_idempotent_with_a_late_host_echo() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_B1));
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);
    DROPS.with_borrow(|drops| assert_eq!(drops.len(), 1));

    let host_echo = tw
        .dom_a
        .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0)));
    assert_eq!(host_echo, None);
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);
    DROPS.with_borrow(|drops| assert_eq!(drops.len(), 1));
}

#[test]
fn foreign_and_host_releases_fall_through_overlapping_rejector() {
    let tw = two_windows(window_b_with_overlapping_rejector);
    let mut sim = tw.sim;
    // The later registry record occupies the exact same rect but rejects every
    // payload. Hover may name it geometrically; release must select B1.
    sim.place_in(
        &tw.dom_a,
        tw.key_b,
        ZONE_B2,
        Rect::new(50.0, 50.0, 100.0, 100.0),
    );

    sim.pick_up(&tw.dom_a, "foreign".to_string());
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.release(&tw.dom_a), Some(ZONE_B1));

    sim.pick_up(&tw.dom_a, "host".to_string());
    assert_eq!(
        tw.dom_b
            .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0))),
        Some(ZONE_B1)
    );
    assert_eq!(sim.completions(&tw.dom_a), vec![true, true]);
    DROPS.with_borrow(|drops| {
        assert_eq!(drops.len(), 2);
        assert!(drops.iter().all(|drop| drop.2 == ZONE_B1));
    });
}

#[test]
fn receiver_reads_the_host_pointer_in_its_own_coordinate_space() {
    let mut tw = two_windows(window_b_edge);
    let mut sim = tw.sim;
    sim.pick_up(&tw.dom_a, "card".to_string());

    // Global (1200, 102) is B-local (100, 51): one pixel below B's top
    // edge. In origin-local coordinates y=102 would choose the bottom edge
    // of B's (50..150) rect, so this distinguishes the coordinate spaces.
    tw.dom_a
        .in_runtime(|| tw.world.track_global(Point::new(1200.0, 102.0)));
    let rec_b = tw.world.record(tw.key_b).unwrap();
    let joined_b = JoinedWindow {
        world: tw.world,
        key: tw.key_b,
        geometry: rec_b.geometry,
    };
    assert_eq!(
        tw.dom_b.in_runtime(|| joined_b.local_pointer()),
        Some(Point::new(100.0, 51.0))
    );
    rerender_both(&mut tw);
    let html = dioxus_ssr::render(&tw.dom_b);
    assert!(
        html.contains("data-edge=\"top\""),
        "receiver edge was wrong: {html}"
    );
}

#[test]
fn receiver_tree_intent_uses_the_receiver_local_pointer() {
    DROPS.with_borrow_mut(|drops| drops.clear());
    let mut dom_a = VirtualDom::new(window_a);
    dom_a.rebuild_in_place();
    let world = WORLD.with_borrow(|slot| *slot).unwrap();
    let mut dom_b = VirtualDom::new(window_b_tree).with_root_context(world);
    dom_b.rebuild_in_place();
    let windows = world.windows();
    let (rec_a, rec_b) = (windows[0], windows[1]);
    dom_a.in_runtime(|| {
        rec_a
            .geometry
            .set(Point::new(0.0, 0.0), (800.0, 600.0), 1.0)
    });
    dom_b.in_runtime(|| {
        rec_b
            .geometry
            .set(Point::new(1000.0, 0.0), (800.0, 600.0), 2.0)
    });
    let tree_zone = dom_b.in_runtime(|| rec_b.registry.records()[0].id);
    let mut sim = drag_sim::<String>();
    sim.place_in(
        &dom_a,
        rec_b.key,
        tree_zone,
        Rect::new(50.0, 50.0, 100.0, 100.0),
    );
    sim.pick_up(&dom_a, "card".to_string());

    // B-local y=51 is in the row's top quarter. The origin-local y=102
    // would resolve to the middle band and incorrectly report `into`.
    dom_a.in_runtime(|| world.track_global(Point::new(1200.0, 102.0)));
    rerender(&mut dom_b);
    let html = dioxus_ssr::render(&dom_b);
    assert!(
        html.contains("data-intent=\"before\""),
        "receiver tree intent was wrong: {html}"
    );
}

#[test]
fn host_drop_applies_live_modifiers_and_resets_them_per_drag() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;
    sim.pick_up(&tw.dom_a, "card".to_string());
    tw.dom_a
        .in_runtime(|| tw.world.update_modifiers(Modifiers::CONTROL));
    assert_eq!(
        tw.dom_b
            .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0))),
        Some(ZONE_B1)
    );

    sim.pick_up(&tw.dom_a, "card-2".to_string());
    tw.dom_a
        .in_runtime(|| tw.world.update_modifiers(Modifiers::ALT));
    assert_eq!(
        tw.dom_b
            .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0))),
        Some(ZONE_B1)
    );

    sim.pick_up(&tw.dom_a, "card-3".to_string());
    assert_eq!(
        tw.dom_b
            .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0))),
        Some(ZONE_B1)
    );
    EFFECTS.with_borrow(|effects| {
        assert_eq!(
            effects.as_slice(),
            &[DropEffect::Copy, DropEffect::Link, DropEffect::Move]
        )
    });
}

#[test]
fn host_drive_cannot_hijack_a_keyboard_world_drag() {
    let tw = two_windows(window_b);
    let mut ctx = tw.world.context();
    tw.dom_a.in_runtime(|| {
        ctx.start(
            "keyboard".to_string(),
            Some(ZONE_A1),
            Point::new(11.0, 12.0),
            Point::default(),
            DropEffect::Move,
            DragMode::Keyboard,
        );
        tw.world.begin_from(tw.key_a);
        tw.world.track_global(Point::new(1200.0, 200.0));
        assert_eq!(tw.world.drop_at_global(Point::new(1200.0, 200.0)), None);
    });
    assert!(ctx.dragging());
    assert_eq!(ctx.pointer(), Point::new(11.0, 12.0));
    assert_eq!(ctx.over(), None);
    assert_eq!(tw.world.origin_window(), Some(tw.key_a));

    drop(tw.dom_a);
    assert!(!ctx.dragging());
    assert!(ctx.payload().is_none());
    assert_eq!(tw.world.origin_window(), None);
}

#[test]
fn source_success_is_committed_before_receiver_user_code() {
    let tw = two_windows(window_b_observes_committed_source);
    let mut sim = tw.sim;
    sim.pick_up_from(&tw.dom_a, "card".to_string(), Some(ZONE_A1));

    assert_eq!(
        tw.dom_b
            .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0))),
        Some(ZONE_B1)
    );
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);
    assert_eq!(tw.world.drag_session(), None);
}

#[test]
fn receiver_started_drag_does_not_inherit_finalized_source_session() {
    let tw = two_windows(window_b_starts_replacement_drag);
    let mut sim = tw.sim;
    sim.pick_up(&tw.dom_a, "original".to_string());

    assert_eq!(
        tw.dom_b
            .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0))),
        Some(ZONE_B1)
    );
    assert_eq!(sim.completions(&tw.dom_a), vec![true]);
    assert_eq!(tw.world.context().payload().as_deref(), Some("replacement"));
    assert!(tw.world.context().dragging());
    assert_eq!(tw.world.origin_window(), Some(tw.key_b));
    assert_eq!(tw.world.drag_session(), None);
    tw.dom_b.in_runtime(|| tw.world.cancel_drag());
}

#[test]
fn ineligible_window_is_excluded_without_losing_geometry() {
    let tw = two_windows(window_b);
    let rec_a = tw.world.record(tw.key_a).unwrap();
    let rec_b = tw.world.record(tw.key_b).unwrap();
    tw.dom_b.in_runtime(|| {
        rec_b
            .geometry
            .set(Point::new(0.0, 0.0), (800.0, 600.0), 1.0);
        rec_b.geometry.mark_focused();
    });
    let point = Point::new(100.0, 100.0);
    assert_eq!(tw.world.window_under(point).map(|r| r.key), Some(tw.key_b));

    tw.dom_b.in_runtime(|| rec_b.geometry.set_eligible(false));
    assert!(!tw.dom_b.in_runtime(|| rec_b.geometry.live()));
    assert_eq!(tw.world.window_under(point).map(|r| r.key), Some(tw.key_a));
    assert_eq!(
        rec_b.geometry.to_client(Point::new(100.0, 100.0)),
        Some(Point::new(100.0, 100.0)),
        "ineligibility must retain the last geometry"
    );

    tw.dom_b.in_runtime(|| rec_b.geometry.set_eligible(true));
    assert!(tw.dom_b.in_runtime(|| rec_b.geometry.live()));
    assert_eq!(tw.world.window_under(point).map(|r| r.key), Some(tw.key_b));
    assert!(rec_a.geometry.contains_global(point));
}

#[test]
fn live_geometry_status_reacts_when_capability_is_lost() {
    let tw = two_windows(window_b);
    let geometry = tw.world.record(tw.key_b).unwrap().geometry;
    let mut status = VirtualDom::new(geometry_status).with_root_context(geometry);
    status.rebuild_in_place();
    assert!(dioxus_ssr::render(&status).contains("live"));

    tw.dom_b.in_runtime(|| geometry.set_eligible(false));
    status.render_immediate(&mut NoOpMutations);
    assert!(dioxus_ssr::render(&status).contains("inert"));
}

#[test]
fn duplicate_zone_ids_are_qualified_by_window() {
    DROPS.with_borrow_mut(|drops| drops.clear());
    let mut dom_a = VirtualDom::new(window_a);
    dom_a.rebuild_in_place();
    let world = WORLD.with_borrow(|slot| *slot).unwrap();
    let mut dom_b = VirtualDom::new(window_b_duplicate_id).with_root_context(world);
    dom_b.rebuild_in_place();
    let windows = world.windows();
    let (rec_a, rec_b) = (windows[0], windows[1]);
    dom_a.in_runtime(|| {
        rec_a
            .geometry
            .set(Point::new(0.0, 0.0), (800.0, 600.0), 1.0)
    });
    dom_b.in_runtime(|| {
        rec_b
            .geometry
            .set(Point::new(1000.0, 0.0), (800.0, 600.0), 2.0)
    });
    let mut sim = drag_sim::<String>();
    sim.place(&dom_a, ZONE_A1, Rect::new(0.0, 100.0, 200.0, 80.0));
    sim.place_in(
        &dom_a,
        rec_b.key,
        ZONE_A1,
        Rect::new(50.0, 50.0, 100.0, 100.0),
    );
    sim.pick_up_from(&dom_a, "card".to_string(), Some(ZONE_A1));
    sim.move_to(&dom_a, Point::new(100.0, 140.0));
    assert_eq!(
        world.source_location(),
        Some(ZoneLocation {
            window: rec_a.key,
            zone: ZONE_A1,
        })
    );
    assert_eq!(
        world.over_location(),
        Some(ZoneLocation {
            window: rec_a.key,
            zone: ZONE_A1,
        })
    );
    rerender(&mut dom_a);
    assert!(dioxus_ssr::render(&dom_a).contains("data-over"));

    // Move to the same legacy id in B. `DndContext::over()` stays equal,
    // so only the qualified location can transfer the highlight.
    sim.move_to(&dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.over(&dom_a), Some(ZONE_A1));
    assert_eq!(
        world.over_location(),
        Some(ZoneLocation {
            window: rec_b.key,
            zone: ZONE_A1,
        })
    );
    rerender(&mut dom_a);
    rerender(&mut dom_b);
    assert!(!dioxus_ssr::render(&dom_a).contains("data-over"));
    assert!(dioxus_ssr::render(&dom_b).contains("data-over"));

    assert_eq!(sim.release(&dom_a), Some(ZONE_A1));
    assert_eq!(sim.completions(&dom_a), vec![true]);
    DROPS.with_borrow(|drops| {
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].0, "B", "qualified registry must receive the drop");
    });
}

#[test]
fn host_side_drop_outside_every_window_cancels() {
    let tw = two_windows(window_b);
    let mut sim = tw.sim;

    sim.pick_up(&tw.dom_a, "card".to_string());
    assert_eq!(tw.world.origin_window(), tw.sim.window_key());
    let dropped = tw
        .dom_a
        .in_runtime(|| tw.world.drop_at_global(Point::new(5000.0, 5000.0)));
    assert_eq!(dropped, None);
    assert!(!sim.dragging(&tw.dom_a));
    assert_eq!(sim.completions(&tw.dom_a), vec![false]);
    tw.dom_a.in_runtime(|| tw.world.cancel_drag());
    assert_eq!(sim.completions(&tw.dom_a), vec![false]);
    DROPS.with_borrow(|d| assert!(d.is_empty()));

    // The cancelled source is immediately reusable, and the replacement
    // owns a fresh completion generation.
    sim.pick_up(&tw.dom_a, "card-again".to_string());
    assert!(sim.dragging(&tw.dom_a));
    tw.dom_a.in_runtime(|| tw.world.cancel_drag());
    assert_eq!(sim.completions(&tw.dom_a), vec![false, false]);
}

#[test]
fn without_geometry_the_world_degrades_to_single_window_drags() {
    // Build both windows but never feed geometry: the Wayland story.
    DROPS.with_borrow_mut(|d| d.clear());
    let mut dom_a = VirtualDom::new(window_a);
    dom_a.rebuild_in_place();
    let world = WORLD.with_borrow(|w| *w).expect("window A created a world");
    let mut dom_b = VirtualDom::new(window_b).with_root_context(world);
    dom_b.rebuild_in_place();

    let mut sim = drag_sim::<String>();
    sim.place(&dom_a, ZONE_A1, Rect::new(0.0, 100.0, 200.0, 80.0));

    sim.pick_up(&dom_a, "card".to_string());
    sim.move_to(&dom_a, Point::new(100.0, 140.0));
    assert_eq!(sim.over(&dom_a), Some(ZONE_A1), "local hit-testing intact");
    assert_eq!(sim.release(&dom_a), Some(ZONE_A1));
    DROPS.with_borrow(|d| assert_eq!(d[0].0, "A"));
    let _ = dom_b;
}

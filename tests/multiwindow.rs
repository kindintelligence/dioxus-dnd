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
use dioxus_dnd::prelude::*;
use dioxus_dnd::test::{drag_sim, rerender, DragSim, DragSimProbe};

const ZONE_A1: ZoneId = ZoneId(1);
const ZONE_B1: ZoneId = ZoneId(2);

thread_local! {
    static WORLD: RefCell<Option<DndWorld<String>>> = const { RefCell::new(None) };
    /// (window tag, payload, to, client point) per delivered drop.
    static DROPS: RefCell<Vec<(&'static str, String, ZoneId, Point)>> =
        const { RefCell::new(Vec::new()) };
}

fn log_drop(tag: &'static str, o: &DropOutcome<String>) {
    DROPS.with_borrow_mut(|d| d.push((tag, o.payload.clone(), o.to, o.client)));
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
    sim.place_in(&dom_a, rec_b.key, ZONE_B1, Rect::new(50.0, 50.0, 100.0, 100.0));

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
    sim.place_in(&tw.dom_a, tw.key_b, ZONE_B1, Rect::new(250.0, 50.0, 100.0, 100.0));
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

    sim.pick_up(&tw.dom_a, "card".to_string());
    sim.move_to(&tw.dom_a, Point::new(1200.0, 200.0));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));

    // Window B closes mid-drag: dropping its VirtualDom unmounts the
    // provider, which leaves the world.
    drop(tw.dom_b);
    assert_eq!(tw.dom_a.in_runtime(|| tw.world.windows().len()), 1);
    assert_eq!(sim.over(&tw.dom_a), None, "hover into the dead window cleared");
    assert!(sim.dragging(&tw.dom_a), "the drag itself survives");

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
    assert!(tw.dom_a.in_runtime(|| ctx.payload().is_some()));

    // The RECEIVING window presents the glide.
    rerender_both(&mut tw);
    assert!(!dioxus_ssr::render(&tw.dom_a).contains("GHOST"));
    assert!(dioxus_ssr::render(&tw.dom_b).contains("GHOST"));

    // The glide completing resets everything (headless stand-in for the
    // overlay's transitionend).
    tw.dom_b.in_runtime(|| {
        let mut ctx = ctx;
        ctx.finish_settle();
    });
    tw.dom_b.render_immediate(&mut NoOpMutations);
    assert!(tw.dom_a.in_runtime(|| ctx.payload().is_none()));
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
    tw.dom_a.in_runtime(|| tw.world.track_global(Point::new(1200.0, 200.0)));
    assert_eq!(sim.over(&tw.dom_a), Some(ZONE_B1));
    rerender_both(&mut tw);
    assert!(dioxus_ssr::render(&tw.dom_b).contains("data-over"));
    // The pointer is stored in ORIGIN-window client px (the anchor space).
    assert_eq!(
        tw.dom_a.in_runtime(|| tw.world.context().pointer()),
        Point::new(1200.0, 200.0),
    );

    // Over dead space: hover clears, drag continues.
    tw.dom_a.in_runtime(|| tw.world.track_global(Point::new(5000.0, 5000.0)));
    assert_eq!(sim.over(&tw.dom_a), None);
    assert!(sim.dragging(&tw.dom_a));

    // A non-origin window's first event mid-drag means the button is up:
    // the glue completes the drop at that global position.
    let dropped = tw
        .dom_b
        .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0)));
    assert_eq!(dropped, Some(ZONE_B1));
    assert!(!sim.dragging(&tw.dom_a));
    DROPS.with_borrow(|d| {
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, "B");
        assert_eq!(d[0].3, Point::new(100.0, 100.0), "B-local drop coords");
    });

    // Idempotence: a late echo (webview pointerup arriving after the
    // host already delivered) is a harmless no-op.
    let echo = tw
        .dom_a
        .in_runtime(|| tw.world.drop_at_global(Point::new(1200.0, 200.0)));
    assert_eq!(echo, None);
    DROPS.with_borrow(|d| assert_eq!(d.len(), 1));
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
    DROPS.with_borrow(|d| assert!(d.is_empty()));
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

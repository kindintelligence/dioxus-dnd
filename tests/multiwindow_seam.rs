//! Multi-window seam probe (TODO 3.5): two headless `VirtualDom`s on one
//! thread stand in for two dioxus-desktop windows (which is exactly what
//! they are - desktop polls every window's vdom on the main thread).
//! Verifies the two load-bearing assumptions of the shared-world design:
//!
//! 1. A `Signal` created in window A's runtime, handed to window B via
//!    `with_root_context`, re-renders B when A writes it.
//! 2. A `Callback` created in window B can be invoked from window A's
//!    runtime context, and its signal writes still re-render B.
//!
//! What this cannot cover: the desktop waker actually waking a parked
//! window (verified by source reading of dioxus-desktop's tao proxy), and
//! anything webview/OS-level. Those stay in the display-session probe.

use dioxus::dioxus_core::NoOpMutations;
use dioxus::prelude::*;
use std::cell::RefCell;

thread_local! {
    static SIGNAL_SLOT: RefCell<Option<Signal<i32>>> = const { RefCell::new(None) };
    static CALLBACK_SLOT: RefCell<Option<Callback<i32>>> = const { RefCell::new(None) };
}

/// Window A: owns the shared signal, parks a handle for the test to grab.
fn window_a() -> Element {
    let sig = use_hook(|| Signal::new_in_scope(0i32, ScopeId::ROOT));
    SIGNAL_SLOT.with(|s| *s.borrow_mut() = Some(sig));
    rsx! {
        div { "a:{sig}" }
    }
}

/// Window B: joins the signal it was handed through root context.
fn window_b() -> Element {
    let sig = use_context::<Signal<i32>>();
    rsx! {
        div { "b:{sig}" }
    }
}

#[test]
fn signal_written_in_window_a_rerenders_window_b() {
    let mut dom_a = VirtualDom::new(window_a);
    dom_a.rebuild_in_place();
    let sig = SIGNAL_SLOT
        .with(|s| *s.borrow())
        .expect("window A parked its signal");

    let mut dom_b = VirtualDom::new(window_b).with_root_context(sig);
    dom_b.rebuild_in_place();
    assert!(dioxus_ssr::render(&dom_b).contains("b:0"));

    // The write happens inside A's runtime, like a drag event handler would.
    let mut sig_in_a = sig;
    dom_a.in_runtime(|| sig_in_a.set(42));

    dom_b.render_immediate(&mut NoOpMutations);
    assert!(
        dioxus_ssr::render(&dom_b).contains("b:42"),
        "window B did not observe window A's signal write"
    );

    // And A itself still re-renders too.
    dom_a.render_immediate(&mut NoOpMutations);
    assert!(dioxus_ssr::render(&dom_a).contains("a:42"));
}

/// Window B for the callback test: exposes an `on_drop`-shaped callback
/// that writes one of B's own signals.
fn window_b_with_callback() -> Element {
    let mut received = use_signal(|| -1i32);
    let cb = use_callback(move |v: i32| received.set(v));
    CALLBACK_SLOT.with(|s| *s.borrow_mut() = Some(cb));
    rsx! {
        div { "got:{received}" }
    }
}

#[test]
fn callback_made_in_window_b_fires_from_window_a() {
    let mut dom_a = VirtualDom::new(window_a);
    dom_a.rebuild_in_place();

    let mut dom_b = VirtualDom::new(window_b_with_callback);
    dom_b.rebuild_in_place();
    assert!(dioxus_ssr::render(&dom_b).contains("got:-1"));
    let cb = CALLBACK_SLOT
        .with(|s| *s.borrow())
        .expect("window B parked its callback");

    // Deliver a "drop" from inside window A's runtime, the way a shared
    // registry would invoke the target zone's on_drop.
    dom_a.in_runtime(|| cb.call(7));

    dom_b.render_immediate(&mut NoOpMutations);
    assert!(
        dioxus_ssr::render(&dom_b).contains("got:7"),
        "window B's callback did not run / did not re-render B when \
         invoked from window A's runtime"
    );
}

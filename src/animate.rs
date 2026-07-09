//! Drop animations.
//!
//! With the `web` feature (on a web renderer), the reorder glide is armed
//! synchronously on the real DOM element - invert, forced style flush,
//! release - so it cannot race the browser's paint schedule, and the
//! animation itself is a plain compositor-driven CSS transition. Without
//! it, a render-twice fallback stands in; that path is **experimental** -
//! it depends on the browser painting the inverted frame between two
//! commits, so validate it in your target renderer and tune `duration` to
//! taste.
//!
//! [`FlipItem`] implements the FLIP technique (First–Last–Invert–Play) for
//! reorder transitions: when your list order changes, each item measures
//! where it moved from, renders instantly *back* at its old position via a
//! transform, then releases the transform with a CSS transition - so tiles
//! appear to glide to their new slots.
//!
//! You drive it with an `epoch` counter: bump it whenever order changes.
//!
//! ```text
//! let mut items = use_signal(|| vec![/* … */]);
//! let mut epoch = use_signal(|| 0usize);
//! rsx! {
//!     SortableList {
//!         len: items.read().len(),
//!         render: move |ix: usize| rsx! {
//!             FlipItem { epoch: epoch(), Row { item: items.read()[ix].clone() } }
//!         },
//!         on_sort: move |ev: SortEvent| {
//!             apply_sort(&mut items.write(), ev);
//!             epoch += 1;
//!         },
//!     }
//! }
//! ```
//!
//! **Drop-settle** (the ghost gliding into the receiving zone on drop) is
//! built in: set `settle: true` on
//! [`crate::core::components::DragOverlay`]. **Snap-back on cancel** needs
//! no Rust at all - it's a CSS recipe: give the overlay's child
//! `transition: transform 150ms ease` and revert your item's
//! `data-dragging` styles with a transition.

use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::a11y::use_reduced_motion_css;
use crate::core::{platform, Point, Rect};

/// FLIP animation phase (render-twice fallback only; the `web` path hands
/// the whole sequence to the DOM in one synchronous step).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum FlipPhase {
    /// At rest (transition armed, no transform).
    #[default]
    Rest,
    /// Rendered at the *old* position via an instant inverse transform.
    Invert(Point),
}

/// The inline style of an inverted item: parked at its old position, no
/// transition. Shared by both paths so they cannot drift.
fn invert_style(d: Point) -> String {
    format!(
        "transform: translate({}px, {}px); transition: none;",
        d.x, d.y
    )
}

/// The inline style of an at-rest item: no transform, transition armed.
/// Also what [`platform::flip_transform`] leaves on the real element, so the
/// virtual DOM's view of the attribute stays truthful.
fn rest_style(duration: f64, easing: &str) -> String {
    format!("transform: none; transition: transform {duration}ms {easing};")
}

/// Wraps one list/grid item and glides it to its new position whenever
/// `epoch` changes. See the module docs for the driving pattern.
#[component]
pub fn FlipItem(
    /// Bump this whenever the surrounding order changes.
    epoch: usize,
    /// Transition duration in milliseconds.
    #[props(default = 200.0)]
    duration: f64,
    /// CSS easing function.
    #[props(default = "ease".to_string())]
    easing: String,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mounted = use_signal(|| None::<Rc<MountedData>>);
    let prev = use_signal(|| None::<Rect>);
    let mut phase = use_signal(FlipPhase::default);

    // First & Last & Invert: on every epoch change, measure the new
    // position, and if the item moved, run the glide. The synchronous DOM
    // handoff is preferred; when it isn't available, snap the inverse
    // transform on through a render instead.
    use_effect(use_reactive!(|epoch, duration, easing| {
        let _ = epoch;
        let Some(m) = mounted.peek().clone() else {
            return;
        };
        let mut prev = prev;
        spawn(async move {
            if let Ok(r) = m.get_client_rect().await {
                let now = Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height);
                if let Some(old) = *prev.peek() {
                    let d = Point::new(old.x - now.x, old.y - now.y);
                    if d.x != 0.0 || d.y != 0.0 {
                        let handed_off = platform::flip_transform(
                            &m,
                            &invert_style(d),
                            &rest_style(duration, &easing),
                        );
                        if !handed_off {
                            phase.set(FlipPhase::Invert(d));
                        }
                    }
                }
                prev.set(Some(now));
            }
        });
    }));

    // Play (fallback path only): once the inverted frame has committed,
    // release the transform; the armed CSS transition glides the item home.
    // (Effects run after the render commits, giving the browser a painted
    // "old position" frame.)
    use_effect(move || {
        if matches!(phase(), FlipPhase::Invert(_)) {
            phase.set(FlipPhase::Rest);
        }
    });

    let style = match phase() {
        FlipPhase::Invert(d) => invert_style(d),
        FlipPhase::Rest => rest_style(duration, &easing),
    };
    // The glide is an inline transition; honor prefers-reduced-motion.
    let reduced_motion_css = use_reduced_motion_css();

    rsx! {
        {reduced_motion_css}
        div {
            style: "{style}",
            "data-dnd-motion": true,
            onmounted: move |evt: Event<MountedData>| {
                let mut mounted = mounted;
                mounted.set(Some(evt.data()));
            },
            ..attributes,
            {children}
        }
    }
}

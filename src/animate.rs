//! Drop animations. **Experimental** - this module is the one part of the
//! crate whose behavior depends on browser paint timing rather than pure
//! logic; validate it in your target renderer and tune `duration` to taste.
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
//! **Snap-back on cancel** needs no Rust at all - it's a CSS recipe. Pointer
//! drags via `Draggable` use your `DragOverlay`; give the overlay's child
//! `transition: transform 150ms ease` and render it conditionally on
//! `dnd.dragging()` - reverting your item's `data-dragging` styles with a
//! transition produces the settle effect.

use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::core::{Point, Rect};

/// FLIP animation phase.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum FlipPhase {
    /// At rest (transition armed, no transform).
    #[default]
    Rest,
    /// Rendered at the *old* position via an instant inverse transform.
    Invert(Point),
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
    // position, and if the item moved, snap an inverse transform on.
    use_effect(use_reactive!(|epoch| {
        let _ = epoch;
        let Some(m) = mounted.peek().clone() else {
            return;
        };
        let mut prev = prev;
        spawn(async move {
            if let Ok(r) = m.get_client_rect().await {
                let now = Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height);
                if let Some(old) = *prev.peek() {
                    let dx = old.x - now.x;
                    let dy = old.y - now.y;
                    if dx != 0.0 || dy != 0.0 {
                        phase.set(FlipPhase::Invert(Point::new(dx, dy)));
                    }
                }
                prev.set(Some(now));
            }
        });
    }));

    // Play: once the inverted frame has committed, release the transform;
    // the armed CSS transition glides the item home. (Effects run after the
    // render commits, giving the browser a painted "old position" frame.)
    use_effect(move || {
        if matches!(phase(), FlipPhase::Invert(_)) {
            phase.set(FlipPhase::Rest);
        }
    });

    let style = match phase() {
        FlipPhase::Invert(d) => {
            format!(
                "transform: translate({}px, {}px); transition: none;",
                d.x, d.y
            )
        }
        FlipPhase::Rest => {
            format!("transform: none; transition: transform {duration}ms {easing};")
        }
    };

    rsx! {
        div {
            style: "{style}",
            onmounted: move |evt: Event<MountedData>| {
                let mut mounted = mounted;
                mounted.set(Some(evt.data()));
            },
            ..attributes,
            {children}
        }
    }
}

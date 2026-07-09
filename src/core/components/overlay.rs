//! The pointer-pinned ghost: [`DragOverlay`] (with the drop-settle glide)
//! and [`SettleSlot`], the wrapper that makes a settling drop read as one
//! object.

use dioxus::html::MountedData;
use dioxus::prelude::*;

use std::rc::Rc;

use crate::core::hooks::{use_dnd, SettleFlag};
use crate::core::types::{DragMode, Point, Rect};
use crate::core::world::WorldMembership;

use super::merge_style;

/// The functional inline style for a pointer-pinned "ghost": fixed to `pos`
/// (a viewport-space top-left), out of flow, click-through, above the page.
/// Kept as a single `fn` so this exact rule has one definition, shared by
/// every overlay in the crate.
pub(crate) fn overlay_style(pos: Point) -> String {
    format!(
        "position: fixed; left: {}px; top: {}px; pointer-events: none; z-index: 9999;",
        pos.x, pos.y
    )
}

/// Renders its children pinned to the pointer while a drag is in flight -
/// a custom "ghost" that follows the cursor.
///
/// Extra attributes (`class`, …) are forwarded to the wrapper div, so the
/// ghost styles directly - e.g. Tailwind
/// `class: "rotate-3 scale-105 shadow-xl"`. A forwarded `style` is merged
/// after the functional positioning rather than replacing it.
///
/// With `settle: true`, a successful pointer drop doesn't vanish the ghost:
/// it glides from the release point until its center meets the receiving
/// zone's center, then unmounts - the drop-settle animation. During the
/// glide the drag context is *settling*: `dragging()` is already false
/// (zones have unlit), but `payload()` stays readable so the ghost keeps
/// its content. The glide honors `prefers-reduced-motion` via
/// `data-dnd-motion` (it snaps near-instantly, and cleanup still runs
/// because `transitionend` still fires). Cancelled drags and keyboard
/// drops never settle.
///
/// Note: the ghost follows the shared context's pointer position, which
/// pointer drags update on every move. Keyboard drags carry no pointer, so
/// during one the ghost sits at the viewport origin - check `dnd.mode()`
/// and skip rendering it if that matters to you.
#[component]
pub fn DragOverlay<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
    /// Glide the ghost into the receiving zone on drop instead of
    /// vanishing. Off by default.
    #[props(default)]
    settle: bool,
    /// Settle transition duration in milliseconds.
    #[props(default = 200.0)]
    duration: f64,
    /// CSS easing function for the settle glide.
    #[props(default = "ease".to_string())]
    easing: String,
    /// Size the ghost to the grabbed element's measured rect. With it, the
    /// `pointer - grab` anchoring is exact by construction: the ghost
    /// appears precisely over what was picked up, whatever your ghost rsx
    /// renders inside. The ghost waits for the pickup measurement (at most
    /// a frame behind `Draggable`; custom sources must call
    /// `set_source_rect` or it stays hidden). Off by default (the ghost
    /// sizes to its content).
    #[props(default)]
    match_source: bool,
    /// Fired when the drop-settle finishes (including the degenerate
    /// no-glide cases), so completion effects can start as the ghost lands
    /// instead of racing it. Never fires for cancelled drags, and not when
    /// the overlay unmounts mid-glide.
    #[props(default)]
    on_settled: Option<EventHandler<()>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let _ = phantom;
    let mut dnd = use_dnd::<T>();
    // Multi-window: when the provider joined a `DndWorld`, exactly one
    // joined window presents the ghost each frame (the one under the
    // global pointer; the receiving one during a settle).
    let membership = try_use_context::<WorldMembership<T>>().and_then(|m| m.0);

    // Arm settle-aware drops for this provider while mounted. Draggables
    // check the flag at delivery time, so mount order doesn't matter.
    let flag = try_use_context::<SettleFlag<T>>();
    use_hook(move || {
        if settle {
            if let Some(mut f) = flag {
                f.armed.set(true);
            }
        }
    });
    use_drop(move || {
        if settle {
            if let Some(mut f) = flag {
                f.armed.set(false);
            }
            // Unmounting mid-glide: nobody is left to hear transitionend,
            // so reset now (guarded no-op otherwise). The aliveness gate
            // covers app shutdown in multi-window use, where the shared
            // context can die before this window's overlay unmounts.
            if dnd.alive() {
                dnd.finish_settle();
            }
        }
    });

    let mut node = use_signal(|| None::<Rc<MountedData>>);
    // The played glide: `Some(delta)` once the ghost has been measured and
    // the transform released toward the target.
    let mut glide = use_signal(|| None::<Point>);
    // The settle transition is inline; honor prefers-reduced-motion. Only
    // an overlay that settles claims the subtree's stylesheet slot.
    let reduced_motion_css = crate::a11y::use_reduced_motion_css_if(settle);

    // Every way a settle can complete funnels through here, so `on_settled`
    // fires exactly once per landed drop - glide or no glide.
    let mut settled = move || {
        dnd.finish_settle();
        if let Some(h) = &on_settled {
            h.call(());
        }
    };

    // The ghost's own rect, measured once per settle; retargets reuse it
    // (the layout rect never moves - the glide is pure transform).
    let mut from = use_signal(|| None::<Rect>);
    let mut measuring = use_signal(|| false);

    // Measure & play (FLIP, like FlipItem): the settled frame commits at
    // the release position with the transition armed; this effect then
    // measures the ghost and releases the transform toward the settle rect.
    // The effect subscribes to `settling()`, so a `retarget_settle` (the
    // landed element announcing its real position, see `SettleSlot`) reruns
    // it and re-aims the transform - CSS transitions continue smoothly from
    // wherever the ghost currently is, mid-glide included.
    use_effect(move || {
        match dnd.settling() {
            Some(to) if settle => {
                if let Some(f) = *from.peek() {
                    let d = to.center() - f.center();
                    // A sub-pixel glide would produce no transition (and
                    // thus no transitionend) - but only when none is
                    // already running; a retarget of a live glide always
                    // ends in a transitionend.
                    if d.x.abs() < 1.0 && d.y.abs() < 1.0 && glide.peek().is_none() {
                        settled();
                    } else {
                        glide.set(Some(d));
                    }
                    return;
                }
                if *measuring.peek() {
                    // A retarget landed mid-measure; the pending measurement
                    // reads the latest settle rect when it completes.
                    return;
                }
                let Some(m) = node.peek().clone() else {
                    // Never mounted (e.g. keyboard-only ghost skipped) -
                    // nothing to animate.
                    settled();
                    return;
                };
                measuring.set(true);
                spawn(async move {
                    let r = m.get_client_rect().await;
                    measuring.set(false);
                    let Ok(r) = r else {
                        settled();
                        return;
                    };
                    let f = Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height);
                    from.set(Some(f));
                    // Aim at the *current* settle rect - a retarget may
                    // have arrived while the measurement was in flight.
                    let Some(to) = dnd.settling() else {
                        settled();
                        return;
                    };
                    let d = to.center() - f.center();
                    if d.x.abs() < 1.0 && d.y.abs() < 1.0 {
                        settled();
                    } else {
                        glide.set(Some(d));
                    }
                });
            }
            _ => {
                if glide.peek().is_some() {
                    glide.set(None);
                }
                if from.peek().is_some() {
                    from.set(None);
                }
            }
        }
    });

    let settling = settle && dnd.settling().is_some();
    if !dnd.dragging() && !settling {
        return rsx! {};
    }
    // A keyboard drag has no meaningful pointer - rendering would pin the
    // ghost to the viewport corner. Zones already highlight via data-over,
    // and the LiveRegion narrates; the ghost is pointer furniture.
    if dnd.mode() == DragMode::Keyboard {
        return rsx! {};
    }
    // A size-matched ghost waits for the pickup measurement (at most a
    // frame behind `Draggable`): rendering content-sized first would
    // visibly pop to the matched size when the rect lands. Custom drag
    // sources must call `set_source_rect`, or the ghost stays hidden.
    if match_source && dnd.dragging() && dnd.source_rect().is_none() {
        return rsx! {};
    }
    // Multi-window presentation: the world elects one window's overlay per
    // frame and hands it the anchor in ITS client px, plus the
    // origin-to-here scale ratio so a size-matched ghost keeps its physical
    // size across differently-scaled windows. Without a world this is the
    // classic raw anchor.
    let (anchor, scale_ratio) = match membership {
        Some(j) => match j.present_overlay(dnd.pointer(), dnd.grab(), settling) {
            Some(placed) => placed,
            None => return rsx! {},
        },
        None => (dnd.pointer() - dnd.grab(), 1.0),
    };

    // Size-matched ghost: the grabbed element's measured rect, border-box
    // so the ghost's own padding/border stay inside it.
    let size = match_source
        .then(|| dnd.source_rect())
        .flatten()
        .map(|r| {
            format!(
                " width: {}px; height: {}px; box-sizing: border-box;",
                r.width * scale_ratio,
                r.height * scale_ratio
            )
        })
        .unwrap_or_default();
    let functional = if settling {
        let transform = match glide() {
            Some(d) => format!("translate({}px, {}px)", d.x, d.y),
            None => "none".to_string(),
        };
        format!(
            "{}{size} transform: {transform}; transition: transform {duration}ms {easing};",
            overlay_style(anchor),
        )
    } else {
        format!("{}{size}", overlay_style(anchor))
    };
    let mut attributes = attributes;
    let style = merge_style(&mut attributes, &functional);
    rsx! {
        {reduced_motion_css}
        div {
            style: style,
            "data-dnd-motion": if settle { "true" },
            onmounted: move |evt: Event<MountedData>| node.set(Some(evt.data())),
            ontransitionend: move |_| {
                // The only transition this element runs is the settle glide;
                // finish_settle (inside `settled`) is a guarded no-op
                // against stray bubbles.
                if settling && glide.peek().is_some() {
                    settled();
                }
            },
            ..attributes,
            {children}
        }
    }
}

/// Wraps the element a drop just created so the drop-settle reads as ONE
/// object: while the ghost glides, the wrapper holds the element's space
/// but keeps it invisible (no "second copy" next to the ghost), re-aims the
/// glide at its own measured rect (the ghost lands exactly where the
/// element is, not at the zone's center), and reveals the element the
/// instant the ghost unmounts.
///
/// Set `active: true` only on the just-landed element - typically by
/// remembering the dropped payload's id in your `on_drop` handler and
/// comparing. Inert while nothing is settling (keyboard drops, cancelled
/// drags, overlays without `settle`), so it is always safe to render.
///
/// ```text
/// on_drop: move |o: DropOutcome<Card>| { landed.set(Some(o.payload.id)); /* model */ },
/// // ...
/// SettleSlot::<Card> { active: landing() == Some(card.id),
///     Draggable::<Card> { payload: card.clone(), CardFace { card } }
/// }
/// ```
#[component]
pub fn SettleSlot<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
    /// True on the element the current settle is delivering.
    active: bool,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let _ = phantom;
    let mut dnd = use_dnd::<T>();
    let mut node = use_signal(|| None::<Rc<MountedData>>);

    let retarget = move |m: Rc<MountedData>| {
        spawn(async move {
            if let Ok(r) = m.get_client_rect().await {
                dnd.retarget_settle(Rect::new(
                    r.origin.x,
                    r.origin.y,
                    r.size.width,
                    r.size.height,
                ));
            }
        });
    };
    // The landed element usually mounts fresh (the drop re-rendered the
    // model), so onmounted below re-aims. This effect covers the other
    // order - `active` turning true on an already-mounted element.
    use_effect(use_reactive!(|active| {
        if active && dnd.settling().is_some() {
            if let Some(m) = node.peek().clone() {
                retarget(m);
            }
        }
    }));

    // Reading `settling()` here subscribes the reveal: the moment
    // finish_settle resets the state, the wrapper re-renders visible. Both
    // states write an explicit value - updating a style string to "" can
    // leave the old declaration standing.
    let hidden = active && dnd.settling().is_some();
    let mut attributes = attributes;
    let style = merge_style(
        &mut attributes,
        if hidden {
            "visibility: hidden;"
        } else {
            "visibility: visible;"
        },
    );
    rsx! {
        div {
            style: style,
            "data-settling": if hidden { "true" },
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                node.set(Some(m.clone()));
                if active && dnd.settling().is_some() {
                    retarget(m);
                }
            },
            ..attributes,
            {children}
        }
    }
}

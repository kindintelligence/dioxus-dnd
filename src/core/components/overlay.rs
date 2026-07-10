//! The pointer-pinned ghost: [`DragOverlay`] (with the drop-settle glide)
//! and [`SettleSlot`], the wrapper that makes a settling drop read as one
//! object.

use dioxus::html::MountedData;
use dioxus::prelude::*;

use std::{cell::Cell, rc::Rc};

use crate::core::hooks::{use_dnd, SettleFlag};
use crate::core::types::{DragId, DragMode, Point, Rect};
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct SettleGlide {
    generation: u64,
    delta: Point,
}

fn glide_for_generation(
    glide: Option<SettleGlide>,
    generation: Option<u64>,
) -> Option<SettleGlide> {
    glide.filter(|glide| Some(glide.generation) == generation)
}

fn overlay_generation_key(generation: Option<u64>) -> String {
    generation.map_or_else(
        || "drag".to_string(),
        |generation| format!("settle-{generation}"),
    )
}

fn cleanup_generation(still_armed: bool, owned: Option<u64>, live: Option<u64>) -> Option<u64> {
    if still_armed {
        live.or(owned)
    } else {
        owned
    }
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
    let settle_token = move || match membership {
        Some(joined) => joined.world.settle_token(joined.key),
        None => dnd.settling().map(|_| 0),
    };
    // `use_drop` keeps its initial closure, so carry the latest generation
    // owned by this component scope in stable, nonreactive storage. An old
    // scope must never look up and finish a same-window successor's token.
    let owned_settle_generation = use_hook(|| Rc::new(Cell::new(None::<u64>)));
    let render_settle_generation = settle_token();
    owned_settle_generation.set(render_settle_generation);
    let settle_capability = use_hook(|| DragId::auto().0);

    // Arm settle-aware drops for this provider while mounted. Draggables
    // check the flag at delivery time, so mount order doesn't matter.
    let flag = try_use_context::<SettleFlag<T>>();
    use_hook(move || {
        if settle {
            if let Some(f) = flag {
                f.arm(settle_capability);
            }
        }
    });
    use_drop(move || {
        if settle {
            // A replacement overlay supersedes this capability before the
            // old scope drops. Only the still-armed scope may adopt a claim
            // that arrived before its first settling render.
            let still_armed = flag.is_none_or(|flag| flag.release(settle_capability));
            // Unmounting mid-glide: nobody is left to hear transitionend,
            // so reset now (guarded no-op otherwise). The aliveness gate
            // covers app shutdown in multi-window use, where the shared
            // context can die before this window's overlay unmounts.
            if dnd.alive() {
                match membership {
                    Some(joined) => {
                        let generation = cleanup_generation(
                            still_armed,
                            owned_settle_generation.get(),
                            joined.world.peek_settle_token(joined.key),
                        );
                        if let Some(generation) = generation {
                            joined
                                .world
                                .finish_settle_generation(joined.key, generation);
                        }
                    }
                    None if still_armed => dnd.finish_settle(),
                    None => {}
                }
            }
        }
    });

    // Tag mounted data with the generation of the keyed DOM node that owns
    // it. A successor must never measure its predecessor's detached node.
    let mut node = use_signal(|| None::<(Option<u64>, Rc<MountedData>)>);
    // The played glide: `Some(delta)` once the ghost has been measured and
    // the transform released toward the target.
    let mut glide = use_signal(|| None::<SettleGlide>);
    // The settle transition is inline; honor prefers-reduced-motion. Only
    // an overlay that settles claims the subtree's stylesheet slot.
    let reduced_motion_css = crate::a11y::use_reduced_motion_css_if(settle);

    // Every way a settle can complete funnels through here, so `on_settled`
    // fires exactly once per landed drop - glide or no glide.
    let mut settled = move |generation: u64| {
        let finished = match membership {
            Some(joined) => joined
                .world
                .finish_settle_generation(joined.key, generation),
            None => {
                let was_settling = dnd.settling().is_some();
                dnd.finish_settle();
                was_settling
            }
        };
        if finished {
            if let Some(h) = &on_settled {
                h.call(());
            }
        }
    };

    // The ghost's own rect, measured once per settle; retargets reuse it
    // (the layout rect never moves - the glide is pure transform).
    let mut from = use_signal(|| None::<Rect>);
    // The generation whose measurement is in flight. Tagging this state is
    // load-bearing: an older task must neither block nor clear its successor.
    let mut measuring = use_signal(|| None::<u64>);
    let mut measured_generation = use_signal(|| None::<u64>);

    // Measure & play (FLIP, like FlipItem): the settled frame commits at
    // the release position with the transition armed; this effect then
    // measures the ghost and releases the transform toward the settle rect.
    // The effect subscribes to `settling()`, so a `retarget_settle` (the
    // landed element announcing its real position, see `SettleSlot`) reruns
    // it and re-aims the transform - CSS transitions continue smoothly from
    // wherever the ghost currently is, mid-glide included.
    use_effect(move || {
        let token = settle_token();
        match (dnd.settling(), token) {
            (Some(to), Some(generation)) if settle => {
                if *measured_generation.peek() != Some(generation) {
                    measured_generation.set(Some(generation));
                    from.set(None);
                    glide.set(None);
                }
                if let Some(f) = *from.peek() {
                    let d = to.center() - f.center();
                    // A sub-pixel glide would produce no transition (and
                    // thus no transitionend) - but only when none is
                    // already running; a retarget of a live glide always
                    // ends in a transitionend.
                    let playing = glide_for_generation(*glide.peek(), Some(generation)).is_some();
                    if d.x.abs() < 1.0 && d.y.abs() < 1.0 && !playing {
                        settled(generation);
                    } else {
                        glide.set(Some(SettleGlide {
                            generation,
                            delta: d,
                        }));
                    }
                    return;
                }
                if *measuring.peek() == Some(generation) {
                    // A retarget landed mid-measure; the pending measurement
                    // reads the latest settle rect when it completes.
                    return;
                }
                // Subscribe here: a generation key remount replaces the old
                // MountedData asynchronously, and its onmounted write must
                // wake this measurement effect.
                let mounted_node = node.read().clone();
                let Some((node_generation, m)) = mounted_node else {
                    // The pointer ghost mounts in this render and will wake
                    // the effect. A keyboard-only custom settle has no ghost
                    // to animate, so it can finish immediately.
                    if dnd.mode() == DragMode::Keyboard {
                        settled(generation);
                    }
                    return;
                };
                if node_generation != Some(generation) {
                    // A generation key change has retired this mounted node;
                    // wait for the successor's onmounted handle.
                    return;
                }
                measuring.set(Some(generation));
                spawn(async move {
                    let r = m.get_client_rect().await;
                    // Clear only this task's tag. A successor may already
                    // have installed its own measurement generation.
                    if *measuring.peek() == Some(generation) {
                        measuring.set(None);
                    }
                    if settle_token() != Some(generation) {
                        return;
                    }
                    let Ok(r) = r else {
                        settled(generation);
                        return;
                    };
                    let f = Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height);
                    from.set(Some(f));
                    // Aim at the *current* settle rect - a retarget may
                    // have arrived while the measurement was in flight.
                    let Some(to) = dnd.settling() else {
                        settled(generation);
                        return;
                    };
                    let d = to.center() - f.center();
                    if d.x.abs() < 1.0 && d.y.abs() < 1.0 {
                        settled(generation);
                    } else {
                        glide.set(Some(SettleGlide {
                            generation,
                            delta: d,
                        }));
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
                if measured_generation.peek().is_some() {
                    measured_generation.set(None);
                }
                if measuring.peek().is_some() {
                    measuring.set(None);
                }
            }
        }
    });

    let settle_generation = render_settle_generation;
    let settling = settle && settle_generation.is_some();
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
    let played_glide = glide_for_generation(glide(), settle_generation);
    let functional = if settling {
        let transform = match played_glide {
            Some(glide) => format!("translate({}px, {}px)", glide.delta.x, glide.delta.y),
            None => "none".to_string(),
        };
        format!(
            "{}{size} transform: {transform}; transition: transform {duration}ms {easing};",
            overlay_style(anchor),
        )
    } else {
        format!("{}{size}", overlay_style(anchor))
    };
    let overlay_key = overlay_generation_key(settle_generation);
    let mut attributes = attributes;
    let style = merge_style(&mut attributes, &functional);
    rsx! {
        // A one-item keyed list forces an actual DOM-node replacement when
        // the settle generation changes. A key on a fixed single child is
        // only an identity hint and may be reused by the renderer.
        for node_key in [overlay_key] {
            div {
                key: "{node_key}",
                style: style.clone(),
                "data-dnd-motion": if settle { "true" },
                onmounted: move |evt: Event<MountedData>| {
                    // A retired keyed node may report onmounted after its
                    // successor. Never let it overwrite the current handle.
                    if settle_token() == settle_generation {
                        node.set(Some((settle_generation, evt.data())));
                    }
                },
                ontransitionend: move |_| {
                    // The node key preserves this handler's generation if an old
                    // transition event was already queued while a successor
                    // rendered. The live token check covers the inverse ordering.
                    if let Some(glide) = played_glide {
                        if settle_token() == Some(glide.generation) {
                            settled(glide.generation);
                        }
                    }
                },
                ..attributes.clone(),
                {children.clone()}
            }
        }
        {reduced_motion_css}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_glide_is_not_relabelled_as_its_successor() {
        let stale = SettleGlide {
            generation: 7,
            delta: Point::new(10.0, 20.0),
        };
        assert_eq!(glide_for_generation(Some(stale), Some(7)), Some(stale));
        assert_eq!(glide_for_generation(Some(stale), Some(8)), None);
        assert_ne!(
            overlay_generation_key(Some(7)),
            overlay_generation_key(Some(8))
        );
        assert_eq!(cleanup_generation(false, Some(7), Some(8)), Some(7));
        assert_eq!(cleanup_generation(true, None, Some(8)), Some(8));
        assert_eq!(cleanup_generation(true, Some(7), Some(8)), Some(8));
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
    let membership = try_use_context::<WorldMembership<T>>().and_then(|m| m.0);
    let mut node = use_signal(|| None::<Rc<MountedData>>);

    let settle_token = move || match membership {
        Some(joined) => joined.world.settle_token(joined.key),
        None => dnd.settling().map(|_| 0),
    };

    let retarget = move |m: Rc<MountedData>, generation: u64| {
        spawn(async move {
            if let Ok(r) = m.get_client_rect().await {
                if settle_token() == Some(generation) {
                    dnd.retarget_settle(Rect::new(
                        r.origin.x,
                        r.origin.y,
                        r.size.width,
                        r.size.height,
                    ));
                }
            }
        });
    };
    // The landed element usually mounts fresh (the drop re-rendered the
    // model), so onmounted below re-aims. This effect covers the other
    // order - `active` turning true on an already-mounted element.
    use_effect(use_reactive!(|active| {
        if active {
            if let (Some(m), Some(generation)) = (node.peek().clone(), settle_token()) {
                retarget(m, generation);
            }
        }
    }));

    // Reading `settling()` here subscribes the reveal: the moment
    // finish_settle resets the state, the wrapper re-renders visible. Both
    // states write an explicit value - updating a style string to "" can
    // leave the old declaration standing.
    let hidden = active && settle_token().is_some();
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
                if active {
                    if let Some(generation) = settle_token() {
                        retarget(m, generation);
                    }
                }
            },
            ..attributes,
            {children}
        }
    }
}

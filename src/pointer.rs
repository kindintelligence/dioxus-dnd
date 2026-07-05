//! Instant, consistent touch (and pen) support. Some mobile browsers can
//! fire native HTML5 drag events from a touch long-press on `draggable`
//! elements (Safari on iOS/iPadOS 15+; reports for Chrome on Android
//! conflict), but the hold delay is browser-controlled and support is
//! inconsistent across the mobile landscape. This module drives the same
//! shared [`crate::core::DndContext`] from pointer events instead: the drag
//! starts after a small movement threshold with no hold, behaves the same
//! in every browser, and uses your `DragOverlay` as the ghost. Where native
//! long-press exists, it keeps working as a fallback on plain `Draggable`s.
//!
//! [`PointerDraggable`] *composes* the core [`Draggable`]: mouse users get
//! the native HTML5 drag path, while touch/pen input is handled with
//! `pointerdown` → `pointermove` → `pointerup`, hit-testing the registered
//! drop zones' cached client rects to decide where the drop lands. Touch
//! pointers have implicit pointer capture, so the originating element keeps
//! receiving moves for the whole gesture.
//!
//! Two things to know:
//! - The wrapper sets `touch-action: none` so the browser doesn't hijack the
//!   gesture for scrolling. If your list must also scroll by touch, consider
//!   a drag handle: put `PointerDraggable` on the handle only.
//! - A small movement threshold (default 8 px) distinguishes drags from taps.

use dioxus::prelude::*;

use crate::core::{
    transition, use_dnd, use_zone_registry, DragMode, Draggable, DropEffect, DropOutcome,
    GestureEffect, GestureEvent, GesturePhase, Point, ZoneId,
};

/// Pointer position from a pointer event, in client coordinates.
pub(crate) fn pointer_client(evt: &PointerEvent) -> Point {
    let c = evt.client_coordinates();
    Point::new(c.x, c.y)
}

/// A draggable that works for mouse *and* touch/pen.
///
/// Mouse drags go through the native HTML5 path (inner core `Draggable`);
/// touch and pen drags are synthesized from pointer events. Both feed the
/// same context, so your `DropZone`s don't care which path delivered the
/// payload — touch drops arrive through the zone registry with correct
/// client/element coordinates.
#[component]
pub fn PointerDraggable<T: Clone + PartialEq + 'static>(
    /// The value delivered on drop.
    payload: T,
    /// The zone this item lives in (reported in `DropOutcome::from`).
    #[props(default)]
    zone: Option<ZoneId>,
    /// Drop effect. Defaults to `Move`.
    #[props(default)]
    effect: DropEffect,
    /// Disable dragging without unmounting.
    #[props(default)]
    disabled: bool,
    /// Label for screen-reader announcements (forwarded to the inner
    /// `Draggable`).
    #[props(default)]
    label: Option<String>,
    /// Movement (px) before a touch counts as a drag rather than a tap.
    #[props(default = 8.0)]
    threshold: f64,
    /// Fired when a drag begins (either path).
    #[props(default)]
    on_drag_start: Option<EventHandler<()>>,
    /// Fired when the drag ends; `true` if a zone consumed the payload.
    #[props(default)]
    on_drag_end: Option<EventHandler<bool>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut dnd = use_dnd::<T>();
    let registry = use_zone_registry::<T>();
    // The gesture lifecycle is a formal state machine (see `core::machine`):
    // handlers feed it events and act on the effects it returns.
    let mut phase = use_signal(|| GesturePhase::Idle);
    let mut step = move |event: GestureEvent, threshold: f64| -> GestureEffect {
        let (next, fx) = transition(*phase.peek(), event, threshold);
        phase.set(next);
        fx
    };

    let touch_payload = payload.clone();

    // Deliver to a specific zone. Returns true if the drop landed.
    let mut deliver_to = move |target: ZoneId, point: Point, effect: DropEffect| -> bool {
        let Some(record) = registry.get(target) else {
            return false;
        };
        let Some(p) = dnd.payload() else {
            return false;
        };
        if !record.accepts_payload(&p) {
            return false;
        }
        let origin = (*record.rect.peek())
            .map(|r| r.origin())
            .unwrap_or_default();
        if let Some((p, from)) = dnd.take() {
            record.on_drop.call(DropOutcome {
                payload: p,
                from,
                to: target,
                effect,
                client: point,
                element: point - origin,
            });
            return true;
        }
        false
    };

    rsx! {
        div {
            style: "touch-action: none;",
            onpointerdown: move |evt: PointerEvent| {
                if disabled || !evt.is_primary() || evt.pointer_type() == "mouse" {
                    // Mouse uses the native HTML5 path of the inner Draggable.
                    return;
                }
                let _ = step(
                    GestureEvent::Down { at: pointer_client(&evt), pointer_id: evt.pointer_id() },
                    threshold,
                );
            },
            onpointermove: move |evt: PointerEvent| {
                let event = GestureEvent::Move {
                    at: pointer_client(&evt),
                    pointer_id: evt.pointer_id(),
                };
                match step(event, threshold) {
                    GestureEffect::Begin { origin, at } => {
                        dnd.start(
                            touch_payload.clone(),
                            zone,
                            at,
                            at - origin, // grab offset: travel from the press point
                            effect,
                            DragMode::Pointer,
                        );
                        // Rects go stale on scroll/layout; refresh at drag start.
                        registry.refresh_rects();
                        if let Some(h) = &on_drag_start {
                            h.call(());
                        }
                    }
                    GestureEffect::Track { at } => {
                        dnd.update_pointer(at);
                        // Track hover for zone highlighting.
                        match registry.hit_test(at) {
                            Some(z) => dnd.enter(z),
                            None => {
                                if let Some(over) = dnd.over() {
                                    dnd.leave(over);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            },
            onpointerup: move |evt: PointerEvent| {
                let event = GestureEvent::Up {
                    at: pointer_client(&evt),
                    pointer_id: evt.pointer_id(),
                };
                let GestureEffect::Drop { at: point } = step(event, threshold) else {
                    return; // tap, or a foreign pointer's release
                };
                // Fast path: cached rects contain the point.
                if let Some(target) = registry.hit_test(point) {
                    let dropped = deliver_to(target, point, effect);
                    if !dropped {
                        dnd.cancel();
                    }
                    if let Some(h) = &on_drag_end {
                        h.call(dropped);
                    }
                    return;
                }
                // Miss: rects may be stale (scroll/resize mid-drag). Re-measure,
                // then retry with a closest-center fallback for gutter drops.
                let on_drag_end = on_drag_end;
                spawn(async move {
                    registry.measure_all().await;
                    let target = dnd
                        .payload()
                        .and_then(|p| registry.hit_test_closest(point, &p, 48.0));
                    let dropped = match target {
                        Some(t) => deliver_to(t, point, effect),
                        None => false,
                    };
                    if !dropped {
                        dnd.cancel();
                    }
                    if let Some(h) = &on_drag_end {
                        h.call(dropped);
                    }
                });
            },
            onpointercancel: move |_| {
                if step(GestureEvent::Cancel, threshold) == GestureEffect::Abort {
                    dnd.cancel();
                    if let Some(h) = &on_drag_end {
                        h.call(false);
                    }
                }
            },
            ..attributes,
            Draggable::<T> {
                payload,
                zone,
                effect,
                disabled,
                label,
                on_drag_start,
                on_drag_end,
                {children}
            }
        }
    }
}

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
//! [`PointerDraggable`] *composes* the core [`Draggable`]. By default mouse,
//! touch and pen all use `pointerdown` → `pointermove` → `pointerup`,
//! hit-testing the registered drop zones' cached client rects to decide where
//! the drop lands. Set [`DragInputMode::Native`] or [`DragInputMode::Hybrid`]
//! when you need the browser's HTML5 drag path.
//!
//! Two things to know:
//! - The wrapper sets `touch-action: none` so the browser doesn't hijack the
//!   gesture for scrolling. If your list must also scroll by touch, consider
//!   a drag handle: put `PointerDraggable` on the handle only.
//! - A small movement threshold (default 8 px) distinguishes drags from taps.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::core::components::merge_style;
use crate::core::{
    effective_effect, platform, transition, use_dnd, use_zone_registry, DragInputMode, DragMode,
    Draggable, DropEffect, DropOutcome, GestureEffect, GestureEvent, GesturePhase, Point, ZoneId,
};

/// Pointer position from a pointer event, in client coordinates.
pub(crate) fn pointer_client(evt: &PointerEvent) -> Point {
    let c = evt.client_coordinates();
    Point::new(c.x, c.y)
}

/// A draggable that works for mouse *and* touch/pen.
///
/// By default mouse, touch and pen drags are synthesized from pointer events,
/// using your `DragOverlay` instead of the browser's drag image. Set `input`
/// to [`DragInputMode::Native`] or [`DragInputMode::Hybrid`] when you need the
/// browser's HTML5 drag path.
///
/// Like [`Draggable`], the wrapper (where your forwarded `class` lands)
/// carries `data-dragging="true"` while this payload is in flight and
/// `data-disabled="true"` when disabled - absent otherwise, so
/// presence-based selectors (Tailwind `data-dragging:opacity-50`) work
/// directly.
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
    /// Which input/browser drag path this source should use. Defaults to
    /// pointer events for mouse, touch and pen.
    #[props(default)]
    input: DragInputMode,
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
    // The wrapper's DOM handle, captured on mount, so a pointer drag can grab
    // pointer capture (see `core::platform`) and keep receiving move/up even
    // after the cursor leaves this element. Inert without the `web` feature.
    let mut node = use_signal(|| None::<Rc<MountedData>>);
    // Where inside the element the pointer pressed, recorded on pointerdown so
    // the drag carries a real grab offset (like the native path's
    // `element_point`) - drives `DragOverlay` placement and exact
    // `CanvasDropZone` drops, instead of the bare threshold travel.
    let mut press_offset = use_signal(Point::default);
    // Modifier keys held at drop time, sampled from the pointer events. The
    // native path resolves Ctrl/Cmd=copy, Alt=link from the drop event's
    // modifiers; the pointer path has no such event, so we track them here and
    // apply the same `effective_effect` convention when the drop lands.
    let mut mods = use_signal(Modifiers::empty);

    let touch_payload = payload.clone();
    let attr_payload = payload.clone();
    // A caller-supplied `style` must not replace `touch-action: none` (the
    // drag would silently turn into a scroll) - merge instead.
    let mut attributes = attributes;
    let style = merge_style(&mut attributes, "touch-action: none;");

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
        // Read the grab offset before `take` resets the drag state.
        let mode = dnd.mode();
        let grab = dnd.grab();
        if let Some((p, from)) = dnd.take() {
            record.on_drop.call(DropOutcome {
                payload: p,
                from,
                to: target,
                effect,
                mode,
                client: point,
                element: point - origin,
                grab,
            });
            return true;
        }
        false
    };

    // Resolve and deliver a drop at `point`. Shared by the normal pointer-up
    // and by the capture-free recovery in `onpointermove`.
    let mut finish_drop = move |point: Point| {
        // Apply the modifier-key convention (Ctrl/Cmd=copy, Alt=link) held at
        // release, matching the native `DropZone` path.
        let effect = effective_effect(effect, *mods.peek());
        // Fast path: the topmost cached rect containing the point, if it
        // accepts the payload.
        if let Some(target) = registry.hit_test(point) {
            if deliver_to(target, point, effect) {
                if let Some(h) = &on_drag_end {
                    h.call(true);
                }
                return;
            }
            // Fell through: the topmost zone rejected the payload. Don't cancel
            // yet - a zone *under* it (or nearby) may accept. Retry below.
        }
        // Miss or rejection: rects may be stale (scroll/resize mid-drag), or the
        // geometric top zone rejects. Re-measure, then retry with an
        // acceptance-aware closest-center fallback for gutter/overlap drops.
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
    };

    rsx! {
        div {
            style: style,
            "data-dragging": if dnd.dragging() && dnd.payload().as_ref() == Some(&attr_payload) { "true" },
            "data-disabled": if disabled { "true" },
            onmounted: move |evt: Event<MountedData>| node.set(Some(evt.data())),
            onpointerdown: move |evt: PointerEvent| {
                let pointer_type = evt.pointer_type();
                if disabled || !evt.is_primary() || !input.uses_pointer(&pointer_type) {
                    return;
                }
                // Grab pointer capture up front so move/up keep arriving if the
                // cursor leaves this element mid-drag (no-op without `web`). A
                // press that resolves as a tap releases it on pointerup.
                if let Some(n) = node.peek().clone() {
                    platform::capture_pointer(&n, evt.pointer_id());
                }
                let o = evt.element_coordinates();
                press_offset.set(Point::new(o.x, o.y));
                let _ = step(
                    GestureEvent::Down { at: pointer_client(&evt), pointer_id: evt.pointer_id() },
                    threshold,
                );
            },
            onpointermove: move |evt: PointerEvent| {
                let at = pointer_client(&evt);
                mods.set(evt.modifiers());
                // Capture-free recovery: Dioxus 0.8 exposes no pointer-capture
                // API without web-sys, so a mouse released while off this
                // element never delivers a `pointerup` here. If it returns over
                // the element mid-drag with no button held, finish the drop
                // rather than tracking a phantom drag. Touch/pen hold a button
                // through contact, so this only trips for a released mouse.
                let event = if matches!(*phase.peek(), GesturePhase::Dragging { .. })
                    && evt.held_buttons().is_empty()
                {
                    if let Some(n) = node.peek().clone() {
                        platform::release_pointer(&n, evt.pointer_id());
                    }
                    GestureEvent::Up { at, pointer_id: evt.pointer_id() }
                } else {
                    GestureEvent::Move { at, pointer_id: evt.pointer_id() }
                };
                match step(event, threshold) {
                    GestureEffect::Begin { at, .. } => {
                        dnd.start(
                            touch_payload.clone(),
                            zone,
                            at,
                            *press_offset.peek(), // grab: offset within the element at press
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
                    GestureEffect::Drop { at: point } => finish_drop(point),
                    _ => {}
                }
            },
            onpointerup: move |evt: PointerEvent| {
                if let Some(n) = node.peek().clone() {
                    platform::release_pointer(&n, evt.pointer_id());
                }
                mods.set(evt.modifiers());
                let GestureEffect::Drop { at: point } = step(
                    GestureEvent::Up { at: pointer_client(&evt), pointer_id: evt.pointer_id() },
                    threshold,
                ) else {
                    return; // tap, or a foreign pointer's release
                };
                finish_drop(point);
            },
            onpointercancel: move |evt: PointerEvent| {
                if let Some(n) = node.peek().clone() {
                    platform::release_pointer(&n, evt.pointer_id());
                }
                if step(GestureEvent::Cancel, threshold) == GestureEffect::Abort {
                    dnd.cancel();
                    if let Some(h) = &on_drag_end {
                        h.call(false);
                    }
                }
            },
            // If the browser yanks pointer capture mid-drag (only reachable with
            // the `web` feature, which sets it), abort cleanly. After a normal
            // pointerup the machine is already Idle, so this no-ops.
            onlostpointercapture: move |_| {
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
                native: input.uses_native(),
                label,
                on_drag_start,
                on_drag_end,
                {children}
            }
        }
    }
}

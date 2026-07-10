//! The [`Draggable`] drag source: pointer and keyboard interaction, the
//! pointer-capture substitute, and the hierarchical keyboard navigation
//! that walks the zone registry.

use dioxus::html::MountedData;
use dioxus::prelude::*;

use std::rc::Rc;

use crate::core::hooks::{use_dnd, use_zone_registry, SettleFlag};
use crate::core::strings::use_dnd_strings;
use crate::core::types::{
    effective_effect, Direction, DragMode, DropEffect, DropOutcome, Point, PointerKind, Rect,
    TouchSense, ZoneId,
};
use crate::core::world::{WorldHit, WorldMembership};
use crate::core::{
    platform, transition_with, GestureEffect, GestureEvent, GesturePhase, Promotion,
};

use super::delivery::{deliver_drop, RELEASE_RECOVERY_MOVES};
use super::merge_style;
use super::pointer::{pointer_client, primary_press, touch_style, HoldTimer};

/// Internal: which hierarchical move an arrow key requested.
#[derive(Debug, Clone, Copy, PartialEq)]
enum NavKey {
    Next,
    Prev,
    Descend,
    Ascend,
}

/// Map an arrow key to a hierarchical move, honoring layout direction:
/// horizontal arrows mirror under RTL (the WAI-ARIA tree convention), so
/// "into" is always the arrow pointing along reading order. Pure, for
/// testability.
fn nav_key(key: &Key, dir: Direction) -> Option<NavKey> {
    Some(match (key, dir) {
        (Key::ArrowDown, _) => NavKey::Next,
        (Key::ArrowUp, _) => NavKey::Prev,
        (Key::ArrowRight, Direction::Ltr) | (Key::ArrowLeft, Direction::Rtl) => NavKey::Descend,
        (Key::ArrowLeft, Direction::Ltr) | (Key::ArrowRight, Direction::Rtl) => NavKey::Ascend,
        _ => return None,
    })
}

fn keyboard_drop_points(rect: Option<Rect>) -> (Point, Point) {
    match rect {
        Some(r) => {
            let client = r.center();
            (client, client - r.origin())
        }
        None => (Point::default(), Point::default()),
    }
}

/// Wraps its children in a focusable pointer/keyboard drag source and pushes
/// `payload` into the shared context on drag start.
///
/// Any extra attributes (`class`, `style`, `id`…) are forwarded to the div.
///
/// While this element's payload is in flight the div carries
/// `data-dragging="true"`, and `data-disabled="true"` when `disabled` -
/// both are *absent* otherwise, so presence-based selectors (CSS
/// `[data-dragging]`, Tailwind `data-dragging:opacity-50`) work directly.
#[component]
pub fn Draggable<T: Clone + PartialEq + 'static>(
    /// The value delivered to whichever `DropZone` receives this drag.
    payload: T,
    /// The zone this item currently lives in (reported in `DropOutcome::from`).
    #[props(default)]
    zone: Option<ZoneId>,
    /// Drop effect. Defaults to `Move`.
    #[props(default)]
    effect: DropEffect,
    /// Disable dragging without unmounting.
    #[props(default)]
    disabled: bool,
    /// Movement in CSS px before a pointer press becomes a drag.
    #[props(default = 8.0)]
    threshold: f64,
    /// How a finger shares this element with native scrolling.
    /// [`TouchSense::Auto`] (default) keeps vertical swipes scrolling the
    /// page and picks up on a short hold or a sideways pull;
    /// [`TouchSense::Immediate`] owns every touch from the first pixel.
    /// Mouse and pen drags are identical under both.
    #[props(default)]
    touch: TouchSense,
    /// Human label used in screen-reader announcements ("Picked up {label}").
    #[props(default)]
    label: Option<String>,
    /// Fired when a drag begins.
    #[props(default)]
    on_drag_start: Option<EventHandler<()>>,
    /// Fired when the drag ends; `true` if a zone consumed the payload,
    /// `false` if it was cancelled.
    #[props(default)]
    on_drag_end: Option<EventHandler<bool>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut dnd = use_dnd::<T>();
    let registry = use_zone_registry::<T>();
    let settle_flag = try_use_context::<SettleFlag<T>>();
    // Multi-window: when the provider joined a `DndWorld`, pointer moves
    // and releases resolve across every joined window. `None` (the normal
    // single-window case) leaves every path below exactly as it was.
    let membership = try_use_context::<WorldMembership<T>>().and_then(|m| m.0);
    // Everything the keyboard path voices, localizable through context.
    let strings = use_dnd_strings();
    // Separate clones for the two closures that need the payload.
    let kb_payload = payload.clone();
    let pointer_payload = payload.clone();
    let kb_label = label.clone();
    // Comparing against the context payload (rather than a local flag) means
    // the attribute is also correct when a custom source started the drag.
    let attr_payload = payload.clone();
    // For claiming a keyboard drop's focus restoration on mount.
    let mount_payload = payload.clone();
    let mut phase = use_signal(|| GesturePhase::Idle);
    // Did native pointer capture engage for the current press? When it
    // did, events retarget to this element and no capture substitute is
    // needed (or wanted - see the layer below).
    let mut captured = use_signal(|| false);
    // Consecutive empty-held moves seen mid-drag (lost-release debounce).
    let mut empty_held_moves = use_signal(|| 0u8);
    // Some(pid) while a touch press under `Auto` waits on its hold timer;
    // doubles as the timer element's render condition.
    let mut hold_pid = use_signal(|| None::<i32>);
    let mut step = move |event: GestureEvent, threshold: f64| -> GestureEffect {
        let promotion = if hold_pid.peek().is_some() {
            Promotion::HoldOrSideways
        } else {
            Promotion::Distance
        };
        let (next, fx) = transition_with(*phase.peek(), event, threshold, promotion);
        phase.set(next);
        // Any exit from Pressed retires the pending hold - the drag began,
        // the press tapped out, or a vertical pull yielded to the scroll.
        if hold_pid.peek().is_some() && !matches!(next, GesturePhase::Pressed { .. }) {
            hold_pid.set(None);
        }
        fx
    };
    let mut node = use_signal(|| None::<Rc<MountedData>>);
    let mut press_offset = use_signal(Point::default);
    // The initiating press's device kind, recorded into the drag state at
    // promotion so host-side glue can tell captured pointers (touch) from
    // blind ones (mouse/pen) - see `PointerKind`.
    let mut press_kind = use_signal(PointerKind::default);
    // The element's rect, measured at press time - so a promotion can hand
    // the ghost its size synchronously. Measuring at Begin instead left the
    // `match_source` overlay blank for the measurement roundtrip (~a few
    // frames), a visible pop-in at every pickup.
    let mut press_rect = use_signal(|| None::<Rect>);
    let mut mods = use_signal(Modifiers::empty);
    let mut attributes = attributes;
    let style = merge_style(&mut attributes, touch_style(touch));

    // Begin is reachable from two places - a pointer-move promotion and the
    // hold timer's alarm - so the sequence lives in one callback.
    let begin_drag = use_callback(move |at: Point| {
        dnd.start(
            pointer_payload.clone(),
            zone,
            at,
            *press_offset.peek(),
            effect,
            DragMode::Pointer,
        );
        dnd.set_pointer_kind(*press_kind.peek());
        // Dress a size-matched ghost immediately from the press-time
        // measurement; fall back to measuring now only if the press's
        // measurement hasn't landed yet (a press promoted within a frame).
        if let Some(r) = *press_rect.peek() {
            dnd.set_source_rect(Some(r));
        } else if let Some(m) = node.peek().clone() {
            let mut dnd = dnd;
            spawn(async move {
                if let Ok(r) = m.get_client_rect().await {
                    dnd.set_source_rect(Some(Rect::new(
                        r.origin.x,
                        r.origin.y,
                        r.size.width,
                        r.size.height,
                    )));
                }
            });
        }
        // A world drag anchors its coordinates to this window and needs
        // every joined window's rects fresh, not just this one's.
        match membership {
            Some(j) => {
                j.world.begin_from(j.key);
                j.world.refresh_all_rects();
            }
            None => registry.refresh_rects(),
        }
        if let Some(h) = &on_drag_start {
            h.call(());
        }
    });

    let mut deliver_to = move |target: ZoneId, point: Point, effect: DropEffect| -> bool {
        deliver_drop(registry, &mut dnd, settle_flag, target, point, effect)
    };

    let mut finish_drop = move |point: Point| {
        let effect = effective_effect(effect, *mods.peek());
        // A release the world resolves into a FOREIGN window delivers
        // there: that window's registry and settle flag, coordinates in
        // its client px (including its own 48px snap, in its own CSS px).
        // Own-window and unresolved releases (no geometry, outside every
        // window) fall through to the classic path below, so
        // single-window behavior is untouched - origin-window snap
        // included.
        if let Some(j) = membership {
            if let Some((rec, local)) = j.foreign_window_under(point) {
                let mut dnd = dnd;
                spawn(async move {
                    let target = match rec.registry.hit_test(local) {
                        Some(t) => Some(t),
                        None => {
                            rec.registry.measure_all().await;
                            dnd.payload()
                                .and_then(|p| rec.registry.hit_test_closest(local, &p, 48.0))
                        }
                    };
                    let dropped = target
                        .map(|t| {
                            deliver_drop(rec.registry, &mut dnd, Some(rec.settle), t, local, effect)
                        })
                        .unwrap_or(false);
                    if dropped {
                        // If the drop settled, the ghost glides in the
                        // receiving window (a guarded no-op otherwise).
                        j.world.present_settle_in(rec.key);
                    } else {
                        dnd.cancel();
                    }
                    if let Some(h) = &on_drag_end {
                        h.call(dropped);
                    }
                });
                return;
            }
        }
        if let Some(target) = registry.hit_test(point) {
            if deliver_to(target, point, effect) {
                if let Some(h) = &on_drag_end {
                    h.call(true);
                }
                return;
            }
        }
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
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                node.set(Some(m.clone()));
                // Focus continuity for keyboard drops: if this mount IS the
                // just-dropped payload landing in its new place, take the
                // focus the browser dropped when the source unmounted.
                if !disabled && dnd.claim_refocus(&mount_payload) {
                    spawn(async move {
                        let _ = m.set_focus(true).await;
                    });
                }
            },
            onpointerdown: move |evt: PointerEvent| {
                if disabled || !primary_press(&evt) {
                    return;
                }
                empty_held_moves.set(0);
                // Suppress the press's default actions - the same line the
                // sortable rows carry. The one that matters: `tabindex=0`
                // makes this div mouse-focusable as a browser side effect,
                // and that stray focus outlives the drop (the model mutates,
                // nodes get reused, and the ring can surface on an unrelated
                // item). Keyboard focus via Tab is untouched, and clicks
                // on inner controls still fire (`click` is not a
                // compatibility mouse event).
                evt.prevent_default();
                evt.stop_propagation();
                captured.set(match node.peek().clone() {
                    Some(n) => platform::capture_pointer(&n, evt.pointer_id()),
                    None => false,
                });
                let o = evt.element_coordinates();
                press_offset.set(Point::new(o.x, o.y));
                press_kind.set(PointerKind::from_pointer_type(&evt.pointer_type()));
                // Measure at press so a later promotion can size the ghost
                // without waiting on a roundtrip (see `press_rect`).
                press_rect.set(None);
                if let Some(m) = node.peek().clone() {
                    spawn(async move {
                        if let Ok(r) = m.get_client_rect().await {
                            press_rect.set(Some(Rect::new(
                                r.origin.x,
                                r.origin.y,
                                r.size.width,
                                r.size.height,
                            )));
                        }
                    });
                }
                // A machine still in Dragging while the shared context is
                // idle is the corpse of an externally-completed drag: host
                // glue ended it (`drop_at_global` / `cancel_drag`, e.g. a
                // cross-window or dead-space mouse release) and the matching
                // pointerup never reached this element - outside the
                // viewport it targets `<html>`, which no handler hears.
                // Reset before pressing, or the corpse eats this press
                // ((Dragging, Down) is deliberately inert so a second
                // pointer can't steal a live gesture). Silent: the drag's
                // end was already delivered/announced by whoever ended it.
                if !dnd.dragging() && matches!(*phase.peek(), GesturePhase::Dragging { .. }) {
                    let _ = step(GestureEvent::Cancel, threshold);
                }
                let pid = evt.pointer_id();
                let _ = step(
                    GestureEvent::Down { at: pointer_client(&evt), pointer_id: pid },
                    threshold,
                );
                // Arm the long-press clock: fingers (and pens) under `Auto`
                // promote on hold-or-sideways; mice promote on travel alone.
                if touch == TouchSense::Auto
                    && evt.pointer_type() != "mouse"
                    && matches!(*phase.peek(), GesturePhase::Pressed { pointer_id, .. } if pointer_id == pid)
                {
                    hold_pid.set(Some(pid));
                }
            },
            onpointermove: move |evt: PointerEvent| {
                let at = pointer_client(&evt);
                mods.set(evt.modifiers());
                // Lost-release recovery, debounced: only a RUN of empty-
                // held moves is believed (see RELEASE_RECOVERY_MOVES).
                let released = if matches!(*phase.peek(), GesturePhase::Dragging { .. })
                    && evt.held_buttons().is_empty()
                {
                    let streak = empty_held_moves.peek().saturating_add(1);
                    empty_held_moves.set(streak);
                    streak >= RELEASE_RECOVERY_MOVES
                } else {
                    if *empty_held_moves.peek() != 0 {
                        empty_held_moves.set(0);
                    }
                    false
                };
                let event = if released {
                    if let Some(n) = node.peek().clone() {
                        platform::release_pointer(&n, evt.pointer_id());
                    }
                    GestureEvent::Up { at, pointer_id: evt.pointer_id() }
                } else {
                    GestureEvent::Move { at, pointer_id: evt.pointer_id() }
                };
                match step(event, threshold) {
                    GestureEffect::Begin { at, .. } => begin_drag.call(at),
                    GestureEffect::Track { at } => {
                        dnd.update_pointer(at);
                        // World-resolved hits are authoritative even when
                        // zoneless: a foreign window IN FRONT of one of our
                        // zones must not let the covered zone light up.
                        let hit = match membership.map(|j| j.zone_under(at)) {
                            Some(WorldHit::Zone(z)) => Some(z),
                            Some(WorldHit::Window) => None,
                            Some(WorldHit::Unresolved) | None => registry.hit_test(at),
                        };
                        match hit {
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
                    return;
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
            onlostpointercapture: move |_| {
                if step(GestureEvent::Cancel, threshold) == GestureEffect::Abort {
                    dnd.cancel();
                    if let Some(h) = &on_drag_end {
                        h.call(false);
                    }
                }
            },
            // A promoted drag owns the touch: cancel its moves so the
            // browser can't start a pan mid-drag. (`touch-action` is only
            // consulted at gesture start, so `pan-y` alone can't do this.)
            // dioxus-web's delegated listener is non-passive - see the
            // touch-sensor browser spec.
            ontouchmove: move |evt: TouchEvent| {
                if matches!(*phase.peek(), GesturePhase::Dragging { .. }) {
                    evt.prevent_default();
                }
            },
            // Android pops a context menu on touch long-press (the iOS
            // callout is already off via touch_style); mid-gesture that
            // would tear the hold or the drag. Idle presses keep the menu.
            oncontextmenu: move |evt: Event<MouseData>| {
                if !matches!(*phase.peek(), GesturePhase::Idle) {
                    evt.prevent_default();
                }
            },
            // --- keyboard interaction ---------------------------------
            // Space/Enter picks the item up, arrow keys cycle acceptable
            // zones, Space/Enter drops, Escape cancels. Announcements go
            // through the context; render `a11y::LiveRegion` to voice them.
            tabindex: if disabled { -1_i64 } else { 0 },
            role: "button",
            aria_roledescription: "draggable",
            onkeydown: move |evt: KeyboardEvent| {
                if disabled {
                    return;
                }
                let registry = registry;
                let key = evt.key();
                let is_activate = matches!(key, Key::Enter)
                    || matches!(&key, Key::Character(c) if c == " ");
                let kb_drag = dnd.dragging() && dnd.mode() == DragMode::Keyboard;

                if !dnd.dragging() && is_activate {
                    evt.prevent_default();
                    dnd.start(
                        kb_payload.clone(),
                        zone,
                        Point::default(),
                        Point::default(),
                        effect,
                        DragMode::Keyboard,
                    );
                    // Measure zones so arrow-key order can follow visual
                    // (top-to-bottom, left-to-right) layout.
                    registry.refresh_rects();
                    let name = kb_label.clone().unwrap_or_else(|| (strings.item)());
                    dnd.announce((strings.picked_up)(&name));
                    if let Some(h) = &on_drag_start {
                        h.call(());
                    }
                    return;
                }

                if !kb_drag {
                    return;
                }

                // Hierarchical navigation (WAI-ARIA tree convention):
                // Up/Down cycle siblings at the current level; the arrow
                // along reading order descends into the hovered zone's
                // children; the opposite one ascends to its parent (both
                // mirror under RTL). In flat apps (no nesting) they fall
                // back to next/previous, preserving the simple behavior.
                let nav = nav_key(&key, registry.direction());
                if let (Some(nav), Some(p)) = (nav, dnd.payload()) {
                    evt.prevent_default();
                    let over = dnd.over();
                    let next = match nav {
                        NavKey::Next => registry.step_sibling(over, &p, 1),
                        NavKey::Prev => registry.step_sibling(over, &p, -1),
                        NavKey::Descend => over
                            .and_then(|z| registry.first_child(z, &p))
                            .or_else(|| registry.step_sibling(over, &p, 1)),
                        NavKey::Ascend => over
                            .and_then(|z| registry.ascend(z))
                            .or_else(|| registry.step_sibling(over, &p, -1)),
                    };
                    if let Some(next) = next {
                        dnd.enter(next);
                        let record = registry.get(next);
                        let name = record
                            .as_ref()
                            .and_then(|z| z.label.clone())
                            .unwrap_or_else(|| (strings.zone)(next.0));
                        let inside = record
                            .as_ref()
                            .and_then(|z| z.parent)
                            .and_then(|pid| registry.get(pid))
                            .and_then(|pz| pz.label);
                        match inside {
                            Some(parent) => dnd.announce((strings.over_inside)(&name, &parent)),
                            None => dnd.announce((strings.over)(&name)),
                        }
                    } else {
                        dnd.announce((strings.no_targets)());
                    }
                    return;
                }

                if is_activate {
                    evt.prevent_default();
                    // A custom source can enter() an id from another type's
                    // registry; falling back keeps Enter from dying silently.
                    let target = dnd.over().filter(|z| registry.contains(*z)).or_else(|| {
                        dnd.payload().and_then(|p| registry.step_zone(None, &p, 1))
                    });
                    let Some(target) = target else {
                        dnd.announce((strings.no_target_selected)());
                        return;
                    };
                    if let Some(record) = registry.get(target) {
                        if let Some((p, from)) = dnd.take() {
                            let (client, element) =
                                keyboard_drop_points(registry.cached_rect(target));
                            // The drop will re-mount the moved item and the
                            // browser will dump focus on <body> when this
                            // element unmounts; the landing Draggable claims
                            // this request on mount and focuses itself.
                            dnd.request_refocus(p.clone());
                            record.on_drop.call(DropOutcome {
                                payload: p,
                                from,
                                to: target,
                                effect,
                                mode: DragMode::Keyboard,
                                client,
                                element,
                                grab: Point::default(),
                                edge: None,
                            });
                            let name = record
                                .label
                                .unwrap_or_else(|| (strings.zone)(target.0));
                            dnd.announce((strings.dropped_in)(&name));
                            if let Some(h) = &on_drag_end {
                                h.call(true);
                            }
                        }
                    }
                    return;
                }

                if matches!(key, Key::Escape) {
                    evt.prevent_default();
                    dnd.cancel();
                    dnd.announce((strings.cancelled)());
                    if let Some(h) = &on_drag_end {
                        h.call(false);
                    }
                }
            },
            ..attributes,
            // Pointer-capture SUBSTITUTE, rendered only when native capture
            // did not engage. With capture (the `web` feature), events
            // retarget to this element already - and the layer must not
            // exist, so the page's own hit-testing (`elementFromPoint`
            // introspection included) stays untouched. Without capture
            // (desktop webviews, web without the feature) nothing
            // retargets: the moment the cursor left this element mid-drag
            // the move stream died and the ghost froze. This full-viewport
            // child then owns every pointer event and lets it bubble to
            // the handlers above - no separate handlers, no renderer API.
            // Gated on the shared context too, so a drag completed from
            // outside this element (host-driven drop, another window's
            // delivery) can never leave a stale layer eating input.
            // (Being position: fixed, it is clipped by any transformed
            // ancestor - the standard containing-block caveat, shared with
            // the overlay.)
            if matches!(phase(), GesturePhase::Dragging { .. }) && dnd.dragging() && !captured() {
                div {
                    style: "position: fixed; inset: 0; z-index: 9998; touch-action: none;",
                    aria_hidden: true,
                }
            }
            // Armed only while a touch press waits under `Auto`; the alarm
            // promotes exactly like a threshold crossing, at the origin.
            if let Some(pid) = hold_pid() {
                HoldTimer {
                    pointer_id: pid,
                    on_hold: move |pid| {
                        if let GestureEffect::Begin { at, .. } =
                            step(GestureEvent::Hold { pointer_id: pid }, threshold)
                        {
                            begin_drag.call(at);
                        }
                    },
                }
            }
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Horizontal arrows mirror under RTL: "descend into" is always the
    /// arrow pointing along reading order. Vertical arrows never mirror.
    #[test]
    fn nav_keys_mirror_under_rtl() {
        for dir in [Direction::Ltr, Direction::Rtl] {
            assert_eq!(nav_key(&Key::ArrowDown, dir), Some(NavKey::Next));
            assert_eq!(nav_key(&Key::ArrowUp, dir), Some(NavKey::Prev));
            assert_eq!(nav_key(&Key::Enter, dir), None);
        }
        assert_eq!(
            nav_key(&Key::ArrowRight, Direction::Ltr),
            Some(NavKey::Descend)
        );
        assert_eq!(
            nav_key(&Key::ArrowLeft, Direction::Ltr),
            Some(NavKey::Ascend)
        );
        assert_eq!(
            nav_key(&Key::ArrowRight, Direction::Rtl),
            Some(NavKey::Ascend)
        );
        assert_eq!(
            nav_key(&Key::ArrowLeft, Direction::Rtl),
            Some(NavKey::Descend)
        );
    }

    #[test]
    fn keyboard_drop_points_use_zone_center_and_element_offset() {
        let rect = Rect::new(40.0, 80.0, 200.0, 100.0);
        let (client, element) = keyboard_drop_points(Some(rect));

        assert_eq!(client, Point::new(140.0, 130.0));
        assert_eq!(element, Point::new(100.0, 50.0));
    }

    #[test]
    fn keyboard_drop_points_fall_back_to_origin_without_rect() {
        let (client, element) = keyboard_drop_points(None);

        assert_eq!(client, Point::default());
        assert_eq!(element, Point::default());
    }
}

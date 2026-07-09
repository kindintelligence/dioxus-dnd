//! Ready-made components over the shared drag context.
//!
//! ```text
//! rsx! {
//!     DndProvider::<Card> {
//!         Draggable::<Card> { payload: card.clone(), "Drag me" }
//!         DropZone::<Card> {
//!             on_drop: move |outcome: DropOutcome<Card>| { /* ... */ },
//!             "Drop here"
//!         }
//!     }
//! }
//! ```

use dioxus::html::MountedData;
use dioxus::prelude::*;

use std::rc::Rc;

use super::hooks::{
    use_bridge_world, use_dnd, use_dnd_provider, use_zone_id, use_zone_registry, SettleFlag,
};
use super::registry::{ZoneRecord, ZoneRegistry};
use super::state::DndContext;
use super::strings::use_dnd_strings;
use super::{platform, transition_with, GestureEffect, GestureEvent, GesturePhase, Promotion};

/// Context marker a `DropZone` provides so zones nested inside it can
/// discover their parent - powering hierarchical keyboard traversal with no
/// configuration.
#[derive(Clone, Copy, PartialEq)]
pub struct ParentZone(pub ZoneId);

/// Internal: which hierarchical move an arrow key requested.
#[derive(Debug, Clone, Copy, PartialEq)]
enum NavKey {
    Next,
    Prev,
    Descend,
    Ascend,
}
use super::types::{
    edge_of, effective_effect, Direction, DragMode, DropEffect, DropOutcome, EdgeSet, Point, Rect,
    TouchSense, ZoneId,
};

/// How long a touch must stay put before [`TouchSense::Auto`] promotes the
/// press to a drag - the familiar mobile long-press beat (dnd-kit and iOS
/// both sit around this value).
pub(crate) const HOLD_DELAY_MS: f64 = 250.0;

/// The functional inline style for a drag surface under each touch policy.
/// `Auto` also pins down selection: a long-press that starts selecting text
/// (or popping the iOS callout) would eat the hold.
pub(crate) fn touch_style(touch: TouchSense) -> &'static str {
    match touch {
        // `pinch-zoom` stays allowed: two fingers were never a drag, and
        // zooming is an accessibility floor.
        TouchSense::Auto => {
            "touch-action: pan-y pinch-zoom; user-select: none; \
             -webkit-user-select: none; -webkit-touch-callout: none;"
        }
        TouchSense::Immediate => {
            "touch-action: none; user-select: none; -webkit-user-select: none; \
             -webkit-touch-callout: none;"
        }
    }
}

/// The long-press clock for [`TouchSense::Auto`], with no timer dependency:
/// a zero-size element runs a no-op CSS animation for the hold duration and
/// `animationend` is the alarm. Mounting arms it, unmounting (the gesture
/// resolved some other way) cancels it - the element's lifecycle IS the
/// timer's, so a stale callback can't outlive its press. Works on any
/// renderer with CSS animations; where there are none, `Auto` quietly loses
/// only its long-press path (sideways pulls still drag).
#[component]
pub(crate) fn HoldTimer(pointer_id: i32, on_hold: EventHandler<i32>) -> Element {
    rsx! {
        // The inline `display: none` matters: dioxus-web renders a bare
        // `style {}` element visibly, so without it the keyframes rule
        // flashes as page text on every press (same guard as
        // `a11y::use_reduced_motion_css`).
        style { style: "display: none;",
            "@keyframes dnd-hold-timer {{ from {{ opacity: 0.99; }} to {{ opacity: 1; }} }}"
        }
        div {
            style: "position: absolute; width: 0; height: 0; overflow: hidden; \
                    animation: dnd-hold-timer {HOLD_DELAY_MS}ms linear forwards;",
            aria_hidden: true,
            onanimationend: move |_| on_hold.call(pointer_id),
        }
    }
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

/// Pull a user-provided `style` out of forwarded attributes and append it to
/// a functional inline style. Spread attributes land after static ones and
/// replace them wholesale, so without this a caller passing any `style`
/// would silently delete functional CSS (`touch-action`, overlay
/// positioning). The user's declarations come last, so they still win on a
/// per-property basis.
pub(crate) fn merge_style(attributes: &mut Vec<Attribute>, functional: &str) -> String {
    let user = attributes
        .iter()
        .position(|a| a.name == "style")
        .map(|i| attributes.remove(i));
    match user.map(|a| a.value) {
        Some(dioxus::core::AttributeValue::Text(s)) => format!("{functional} {s}"),
        _ => functional.to_string(),
    }
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

/// Provides a `DndContext<T>` to its children.
#[component]
pub fn DndProvider<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
    /// Layout direction: `Direction::Rtl` mirrors keyboard navigation and
    /// spatial zone ordering to follow the visual right-to-left flow.
    #[props(default)]
    dir: Direction,
    children: Element,
) -> Element {
    let _ = phantom;
    use_dnd_provider::<T>();
    // Synced every render (a compare-and-set no-op when unchanged), so a
    // live direction switch propagates.
    use_zone_registry::<T>().set_direction(dir);
    rsx! {
        {children}
    }
}

fn pointer_client(evt: &PointerEvent) -> Point {
    let c = evt.client_coordinates();
    Point::new(c.x, c.y)
}

/// Deliver the in-flight payload to `target`: acceptance check, settle
/// routing, outcome construction, the zone's callback. THE drop path - the
/// `Draggable` pointer gesture and [`crate::test::DragSim`] both end here,
/// so headless tests exercise exactly what production drops run.
pub(crate) fn deliver_drop<T: Clone + PartialEq + 'static>(
    registry: ZoneRegistry<T>,
    dnd: &mut DndContext<T>,
    settle_flag: Option<SettleFlag<T>>,
    target: ZoneId,
    point: Point,
    effect: DropEffect,
) -> bool {
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
    let mode = dnd.mode();
    let grab = dnd.grab();
    // A settle-enabled overlay glides the ghost into the target zone:
    // route the drop through the settling take so the payload stays
    // readable while it animates. Pointer drops only - a keyboard drag
    // renders no positioned ghost to glide.
    let settle_to = match settle_flag {
        Some(f) if mode == DragMode::Pointer && *f.armed.peek() => *record.rect.peek(),
        _ => None,
    };
    let taken = match settle_to {
        Some(to) => dnd.take_settling(to),
        None => dnd.take(),
    };
    if let Some((p, from)) = taken {
        record.on_drop.call(DropOutcome {
            payload: p,
            from,
            to: target,
            effect,
            mode,
            client: point,
            element: point - origin,
            grab,
            // The receiving zone fills this in when it opted in.
            edge: None,
        });
        return true;
    }
    false
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
        registry.refresh_rects();
        if let Some(h) = &on_drag_start {
            h.call(());
        }
    });

    let mut deliver_to = move |target: ZoneId, point: Point, effect: DropEffect| -> bool {
        deliver_drop(registry, &mut dnd, settle_flag, target, point, effect)
    };

    let mut finish_drop = move |point: Point| {
        let effect = effective_effect(effect, *mods.peek());
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
                if disabled || !evt.is_primary() {
                    return;
                }
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
                if let Some(n) = node.peek().clone() {
                    platform::capture_pointer(&n, evt.pointer_id());
                }
                let o = evt.element_coordinates();
                press_offset.set(Point::new(o.x, o.y));
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
                    GestureEffect::Begin { at, .. } => begin_drag.call(at),
                    GestureEffect::Track { at } => {
                        dnd.update_pointer(at);
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
                            let (client, element) = keyboard_drop_points(*record.rect.peek());
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

/// A region that accepts drags carrying `T`.
///
/// Handles the HTML5 boilerplate for you: `preventDefault` on dragover,
/// enter/leave depth counting (so child elements don't cause hover flicker),
/// and acceptance filtering.
///
/// Styling hooks: while an acceptable drag is in flight anywhere, the div
/// carries `data-active="true"` (reveal your drop targets); while that drag
/// hovers *this* zone it also carries `data-over="true"` (highlight it).
/// Both are absent otherwise, so presence-based selectors (CSS
/// `[data-over]`, Tailwind `data-over:ring-2`) work directly. Driven by the
/// shared context, so they light up for pointer, touch and keyboard drags
/// alike.
///
/// Opting into `edge` adds the closest-edge signal for insertion
/// indicators: while an acceptable *pointer* drag hovers this zone, the div
/// also carries `data-edge="top" | "right" | "bottom" | "left"` (the zone
/// edge nearest the pointer, live on every move - see [`edge_of`]), and the
/// delivered [`DropOutcome::edge`] records it at release. Style it with
/// value selectors, e.g. Tailwind
/// `data-[edge=top]:shadow-[0_-2px_0_0_currentColor]`.
#[component]
pub fn DropZone<T: Clone + PartialEq + 'static>(
    /// Stable identity for this zone. Auto-generated if omitted.
    #[props(default)]
    id: Option<ZoneId>,
    /// Human label for screen-reader announcements ("Over {label}").
    #[props(default)]
    label: Option<String>,
    /// Return `false` to reject a payload (zone won't highlight or accept it).
    #[props(default)]
    accepts: Option<Callback<T, bool>>,
    /// Track the zone edge nearest the pointer: `EdgeSet::Vertical` for
    /// top/bottom (a vertical stack), `EdgeSet::Horizontal` for left/right,
    /// `EdgeSet::All` for all four. Renders `data-edge` while hovered and
    /// fills [`DropOutcome::edge`]. Off (absent, `None`) by default.
    #[props(default)]
    edge: Option<EdgeSet>,
    /// Fired on a successful drop.
    on_drop: EventHandler<DropOutcome<T>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let dnd = use_dnd::<T>();
    let mut registry = use_zone_registry::<T>();
    let auto_id = use_zone_id();
    let zone_id = id.unwrap_or(auto_id);
    // Nesting is automatic: a DropZone inside another discovers its parent
    // via context, and provides itself to zones deeper down.
    let parent = try_use_context::<ParentZone>().map(|p| p.0);
    use_context_provider(|| ParentZone(zone_id));
    let mounted = use_signal(|| None::<Rc<MountedData>>);
    let rect = use_signal(|| None::<super::types::Rect>);

    // Register with the zone registry so keyboard navigation and pointer
    // hit-testing can find this zone. Callbacks are stable handles, so
    // registering once per mount is enough.
    use_hook(|| {
        registry.register(ZoneRecord {
            id: zone_id,
            parent,
            label: label.clone(),
            // The zone (not the drag source) owns the edge signal: it knows
            // its own rect and whether it opted in, so it enriches the
            // outcome on the way to the app's handler.
            on_drop: Callback::new(move |mut o: DropOutcome<T>| {
                if let Some(set) = edge {
                    if o.mode == DragMode::Pointer {
                        if let Some(r) = *rect.peek() {
                            o.edge = Some(edge_of(o.client, r, set));
                        }
                    }
                }
                on_drop.call(o)
            }),
            accepts,
            mounted,
            rect,
        });
    });
    use_drop(move || {
        registry.unregister(zone_id);
    });
    // Keep the registered label in sync if the prop changes across renders.
    // Registry readers only `peek`, so this render-time write can't loop.
    registry.sync_label(zone_id, label.clone());

    let acceptable = move || -> bool {
        match dnd.payload() {
            Some(p) => accepts.map(|cb| cb.call(p)).unwrap_or(true),
            None => false,
        }
    };
    // Live closest-edge readout while an acceptable pointer drag hovers.
    // Guards run cheapest-first, and the pointer signal is only read (so
    // this zone only re-renders per pointer move) once actually hovered
    // with the prop set.
    let live_edge = move || -> Option<&'static str> {
        let set = edge?;
        if dnd.over() != Some(zone_id) || dnd.mode() != DragMode::Pointer || !acceptable() {
            return None;
        }
        let r = (*rect.peek())?;
        Some(edge_of(dnd.pointer(), r, set).as_str())
    };

    rsx! {
        div {
            "data-active": if dnd.dragging() && acceptable() { "true" },
            "data-over": if dnd.over() == Some(zone_id) && acceptable() { "true" },
            "data-edge": live_edge(),
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                let mut mounted = mounted;
                let mut rect = rect;
                mounted.set(Some(m.clone()));
                // Measure immediately, not just at drag start: a zone that
                // mounts mid-drag (a virtualized list recycling rows under
                // the pointer) missed the pickup measurement, and the last
                // scroll ping ran before this row rendered. Hit-testing
                // must see the zone as soon as it exists.
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        rect.set(Some(Rect::new(
                            r.origin.x,
                            r.origin.y,
                            r.size.width,
                            r.size.height,
                        )));
                    }
                });
            },
            ..attributes,
            {children}
        }
    }
}

/// A drop target registered in two payload worlds at once - the bridge
/// between two coexisting providers (`DndProvider<A>` and `DndProvider<B>`).
///
/// Zone ids are process-global while registries are per-type, so one element
/// can hold the *same* `ZoneId` in both registries, sharing its
/// `mounted`/`rect` signals. Each world's machinery - hit-testing, `accepts`
/// filtering, keyboard navigation - then finds the zone independently, and
/// every drop arrives through its own typed callback: an `A` drag can only
/// reach `on_drop_a`, a `B` drag only `on_drop_b`. No downcasts, no shared
/// erased channel.
///
/// Reach for this only when two providers genuinely coexist (say, tickets
/// and teammates as separate features). If one drag world merely carries
/// several shapes, make the payload an enum and use a plain [`DropZone`].
/// For more than two worlds, generate a component for your exact type list
/// with [`crate::bridge_drop_zone!`] - or go lower-level and call
/// [`use_bridge_world`] once per world yourself.
///
/// Styling hooks match `DropZone`: `data-active="true"` while an acceptable
/// drag from *either* world is in flight, `data-over="true"` while one
/// hovers this zone.
#[component]
pub fn BridgeDropZone<A: Clone + PartialEq + 'static, B: Clone + PartialEq + 'static>(
    /// Stable identity for this zone, valid in both worlds. Auto-generated
    /// if omitted.
    #[props(default)]
    id: Option<ZoneId>,
    /// Human label for screen-reader announcements, used by both worlds.
    #[props(default)]
    label: Option<String>,
    /// Return `false` to reject a payload from the first world.
    #[props(default)]
    accepts_a: Option<Callback<A, bool>>,
    /// Return `false` to reject a payload from the second world.
    #[props(default)]
    accepts_b: Option<Callback<B, bool>>,
    /// Fired when a drag from the first world drops here.
    on_drop_a: EventHandler<DropOutcome<A>>,
    /// Fired when a drag from the second world drops here.
    on_drop_b: EventHandler<DropOutcome<B>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let auto_id = use_zone_id();
    let zone_id = id.unwrap_or(auto_id);
    let parent = try_use_context::<ParentZone>().map(|p| p.0);
    // One unambiguous parent id that resolves in both registries, so nested
    // zones of either type ascend correctly.
    use_context_provider(|| ParentZone(zone_id));
    let mounted = use_signal(|| None::<Rc<MountedData>>);
    let rect = use_signal(|| None::<super::types::Rect>);

    // One `use_bridge_world` per world: same id, same shared mounted/rect
    // signals, each drop through its own typed callback.
    let a = use_bridge_world::<A>(
        zone_id,
        parent,
        label.clone(),
        accepts_a,
        on_drop_a,
        mounted,
        rect,
    );
    let b = use_bridge_world::<B>(zone_id, parent, label, accepts_b, on_drop_b, mounted, rect);

    rsx! {
        div {
            "data-active": if a.active || b.active { "true" },
            "data-over": if a.over || b.over { "true" },
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                let mut mounted = mounted;
                let mut rect = rect;
                mounted.set(Some(m.clone()));
                // Same as DropZone: measure at mount so a bridge appearing
                // mid-drag is immediately hit-testable in both worlds (the
                // rect signal is shared, so one measurement serves both).
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        rect.set(Some(Rect::new(
                            r.origin.x,
                            r.origin.y,
                            r.size.width,
                            r.size.height,
                        )));
                    }
                });
            },
            ..attributes,
            {children}
        }
    }
}

/// Generate a bridge drop-zone component for **any number** of coexisting
/// payload worlds - [`BridgeDropZone`]'s recipe, packaged for N > 2 without
/// `dyn Any` (Rust has no variadic generics, so the component is generated
/// per concrete type list rather than parameterized over one).
///
/// Each `(Type, accepts_prop, on_drop_prop)` row becomes one world: an
/// optional `accepts_prop: Callback<Type, bool>` filter and a required
/// `on_drop_prop: EventHandler<DropOutcome<Type>>`. The generated component
/// also takes the shared `id`/`label` props, forwards extra attributes to
/// its div, and carries the same styling hooks as [`DropZone`]
/// (`data-active` / `data-over`, lit by whichever world's drag qualifies).
///
/// Requires `use dioxus::prelude::*;` in scope, and an ancestor
/// `DndProvider` for every listed type. Before reaching for three worlds,
/// consider whether one provider with an enum payload reads better.
///
/// ```text
/// use dioxus::prelude::*;
/// use dioxus_dnd::prelude::*;
///
/// dioxus_dnd::bridge_drop_zone!(pub StandupZone {
///     (Ticket, accepts_ticket, on_drop_ticket),
///     (Person, accepts_person, on_drop_person),
///     (Alert, accepts_alert, on_drop_alert),
/// });
///
/// rsx! {
///     StandupZone {
///         label: "agenda",
///         accepts_ticket: move |t: Ticket| !t.done,
///         on_drop_ticket: move |o: DropOutcome<Ticket>| { /* … */ },
///         on_drop_person: move |o: DropOutcome<Person>| { /* … */ },
///         on_drop_alert: move |o: DropOutcome<Alert>| { /* … */ },
///         "standup agenda"
///     }
/// }
/// ```
#[macro_export]
macro_rules! bridge_drop_zone {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident {
            $( ($ty:ty, $accepts:ident, $on_drop:ident) ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[::dioxus::prelude::component]
        #[allow(non_snake_case)]
        $vis fn $name(
            /// Stable identity for this zone, valid in every world.
            /// Auto-generated if omitted.
            #[props(default)]
            id: ::std::option::Option<$crate::core::ZoneId>,
            /// Human label for screen-reader announcements, used by every
            /// world.
            #[props(default)]
            label: ::std::option::Option<::std::string::String>,
            $(
                #[props(default)]
                $accepts: ::std::option::Option<::dioxus::prelude::Callback<$ty, bool>>,
                $on_drop: ::dioxus::prelude::EventHandler<$crate::core::DropOutcome<$ty>>,
            )+
            #[props(extends = div, extends = GlobalAttributes)]
            attributes: ::std::vec::Vec<::dioxus::prelude::Attribute>,
            children: ::dioxus::prelude::Element,
        ) -> ::dioxus::prelude::Element {
            use ::dioxus::prelude::*;

            let auto_id = $crate::core::use_zone_id();
            let zone_id = id.unwrap_or(auto_id);
            let parent = try_use_context::<$crate::core::ParentZone>().map(|p| p.0);
            // One unambiguous parent id that resolves in every registry, so
            // nested zones of any listed type ascend correctly.
            use_context_provider(|| $crate::core::ParentZone(zone_id));
            let mounted = use_signal(|| ::std::option::Option::<
                ::std::rc::Rc<::dioxus::html::MountedData>,
            >::None);
            let rect = use_signal(|| ::std::option::Option::<$crate::core::Rect>::None);

            let mut active = false;
            let mut over = false;
            $(
                let world = $crate::core::use_bridge_world::<$ty>(
                    zone_id,
                    parent,
                    label.clone(),
                    $accepts,
                    $on_drop,
                    mounted,
                    rect,
                );
                active |= world.active;
                over |= world.over;
            )+

            rsx! {
                div {
                    "data-active": if active { "true" },
                    "data-over": if over { "true" },
                    onmounted: move |evt: Event<::dioxus::html::MountedData>| {
                        let m = evt.data();
                        let mut mounted = mounted;
                        let mut rect = rect;
                        mounted.set(Some(m.clone()));
                        // Same as DropZone: measure at mount so a bridge
                        // appearing mid-drag is immediately hit-testable in
                        // every world (the rect signal is shared, so one
                        // measurement serves all).
                        spawn(async move {
                            if let Ok(r) = m.get_client_rect().await {
                                rect.set(Some($crate::core::Rect::new(
                                    r.origin.x,
                                    r.origin.y,
                                    r.size.width,
                                    r.size.height,
                                )));
                            }
                        });
                    },
                    ..attributes,
                    {children}
                }
            }
        }
    };
}

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
            // so reset now (guarded no-op otherwise).
            dnd.finish_settle();
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

    // Size-matched ghost: the grabbed element's measured rect, border-box
    // so the ghost's own padding/border stay inside it.
    let size = match_source
        .then(|| dnd.source_rect())
        .flatten()
        .map(|r| {
            format!(
                " width: {}px; height: {}px; box-sizing: border-box;",
                r.width, r.height
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
            overlay_style(dnd.pointer() - dnd.grab()),
        )
    } else {
        format!("{}{size}", overlay_style(dnd.pointer() - dnd.grab()))
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

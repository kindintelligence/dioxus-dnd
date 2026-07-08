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

use super::hooks::{use_dnd, use_dnd_provider, use_zone_id, use_zone_registry};
use super::registry::ZoneRecord;
use super::{platform, transition, GestureEffect, GestureEvent, GesturePhase};

/// Context marker a `DropZone` provides so zones nested inside it can
/// discover their parent - powering hierarchical keyboard traversal with no
/// configuration.
#[derive(Clone, Copy, PartialEq)]
pub struct ParentZone(pub ZoneId);

/// Internal: which hierarchical move an arrow key requested.
#[derive(Clone, Copy)]
enum NavKey {
    Next,
    Prev,
    Descend,
    Ascend,
}
use super::types::{effective_effect, DragMode, DropEffect, DropOutcome, Point, Rect, ZoneId};

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
    children: Element,
) -> Element {
    let _ = phantom;
    use_dnd_provider::<T>();
    rsx! {
        {children}
    }
}

fn pointer_client(evt: &PointerEvent) -> Point {
    let c = evt.client_coordinates();
    Point::new(c.x, c.y)
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
    // Separate clones for the two closures that need the payload.
    let kb_payload = payload.clone();
    let pointer_payload = payload.clone();
    let kb_label = label.clone();
    // Comparing against the context payload (rather than a local flag) means
    // the attribute is also correct when a custom source started the drag.
    let attr_payload = payload.clone();
    let mut phase = use_signal(|| GesturePhase::Idle);
    let mut step = move |event: GestureEvent, threshold: f64| -> GestureEffect {
        let (next, fx) = transition(*phase.peek(), event, threshold);
        phase.set(next);
        fx
    };
    let mut node = use_signal(|| None::<Rc<MountedData>>);
    let mut press_offset = use_signal(Point::default);
    let mut mods = use_signal(Modifiers::empty);
    let mut attributes = attributes;
    let style = merge_style(&mut attributes, "touch-action: none;");

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
            onmounted: move |evt: Event<MountedData>| node.set(Some(evt.data())),
            onpointerdown: move |evt: PointerEvent| {
                if disabled || !evt.is_primary() {
                    return;
                }
                evt.stop_propagation();
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
                            pointer_payload.clone(),
                            zone,
                            at,
                            *press_offset.peek(),
                            effect,
                            DragMode::Pointer,
                        );
                        registry.refresh_rects();
                        if let Some(h) = &on_drag_start {
                            h.call(());
                        }
                    }
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
                    let name = kb_label.clone().unwrap_or_else(|| "item".to_string());
                    dnd.announce(format!(
                        "Picked up {name}. Use arrow keys to choose a drop target, Enter to drop, Escape to cancel."
                    ));
                    if let Some(h) = &on_drag_start {
                        h.call(());
                    }
                    return;
                }

                if !kb_drag {
                    return;
                }

                // Hierarchical navigation (WAI-ARIA tree convention):
                // Up/Down cycle siblings at the current level; Right
                // descends into the hovered zone's children; Left ascends
                // to its parent. In flat apps (no nesting) Right/Left fall
                // back to next/previous, preserving the simple behavior.
                let nav = match key {
                    Key::ArrowDown => Some(NavKey::Next),
                    Key::ArrowUp => Some(NavKey::Prev),
                    Key::ArrowRight => Some(NavKey::Descend),
                    Key::ArrowLeft => Some(NavKey::Ascend),
                    _ => None,
                };
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
                            .unwrap_or_else(|| format!("zone {}", next.0));
                        let inside = record
                            .as_ref()
                            .and_then(|z| z.parent)
                            .and_then(|pid| registry.get(pid))
                            .and_then(|pz| pz.label);
                        match inside {
                            Some(parent) => dnd.announce(format!("Over {name}, inside {parent}.")),
                            None => dnd.announce(format!("Over {name}.")),
                        }
                    } else {
                        dnd.announce("No drop targets available.");
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
                        dnd.announce("No drop target selected.");
                        return;
                    };
                    if let Some(record) = registry.get(target) {
                        if let Some((p, from)) = dnd.take() {
                            let (client, element) = keyboard_drop_points(*record.rect.peek());
                            record.on_drop.call(DropOutcome {
                                payload: p,
                                from,
                                to: target,
                                effect,
                                mode: DragMode::Keyboard,
                                client,
                                element,
                                grab: Point::default(),
                            });
                            let name = record
                                .label
                                .unwrap_or_else(|| format!("zone {}", target.0));
                            dnd.announce(format!("Dropped in {name}."));
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
                    dnd.announce("Drag cancelled.");
                    if let Some(h) = &on_drag_end {
                        h.call(false);
                    }
                }
            },
            ..attributes,
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
            on_drop: Callback::new(move |o| on_drop.call(o)),
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

    rsx! {
        div {
            "data-active": if dnd.dragging() && acceptable() { "true" },
            "data-over": if dnd.over() == Some(zone_id) && acceptable() { "true" },
            onmounted: move |evt: Event<MountedData>| {
                let mut mounted = mounted;
                mounted.set(Some(evt.data()));
            },
            ..attributes,
            {children}
        }
    }
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
/// Note: the ghost follows the shared context's pointer position, which
/// pointer drags update on every move. Keyboard drags carry no pointer, so
/// during one the ghost sits at the viewport origin - check `dnd.mode()`
/// and skip rendering it if that matters to you.
#[component]
pub fn DragOverlay<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let _ = phantom;
    let dnd = use_dnd::<T>();
    if !dnd.dragging() {
        return rsx! {};
    }
    let mut attributes = attributes;
    let style = merge_style(&mut attributes, &overlay_style(dnd.pointer() - dnd.grab()));
    rsx! {
        div {
            style: style,
            ..attributes,
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

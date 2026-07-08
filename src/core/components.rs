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

use super::hooks::{use_dnd, use_dnd_provider, use_zone_id, use_zone_registry, SettleFlag};
use super::registry::ZoneRecord;
use super::{platform, transition, GestureEffect, GestureEvent, GesturePhase};

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
    ZoneId,
};

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
    let settle_flag = try_use_context::<SettleFlag<T>>();
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

                                edge: None,                            });
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
                let mut mounted = mounted;
                mounted.set(Some(evt.data()));
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
/// For more than two worlds, register a shared id yourself with
/// `use_zone_registry` - this component is that recipe, packaged for the
/// common pair.
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
    let dnd_a = use_dnd::<A>();
    let dnd_b = use_dnd::<B>();
    let mut reg_a = use_zone_registry::<A>();
    let mut reg_b = use_zone_registry::<B>();
    let auto_id = use_zone_id();
    let zone_id = id.unwrap_or(auto_id);
    let parent = try_use_context::<ParentZone>().map(|p| p.0);
    // One unambiguous parent id that resolves in both registries, so nested
    // zones of either type ascend correctly.
    use_context_provider(|| ParentZone(zone_id));
    let mounted = use_signal(|| None::<Rc<MountedData>>);
    let rect = use_signal(|| None::<super::types::Rect>);

    // Signals are Copy handles, so both records genuinely share one
    // mounted/rect pair: either world's refresh_rects() re-measures the one
    // rectangle both registries see.
    use_hook(|| {
        reg_a.register(ZoneRecord {
            id: zone_id,
            parent,
            label: label.clone(),
            on_drop: Callback::new(move |o| on_drop_a.call(o)),
            accepts: accepts_a,
            mounted,
            rect,
        });
        reg_b.register(ZoneRecord {
            id: zone_id,
            parent,
            label: label.clone(),
            on_drop: Callback::new(move |o| on_drop_b.call(o)),
            accepts: accepts_b,
            mounted,
            rect,
        });
    });
    use_drop(move || {
        reg_a.unregister(zone_id);
        reg_b.unregister(zone_id);
    });
    reg_a.sync_label(zone_id, label.clone());
    reg_b.sync_label(zone_id, label);

    let acceptable_a = move || -> bool {
        match dnd_a.payload() {
            Some(p) => accepts_a.map(|cb| cb.call(p)).unwrap_or(true),
            None => false,
        }
    };
    let acceptable_b = move || -> bool {
        match dnd_b.payload() {
            Some(p) => accepts_b.map(|cb| cb.call(p)).unwrap_or(true),
            None => false,
        }
    };

    rsx! {
        div {
            "data-active": if (dnd_a.dragging() && acceptable_a()) || (dnd_b.dragging() && acceptable_b()) { "true" },
            "data-over": if (dnd_a.over() == Some(zone_id) && acceptable_a())
                || (dnd_b.over() == Some(zone_id) && acceptable_b()) { "true" },
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

    // Measure & play (FLIP, like FlipItem): the settled frame commits at
    // the release position with the transition armed; this effect then
    // measures the ghost and releases the transform, so the browser glides
    // its center onto the zone's center.
    use_effect(move || {
        match dnd.settling() {
            Some(to) if settle => {
                if glide.peek().is_some() {
                    return;
                }
                let Some(m) = node.peek().clone() else {
                    // Never mounted (e.g. keyboard-only ghost skipped) -
                    // nothing to animate.
                    dnd.finish_settle();
                    return;
                };
                spawn(async move {
                    let Ok(r) = m.get_client_rect().await else {
                        dnd.finish_settle();
                        return;
                    };
                    let from = Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height);
                    let d = to.center() - from.center();
                    // A sub-pixel glide would produce no transition (and
                    // thus no transitionend) - finish immediately.
                    if d.x.abs() < 1.0 && d.y.abs() < 1.0 {
                        dnd.finish_settle();
                    } else {
                        glide.set(Some(d));
                    }
                });
            }
            _ => {
                if glide.peek().is_some() {
                    glide.set(None);
                }
            }
        }
    });

    let settling = settle && dnd.settling().is_some();
    if !dnd.dragging() && !settling {
        return rsx! {};
    }

    let functional = if settling {
        let transform = match glide() {
            Some(d) => format!("translate({}px, {}px)", d.x, d.y),
            None => "none".to_string(),
        };
        format!(
            "{} transform: {transform}; transition: transform {duration}ms {easing};",
            overlay_style(dnd.pointer() - dnd.grab()),
        )
    } else {
        overlay_style(dnd.pointer() - dnd.grab())
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
                // finish_settle is a guarded no-op against stray bubbles.
                if settling && glide.peek().is_some() {
                    dnd.finish_settle();
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

//! Drop targets: [`DropZone`], the two-world [`BridgeDropZone`], and the
//! N-world [`crate::bridge_drop_zone!`] macro, plus the [`ParentZone`]
//! context marker nested zones discover their parent through.

use dioxus::html::MountedData;
use dioxus::prelude::*;

use std::rc::Rc;

use crate::core::hooks::{
    use_bridge_world, use_dnd, use_zone_id, use_zone_registry, BridgeGeometry,
};
use crate::core::registry::{ZonePolicy, ZoneRecord};
use crate::core::types::{edge_of, DragMode, DropOutcome, EdgeSet, Rect, ZoneId};
use crate::core::world::use_joined_window;
use crate::core::{DropEffects, DropQuery};

/// Context marker a `DropZone` provides so zones nested inside it can
/// discover their parent - powering hierarchical keyboard traversal with no
/// configuration.
#[derive(Clone, Copy, PartialEq)]
pub struct ParentZone(pub ZoneId);

#[derive(Clone, Copy, PartialEq)]
struct LiveParentZone(Signal<ZoneId>);

/// Read the nearest parent zone, including a bridge whose identity can change.
///
/// Public only for `bridge_drop_zone!` expansions in downstream crates.
#[doc(hidden)]
pub fn use_parent_zone() -> Option<ZoneId> {
    let live = try_use_context::<LiveParentZone>();
    let fixed = try_use_context::<ParentZone>();
    live.map(|parent| *parent.0.read())
        .or_else(|| fixed.map(|parent| parent.0))
}

/// Keyed context boundary used by the exported bridge macro.
///
/// This is public only because macro expansion happens in downstream crates.
#[doc(hidden)]
#[component]
pub fn BridgeParentZoneBoundary(zone_id: ZoneId, children: Element) -> Element {
    let mut live = use_signal(|| zone_id);
    use_effect(use_reactive!(|(zone_id)| {
        if *live.peek() != zone_id {
            live.set(zone_id);
        }
    }));
    provide_context(LiveParentZone(live));
    provide_context(ParentZone(zone_id));
    rsx! { {children} }
}

#[component]
fn FixedParentZoneBoundary(zone_id: ZoneId, children: Element) -> Element {
    provide_context(ParentZone(zone_id));
    rsx! { {children} }
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
///
/// Overlap precedence follows registry order, not browser paint order: among
/// overlapping acceptable zones, the later record receives the drop. CSS
/// `z-index`, stacking contexts, and portals are not inspected. Keep registry
/// and visual order aligned when targets overlap, or avoid the overlap; a
/// rejecting later record is skipped at release. Replacing a same-id record
/// retains its slot.
#[component]
pub fn DropZone<T: Clone + PartialEq + 'static>(
    /// Stable identity for this zone. Auto-generated if omitted.
    #[props(default)]
    id: Option<ZoneId>,
    /// Human label for screen-reader announcements ("Over {label}").
    #[props(default)]
    label: Option<String>,
    /// Return `false` to reject a payload (zone won't highlight or accept it).
    /// Keep this predicate cheap. Registry queries snapshot their candidates
    /// before invoking application callbacks, so reentrant registry work does
    /// not collide with a live signal borrow.
    #[props(default)]
    accepts: Option<Callback<T, bool>>,
    /// Rich acceptance predicate with source, effect, input mode, pointer
    /// kind, and drag identity.
    #[props(default)]
    accepts_query: Option<Callback<DropQuery<T>, bool>>,
    /// Effects this zone supports. Defaults to all effects for 3.x
    /// compatibility; prefer `DropEffects::STANDARD` for new zones.
    #[props(default)]
    allowed_effects: DropEffects,
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
    let auto_id = use_zone_id();
    let zone_id = id.unwrap_or(auto_id);
    let parent = use_parent_zone();
    rsx! {
        for (keyed_zone_id, keyed_parent) in [(zone_id, parent)] {
            DropZoneInstance::<T> {
                key: "{keyed_zone_id.0}:{keyed_parent:?}",
                zone_id: keyed_zone_id,
                parent: keyed_parent,
                provide_parent: true,
                label: label.clone(),
                accepts,
                accepts_query,
                allowed_effects,
                edge,
                on_drop,
                attributes: attributes.clone(),
                {children.clone()}
            }
        }
    }
}

#[component]
fn DropZoneInstance<T: Clone + PartialEq + 'static>(
    zone_id: ZoneId,
    parent: Option<ZoneId>,
    provide_parent: bool,
    label: Option<String>,
    accepts: Option<Callback<T, bool>>,
    accepts_query: Option<Callback<DropQuery<T>, bool>>,
    allowed_effects: DropEffects,
    edge: Option<EdgeSet>,
    on_drop: EventHandler<DropOutcome<T>>,
    attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let dnd = use_dnd::<T>();
    let joined = use_joined_window::<T>();
    let mut registry = use_zone_registry::<T>();
    // Nesting is automatic: a DropZone inside another discovers its parent
    // via context, and provides itself to zones deeper down.
    // Register with the zone registry so keyboard navigation and pointer
    // hit-testing can find this zone. Callbacks are stable handles, so
    // registering once per mount is enough.
    let registered_label = label.clone();
    let registration = use_hook(|| {
        registry.register_with_policy(
            ZoneRecord {
                id: zone_id,
                parent,
                label: registered_label.clone(),
                on_drop: Callback::new(move |outcome: DropOutcome<T>| on_drop.call(outcome)),
                accepts,
                mounted: None,
                rect: None,
            },
            ZonePolicy {
                accepts_query,
                allowed_effects,
                edge,
            },
        )
    });
    use_drop(move || {
        registry.unregister_registration(registration);
    });
    let label_for_sync = label.clone();
    use_effect(use_reactive!(|(label_for_sync)| {
        registry.sync_label(zone_id, label_for_sync);
    }));
    use_effect(use_reactive!(|(parent)| {
        registry.sync_parent(registration, parent);
    }));
    use_effect(use_reactive!(|(
        accepts,
        accepts_query,
        allowed_effects,
        edge,
    )| {
        registry.sync_policy(
            registration,
            accepts,
            ZonePolicy {
                accepts_query,
                allowed_effects,
                edge,
            },
        );
    }));

    let acceptable = move || -> bool {
        let Some(payload) = dnd.payload() else {
            return false;
        };
        let query = super::delivery::drop_query(&dnd, payload.clone(), dnd.proposed_effect());
        query.proposed_effect != crate::core::DropEffect::None
            && accepts.is_none_or(|callback| callback.call(payload.clone()))
            && accepts_query.is_none_or(|callback| callback.call(query.clone()))
            && allowed_effects.negotiate(query.proposed_effect).is_some()
    };
    let is_over = move || match joined {
        Some(joined) => joined.is_over(zone_id),
        None => dnd.over() == Some(zone_id),
    };
    // Live closest-edge readout while an acceptable pointer drag hovers.
    // Guards run cheapest-first, and the pointer signal is only read (so
    // this zone only re-renders per pointer move) once actually hovered
    // with the prop set.
    let live_edge = move || -> Option<&'static str> {
        let set = edge?;
        if !is_over() || dnd.mode() != DragMode::Pointer || !acceptable() {
            return None;
        }
        let r = registry.cached_rect(zone_id)?;
        let pointer = joined
            .and_then(|joined| joined.local_pointer())
            .unwrap_or_else(|| dnd.pointer());
        Some(edge_of(pointer, r, set).as_str())
    };
    let mut attributes = attributes;
    super::protect_attributes(
        &mut attributes,
        &["data-active", "data-over", "data-edge", "onmounted"],
    );

    let content = if provide_parent {
        rsx! {
            FixedParentZoneBoundary { zone_id, {children} }
        }
    } else {
        children
    };

    rsx! {
        div {
            "data-active": if dnd.dragging() && acceptable() { "true" },
            "data-over": if is_over() && acceptable() { "true" },
            "data-edge": live_edge(),
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                let mut registry = registry;
                registry.set_mounted(registration, m.clone());
                // Measure immediately, not just at drag start: a zone that
                // mounts mid-drag (a virtualized list recycling rows under
                // the pointer) missed the pickup measurement, and the last
                // scroll ping ran before this row rendered. Hit-testing
                // must see the zone as soon as it exists.
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        registry.set_rect_if_present(registration, Rect::new(
                            r.origin.x,
                            r.origin.y,
                            r.size.width,
                            r.size.height,
                        ));
                    }
                });
            },
            ..attributes,
            {content}
        }
    }
}

/// Internal flat target: registered like a `DropZone`, but it deliberately
/// does not become the hierarchical parent of the zones rendered inside it.
#[component]
pub(crate) fn FlatDropZone<T: Clone + PartialEq + 'static>(
    zone_id: ZoneId,
    #[props(default)] label: Option<String>,
    #[props(default)] accepts_query: Option<Callback<DropQuery<T>, bool>>,
    #[props(default)] edge: Option<EdgeSet>,
    on_drop: EventHandler<DropOutcome<T>>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let parent = use_parent_zone();
    rsx! {
        for (keyed_zone_id, keyed_parent) in [(zone_id, parent)] {
            DropZoneInstance::<T> {
                key: "{keyed_zone_id.0}:{keyed_parent:?}",
                zone_id: keyed_zone_id,
                parent: keyed_parent,
                provide_parent: false,
                label: label.clone(),
                accepts: None,
                accepts_query,
                allowed_effects: DropEffects::default(),
                edge,
                on_drop,
                attributes: attributes.clone(),
                {children.clone()}
            }
        }
    }
}

/// A drop target registered in two payload worlds at once - the bridge
/// between two coexisting providers (`DndProvider<A>` and `DndProvider<B>`).
///
/// Zone ids are process-global while registries are per-type, so one element
/// can hold the *same* `ZoneId` in both registries. The element fans its
/// mounted handle and each measurement into both provider-owned geometry
/// records. Each world's machinery - hit-testing, `accepts` filtering,
/// keyboard navigation - then finds the zone independently, and every drop
/// arrives through its own typed callback: an `A` drag can only reach
/// `on_drop_a`, a `B` drag only `on_drop_b`. No downcasts, no shared erased
/// channel.
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
    let parent = use_parent_zone();
    rsx! {
        for (keyed_zone_id, keyed_parent) in [(zone_id, parent)] {
            BridgeDropZoneInstance::<A, B> {
                key: "{keyed_zone_id.0}:{keyed_parent:?}",
                zone_id: keyed_zone_id,
                parent: keyed_parent,
                label: label.clone(),
                accepts_a,
                accepts_b,
                on_drop_a,
                on_drop_b,
                attributes: attributes.clone(),
                {children.clone()}
            }
        }
    }
}

#[component]
fn BridgeDropZoneInstance<A: Clone + PartialEq + 'static, B: Clone + PartialEq + 'static>(
    zone_id: ZoneId,
    parent: Option<ZoneId>,
    label: Option<String>,
    accepts_a: Option<Callback<A, bool>>,
    accepts_b: Option<Callback<B, bool>>,
    on_drop_a: EventHandler<DropOutcome<A>>,
    on_drop_b: EventHandler<DropOutcome<B>>,
    attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    // One unambiguous parent id that resolves in both registries, so nested
    // zones of either type ascend correctly.
    provide_context(ParentZone(zone_id));
    let geometry = use_hook(BridgeGeometry::default);
    // One `use_bridge_world` per world: same id and element, independent
    // provider-owned geometry, each drop through its own typed callback.
    let a = use_bridge_world::<A>(
        zone_id,
        parent,
        label.clone(),
        accepts_a,
        on_drop_a,
        geometry.clone(),
    );
    let b = use_bridge_world::<B>(
        zone_id,
        parent,
        label,
        accepts_b,
        on_drop_b,
        geometry.clone(),
    );
    let mut attributes = attributes;
    super::protect_attributes(&mut attributes, &["data-active", "data-over", "onmounted"]);

    rsx! {
        div {
            "data-active": if a.active || b.active { "true" },
            "data-over": if a.over || b.over { "true" },
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                geometry.set_mounted(&m);
                // Same as DropZone: measure at mount so a bridge appearing
                // mid-drag is immediately hit-testable in both worlds. One
                // DOM read fans out into both provider-owned registries.
                let geometry = geometry.clone();
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        let rect = Rect::new(
                            r.origin.x,
                            r.origin.y,
                            r.size.width,
                            r.size.height,
                        );
                        geometry.set_rect_if_present(rect);
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
            let parent = $crate::core::use_parent_zone();
            let geometry = use_hook($crate::core::BridgeGeometry::default);
            let mut attributes = attributes;
            attributes.retain(|attribute| {
                !matches!(attribute.name, "data-active" | "data-over" | "onmounted")
            });
            let mut active = false;
            let mut over = false;
            $(
                let world = $crate::core::use_bridge_world::<$ty>(
                    zone_id,
                    parent,
                    label.clone(),
                    $accepts,
                    $on_drop,
                    geometry.clone(),
                );
                active |= world.active;
                over |= world.over;
            )+

            rsx! {
                $crate::core::BridgeParentZoneBoundary {
                    key: "{zone_id.0}",
                    zone_id,
                    div {
                        "data-active": if active { "true" },
                        "data-over": if over { "true" },
                        onmounted: move |evt: Event<::dioxus::html::MountedData>| {
                            let m = evt.data();
                            geometry.set_mounted(&m);
                            // Same as DropZone: measure at mount so a bridge
                            // appearing mid-drag is immediately hit-testable in
                            // every world. One DOM read fans out into every
                            // provider-owned registry.
                            let geometry = geometry.clone();
                            spawn(async move {
                                if let Ok(r) = m.get_client_rect().await {
                                    let rect = $crate::core::Rect::new(
                                        r.origin.x,
                                        r.origin.y,
                                        r.size.width,
                                        r.size.height,
                                    );
                                    geometry.set_rect_if_present(rect);
                                }
                            });
                        },
                        ..attributes,
                        {children}
                    }
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::core::{DndProvider, DragMode, DropEffect, Point};

    #[component]
    fn ProposedEffectProbe() -> Element {
        let mut dnd = use_dnd::<u8>();
        use_hook(move || {
            dnd.start(
                7,
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                DragMode::Pointer,
            );
            dnd.set_proposed_effect(DropEffect::Copy);
        });
        rsx! {
            DropZone::<u8> {
                allowed_effects: DropEffects::COPY,
                accepts_query: move |query: DropQuery<u8>| {
                    query.proposed_effect == DropEffect::Copy
                },
                on_drop: move |_| {},
                "copy target"
            }
        }
    }

    fn proposed_effect_app() -> Element {
        rsx! {
            DndProvider::<u8> { ProposedEffectProbe {} }
        }
    }

    #[test]
    fn active_state_uses_the_live_proposed_effect() {
        let mut dom = VirtualDom::new(proposed_effect_app);
        dom.rebuild_in_place();
        let html = dioxus_ssr::render(&dom);
        assert!(
            html.contains(r#"data-active="true""#),
            "copy target stayed dark: {html}"
        );
    }

    #[derive(Clone, Props)]
    struct DynamicIdProps {
        phase: Rc<Cell<bool>>,
    }

    impl PartialEq for DynamicIdProps {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.phase, &other.phase)
        }
    }

    fn dynamic_id_app(props: DynamicIdProps) -> Element {
        let id = if props.phase.get() {
            ZoneId(2)
        } else {
            ZoneId(1)
        };
        rsx! {
            DndProvider::<u8> {
                DropZone::<u8> {
                    id,
                    on_drop: move |_| {},
                    DynamicIdProbe { expected: id }
                }
            }
        }
    }

    #[component]
    fn DynamicIdProbe(expected: ZoneId) -> Element {
        let registry = use_zone_registry::<u8>();
        let stale = if expected == ZoneId(1) {
            ZoneId(2)
        } else {
            ZoneId(1)
        };
        assert!(registry.contains(expected));
        assert!(!registry.contains(stale));
        rsx! { div {} }
    }

    #[test]
    fn changing_an_explicit_id_replaces_the_registered_instance() {
        let phase = Rc::new(Cell::new(false));
        let mut dom = VirtualDom::new_with_props(
            dynamic_id_app,
            DynamicIdProps {
                phase: phase.clone(),
            },
        );
        dom.rebuild_in_place();

        phase.set(true);
        dom.mark_dirty(ScopeId::APP);
        dom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
}

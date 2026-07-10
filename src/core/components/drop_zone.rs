//! Drop targets: [`DropZone`], the two-world [`BridgeDropZone`], and the
//! N-world [`crate::bridge_drop_zone!`] macro, plus the [`ParentZone`]
//! context marker nested zones discover their parent through.

use dioxus::html::MountedData;
use dioxus::prelude::*;

use std::rc::Rc;

use crate::core::hooks::{
    use_bridge_world, use_dnd, use_zone_id, use_zone_registry, BridgeGeometry,
};
use crate::core::registry::ZoneRecord;
use crate::core::types::{edge_of, DragMode, DropOutcome, EdgeSet, Rect, ZoneId};

/// Context marker a `DropZone` provides so zones nested inside it can
/// discover their parent - powering hierarchical keyboard traversal with no
/// configuration.
#[derive(Clone, Copy, PartialEq)]
pub struct ParentZone(pub ZoneId);

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
    // Register with the zone registry so keyboard navigation and pointer
    // hit-testing can find this zone. Callbacks are stable handles, so
    // registering once per mount is enough.
    let registration = use_hook(|| {
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
                        if let Some(r) = registry.cached_rect(zone_id) {
                            o.edge = Some(edge_of(o.client, r, set));
                        }
                    }
                }
                on_drop.call(o)
            }),
            accepts,
            mounted: None,
            rect: None,
        })
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
        let r = registry.cached_rect(zone_id)?;
        Some(edge_of(dnd.pointer(), r, set).as_str())
    };

    rsx! {
        div {
            "data-active": if dnd.dragging() && acceptable() { "true" },
            "data-over": if dnd.over() == Some(zone_id) && acceptable() { "true" },
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
            {children}
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
    let parent = try_use_context::<ParentZone>().map(|p| p.0);
    // One unambiguous parent id that resolves in both registries, so nested
    // zones of either type ascend correctly.
    use_context_provider(|| ParentZone(zone_id));
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
            let parent = try_use_context::<$crate::core::ParentZone>().map(|p| p.0);
            // One unambiguous parent id that resolves in every registry, so
            // nested zones of any listed type ascend correctly.
            use_context_provider(|| $crate::core::ParentZone(zone_id));
            let geometry = use_hook($crate::core::BridgeGeometry::default);
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
    };
}

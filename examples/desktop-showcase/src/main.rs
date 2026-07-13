//! Mission Control: the polished multi-window drag-and-drop showcase.
//!
//! Live signal-backed widgets that keep animating inside the drag ghost
//! while crossing between windows - the thing serialized drag-and-drop
//! cannot do. The payload is a [`model::Widget`]: a live `Signal` handle,
//! which no OS drag protocol could carry.
//!
//! Design: docs/superpowers/specs/2026-07-10-desktop-showcase-design.md

mod layout;
mod model;
mod theme;
mod ticker;
mod widgets;

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{window, Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

use layout::WindowRole;
use model::{Model, Satellite, Widget, DOCK};
use widgets::WidgetCard;

fn main() {
    dioxus::LaunchBuilder::new()
        .with_cfg(
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("dioxus-dnd - mission control")
                    .with_inner_size(LogicalSize::new(660.0, 560.0)),
            ),
        )
        .launch(mission_control);
}

fn mission_control() -> Element {
    let world = use_dnd_world::<Widget>();
    let model = use_dnd_model(Model::new);
    ticker::use_ticker(model.clone());
    use_demo_layout(model.clone(), WindowRole::MissionControl);

    let mut satellite_seq = use_signal(|| 0u32);
    let spawn_model = model.clone();
    let open_satellite = move |_| {
        let n = *satellite_seq.peek() + 1;
        satellite_seq.set(n);
        let satellite = spawn_model.new_satellite(n);
        let mut satellites = spawn_model.satellites;
        satellites.write().push(satellite);
        let dom = world
            .vdom(satellite_window)
            .with_root_context(spawn_model.clone())
            .with_root_context(satellite);
        window().new_window(
            dom,
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title(format!("dioxus-dnd - satellite {n}"))
                    .with_inner_size(LogicalSize::new(360.0, 560.0)),
            ),
        );
    };

    let joined = world.windows().len();
    let linked = match joined {
        1 => "1 window linked".to_string(),
        n => format!("{n} windows linked"),
    };
    rsx! {
        Chrome {
            header: rsx! {
                span { class: "brand", "MISSION CONTROL" }
                span { class: "status-pill", "{linked}" }
                button { class: "spawn", onclick: open_satellite, "Open satellite" }
            },
            WidgetZone {
                zone: DOCK,
                label: "Dock".to_string(),
                widgets: model.dock,
                empty_hint: "All widgets deployed".to_string(),
            }
        }
    }
}

fn satellite_window() -> Element {
    let model = use_context::<Model>();
    ticker::use_ticker(model.clone());
    let satellite = use_context::<Satellite>();
    use_demo_layout(model.clone(), WindowRole::Satellite(satellite.n));
    use_satellite_cleanup(model, satellite);
    rsx! {
        Chrome {
            header: rsx! {
                span { class: "brand", {format!("SATELLITE {:02}", satellite.n)} }
                span { class: "status-pill", "uplinked" }
            },
            WidgetZone {
                zone: satellite.zone,
                label: format!("Satellite {}", satellite.n),
                widgets: satellite.widgets,
                empty_hint: "Drop a live widget here".to_string(),
            }
        }
    }
}

/// Watch the shared layout epoch; each bump snaps THIS window to its demo
/// slot (only a window's own runtime may touch its tao handle).
fn use_demo_layout(model: Model, role: WindowRole) {
    let epoch = model.layout_epoch;
    use_effect(move || {
        if epoch() > 0 {
            layout::snap(role);
        }
    });
}

fn use_satellite_cleanup(model: Model, satellite: Satellite) {
    // Closing a satellite returns its widgets to the dock, retires its zone,
    // and reclaims its independently owned list signal; repeats are inert.
    use_drop(move || {
        let _ = model.close_satellite(satellite);
    });
}

/// Shared per-window shell: provider, styles, bridge, ghost overlay and the
/// accessibility live region.
#[component]
fn Chrome(header: Element, children: Element) -> Element {
    let model = use_context::<Model>();
    rsx! {
        style { {theme::STYLE} }
        MultiWindowProvider::<Widget> {
            div {
                class: "chrome",
                tabindex: "0",
                autofocus: true,
                onkeydown: move |event| {
                    // `D` anywhere: snap every window to the filming layout.
                    if event.key().to_string().eq_ignore_ascii_case("d") {
                        let mut epoch = model.layout_epoch;
                        let next = *epoch.peek() + 1;
                        epoch.set(next);
                    }
                },
                header { class: "chrome-head", {header} }
                {children}
            }
            DragOverlay::<Widget> { match_source: true, class: "ghost",
                GhostCard {}
            }
            LiveRegion::<Widget> {}
        }
    }
}

/// The ghost mirrors the in-flight widget - live, because the payload IS the
/// live signal handle, not a snapshot.
#[component]
fn GhostCard() -> Element {
    let dnd = use_dnd::<Widget>();
    match dnd.payload() {
        Some(widget) => rsx! { WidgetCard { widget } },
        None => rsx! {},
    }
}

/// One drop zone listing live widgets; both window kinds render this.
#[component]
fn WidgetZone(
    zone: ZoneId,
    label: String,
    widgets: Signal<Vec<Widget>>,
    empty_hint: String,
) -> Element {
    let model = use_context::<Model>();
    rsx! {
        DropZone::<Widget> {
            id: zone,
            label: label.clone(),
            class: "zone",
            on_drop: move |o: DropOutcome<Widget>| model.deliver(o.payload, o.to, o.effect),
            for widget in widgets() {
                Draggable::<Widget> {
                    key: "{widget.id}",
                    payload: widget,
                    zone,
                    label: widget.kind.title(),
                    class: "slot",
                    WidgetCard { widget }
                }
            }
            if widgets().is_empty() {
                p { class: "empty", {empty_hint.clone()} }
            }
        }
    }
}

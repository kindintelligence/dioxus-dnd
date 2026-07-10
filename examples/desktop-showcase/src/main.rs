//! Mission Control: the polished multi-window drag-and-drop showcase.
//!
//! Live signal-backed widgets that keep animating inside the drag ghost
//! while crossing between windows - the thing serialized drag-and-drop
//! cannot do. The payload is a [`model::Widget`]: a live `Signal` handle,
//! which no OS drag protocol could carry.
//!
//! Design: docs/superpowers/specs/2026-07-10-desktop-showcase-design.md

mod model;
mod ticker;
mod widgets;

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{window, Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus_dnd::desktop::{use_window_geometry_feed, DragBridge};
use dioxus_dnd::prelude::*;
use std::rc::Rc;

use model::{ModelOwner, Satellite, Widget, DOCK};
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
    use_window_geometry_feed();
    // Provided as context so `WidgetZone` (and satellites, via root context)
    // reach the owner without it being a component prop.
    let owner = use_context_provider(ModelOwner::new);
    ticker::use_ticker(owner.clone());
    let model = owner.model;

    let mut satellite_seq = use_signal(|| 0u32);
    let spawn_owner = owner.clone();
    let open_satellite = move |_| {
        let n = *satellite_seq.peek() + 1;
        satellite_seq.set(n);
        let satellite = spawn_owner.new_satellite(n);
        let mut satellites = model.satellites;
        satellites.write().push(satellite);
        let dom = VirtualDom::new(satellite_window)
            .with_root_context(world)
            .with_root_context(spawn_owner.clone())
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
    rsx! {
        Chrome {
            header: rsx! {
                span { class: "brand", "MISSION CONTROL" }
                span { class: "status-pill", "{joined} window(s) linked" }
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
    use_window_geometry_feed();
    let owner = use_context::<Rc<ModelOwner>>();
    ticker::use_ticker(owner.clone());
    let satellite = use_context::<Satellite>();
    use_satellite_cleanup(owner.clone(), satellite);
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

fn use_satellite_cleanup(owner: Rc<ModelOwner>, satellite: Satellite) {
    // Closing a satellite returns its widgets to the dock, retires its zone,
    // and reclaims its independently owned list signal; repeats are inert.
    use_drop(move || {
        let _ = owner.close_satellite(satellite);
    });
}

/// Shared per-window shell: provider, styles, bridge, ghost overlay and the
/// accessibility live region.
#[component]
fn Chrome(header: Element, children: Element) -> Element {
    rsx! {
        style { {theme::STYLE} }
        DndProvider::<Widget> {
            DragBridge::<Widget> {}
            div { class: "chrome",
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
    let owner = use_context::<Rc<ModelOwner>>();
    rsx! {
        DropZone::<Widget> {
            id: zone,
            label: label.clone(),
            class: "zone",
            on_drop: move |o: DropOutcome<Widget>| owner.deliver(o.payload, o.to, o.effect),
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

/// Placeholder styling so the spike is usable; Task 4 replaces this with the
/// full mission-control theme in `theme.rs`.
mod theme {
    pub const STYLE: &str = r#"
    * { box-sizing: border-box; }
    body { margin: 0; font-family: system-ui, sans-serif; background: #0b0e14; color: #d7e2ea; }
    .chrome { display: flex; flex-direction: column; gap: 12px; padding: 14px; height: 100vh; }
    .chrome-head { display: flex; align-items: center; gap: 10px; }
    .brand { font-size: 13px; letter-spacing: 0.12em; opacity: 0.9; }
    .status-pill { font-size: 11px; opacity: 0.6; margin-left: auto; }
    button.spawn { padding: 6px 12px; border: 1px solid #2c3644; background: #121826; color: #d7e2ea; border-radius: 6px; cursor: pointer; }
    .zone { flex: 1; display: grid; grid-template-columns: 1fr 1fr; gap: 10px; align-content: start; padding: 12px; border: 1px solid #1d2532; border-radius: 12px; background: #0e131d; overflow: auto; }
    .zone[data-over] { border-color: #3ddbd9; }
    .slot { cursor: grab; }
    .widget { border: 1px solid #273140; border-radius: 10px; background: #121826; padding: 10px; display: flex; flex-direction: column; gap: 6px; }
    .widget-head { display: flex; gap: 8px; align-items: center; font-size: 11px; letter-spacing: 0.08em; text-transform: uppercase; opacity: 0.75; }
    .widget-dot { width: 7px; height: 7px; border-radius: 50%; background: currentColor; }
    .widget-body { display: flex; flex-direction: column; gap: 4px; color: #3ddbd9; }
    .widget[data-kind="stopwatch"] .widget-body { color: #f5b63f; }
    .widget[data-kind="ring"] .widget-body { color: #4cd97b; }
    .widget[data-kind="pulse"] .widget-body { color: #ff5d6c; }
    .spark, .ecg { width: 100%; height: 44px; }
    .ring { width: 44px; height: 44px; }
    .ring-track { stroke: #273140; }
    .clock { font-family: ui-monospace, monospace; font-size: 24px; }
    .readout { font-size: 11px; opacity: 0.7; }
    .empty { grid-column: 1 / -1; opacity: 0.45; text-align: center; margin: auto; font-size: 13px; }
    .ghost .widget { box-shadow: 0 10px 30px rgba(0,0,0,0.6); border-color: #3ddbd9; }
    "#;
}

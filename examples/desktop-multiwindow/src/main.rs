//! Two-window desktop drags (TODO 3.5): a board window and a tear-off tray
//! window sharing one `DndWorld<Card>` - drag cards between the windows in
//! both directions, with the ghost handing off between windows mid-drag.
//!
//! The interesting parts:
//! - `use_dnd_world` in the board window creates the shared world; the tray
//!   receives it through `VirtualDom::with_root_context`, and each window's
//!   `DndProvider` joins automatically.
//! - `use_window_geometry_feed` is the desktop glue (the only code here
//!   that touches tao): it feeds the window's position/size/scale into a
//!   `WindowGeometry` on move/resize/focus events. Everything else is the
//!   same dioxus-dnd API the web gallery uses.
//! - On Wayland a window cannot learn its own screen position; the feed
//!   then leaves geometry cleared and drags gracefully stay per-window.
//!   (Try `GDK_BACKEND=x11` under WSLg/X11 for the full cross-window path.)

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::tao::event::{ElementState, Event, MouseButton, WindowEvent};
use dioxus::desktop::tao::keyboard::ModifiersState as TaoModifiers;
#[cfg(target_os = "linux")]
use dioxus::desktop::tao::platform::unix::EventLoopWindowTargetExtUnix;
use dioxus::desktop::{use_wry_event_handler, window, Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus::signals::{AnyStorage, Owner, UnsyncStorage};
use dioxus_dnd::prelude::*;
use std::rc::Rc;

const BOARD: ZoneId = ZoneId(1);
const TRAY: ZoneId = ZoneId(2);

#[derive(Clone, Debug, PartialEq)]
struct Card {
    id: u32,
    label: &'static str,
}

/// Copy handles into app-lifetime signal storage shared by every window.
#[derive(Clone, Copy, PartialEq)]
struct Model {
    board: Signal<Vec<Card>>,
    tray: Signal<Vec<Card>>,
}

/// Owns the model's signal storage independently of any window runtime.
/// Every window holds an `Rc`, so the board may close before its trays and
/// the signals are reclaimed only after the final window releases them.
struct ModelOwner {
    model: Model,
    _owner: Owner<UnsyncStorage>,
}

impl ModelOwner {
    fn new() -> Rc<Self> {
        let owner = UnsyncStorage::owner();
        let model = dioxus::core::with_owner(owner.clone(), || Model {
            board: Signal::new(vec![
                Card {
                    id: 1,
                    label: "Scout the webviews",
                },
                Card {
                    id: 2,
                    label: "Shared-world pointer drags",
                },
                Card {
                    id: 3,
                    label: "Per-window ghost handoff",
                },
                Card {
                    id: 4,
                    label: "Honest platform notes",
                },
            ]),
            tray: Signal::new(Vec::new()),
        });
        Rc::new(Self {
            model,
            _owner: owner,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GlobalGeometry {
    #[default]
    Unknown,
    Available,
    Unavailable,
}

fn global_geometry_for_backend(is_wayland: bool) -> GlobalGeometry {
    if is_wayland {
        GlobalGeometry::Unavailable
    } else {
        GlobalGeometry::Available
    }
}

fn map_modifiers(native: TaoModifiers) -> Modifiers {
    let mut mapped = Modifiers::empty();
    if native.shift_key() {
        mapped.insert(Modifiers::SHIFT);
    }
    if native.control_key() {
        mapped.insert(Modifiers::CONTROL);
    }
    if native.alt_key() {
        mapped.insert(Modifiers::ALT);
    }
    if native.super_key() {
        mapped.insert(Modifiers::META);
    }
    mapped
}

impl Model {
    fn move_card(&self, card: Card, to: ZoneId) {
        let (mut board, mut tray) = (self.board, self.tray);
        board.write().retain(|c| c.id != card.id);
        tray.write().retain(|c| c.id != card.id);
        match to {
            BOARD => board.write().push(card),
            _ => tray.write().push(card),
        }
    }
}

fn main() {
    dioxus::LaunchBuilder::new()
        .with_cfg(
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("dioxus-dnd - board")
                    .with_inner_size(LogicalSize::new(460.0, 640.0)),
            ),
        )
        .launch(board_window);
}

/// The desktop glue (candidate for a future `desktop` feature): provide a
/// `WindowGeometry` for this window and keep it fed from tao events. Call
/// it ABOVE the `DndProvider`, which picks the geometry up from context
/// when it joins the world.
fn use_window_geometry_feed() -> WindowGeometry {
    let geometry = use_context_provider(|| {
        let geometry = WindowGeometry::new();
        // Do not expose plausible-looking coordinates until the event-loop
        // target tells us which Linux backend is actually running.
        geometry.set_eligible(false);
        geometry
    });
    let mut capability = use_signal(GlobalGeometry::default);
    let desktop = window();
    let sample = use_callback(move |_: ()| {
        if *capability.peek() != GlobalGeometry::Available {
            geometry.set_eligible(false);
            geometry.clear();
            return;
        }
        let eligible = desktop.is_visible() && !desktop.is_minimized();
        geometry.set_eligible(eligible);
        if !eligible {
            return;
        }
        let scale = desktop.scale_factor();
        let size = desktop.inner_size();
        match desktop.inner_position() {
            Ok(pos) => geometry.set(
                Point::new(pos.x as f64, pos.y as f64),
                (size.width as f64, size.height as f64),
                scale,
            ),
            Err(_) => {
                geometry.set_eligible(false);
                geometry.clear();
            }
        }
    });
    // WindowEvents arrive pre-filtered to the registering window.
    use_wry_event_handler(move |event, target| {
        if *capability.peek() == GlobalGeometry::Unknown {
            #[cfg(target_os = "linux")]
            let detected = global_geometry_for_backend(target.is_wayland());
            #[cfg(not(target_os = "linux"))]
            let detected = global_geometry_for_backend(false);

            capability.set(detected);
            if detected == GlobalGeometry::Available {
                geometry.mark_focused();
                sample.call(());
            } else {
                geometry.set_eligible(false);
                geometry.clear();
            }
        }
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::CursorEntered { .. }
                | WindowEvent::Focused(false) => sample.call(()),
                WindowEvent::Focused(true) => {
                    geometry.mark_focused();
                    sample.call(());
                }
                WindowEvent::CloseRequested | WindowEvent::Destroyed => {
                    geometry.set_eligible(false);
                    geometry.clear();
                }
                _ => {}
            }
        }
    });
    geometry
}

fn board_window() -> Element {
    let world = use_dnd_world::<Card>();
    let geometry = use_window_geometry_feed();
    let model_owner = use_hook(ModelOwner::new);
    let model = model_owner.model;

    let open_tray = move |_| {
        let dom = VirtualDom::new(tray_window)
            .with_root_context(world)
            .with_root_context(model_owner.clone());
        window().new_window(
            dom,
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("dioxus-dnd - tray")
                    .with_inner_size(LogicalSize::new(360.0, 640.0)),
            ),
        );
    };

    let windows_joined = world.windows().len();
    let cross = geometry.live();
    rsx! {
        Chrome {
            header: rsx! {
                button { onclick: open_tray, "Open tray window" }
                span { class: "status",
                    "{windows_joined} window(s) joined - cross-window "
                    if cross { "on" } else { "off (no geometry: Wayland?)" }
                }
            },
            Column { title: "Board", zone: BOARD, cards: model.board, model }
        }
    }
}

fn tray_window() -> Element {
    use_window_geometry_feed();
    let model_owner = use_context::<Rc<ModelOwner>>();
    let model = model_owner.model;
    rsx! {
        Chrome {
            header: rsx! {
                span { class: "status", "Drag cards to and from the board window" }
            },
            Column { title: "Tray", zone: TRAY, cards: model.tray, model }
        }
    }
}

/// Shared per-window shell: provider, styles, overlay, live region, and
/// the host-side drag bridge.
#[component]
fn Chrome(header: Element, children: Element) -> Element {
    rsx! {
        style { {STYLE} }
        DndProvider::<Card> {
            DragBridge {}
            div { class: "chrome",
                header { {header} }
                {children}
            }
            DragOverlay::<Card> { match_source: true, class: "ghost",
                CardGhost {}
            }
            LiveRegion::<Card> {}
        }
    }
}

/// The cross-window drag bridge (glue, candidate for a `desktop`
/// feature). Webview pointer events stop at the viewport edge, and while
/// a button is held every NON-origin window is fully event-blind (X11
/// implicit grab / AppKit event routing / engine mouse capture) - probed
/// and confirmed on this stack. So:
///
/// - While a drag is in flight and the cursor is outside the ORIGIN
///   window, the origin's bridge polls the global cursor (tao
///   `cursor_position`, works even where events are dead) and feeds
///   `world.track_global` - the ghost and zone hovers keep working
///   across windows.
/// - A primary raw mouse release completes the drag in either the origin or
///   a foreign window. A foreign window's first cursor event remains the
///   fallback for platforms that suppress the raw release. The generation
///   and source callback make native and webview echoes idempotent.
#[component]
fn DragBridge() -> Element {
    let Some(joined) = use_joined_window::<Card>() else {
        return rsx! {};
    };
    let ctx = joined.world.context();

    // A resource restarts by cancelling its previous task whenever the drag,
    // pointer kind, or geometry capability changes. Capturing the generation
    // also prevents a sleeper from attaching itself to a rapid restart.
    let _poller = use_resource(move || {
        let session = joined.world.drag_session();
        let should_poll = ctx.dragging()
            && joined.world.origin_window() == Some(joined.key)
            && joined.world.pointer_kind() == PointerKind::Mouse
            && joined.geometry.live();
        let desktop = window();
        async move {
            let Some(session) = session.filter(|_| should_poll) else {
                return;
            };
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                if !joined.world.is_drag_session(session)
                    || joined.world.origin_window() != Some(joined.key)
                    || joined.world.pointer_kind() != PointerKind::Mouse
                    || !joined.geometry.live()
                {
                    break;
                }
                let Ok(pos) = desktop.cursor_position() else {
                    break;
                };
                let global = Point::new(pos.x, pos.y);
                // Inside the origin window the webview owns the stream;
                // outside it, the poller is the drag's eyes.
                if !joined.geometry.contains_global(global) && joined.world.is_drag_session(session)
                {
                    joined.world.track_global(global);
                }
            }
        }
    });

    // Native release detection and foreign-window fallback.
    use_wry_event_handler(move |event, _| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::ModifiersChanged(modifiers) => {
                    joined.world.update_modifiers(map_modifiers(*modifiers));
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    if ctx.dragging() {
                        joined.world.refresh_all_rects();
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    let session = joined.world.drag_session();
                    if joined.world.pointer_kind() == PointerKind::Mouse
                        && joined.geometry.live()
                        && session.is_some_and(|id| joined.world.is_drag_session(id))
                    {
                        if let Ok(pos) = window().cursor_position() {
                            joined.world.drop_at_global(Point::new(pos.x, pos.y));
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let session = joined.world.drag_session();
                    let dragging_foreign = joined.world.pointer_kind() == PointerKind::Mouse
                        && joined.geometry.live()
                        && joined.world.origin_window().is_some()
                        && joined.world.origin_window() != Some(joined.key)
                        && session.is_some_and(|id| joined.world.is_drag_session(id));
                    if dragging_foreign {
                        // Physical window px -> this window's CSS px -> global.
                        let scale = joined.geometry.scale().max(f64::EPSILON);
                        let client = Point::new(position.x / scale, position.y / scale);
                        if let Some(global) = joined.geometry.to_global(client) {
                            joined.world.drop_at_global(global);
                        }
                    }
                }
                WindowEvent::CursorEntered { .. } => {
                    let session = joined.world.drag_session();
                    let dragging_foreign = joined.world.pointer_kind() == PointerKind::Mouse
                        && joined.geometry.live()
                        && joined.world.origin_window().is_some()
                        && joined.world.origin_window() != Some(joined.key)
                        && session.is_some_and(|id| joined.world.is_drag_session(id));
                    if dragging_foreign {
                        if let Ok(pos) = window().cursor_position() {
                            joined.world.drop_at_global(Point::new(pos.x, pos.y));
                        }
                    }
                }
                _ => {}
            }
        }
    });

    rsx! {}
}

/// The ghost mirrors whatever card is in flight - in whichever window is
/// presenting it this frame.
#[component]
fn CardGhost() -> Element {
    let dnd = use_dnd::<Card>();
    let label = dnd.payload().map(|c| c.label).unwrap_or_default();
    rsx! {
        div { class: "card", "{label}" }
    }
}

#[component]
fn Column(title: &'static str, zone: ZoneId, cards: Signal<Vec<Card>>, model: Model) -> Element {
    rsx! {
        DropZone::<Card> {
            id: zone,
            label: title,
            class: "column",
            on_drop: move |o: DropOutcome<Card>| model.move_card(o.payload, o.to),
            h2 { "{title}" }
            for card in cards() {
                Draggable::<Card> {
                    key: "{card.id}",
                    payload: card.clone(),
                    zone,
                    label: card.label,
                    class: "card",
                    "{card.label}"
                }
            }
            if cards().is_empty() {
                p { class: "empty", "Drop cards here" }
            }
        }
    }
}

const STYLE: &str = r#"
    * { box-sizing: border-box; }
    body { margin: 0; font-family: system-ui, sans-serif; background: #f4f1ea; color: #1f2421; }
    .chrome { display: flex; flex-direction: column; gap: 12px; padding: 16px; height: 100vh; }
    header { display: flex; align-items: center; gap: 12px; }
    .status { font-size: 12px; opacity: 0.7; }
    button { padding: 6px 12px; border: 1px solid #1f2421; background: #fff; border-radius: 6px; cursor: pointer; }
    .column { flex: 1; display: flex; flex-direction: column; gap: 8px; padding: 12px;
              border: 2px solid #cfc8b8; border-radius: 10px; background: #fbf9f4; overflow: auto; }
    .column h2 { margin: 0 0 4px; font-size: 14px; text-transform: uppercase; letter-spacing: 0.08em; }
    .column[data-active] { border-color: #4b6b53; }
    .column[data-over] { border-color: #2e5339; background: #eef3ec; }
    .card { padding: 10px 12px; border: 1px solid #cfc8b8; border-radius: 8px; background: #fff; cursor: grab; }
    .card[data-dragging] { opacity: 0.4; }
    .ghost .card { box-shadow: 0 6px 18px rgba(31, 36, 33, 0.25); border-color: #2e5339; }
    .empty { opacity: 0.5; font-size: 13px; text-align: center; margin: auto; }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    thread_local! {
        static MODEL_SLOT: RefCell<Option<Rc<ModelOwner>>> = const { RefCell::new(None) };
        static SURVIVOR_RENDERS: Cell<usize> = const { Cell::new(0) };
        static SURVIVOR_VIEW: RefCell<String> = const { RefCell::new(String::new()) };
    }

    fn model_creator_window() -> Element {
        let model_owner = use_hook(ModelOwner::new);
        MODEL_SLOT.with_borrow_mut(|slot| *slot = Some(model_owner));
        rsx! {}
    }

    fn model_survivor_window() -> Element {
        let model_owner = use_context::<Rc<ModelOwner>>();
        let board_len = model_owner.model.board.read().len();
        let tray_len = model_owner.model.tray.read().len();
        SURVIVOR_RENDERS.set(SURVIVOR_RENDERS.get() + 1);
        SURVIVOR_VIEW.with_borrow_mut(|view| *view = format!("board:{board_len};tray:{tray_len}"));
        rsx! { div { "board:{board_len};tray:{tray_len}" } }
    }

    #[test]
    fn model_storage_outlives_the_window_that_created_it() {
        let mut creator = VirtualDom::new(model_creator_window);
        creator.rebuild_in_place();
        let survivor = MODEL_SLOT
            .with_borrow_mut(Option::take)
            .expect("creator parked its model owner");
        let model = survivor.model;
        let mut tray = VirtualDom::new(model_survivor_window).with_root_context(survivor.clone());

        SURVIVOR_RENDERS.set(0);
        tray.rebuild_in_place();
        SURVIVOR_VIEW.with_borrow(|view| assert_eq!(view, "board:4;tray:0"));
        drop(creator);
        tray.in_runtime(|| {
            model.move_card(
                Card {
                    id: 1,
                    label: "Scout the webviews",
                },
                TRAY,
            )
        });
        tray.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        SURVIVOR_VIEW.with_borrow(|view| assert_eq!(view, "board:3;tray:1"));
        assert!(SURVIVOR_RENDERS.get() >= 2);
        drop(tray);
        drop(survivor);
        assert!(model.board.try_read().is_err());
        assert!(model.tray.try_read().is_err());
    }

    #[test]
    fn tao_modifiers_map_to_drop_effect_modifiers() {
        let native =
            TaoModifiers::SHIFT | TaoModifiers::CONTROL | TaoModifiers::ALT | TaoModifiers::SUPER;
        let mapped = map_modifiers(native);
        assert!(mapped.contains(Modifiers::SHIFT));
        assert!(mapped.contains(Modifiers::CONTROL));
        assert!(mapped.contains(Modifiers::ALT));
        assert!(mapped.contains(Modifiers::META));
    }

    #[test]
    fn wayland_disables_global_geometry() {
        assert_eq!(
            global_geometry_for_backend(true),
            GlobalGeometry::Unavailable
        );
        assert_eq!(
            global_geometry_for_backend(false),
            GlobalGeometry::Available
        );
    }
}

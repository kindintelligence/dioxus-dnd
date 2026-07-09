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
use dioxus::desktop::tao::event::{DeviceEvent, ElementState, Event, WindowEvent};
use dioxus::desktop::{use_wry_event_handler, window, Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;

const BOARD: ZoneId = ZoneId(1);

#[derive(Clone, Debug, PartialEq)]
struct Card {
    id: u32,
    label: &'static str,
}

/// One open tray window: its own zone id and its own card list. Zone ids
/// must be unique across the whole world - two windows registering the
/// same id would mirror each other's hover highlight and route drops into
/// one shared list (exactly what an early version of this example did).
#[derive(Clone, Copy, PartialEq)]
struct Tray {
    n: u32,
    zone: ZoneId,
    cards: Signal<Vec<Card>>,
}

/// The app model, shared across windows the same way the world is: signals
/// created in the board window, handed to each tray via root context. (All
/// windows run on one thread, so cross-window signals subscribe and wake
/// correctly - the same mechanism the world itself rides on.) Tray card
/// lists are created per-open in ROOT scope, so they outlive re-renders of
/// the opener and die only with the app.
#[derive(Clone, Copy, PartialEq)]
struct Model {
    board: Signal<Vec<Card>>,
    trays: Signal<Vec<Tray>>,
}

impl Model {
    fn move_card(&self, card: Card, to: ZoneId) {
        let mut board = self.board;
        board.write().retain(|c| c.id != card.id);
        for tray in self.trays.peek().iter() {
            let mut cards = tray.cards;
            cards.write().retain(|c| c.id != card.id);
        }
        let tray = self.trays.peek().iter().find(|t| t.zone == to).copied();
        match tray {
            Some(t) => {
                let mut cards = t.cards;
                cards.write().push(card);
            }
            // BOARD, or a tray that closed in the race between hit-test
            // and delivery: the board is the fallback so a card can
            // never vanish.
            None => board.write().push(card),
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
    let geometry = use_context_provider(WindowGeometry::new);
    let desktop = window();
    let sample = use_callback(move |_: ()| {
        let scale = desktop.scale_factor();
        let size = desktop.inner_size();
        match desktop.inner_position() {
            Ok(pos) => geometry.set(
                Point::new(pos.x as f64, pos.y as f64),
                (size.width as f64, size.height as f64),
                scale,
            ),
            // Wayland: window positions are unknowable by design; leave
            // geometry cleared and this window drags per-window only.
            Err(_) => geometry.clear(),
        }
    });
    use_hook(move || {
        sample.call(());
        geometry.mark_focused();
    });
    // WindowEvents arrive pre-filtered to the registering window.
    use_wry_event_handler(move |event, _| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. } => sample.call(()),
                WindowEvent::Focused(true) => {
                    geometry.mark_focused();
                    sample.call(());
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
    let model = use_hook(|| Model {
        board: Signal::new(vec![
            Card { id: 1, label: "Scout the webviews" },
            Card { id: 2, label: "Shared-world pointer drags" },
            Card { id: 3, label: "Per-window ghost handoff" },
            Card { id: 4, label: "Honest platform notes" },
        ]),
        trays: Signal::new(Vec::new()),
    });

    let mut tray_seq = use_signal(|| 0u32);
    let open_tray = move |_| {
        let n = *tray_seq.peek() + 1;
        tray_seq.set(n);
        // ROOT scope: the list must outlive the opener's re-renders and
        // belongs to the app, not to any window (see Model docs).
        let tray = Tray {
            n,
            zone: ZoneId::auto(),
            cards: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
        };
        let mut trays = model.trays;
        trays.write().push(tray);
        let dom = VirtualDom::new(tray_window)
            .with_root_context(world)
            .with_root_context(model)
            .with_root_context(tray);
        window().new_window(
            dom,
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title(format!("dioxus-dnd - tray {n}"))
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
    let model = use_context::<Model>();
    let tray = use_context::<Tray>();
    // Closing a tray returns its cards to the board and retires its zone -
    // cards must never vanish with a window. try_write throughout: this
    // runs in a destructor, and at app shutdown the board's signals may
    // already be gone (a panic here would abort the process).
    use_drop(move || {
        let mut trays = model.trays;
        if let Ok(mut t) = trays.try_write() {
            t.retain(|t| t.zone != tray.zone);
            drop(t);
        }
        let mut cards = tray.cards;
        let orphans: Vec<Card> = cards
            .try_write()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default();
        if orphans.is_empty() {
            return;
        }
        let mut board = model.board;
        if let Ok(mut b) = board.try_write() {
            b.extend(orphans);
        };
    });
    rsx! {
        Chrome {
            header: rsx! {
                span { class: "status", "Drag cards to and from any other window" }
            },
            Column { title: "Tray {tray.n}", zone: tray.zone, cards: tray.cards, model }
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
/// - A NON-origin window receiving any pointer event mid-drag proves the
///   button was released (it was blind while held): its bridge completes
///   the drop at that position via `world.drop_at_global`. Releases back
///   inside the origin window arrive as normal pointerups; both paths
///   are idempotent.
///
/// Windows/WebView2 amendment (probed on Win 11 / WebView2): the engine
/// capture there is the OPPOSITE shape. The origin webview keeps
/// receiving the full mouse stream while the button is held - including
/// moves and the release outside its own viewport - but those events
/// target `<html>` (nothing retargets without pointer capture), so no
/// component handler ever hears them; and tao never fires
/// `CursorMoved`/`MouseInput` at all, because the WebView2 child HWND
/// consumes the messages before the tao window sees them. Both bridge
/// legs above are therefore dead on Windows. The third leg below fixes
/// it one layer lower, with no JS: tao registers Windows raw input
/// (`WM_INPUT`, `DeviceEventFilter::Unfocused` by default) on the event
/// loop's thread target, which no HWND can swallow, and dioxus-desktop
/// forwards `Event::DeviceEvent` to EVERY `use_wry_event_handler`
/// (only `WindowEvent`s are filtered per-window). So the origin's
/// bridge hears the raw button-up wherever it happens and completes the
/// drop at `cursor_position()` - the same global-physical-px source the
/// poller uses. Raw motion also retracks mid-drag at event rate, which
/// out-paces the 30ms poller while the cursor is outside the origin.
/// Releases INSIDE the origin viewport are left to the Draggable's own
/// pointerup (via its capture-substitute layer), keeping single-window
/// semantics - snap, modifiers - exactly as before.
#[component]
fn DragBridge() -> Element {
    let Some(joined) = use_joined_window::<Card>() else {
        return rsx! {};
    };
    let ctx = joined.world.context();

    // Third leg: raw-input release detection + event-rate tracking (see
    // the Windows amendment above). DeviceEvents reach every window's
    // handler; the origin gate keeps exactly one bridge acting.
    //
    // The filter must be `Never` (RIDEV_INPUTSINK): tao's default
    // `Unfocused` delivers WM_INPUT only while the registered target is
    // foreground, and the foreground input owner here is the WebView2
    // child (a different process's HWND tree), so raw input never
    // arrives. `Never` delivers regardless of focus; the `dragging()`
    // gate below keeps the firehose ignored outside drags. Windows-only
    // effect; a documented no-op everywhere else.
    let filter_set = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
    use_wry_event_handler(move |event, target| {
        if !filter_set.get() {
            filter_set.set(true);
            target.set_device_event_filter(dioxus::desktop::tao::event_loop::DeviceEventFilter::Never);
        }
        if !ctx.dragging() || joined.world.origin_window() != Some(joined.key) {
            return;
        }
        let Event::DeviceEvent { event, .. } = event else {
            return;
        };
        let released = matches!(
            event,
            DeviceEvent::Button { button: 1, state: ElementState::Released, .. }
        );
        if !released && !matches!(event, DeviceEvent::MouseMotion { .. }) {
            return;
        }
        // Wayland has no global cursor; the error leaves this leg inert
        // and the per-window paths keep working.
        let Ok(pos) = window().cursor_position() else {
            return;
        };
        let global = Point::new(pos.x, pos.y);
        // Inside the origin viewport the webview owns the gesture (the
        // capture substitute feeds moves, the Draggable's pointerup
        // finishes drops). Outside it, this leg is the drag's ears.
        if joined.geometry.contains_global(global) {
            return;
        }
        if released {
            joined.world.drop_at_global(global);
        } else {
            joined.world.track_global(global);
        }
    });

    // Origin-side poller: spawned when a drag starts, ends itself when
    // the drag does. ~30ms keeps the ghost smooth without busy-waiting.
    use_effect(move || {
        if !ctx.dragging() {
            return;
        }
        if joined.world.origin_window() != Some(joined.key) {
            return;
        }
        let desktop = window();
        spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                if !ctx.dragging() {
                    break;
                }
                // Wayland: no global cursor by design - the bridge simply
                // never engages and drags stay per-window.
                let Ok(pos) = desktop.cursor_position() else {
                    break;
                };
                let global = Point::new(pos.x, pos.y);
                // Inside the origin window the webview owns the stream;
                // outside it, the poller is the drag's eyes.
                if !joined.geometry.contains_global(global) {
                    joined.world.track_global(global);
                }
            }
        });
    });

    // Foreign-side release detection.
    use_wry_event_handler(move |event, _| {
        let dragging_foreign = ctx.dragging()
            && joined.world.origin_window().is_some()
            && joined.world.origin_window() != Some(joined.key);
        if !dragging_foreign {
            return;
        }
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    // Physical window px -> this window's CSS px -> global.
                    let scale = joined.geometry.scale().max(f64::EPSILON);
                    let client = Point::new(position.x / scale, position.y / scale);
                    if let Some(global) = joined.geometry.to_global(client) {
                        joined.world.drop_at_global(global);
                    }
                }
                WindowEvent::MouseInput { state: ElementState::Released, .. } => {
                    if let Ok(pos) = window().cursor_position() {
                        joined
                            .world
                            .drop_at_global(Point::new(pos.x, pos.y));
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
fn Column(title: String, zone: ZoneId, cards: Signal<Vec<Card>>, model: Model) -> Element {
    rsx! {
        DropZone::<Card> {
            id: zone,
            label: title.clone(),
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

//! Multi-window desktop drags (TODO 3.5): a board window and N tear-off
//! tray windows sharing one `DndWorld<Card>` - drag cards between any
//! windows, with the ghost handing off between windows mid-drag.
//!
//! The interesting parts:
//! - `use_dnd_world` in the board window creates the shared world; each
//!   tray receives it through `VirtualDom::with_root_context`, and each
//!   window's `DndProvider` joins automatically.
//! - The windowing glue is the library's `desktop` feature:
//!   `use_window_geometry_feed` (above the provider) feeds the window's
//!   position/size/scale into a `WindowGeometry`, and `DragBridge`
//!   (inside the provider) is the host-side eyes and ears for pointer
//!   drags that leave the origin window. See `dioxus_dnd::desktop` for
//!   the per-platform mechanics. Everything else is the same dioxus-dnd
//!   API the web gallery uses.
//! - On Wayland a window cannot learn its own screen position; the feed
//!   then leaves geometry cleared and drags gracefully stay per-window.
//!   (Try `GDK_BACKEND=x11` under WSLg/X11 for the full cross-window path.)

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{window, Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus_dnd::desktop::{use_window_geometry_feed, DragBridge};
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
            DragBridge::<Card> {}
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

//! Multi-window desktop drags (TODO 3.5): a board window and N tear-off
//! tray windows sharing one `DndWorld<Card>` - drag cards between any
//! windows, with the ghost handing off between windows mid-drag.
//!
//! The interesting parts:
//! - `use_dnd_world` creates the shared world; `world.vdom(tray_window)`
//!   makes it impossible to spawn a tray without seeding that world.
//! - `use_dnd_model` gives the app-wide board/tray model process-lived
//!   signal storage, so the board window may close before its trays.
//! - Each tray's reclaimable card list is minted under a `DndScope` and
//!   retired only after its cards return to the board.
//! - `MultiWindowProvider` installs the geometry feed, provider and host
//!   bridge in their required nesting order in every window.
//! - On Wayland a window cannot learn its own screen position; the feed
//!   then leaves geometry cleared and drags gracefully stay per-window.
//!   (Try `GDK_BACKEND=x11` under WSLg/X11 for the full cross-window path.)

use dioxus::desktop::tao::dpi::LogicalSize;
use dioxus::desktop::{window, Config, WindowBuilder};
use dioxus::prelude::*;
use dioxus_dnd::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const BOARD: ZoneId = ZoneId(1);

#[derive(Clone, Debug, PartialEq)]
struct Card {
    id: u32,
    label: &'static str,
}

/// One open tray window: its own zone id and its own card list. The world
/// qualifies zone identity by window, so duplicate explicit ids are safe for
/// hover and delivery. This example still assigns unique ids because its
/// shared model uses the bare `ZoneId` as the key for each tray's card list.
#[derive(Clone, Copy, PartialEq)]
struct Tray {
    n: u32,
    zone: ZoneId,
    cards: Signal<Vec<Card>>,
}

/// App-wide signal handles plus reclaimable per-tray scopes. The signals are
/// created by `use_dnd_model`; every live window carries a clone of this
/// handle, so the scope table remains available for its teardown callback.
#[derive(Clone)]
struct Model {
    board: Signal<Vec<Card>>,
    trays: Signal<Vec<Tray>>,
    tray_scopes: Rc<RefCell<HashMap<ZoneId, DndScope>>>,
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.board == other.board
            && self.trays == other.trays
            && Rc::ptr_eq(&self.tray_scopes, &other.tray_scopes)
    }
}

impl Model {
    fn new() -> Self {
        Self {
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
            trays: Signal::new(Vec::new()),
            tray_scopes: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    fn new_tray(&self, n: u32) -> Tray {
        let zone = ZoneId::auto();
        let scope = DndScope::new();
        let tray = scope.with(|| Tray {
            n,
            zone,
            cards: Signal::new(Vec::new()),
        });
        self.tray_scopes.borrow_mut().insert(zone, scope);
        tray
    }

    /// Return a closing tray's cards and reclaim its signal as one operation.
    ///
    /// The caller owns the closing window's sole teardown callback. Desktop
    /// VDOMs render and retire serially on this thread, so all render/event
    /// signal guards have returned before `use_drop` runs. Under that
    /// ownership condition cleanup is deliberately infallible instead of
    /// silently abandoning a transient `try_write` failure with no window
    /// left to retry it.
    fn close_tray(&self, tray: Tray) -> bool {
        let retired_scope = {
            let mut scopes = self.tray_scopes.borrow_mut();
            if !scopes.contains_key(&tray.zone) {
                return false;
            }
            let mut trays = self.trays;
            let mut cards = tray.cards;
            let mut board = self.board;
            // Acquire every guard before mutation. If the quiescent-teardown
            // invariant above is ever violated, unwinding cannot observe a
            // partially retired tray.
            let mut open_trays = trays.write();
            let mut tray_cards = cards.write();
            let mut board_cards = board.write();

            open_trays.retain(|open| open.zone != tray.zone);
            board_cards.extend(std::mem::take(&mut *tray_cards));
            scopes
                .remove(&tray.zone)
                .expect("the checked tray scope remains locked until retirement")
        };
        drop(retired_scope);
        true
    }

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
    let model = use_dnd_model(Model::new);

    let mut tray_seq = use_signal(|| 0u32);
    let model_for_open = model.clone();
    let open_tray = move |_| {
        let n = *tray_seq.peek() + 1;
        tray_seq.set(n);
        let tray = model_for_open.new_tray(n);
        let mut trays = model_for_open.trays;
        trays.write().push(tray);
        let dom = world
            .vdom(tray_window)
            .with_root_context(model_for_open.clone())
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

    rsx! {
        Chrome {
            header: rsx! {
                button { onclick: open_tray, "Open tray window" }
                WorldStatus { world }
            },
            Column { title: "Board", zone: BOARD, cards: model.board, model }
        }
    }
}

fn tray_window() -> Element {
    let model = use_context::<Model>();
    let tray = use_context::<Tray>();
    use_tray_cleanup(model.clone(), tray);
    rsx! {
        Chrome {
            header: rsx! {
                span { class: "status", "Drag cards to and from any other window" }
            },
            Column { title: "Tray {tray.n}", zone: tray.zone, cards: tray.cards, model }
        }
    }
}

fn use_tray_cleanup(model: Model, tray: Tray) {
    // Closing a tray returns its cards to the board, retires its zone, and
    // reclaims its independently scoped signal. `false` means this exact tray
    // was already retired, so repeated teardown stays inert.
    use_drop(move || {
        let _ = model.close_tray(tray);
    });
}

/// Shared per-window shell. MultiWindowProvider owns the fixed desktop
/// wiring; the overlay and live region remain app-styled children.
#[component]
fn Chrome(header: Element, children: Element) -> Element {
    rsx! {
        style { {STYLE} }
        MultiWindowProvider::<Card> {
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

#[component]
fn WorldStatus(world: DndWorld<Card>) -> Element {
    let geometry = use_context::<WindowGeometry>();
    let windows_joined = world.windows().len();
    let cross = geometry.live();
    rsx! {
        span { class: "status",
            "{windows_joined} window(s) joined - cross-window "
            if cross { "on" } else { "off (no geometry: Wayland?)" }
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

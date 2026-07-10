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
use dioxus::signals::{AnyStorage, Owner, UnsyncStorage};
use dioxus_dnd::desktop::{use_window_geometry_feed, DragBridge};
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

/// Copy handles into model-owned signal storage shared by every window.
#[derive(Clone, Copy, PartialEq)]
struct Model {
    board: Signal<Vec<Card>>,
    trays: Signal<Vec<Tray>>,
}

/// Owns the model independently of any one window runtime. Every window holds
/// an `Rc`, so the board may close before its trays; storage is reclaimed only
/// after the final window releases it. This is application-state lifetime,
/// not desktop platform glue.
struct ModelOwner {
    model: Model,
    _owner: Owner<UnsyncStorage>,
    tray_owners: RefCell<HashMap<ZoneId, Owner<UnsyncStorage>>>,
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
            trays: Signal::new(Vec::new()),
        });
        Rc::new(Self {
            model,
            _owner: owner,
            tray_owners: RefCell::new(HashMap::new()),
        })
    }

    fn new_tray(&self, n: u32) -> Tray {
        let zone = ZoneId::auto();
        let owner = UnsyncStorage::owner();
        let tray = dioxus::core::with_owner(owner.clone(), || Tray {
            n,
            zone,
            cards: Signal::new(Vec::new()),
        });
        self.tray_owners.borrow_mut().insert(zone, owner);
        tray
    }

    /// Return a closing tray's cards and reclaim its signal as one operation.
    ///
    /// The caller owns the closing window's sole teardown callback. Desktop
    /// VDOMs render and retire serially on this thread, so all render/event
    /// signal guards have returned before `use_drop` runs; its captured `Rc`
    /// also pins every owner below. Under that ownership condition cleanup is
    /// deliberately infallible instead of silently abandoning a transient
    /// `try_write` failure with no window left to retry it.
    fn close_tray(&self, tray: Tray) -> bool {
        let model = self.model;
        let retired_owner = {
            let mut owners = self.tray_owners.borrow_mut();
            if !owners.contains_key(&tray.zone) {
                return false;
            }
            let mut trays = model.trays;
            let mut cards = tray.cards;
            let mut board = model.board;
            // Acquire every guard before mutation. If the quiescent-teardown
            // invariant above is ever violated, unwinding cannot observe a
            // partially retired tray.
            let mut open_trays = trays.write();
            let mut tray_cards = cards.write();
            let mut board_cards = board.write();

            open_trays.retain(|open| open.zone != tray.zone);
            board_cards.extend(std::mem::take(&mut *tray_cards));
            owners
                .remove(&tray.zone)
                .expect("the checked tray owner remains locked until retirement")
        };
        drop(retired_owner);
        true
    }
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
    let model_owner = use_hook(ModelOwner::new);
    let model = model_owner.model;

    let mut tray_seq = use_signal(|| 0u32);
    let open_tray = move |_| {
        let n = *tray_seq.peek() + 1;
        tray_seq.set(n);
        let tray = model_owner.new_tray(n);
        let mut trays = model.trays;
        trays.write().push(tray);
        let dom = VirtualDom::new(tray_window)
            .with_root_context(world)
            .with_root_context(model_owner.clone())
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
    let model_owner = use_context::<Rc<ModelOwner>>();
    let model = model_owner.model;
    let tray = use_context::<Tray>();
    use_tray_cleanup(model_owner, tray);
    rsx! {
        Chrome {
            header: rsx! {
                span { class: "status", "Drag cards to and from any other window" }
            },
            Column { title: "Tray {tray.n}", zone: tray.zone, cards: tray.cards, model }
        }
    }
}

fn use_tray_cleanup(model_owner: Rc<ModelOwner>, tray: Tray) {
    // Closing a tray returns its cards to the board, retires its zone, and
    // reclaims its independently owned signal. The captured owner pins all
    // shared storage through this destructor callback. `false` means this
    // exact tray was already retired, so repeated teardown stays inert.
    use_drop(move || {
        let _ = model_owner.close_tray(tray);
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    type ModelSlot = Rc<RefCell<Option<(Rc<ModelOwner>, Tray, Tray)>>>;

    #[derive(Default)]
    struct SurvivorProbe {
        renders: Cell<usize>,
        view: RefCell<String>,
    }

    fn model_creator_window() -> Element {
        let slot = use_context::<ModelSlot>();
        let model_owner = use_hook(ModelOwner::new);
        let trays_owner = model_owner.clone();
        let trays = use_hook(move || {
            let tray_1 = trays_owner.new_tray(1);
            let tray_2 = trays_owner.new_tray(2);
            let mut trays = trays_owner.model.trays;
            trays.write().extend([tray_1, tray_2]);
            (tray_1, tray_2)
        });
        *slot.borrow_mut() = Some((model_owner, trays.0, trays.1));
        rsx! {}
    }

    fn model_survivor_window() -> Element {
        let tray = use_context::<Tray>();
        let probe = use_context::<Rc<SurvivorProbe>>();
        let tray_len = tray.cards.read().len();
        probe.renders.set(probe.renders.get() + 1);
        *probe.view.borrow_mut() = format!("tray:{tray_len}");
        rsx! { div { "tray:{tray_len}" } }
    }

    fn model_closing_tray_window() -> Element {
        let model_owner = use_context::<Rc<ModelOwner>>();
        let tray = use_context::<Tray>();
        use_tray_cleanup(model_owner, tray);
        rsx! {}
    }

    #[test]
    fn creator_and_sibling_may_close_before_a_surviving_tray() {
        let slot = ModelSlot::default();
        let mut creator = VirtualDom::new(model_creator_window).with_root_context(slot.clone());
        creator.rebuild_in_place();
        let (survivor_owner, tray_1, tray_2) = slot
            .borrow_mut()
            .take()
            .expect("creator parked its model owner and trays");
        let model = survivor_owner.model;
        let probe = Rc::new(SurvivorProbe::default());
        let mut closing_tray = VirtualDom::new(model_closing_tray_window)
            .with_root_context(survivor_owner.clone())
            .with_root_context(tray_1);
        let mut survivor = VirtualDom::new(model_survivor_window)
            .with_root_context(survivor_owner.clone())
            .with_root_context(tray_2)
            .with_root_context(probe.clone());

        closing_tray.rebuild_in_place();
        survivor.rebuild_in_place();
        assert_eq!(*probe.view.borrow(), "tray:0");
        assert_eq!(model.board.peek().len(), 4);
        assert_eq!(model.trays.peek().len(), 2);

        // Exact reported close order: the board/creator closes, then tray 1.
        // Tray 2 owns the remaining `Rc` and must still be able to mutate and
        // rerender the shared model without touching dropped storage.
        drop(creator);
        survivor.in_runtime(|| {
            model.move_card(
                Card {
                    id: 1,
                    label: "Scout the webviews",
                },
                tray_1.zone,
            )
        });
        survivor.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        assert_eq!(*probe.view.borrow(), "tray:0");
        assert_eq!(model.board.peek().len(), 3);
        assert_eq!(model.trays.peek().len(), 2);
        assert_eq!(tray_1.cards.peek().len(), 1);
        let renders_before_close = probe.renders.get();

        // Dropping the tray VDOM exercises the same `use_drop` hook as the
        // real desktop window. Its card returns before its owner is retired.
        drop(closing_tray);
        survivor.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        assert_eq!(probe.renders.get(), renders_before_close);
        assert_eq!(model.board.peek().len(), 4);
        assert_eq!(model.trays.peek().len(), 1);
        assert!(tray_1.cards.try_read().is_err());
        let renders_after_close = probe.renders.get();

        survivor.in_runtime(|| {
            model.move_card(
                Card {
                    id: 1,
                    label: "Scout the webviews",
                },
                tray_2.zone,
            )
        });
        survivor.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        assert_eq!(*probe.view.borrow(), "tray:1");
        assert_eq!(model.board.peek().len(), 3);
        assert_eq!(model.trays.peek().len(), 1);
        assert!(probe.renders.get() > renders_after_close);

        drop(survivor);
        assert_eq!(Rc::strong_count(&survivor_owner), 1);
        drop(survivor_owner);
        assert!(model.board.try_read().is_err());
        assert!(model.trays.try_read().is_err());
        assert!(tray_2.cards.try_read().is_err());
    }
}

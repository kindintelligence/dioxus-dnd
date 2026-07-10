//! Shared live model: the widgets, who holds them, and how they move.
//!
//! Every widget's state is a `Signal<WidgetState>` in model-owned storage
//! shared by all windows. The drag payload is [`Widget`] - a handle, not a
//! serialized snapshot - which is exactly what the showcase demonstrates:
//! the ghost keeps rendering fresh state mid-drag because it holds the same
//! live signal every window renders from.

use dioxus::prelude::*;
use dioxus::signals::{AnyStorage, Owner, UnsyncStorage};
use dioxus_dnd::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Mission Control's dock zone. Satellites mint their own ids.
pub const DOCK: ZoneId = ZoneId(1);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WidgetKind {
    Sparkline,
    Stopwatch,
    Ring,
    Pulse,
}

impl WidgetKind {
    /// Stable name used for `data-kind` styling hooks and labels.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sparkline => "sparkline",
            Self::Stopwatch => "stopwatch",
            Self::Ring => "ring",
            Self::Pulse => "pulse",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Sparkline => "Telemetry",
            Self::Stopwatch => "Mission clock",
            Self::Ring => "Deploy",
            Self::Pulse => "Crew pulse",
        }
    }
}

/// One shared state struct for every widget kind; each body reads the
/// fields it cares about. The ticker advances all of them together.
#[derive(Clone, PartialEq, Debug)]
pub struct WidgetState {
    /// 50ms ticks since creation (the stopwatch derives mm:ss.t).
    pub ticks: u64,
    /// Sparkline window, newest last, values in 0..1, capped at 60.
    pub samples: Vec<f64>,
    /// Deploy ring progress in 0..1; wraps.
    pub level: f64,
    /// Pulse readout.
    pub bpm: f64,
    /// xorshift PRNG state, so liveness needs no `rand` dependency.
    pub seed: u64,
}

impl WidgetState {
    pub fn seeded(seed: u64, level: f64) -> Self {
        // Pre-fill the sparkline so the chart reads as live telemetry from
        // the first rendered frame instead of growing from a dot.
        let samples = (0..60)
            .map(|i| 0.5 + 0.28 * (i as f64 * 0.35).sin())
            .collect();
        Self {
            ticks: 0,
            samples,
            level,
            bpm: 72.0,
            seed,
        }
    }
}

/// The drag payload: a live handle, deliberately `Copy`.
#[derive(Clone, Copy, PartialEq)]
pub struct Widget {
    pub id: u32,
    pub kind: WidgetKind,
    pub state: Signal<WidgetState>,
}

/// One open satellite window: its zone and its widget list.
#[derive(Clone, Copy, PartialEq)]
pub struct Satellite {
    pub n: u32,
    pub zone: ZoneId,
    pub widgets: Signal<Vec<Widget>>,
}

/// Copy handles into model-owned signal storage shared by every window.
#[derive(Clone, Copy, PartialEq)]
pub struct Model {
    pub dock: Signal<Vec<Widget>>,
    pub satellites: Signal<Vec<Satellite>>,
}

/// Owns the model independently of any one window runtime (the pattern the
/// close-order regression pinned): every window holds an `Rc`, Mission
/// Control may close before its satellites, and storage is reclaimed only
/// after the final window releases it.
pub struct ModelOwner {
    pub model: Model,
    root: Owner<UnsyncStorage>,
    satellite_owners: RefCell<HashMap<ZoneId, Owner<UnsyncStorage>>>,
    next_id: Cell<u32>,
    ticker_claimed: AtomicBool,
}

impl ModelOwner {
    pub fn new() -> Rc<Self> {
        let root = UnsyncStorage::owner();
        let model = dioxus::core::with_owner(root.clone(), || {
            let dock = vec![
                Widget {
                    id: 1,
                    kind: WidgetKind::Sparkline,
                    state: Signal::new(WidgetState::seeded(0x5EED_0001, 0.15)),
                },
                Widget {
                    id: 2,
                    kind: WidgetKind::Stopwatch,
                    state: Signal::new(WidgetState::seeded(0x5EED_0002, 0.40)),
                },
                Widget {
                    id: 3,
                    kind: WidgetKind::Ring,
                    state: Signal::new(WidgetState::seeded(0x5EED_0003, 0.62)),
                },
                Widget {
                    id: 4,
                    kind: WidgetKind::Pulse,
                    state: Signal::new(WidgetState::seeded(0x5EED_0004, 0.85)),
                },
            ];
            Model {
                dock: Signal::new(dock),
                satellites: Signal::new(Vec::new()),
            }
        });
        Rc::new(Self {
            model,
            root,
            satellite_owners: RefCell::new(HashMap::new()),
            next_id: Cell::new(5),
            ticker_claimed: AtomicBool::new(false),
        })
    }

    fn mint_id(&self) -> u32 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        id
    }

    pub fn new_satellite(&self, n: u32) -> Satellite {
        let zone = ZoneId::auto();
        let owner = UnsyncStorage::owner();
        let satellite = dioxus::core::with_owner(owner.clone(), || Satellite {
            n,
            zone,
            widgets: Signal::new(Vec::new()),
        });
        self.satellite_owners.borrow_mut().insert(zone, owner);
        satellite
    }

    /// Return a closing satellite's widgets to the dock and reclaim its list
    /// signal as one operation. Repeated teardown is inert. Same quiescent-
    /// teardown reasoning as the desktop-multiwindow example: the sole
    /// `use_drop` callback runs after all render/event guards returned, and
    /// the captured `Rc` pins every owner below.
    pub fn close_satellite(&self, satellite: Satellite) -> bool {
        let model = self.model;
        let retired_owner = {
            let mut owners = self.satellite_owners.borrow_mut();
            if !owners.contains_key(&satellite.zone) {
                return false;
            }
            let mut satellites = model.satellites;
            let mut widgets = satellite.widgets;
            let mut dock = model.dock;
            let mut open = satellites.write();
            let mut leaving = widgets.write();
            let mut docked = dock.write();

            open.retain(|s| s.zone != satellite.zone);
            docked.extend(std::mem::take(&mut *leaving));
            owners
                .remove(&satellite.zone)
                .expect("the checked satellite owner remains locked until retirement")
        };
        drop(retired_owner);
        true
    }

    /// Fork a widget into an independently ticking twin: fresh id, state
    /// seeded from the source's CURRENT values, signal on the root owner so
    /// the clone outlives whichever window minted it.
    pub fn clone_widget(&self, widget: &Widget) -> Widget {
        let snapshot = widget.state.peek().clone();
        let state = dioxus::core::with_owner(self.root.clone(), || Signal::new(snapshot));
        Widget {
            id: self.mint_id(),
            kind: widget.kind,
            state,
        }
    }

    /// Apply a drop: `Copy` clones a live twin into the target while the
    /// source keeps the original; everything else (Move, and Link which this
    /// example treats as Move) relocates the dragged widget. A target zone
    /// that vanished mid-flight falls back to the dock - a widget can never
    /// be lost.
    pub fn deliver(&self, widget: Widget, to: ZoneId, effect: DropEffect) {
        let landing = match effect {
            DropEffect::Copy => self.clone_widget(&widget),
            _ => {
                self.detach(widget.id);
                widget
            }
        };
        let model = self.model;
        let satellite = model
            .satellites
            .peek()
            .iter()
            .find(|s| s.zone == to)
            .copied();
        match satellite {
            Some(s) => {
                let mut widgets = s.widgets;
                widgets.write().push(landing);
            }
            None => {
                let mut dock = model.dock;
                dock.write().push(landing);
            }
        }
    }

    fn detach(&self, id: u32) {
        let model = self.model;
        let mut dock = model.dock;
        dock.write().retain(|w| w.id != id);
        for satellite in model.satellites.peek().iter() {
            let mut widgets = satellite.widgets;
            widgets.write().retain(|w| w.id != id);
        }
    }

    /// Ticker ownership: exactly one window drives liveness at a time; a
    /// closing owner releases the claim so a survivor adopts it.
    pub fn claim_ticker(&self) -> bool {
        !self.ticker_claimed.swap(true, Ordering::AcqRel)
    }

    pub fn release_ticker(&self) {
        self.ticker_claimed.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    type OwnerSlot = Rc<RefCell<Option<Rc<ModelOwner>>>>;

    fn creator_window() -> Element {
        let slot = use_context::<OwnerSlot>();
        let owner = use_hook(ModelOwner::new);
        *slot.borrow_mut() = Some(owner);
        rsx! {}
    }

    fn closing_satellite_window() -> Element {
        let owner = use_context::<Rc<ModelOwner>>();
        let satellite = use_context::<Satellite>();
        use_drop(move || {
            let _ = owner.close_satellite(satellite);
        });
        rsx! {}
    }

    fn park_owner() -> (Rc<ModelOwner>, VirtualDom) {
        let slot = OwnerSlot::default();
        let mut creator = VirtualDom::new(creator_window).with_root_context(slot.clone());
        creator.rebuild_in_place();
        let owner = slot.borrow_mut().take().expect("creator parked the owner");
        (owner, creator)
    }

    #[test]
    fn satellite_close_returns_widgets_and_repeats_inert() {
        let (owner, creator) = park_owner();
        // Owner-creating and signal-creating calls need a current scope, not
        // just a runtime (Signal::new resolves the current scope id).
        let satellite = creator.in_scope(ScopeId::ROOT, || owner.new_satellite(1));
        creator.in_scope(ScopeId::ROOT, || {
            let mut satellites = owner.model.satellites;
            satellites.write().push(satellite);
        });
        let widget = owner.model.dock.peek()[0];
        creator.in_scope(ScopeId::ROOT, || {
            owner.deliver(widget, satellite.zone, DropEffect::Move)
        });
        assert_eq!(owner.model.dock.peek().len(), 3);
        assert_eq!(satellite.widgets.peek().len(), 1);

        let mut window = VirtualDom::new(closing_satellite_window)
            .with_root_context(owner.clone())
            .with_root_context(satellite);
        window.rebuild_in_place();
        drop(window);

        assert_eq!(owner.model.dock.peek().len(), 4);
        assert!(owner.model.satellites.peek().is_empty());
        assert!(satellite.widgets.try_read().is_err());
        assert!(!owner.close_satellite(satellite));
        assert_eq!(owner.model.dock.peek().len(), 4);
    }

    #[test]
    fn clone_widget_forks_independent_state() {
        let (owner, creator) = park_owner();
        let source = owner.model.dock.peek()[0];
        let clone = creator.in_scope(ScopeId::ROOT, || owner.clone_widget(&source));

        assert_ne!(clone.id, source.id);
        assert_eq!(clone.kind, source.kind);
        assert_eq!(*clone.state.peek(), *source.state.peek());

        creator.in_scope(ScopeId::ROOT, || {
            let mut state = clone.state;
            state.write().ticks = 999;
        });
        assert_eq!(clone.state.peek().ticks, 999);
        assert_eq!(source.state.peek().ticks, 0);
    }

    #[test]
    fn copy_delivery_grows_target_and_keeps_source() {
        let (owner, creator) = park_owner();
        let satellite = creator.in_scope(ScopeId::ROOT, || owner.new_satellite(1));
        creator.in_scope(ScopeId::ROOT, || {
            let mut satellites = owner.model.satellites;
            satellites.write().push(satellite);
        });
        let source = owner.model.dock.peek()[1];
        creator.in_scope(ScopeId::ROOT, || {
            owner.deliver(source, satellite.zone, DropEffect::Copy)
        });

        assert_eq!(
            owner.model.dock.peek().len(),
            4,
            "Copy must not detach the source"
        );
        assert!(owner.model.dock.peek().iter().any(|w| w.id == source.id));
        let landed = satellite.widgets.peek()[0];
        assert_ne!(landed.id, source.id);
        assert_eq!(landed.kind, source.kind);

        // An unknown zone can never lose a widget: it lands in the dock.
        creator.in_scope(ScopeId::ROOT, || {
            owner.deliver(landed, ZoneId(9999), DropEffect::Move)
        });
        assert!(satellite.widgets.peek().is_empty());
        assert_eq!(owner.model.dock.peek().len(), 5);
    }
}

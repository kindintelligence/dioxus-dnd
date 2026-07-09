//! First coverage for the `external::typed` transport (serde feature):
//! the store/retrieve round-trip through a real (mutable) `DataTransfer`,
//! error containment, and the component wrappers' rendered contracts.
//! The full drag arc through genuine browser `DragEvent`s lives in the
//! Playwright spec (`tests/browser/typed-transport.spec.js`) - headless
//! `DragEvent`s can't be synthesized, but the `DataTransfer` seam can.
#![cfg(feature = "serde")]

use std::collections::HashMap;
use std::sync::Mutex;

use dioxus::html::{DataTransfer, NativeDataTransfer};
use dioxus::prelude::*;
use dioxus_dnd::external::typed;
use dioxus_dnd::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Card {
    id: u32,
    name: String,
}

/// A mutable in-memory `DataTransfer` backend: dioxus's own
/// `SerializedDataTransfer` silently drops `set_data`, so the round-trip
/// needs a real store.
#[derive(Default)]
struct MemoryTransfer {
    items: Mutex<HashMap<String, String>>,
}

impl NativeDataTransfer for MemoryTransfer {
    fn get_data(&self, format: &str) -> Option<String> {
        self.items.lock().unwrap().get(format).cloned()
    }
    fn set_data(&self, format: &str, data: &str) -> Result<(), String> {
        self.items
            .lock()
            .unwrap()
            .insert(format.to_string(), data.to_string());
        Ok(())
    }
    fn clear_data(&self, format: Option<&str>) -> Result<(), String> {
        let mut items = self.items.lock().unwrap();
        match format {
            Some(f) => {
                items.remove(f);
            }
            None => items.clear(),
        }
        Ok(())
    }
    fn effect_allowed(&self) -> String {
        "uninitialized".into()
    }
    fn set_effect_allowed(&self, _effect: &str) {}
    fn drop_effect(&self) -> String {
        "none".into()
    }
    fn set_drop_effect(&self, _effect: &str) {}
    fn files(&self) -> Vec<dioxus::html::FileData> {
        Vec::new()
    }
}

#[test]
fn store_and_retrieve_round_trip() {
    let dt = DataTransfer::new(MemoryTransfer::default());
    let card = Card {
        id: 7,
        name: "seven".into(),
    };
    typed::store_in(&dt, &card).expect("store succeeds");

    // The wire format is plain JSON under the documented MIME.
    let raw = dt.get_data(typed::MIME).expect("entry written");
    assert!(raw.contains("\"id\":7"));

    let back: Option<Card> = typed::retrieve_from(&dt).expect("decodes");
    assert_eq!(back, Some(card));
}

#[test]
fn retrieve_distinguishes_absent_from_invalid() {
    // No typed entry at all: not a typed drag - Ok(None).
    let dt = DataTransfer::new(MemoryTransfer::default());
    assert_eq!(typed::retrieve_from::<Card>(&dt), Ok(None));

    // The DOM's getData yields "" for absent formats (never null), so an
    // empty entry is also "not a typed drag" - not a decode error.
    dt.set_data(typed::MIME, "").unwrap();
    assert_eq!(typed::retrieve_from::<Card>(&dt), Ok(None));

    // A typed entry that isn't a Card: an error, not a silent None.
    dt.set_data(typed::MIME, "{\"nope\": true}").unwrap();
    assert!(typed::retrieve_from::<Card>(&dt).is_err());

    // Type mismatch inside valid JSON is an error too.
    dt.set_data(typed::MIME, "{\"id\": \"seven\", \"name\": 7}")
        .unwrap();
    assert!(typed::retrieve_from::<Card>(&dt).is_err());
}

#[test]
fn wire_format_is_compatible_with_plain_json() {
    // Anything that writes plain JSON under the MIME (dioxus-html's own
    // helpers, another app, hand-rolled JS) is readable - and vice versa.
    let dt = DataTransfer::new(MemoryTransfer::default());
    dt.set_data(typed::MIME, "{\"id\": 3, \"name\": \"three\"}")
        .unwrap();
    let card: Option<Card> = typed::retrieve_from(&dt).expect("decodes");
    assert_eq!(
        card,
        Some(Card {
            id: 3,
            name: "three".into()
        })
    );
}

// --- component contracts (SSR) ------------------------------------------

#[test]
fn typed_drag_source_renders_draggable() {
    fn app() -> Element {
        rsx! {
            TypedDragSource::<Card> {
                payload: Card { id: 1, name: "one".into() },
                "drag me"
            }
            TypedDragSource::<Card> {
                payload: Card { id: 2, name: "two".into() },
                disabled: true,
                "not me"
            }
        }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(
        html.contains("draggable=true") || html.contains("draggable=\"true\""),
        "enabled source must be draggable, got: {html}"
    );
    assert!(
        html.contains("draggable=false") || html.contains("draggable=\"false\""),
        "disabled source must not be draggable, got: {html}"
    );
}

#[test]
fn typed_drop_zone_renders_without_hover_state() {
    fn app() -> Element {
        rsx! {
            TypedDropZone::<Card> {
                on_drop: move |_d: TypedDrop<Card>| {},
                "typed zone"
            }
        }
    }
    let mut dom = VirtualDom::new(app);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("typed zone"));
    assert!(
        !html.contains("data-over"),
        "not hovered until a drag enters"
    );
}

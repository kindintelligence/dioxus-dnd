#![doc = include_str!("../../docs/api/localization.md")]

use std::rc::Rc;

use dioxus::prelude::*;

/// A phrase taking two names (the zone, then its parent).
pub type TwoNamePhrase = Rc<dyn Fn(&str, &str) -> String>;

/// The crate's voice, one field per phrase. Every field is a function so
/// translations can reorder, inflect or pluralize freely; build it with
/// struct-update syntax over [`Default::default`] to override only what
/// you translate.
#[derive(Clone)]
pub struct DndStrings {
    /// Voiced when a keyboard drag picks an item up. Receives the
    /// draggable's `label`. This is also the user's manual - keep the
    /// key instructions (arrows, Enter, Escape) in the translation.
    pub picked_up: Rc<dyn Fn(&str) -> String>,
    /// Voiced when keyboard navigation reaches a zone. Receives the zone's
    /// name.
    pub over: Rc<dyn Fn(&str) -> String>,
    /// Voiced when keyboard navigation reaches a zone nested in a labeled
    /// parent. Receives the zone's name, then the parent's.
    pub over_inside: TwoNamePhrase,
    /// Voiced when an arrow key finds nowhere to go.
    pub no_targets: Rc<dyn Fn() -> String>,
    /// Voiced when Enter is pressed with no zone selected.
    pub no_target_selected: Rc<dyn Fn() -> String>,
    /// Voiced when a keyboard drop lands. Receives the zone's name.
    pub dropped_in: Rc<dyn Fn(&str) -> String>,
    /// Voiced when Escape cancels the drag.
    pub cancelled: Rc<dyn Fn() -> String>,
    /// Fallback name for a draggable with no `label`.
    pub item: Rc<dyn Fn() -> String>,
    /// Fallback name for a zone with no `label`. Receives the zone id's
    /// number.
    pub zone: Rc<dyn Fn(u64) -> String>,
    /// `ReorderButtons`: the up button's `aria-label`. Receives the row's
    /// name.
    pub move_up: Rc<dyn Fn(&str) -> String>,
    /// `ReorderButtons`: the down button's `aria-label`. Receives the
    /// row's name.
    pub move_down: Rc<dyn Fn(&str) -> String>,
    /// `ReorderButtons`: fallback name for a row with no `label`. Receives
    /// the 1-based row number.
    pub row: Rc<dyn Fn(usize) -> String>,
    /// `SelectionCount`: the badge text. Receives how many items are in
    /// flight - your chance at real plural rules.
    pub selection_count: Rc<dyn Fn(usize) -> String>,
}

impl Default for DndStrings {
    /// The built-in English.
    fn default() -> Self {
        Self {
            picked_up: Rc::new(|name| {
                format!(
                    "Picked up {name}. Use arrow keys to choose a drop target, \
                     Enter to drop, Escape to cancel."
                )
            }),
            over: Rc::new(|name| format!("Over {name}.")),
            over_inside: Rc::new(|name, parent| format!("Over {name}, inside {parent}.")),
            no_targets: Rc::new(|| "No drop targets available.".to_string()),
            no_target_selected: Rc::new(|| "No drop target selected.".to_string()),
            dropped_in: Rc::new(|name| format!("Dropped in {name}.")),
            cancelled: Rc::new(|| "Drag cancelled.".to_string()),
            item: Rc::new(|| "item".to_string()),
            zone: Rc::new(|n| format!("zone {n}")),
            move_up: Rc::new(|name| format!("Move {name} up")),
            move_down: Rc::new(|name| format!("Move {name} down")),
            row: Rc::new(|n| format!("item {n}")),
            selection_count: Rc::new(|n| format!("{n} item(s)")),
        }
    }
}

impl std::fmt::Debug for DndStrings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DndStrings")
    }
}

/// The subtree's [`DndStrings`], or the English defaults when no ancestor
/// provided one. Captured once per component instance - localize by having
/// the provided closures read your locale state, not by re-providing the
/// struct. Public so custom components voice themselves consistently.
pub fn use_dnd_strings() -> DndStrings {
    use_hook(|| try_consume_context::<DndStrings>().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The built-in English, pinned - these are user-facing contracts apps
    /// may key translations off.
    #[test]
    fn english_defaults() {
        let s = DndStrings::default();
        assert_eq!(
            (s.picked_up)("Piranesi"),
            "Picked up Piranesi. Use arrow keys to choose a drop target, \
             Enter to drop, Escape to cancel."
        );
        assert_eq!((s.over)("Done"), "Over Done.");
        assert_eq!((s.over_inside)("Done", "Board"), "Over Done, inside Board.");
        assert_eq!((s.no_targets)(), "No drop targets available.");
        assert_eq!((s.no_target_selected)(), "No drop target selected.");
        assert_eq!((s.dropped_in)("Done"), "Dropped in Done.");
        assert_eq!((s.cancelled)(), "Drag cancelled.");
        assert_eq!((s.item)(), "item");
        assert_eq!((s.zone)(7), "zone 7");
        assert_eq!((s.move_up)("Draft"), "Move Draft up");
        assert_eq!((s.move_down)("Draft"), "Move Draft down");
        assert_eq!((s.row)(3), "item 3");
        assert_eq!((s.selection_count)(3), "3 item(s)");
    }
}

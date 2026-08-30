#![doc = include_str!("../../../docs/api/drag-and-drop.md")]

use dioxus::prelude::*;

mod delivery;
mod draggable;
mod drop_zone;
mod handle;
mod overlay;
mod pointer;
mod provider;

pub use draggable::Draggable;
pub(crate) use drop_zone::FlatDropZone;
pub use drop_zone::{
    use_parent_zone, BridgeDropZone, BridgeParentZoneBoundary, DropZone, ParentZone,
};
pub use handle::{DragHandle, NoDrag};
pub use overlay::{DragOverlay, SettleSlot};
pub use provider::DndProvider;

pub(crate) use delivery::{
    deliver_drop, drop_query, resolve_drag_hover, resolve_drag_target, DropCompletion, SettleRoute,
    RELEASE_RECOVERY_MOVES,
};
pub(crate) use handle::ActivatorContext;
pub(crate) use overlay::overlay_style;
pub(crate) use pointer::{primary_press, touch_style, HoldTimer};

fn take_text_styles(attributes: &mut Vec<Attribute>) -> String {
    let mut styles = Vec::new();
    attributes.retain(|attribute| {
        if attribute.name != "style" {
            return true;
        }
        if let dioxus::core::AttributeValue::Text(style) = &attribute.value {
            styles.push(style.clone());
        }
        false
    });
    styles.join(" ")
}

/// Merge forwarded styles after configurable component defaults.
pub(crate) fn merge_style_user_last(attributes: &mut Vec<Attribute>, defaults: &str) -> String {
    let user = take_text_styles(attributes);
    format!("{defaults} {user}")
}

/// Merge behavior-critical component styles after every forwarded style.
///
/// Dioxus spreads land after static attributes, so all caller `style`
/// attributes must first be removed from the spread. Putting invariants last
/// then prevents declarations such as `touch-action` or `transform` from
/// disabling the component's behavior.
pub(crate) fn merge_style_invariant_last(
    attributes: &mut Vec<Attribute>,
    invariant: &str,
    invariant_properties: &[&str],
) -> String {
    // Dioxus also accepts each CSS declaration as an individual attribute
    // (`touch_action:`, `transform:`, ...). Those arrive with the `style`
    // namespace and, because the spread is later, would otherwise overwrite
    // the invariant even after every textual `style` fragment was merged.
    attributes.retain(|attribute| {
        attribute.namespace != Some("style") || !invariant_properties.contains(&attribute.name)
    });
    let user = take_text_styles(attributes);
    format!("{user} {invariant}")
}

/// Remove caller attributes whose later spread would replace an invariant
/// listener, state marker, or accessibility attribute owned by a component.
pub(crate) fn protect_attributes(attributes: &mut Vec<Attribute>, protected: &[&str]) {
    attributes.retain(|attribute| !protected.contains(&attribute.name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_listener_and_state_names_are_removed_but_other_attrs_survive() {
        let mut attributes = vec![
            Attribute::new("onclick", "caller", None, false),
            Attribute::new("data-active", "caller", None, false),
            Attribute::new("class", "card", None, false),
        ];

        protect_attributes(&mut attributes, &["onclick", "data-active"]);

        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].name, "class");
    }

    #[test]
    fn every_forwarded_style_is_consumed_in_order() {
        let mut attributes = vec![
            Attribute::new("style", "color: red;", None, false),
            Attribute::new("class", "card", None, false),
            Attribute::new("style", "opacity: .5;", None, false),
        ];

        let style = merge_style_user_last(&mut attributes, "display: grid;");

        assert_eq!(style, "display: grid; color: red; opacity: .5;");
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].name, "class");
    }

    #[test]
    fn invariant_styles_are_emitted_after_user_declarations() {
        let mut attributes = vec![Attribute::new(
            "style",
            "touch-action: auto; transform: scale(2);",
            None,
            false,
        )];

        let style = merge_style_invariant_last(
            &mut attributes,
            "touch-action: none; transform: translate(4px);",
            &["touch-action", "transform"],
        );

        assert_eq!(
            style,
            "touch-action: auto; transform: scale(2); touch-action: none; transform: translate(4px);"
        );
        assert!(attributes.is_empty());
    }

    #[test]
    fn invariant_style_namespace_properties_are_removed_selectively() {
        let mut attributes = vec![
            Attribute::new("touch-action", "auto", Some("style"), false),
            Attribute::new("transform", "scale(2)", Some("style"), false),
            Attribute::new("opacity", "0.5", Some("style"), false),
        ];

        let style = merge_style_invariant_last(
            &mut attributes,
            "touch-action: none; transform: none;",
            &["touch-action", "transform"],
        );

        assert_eq!(style, " touch-action: none; transform: none;");
        assert_eq!(attributes.len(), 1);
        assert_eq!(attributes[0].name, "opacity");
        assert_eq!(attributes[0].namespace, Some("style"));
    }
}

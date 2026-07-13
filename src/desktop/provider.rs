//! The correctly ordered per-window multi-window provider.

use dioxus::prelude::*;

use crate::core::world::WorldMembership;
use crate::core::{Direction, DndProvider, DndWorld};

use super::{use_window_geometry_feed, DragBridge};

/// Wire one dioxus-desktop window into a [`DndWorld`].
///
/// This component structurally enforces the ordering the desktop adapter
/// needs: its geometry feed is mounted above [`DndProvider`], and its
/// [`DragBridge`] is mounted inside the provider after the window joins.
/// Keep app-styled pieces such as `DragOverlay` and `LiveRegion` among the
/// children.
///
/// Mounting without a `DndWorld<T>` in context emits one warning and leaves
/// the provider with its normal isolated, single-window state. Nesting this
/// component below another same-payload `DndProvider<T>` also warns: replace
/// the old provider rather than wrapping it, because only the outer provider
/// may join a world. Create the world with [`crate::core::use_dnd_world`] and
/// seed spawned windows with [`DndWorld::vdom`] to avoid the fallback.
#[component]
pub fn MultiWindowProvider<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
    /// Layout direction forwarded to [`DndProvider`].
    #[props(default)]
    dir: Direction,
    children: Element,
) -> Element {
    let _ = phantom;
    use_window_geometry_feed();
    use_hook(|| {
        let has_world = try_consume_context::<DndWorld<T>>().is_some();
        let has_ancestor_provider = try_consume_context::<WorldMembership<T>>().is_some();
        if let Some(message) = wiring_warning(has_world, has_ancestor_provider) {
            tracing::warn!(
                target: "dioxus_dnd::desktop",
                payload_type = std::any::type_name::<T>(),
                "{message}"
            );
        }
    });

    rsx! {
        DndProvider::<T> { dir,
            DragBridge::<T> {}
            {children}
        }
    }
}

fn wiring_warning(has_world: bool, has_ancestor_provider: bool) -> Option<&'static str> {
    if has_ancestor_provider {
        Some(
            "MultiWindowProvider mounted beneath an existing DndProvider for the same payload; \
             replace that provider instead of wrapping it, and ensure a DndWorld is in context; \
             the nested provider's drags will otherwise remain isolated",
        )
    } else if !has_world {
        Some(
            "MultiWindowProvider mounted without a DndWorld in context; call use_dnd_world in one \
             window and create siblings with DndWorld::vdom; this window's drags will remain isolated",
        )
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiring_diagnostic_covers_missing_world_and_nested_provider() {
        assert!(wiring_warning(true, false).is_none());
        assert!(wiring_warning(false, false)
            .expect("missing world warning")
            .contains("without a DndWorld"));
        assert!(wiring_warning(true, true)
            .expect("nested provider warning")
            .contains("replace that provider"));
        assert!(wiring_warning(false, true)
            .expect("combined warning")
            .contains("ensure a DndWorld"));
    }
}

//! The [`DndProvider`] component: provides a `DndContext<T>` to a subtree.

use dioxus::prelude::*;

use crate::core::collision::ReleasePolicy;
use crate::core::hooks::{use_dnd_provider, use_zone_registry};
use crate::core::types::Direction;

/// Provides a `DndContext<T>` to its children.
#[component]
pub fn DndProvider<T: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<T>,
    /// Layout direction: `Direction::Rtl` mirrors keyboard navigation and
    /// spatial zone ordering to follow the visual right-to-left flow.
    #[props(default)]
    dir: Direction,
    /// Collision detector, recovery radius, and sticky-hover behavior for
    /// this provider's zones.
    #[props(default)]
    release: ReleasePolicy<T>,
    children: Element,
) -> Element {
    let _ = phantom;
    use_dnd_provider::<T>();
    let mut registry = use_zone_registry::<T>();
    // Seed the provider policy before children register or a headless VDOM
    // can drive a drag. The effect below keeps later prop changes reactive.
    use_hook(move || {
        registry.set_direction(dir);
        registry.set_release_policy(release);
    });
    use_effect(use_reactive!(|(dir, release)| {
        registry.set_direction(dir);
        registry.set_release_policy(release);
    }));
    rsx! {
        {children}
    }
}

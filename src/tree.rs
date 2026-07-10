//! Hierarchical drops - file explorers, nested menus, outliners.
//!
//! The classic tree problem: a drop on a node can mean three different things.
//! [`DropIntent`] captures that trichotomy, [`intent_from_offset`] derives it
//! from where inside the row the pointer sits (top quarter = before, bottom
//! quarter = after, middle = into), and [`would_create_cycle`] guards against
//! dropping a node into its own subtree.

use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use crate::core::{
    use_dnd, use_joined_window, use_zone_id, use_zone_registry, DragMode, DropOutcome, ParentZone,
    Rect, ZoneRecord,
};

/// Identifies a tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u64);

impl From<u64> for NodeId {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

/// Where, relative to the target node, the payload should land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropIntent {
    /// Insert as the target's previous sibling.
    Before,
    /// Insert as the target's next sibling.
    After,
    /// Insert as the target's child.
    Into,
}

/// A completed tree drop.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeDropEvent<T> {
    pub payload: T,
    pub target: NodeId,
    pub intent: DropIntent,
}

/// Derive a [`DropIntent`] from the pointer's Y offset within a row of the
/// given height. Top 25% → `Before`, bottom 25% → `After`, middle → `Into`.
///
/// If your rows can't receive children (a flat outline), map `Into` to
/// whichever sibling intent you prefer.
pub fn intent_from_offset(y: f64, row_height: f64) -> DropIntent {
    let h = row_height.max(1.0);
    let ratio = (y / h).clamp(0.0, 1.0);
    if ratio < 0.25 {
        DropIntent::Before
    } else if ratio > 0.75 {
        DropIntent::After
    } else {
        DropIntent::Into
    }
}

/// Would attaching `dragged` under `target` create a cycle? Walks `target`'s
/// ancestry via the `parent_of` lookup you provide.
pub fn would_create_cycle(
    parent_of: impl Fn(NodeId) -> Option<NodeId>,
    dragged: NodeId,
    target: NodeId,
) -> bool {
    if dragged == target {
        return true;
    }
    let mut cursor = Some(target);
    // Bounded walk in case the caller's parent map itself has a cycle.
    for _ in 0..10_000 {
        match cursor {
            Some(n) if n == dragged => return true,
            Some(n) => cursor = parent_of(n),
            None => return false,
        }
    }
    true
}

/// A single tree row that acts as a drop target with intent detection.
///
/// The payload type `T` travels through the shared `DndContext<T>` (use the
/// core `Draggable` on your rows to start drags).
/// While hovered, the wrapper carries `data-intent="before" | "after" |
/// "into"` for styling insertion indicators - for pointer (mouse, touch,
/// pen) and keyboard drags alike. The attribute is absent when not hovered,
/// so both value selectors (Tailwind `data-[intent=into]:bg-blue-50`) and
/// presence selectors (`data-intent:outline`) work.
///
/// Every target also registers itself in the shared zone registry, which is
/// what makes it reachable by pointer hit-testing and keyboard navigation.
/// Keyboard drops land with `Into` intent (the row's center band). At the
/// registry level a target accepts a payload if your `accepts` passes for
/// *any* intent; the exact intent is re-checked at drop time.
#[component]
pub fn TreeNodeTarget<T: Clone + PartialEq + 'static>(
    /// The node this row represents.
    node: NodeId,
    /// Height of the row in pixels, used for the before/into/after bands.
    /// Keep this close to the actual rendered row height: keyboard drops resolve
    /// their intent from the measured row center against this value, so a large
    /// mismatch (e.g. wrapped/custom content taller than the default) can bias a
    /// keyboard drop toward `After`/`Before` instead of `Into`.
    #[props(default = 28.0)]
    row_height: f64,
    /// Reject drops (typically: cycle prevention). Receives `(payload, intent)`.
    #[props(default)]
    accepts: Option<Callback<(T, DropIntent), bool>>,
    on_drop: EventHandler<TreeDropEvent<T>>,
    /// Announced to screen readers during keyboard navigation.
    #[props(default)]
    label: Option<String>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let dnd = use_dnd::<T>();
    let joined = use_joined_window::<T>();
    let mut registry = use_zone_registry::<T>();
    let mut label_now = use_signal(|| label.clone());
    let mut accepts_now = use_signal(|| accepts);
    let mut row_height_now = use_signal(|| row_height);
    let mut on_drop_now = use_signal(|| on_drop);
    let mut node_now = use_signal(|| node);

    if *label_now.peek() != label {
        label_now.set(label.clone());
    }
    if *accepts_now.peek() != accepts {
        accepts_now.set(accepts);
    }
    if *row_height_now.peek() != row_height {
        row_height_now.set(row_height);
    }
    if *on_drop_now.peek() != on_drop {
        on_drop_now.set(on_drop);
    }
    if *node_now.peek() != node {
        node_now.set(node);
    }

    // --- zone registration: makes this row a touch and keyboard target ----
    let zone_id = use_zone_id();
    let parent = try_use_context::<ParentZone>().map(|p| p.0);
    // Registry-level filter: would *any* intent be accepted? (Hover can't
    // know the final band yet; the exact intent is re-checked at drop.)
    let registered_accepts = Callback::new(move |p: T| match *accepts_now.peek() {
        Some(cb) => {
            cb.call((p.clone(), DropIntent::Before))
                || cb.call((p.clone(), DropIntent::After))
                || cb.call((p, DropIntent::Into))
        }
        None => true,
    });
    let registered_drop = Callback::new(move |o: DropOutcome<T>| {
        let it = intent_from_offset(o.element.y, *row_height_now.peek());
        let ok = match *accepts_now.peek() {
            Some(cb) => cb.call((o.payload.clone(), it)),
            None => true,
        };
        if ok {
            on_drop_now.peek().call(TreeDropEvent {
                payload: o.payload,
                target: *node_now.peek(),
                intent: it,
            });
        }
    });
    let registration = use_hook(move || {
        registry.register(ZoneRecord {
            id: zone_id,
            parent,
            label: label_now.peek().clone(),
            on_drop: registered_drop,
            accepts: Some(registered_accepts),
            mounted: None,
            rect: None,
        })
    });
    use_drop(move || {
        registry.unregister(zone_id);
    });
    // Keep the registered label in sync if the prop changes across renders.
    // Registry readers only `peek`, so this render-time write can't loop.
    registry.sync_label(zone_id, label.clone());

    // Pointer drags derive a live band from the shared pointer position, so
    // fingers see the same before/into/after feedback as mice.
    let display_intent = move || -> Option<DropIntent> {
        let over = match joined {
            Some(joined) => joined.is_over(zone_id),
            None => dnd.over() == Some(zone_id),
        };
        if dnd.dragging() && dnd.mode() == DragMode::Pointer && over {
            let r = registry.cached_rect(zone_id)?;
            let pointer = joined
                .and_then(|joined| joined.local_pointer())
                .unwrap_or_else(|| dnd.pointer());
            return Some(intent_from_offset(pointer.y - r.y, row_height));
        }
        None
    };
    let intent_str = move || -> Option<&'static str> {
        match display_intent() {
            Some(DropIntent::Before) => Some("before"),
            Some(DropIntent::After) => Some("after"),
            Some(DropIntent::Into) => Some("into"),
            None => None,
        }
    };
    rsx! {
        div {
            "data-intent": intent_str(),
            onmounted: move |evt: Event<MountedData>| {
                let m: Rc<MountedData> = evt.data();
                let mut registry = registry;
                registry.set_mounted(registration, m.clone());
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        registry.set_rect_if_present(registration, Rect::new(
                            r.origin.x,
                            r.origin.y,
                            r.size.width,
                            r.size.height,
                        ));
                    }
                });
            },
            ..attributes,
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_from_u64() {
        assert_eq!(NodeId::from(42), NodeId(42));
    }

    #[test]
    fn intent_bands() {
        assert_eq!(intent_from_offset(2.0, 28.0), DropIntent::Before);
        assert_eq!(intent_from_offset(14.0, 28.0), DropIntent::Into);
        assert_eq!(intent_from_offset(26.0, 28.0), DropIntent::After);
        // degenerate height doesn't divide by zero
        assert_eq!(intent_from_offset(0.0, 0.0), DropIntent::Before);
    }

    #[test]
    fn intent_bands_use_quarter_boundaries() {
        assert_eq!(intent_from_offset(24.9, 100.0), DropIntent::Before);
        assert_eq!(intent_from_offset(25.0, 100.0), DropIntent::Into);
        assert_eq!(intent_from_offset(75.0, 100.0), DropIntent::Into);
        assert_eq!(intent_from_offset(75.1, 100.0), DropIntent::After);
    }

    #[test]
    fn intent_bands_clamp_out_of_range_offsets() {
        assert_eq!(intent_from_offset(-20.0, 100.0), DropIntent::Before);
        assert_eq!(intent_from_offset(120.0, 100.0), DropIntent::After);
        assert_eq!(intent_from_offset(50.0, -10.0), DropIntent::After);
    }

    #[test]
    fn cycle_detection() {
        // 1 -> 2 -> 3 (3's parent is 2, 2's parent is 1)
        let parent = |n: NodeId| match n.0 {
            3 => Some(NodeId(2)),
            2 => Some(NodeId(1)),
            _ => None,
        };
        // dropping 1 into its grandchild 3 = cycle
        assert!(would_create_cycle(parent, NodeId(1), NodeId(3)));
        // dropping onto itself = cycle
        assert!(would_create_cycle(parent, NodeId(2), NodeId(2)));
        // dropping 3 into the root = fine
        assert!(!would_create_cycle(parent, NodeId(3), NodeId(1)));
    }

    #[test]
    fn cycle_detection_handles_missing_parents() {
        let parent = |n: NodeId| match n.0 {
            9 => Some(NodeId(8)),
            _ => None,
        };

        assert!(!would_create_cycle(parent, NodeId(1), NodeId(9)));
        assert!(!would_create_cycle(parent, NodeId(9), NodeId(1)));
    }

    #[test]
    fn cycle_detection_treats_parent_map_cycles_as_unsafe() {
        let parent = |n: NodeId| match n.0 {
            2 => Some(NodeId(3)),
            3 => Some(NodeId(2)),
            _ => None,
        };

        assert!(would_create_cycle(parent, NodeId(1), NodeId(2)));
    }
}

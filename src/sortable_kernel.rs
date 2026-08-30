//! Stable-ID sortable primitives built on the shared drag-and-drop core.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

use crate::core::components::FlatDropZone;
use crate::core::{
    ActivationPolicy, DndProvider, DragMode, Draggable, DropOutcome, DropQuery, DropZone, Edge,
    EdgeSet, Rect, ZoneId,
};
use crate::sortable::Axis;

const AUTO_GROUP_BASE: u64 = 1 << 32;
static NEXT_GROUP: AtomicU64 = AtomicU64::new(AUTO_GROUP_BASE);

fn checked_render_keys<K, F>(items: &[K], mut render_key: F) -> Vec<String>
where
    K: Clone + Eq + Hash,
    F: FnMut(K) -> String,
{
    let mut ids = HashSet::with_capacity(items.len());
    let mut keys = HashSet::with_capacity(items.len());
    items
        .iter()
        .cloned()
        .map(|item| {
            assert!(
                ids.insert(item.clone()),
                "SortableCollection item ids must be unique"
            );
            let key = render_key(item);
            assert!(
                keys.insert(key.clone()),
                "SortableCollection render keys must be unique"
            );
            key
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SortableGroupId(u64);

impl SortableGroupId {
    /// Construct an explicit group id from the namespace reserved for apps.
    pub const fn new(value: u64) -> Self {
        assert!(
            value < AUTO_GROUP_BASE,
            "explicit sortable group ids must be below 2^32"
        );
        Self(value)
    }

    pub fn auto() -> Self {
        let value = NEXT_GROUP
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .unwrap_or_else(|_| panic!("automatic sortable group id space exhausted"));
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for SortableGroupId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SortablePayload<K> {
    pub group: SortableGroupId,
    pub item: K,
    /// Item position at pickup time.
    pub position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Placement {
    Before,
    After,
    On,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum SortStrategy {
    #[default]
    LinearVertical,
    LinearHorizontal,
    GridInsert,
    GridSwap,
}

impl SortStrategy {
    pub fn linear(axis: Axis) -> Self {
        match axis {
            Axis::Vertical => Self::LinearVertical,
            Axis::Horizontal => Self::LinearHorizontal,
        }
    }

    fn edges(self) -> EdgeSet {
        match self {
            Self::LinearVertical | Self::GridInsert => EdgeSet::Vertical,
            Self::LinearHorizontal => EdgeSet::Horizontal,
            Self::GridSwap => EdgeSet::All,
        }
    }

    fn placement(self, edge: Option<Edge>) -> Placement {
        if self == Self::GridSwap {
            return Placement::On;
        }
        match edge {
            Some(Edge::Top | Edge::Left) => Placement::Before,
            Some(Edge::Bottom | Edge::Right) | None => Placement::After,
        }
    }
}

fn item_drop_placement(
    strategy: SortStrategy,
    mode: DragMode,
    active_group: SortableGroupId,
    target_group: SortableGroupId,
    active_position: usize,
    target_position: usize,
    edge: Option<Edge>,
) -> Placement {
    if strategy == SortStrategy::GridSwap {
        return Placement::On;
    }
    if mode != DragMode::Keyboard {
        return strategy.placement(edge);
    }
    if active_group != target_group {
        return Placement::Before;
    }
    if active_position > target_position {
        Placement::Before
    } else {
        Placement::After
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ReorderEvent<K> {
    pub active: K,
    /// Target item, or `None` for the group background/append target.
    pub over: Option<K>,
    pub from_group: SortableGroupId,
    pub to_group: SortableGroupId,
    pub placement: Placement,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DropPlacement<K> {
    /// Target item, or `None` to append after the last item.
    pub over: Option<K>,
    pub placement: Placement,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ItemTransform<K> {
    pub id: K,
    pub x: f64,
    pub y: f64,
}

impl<K> SortablePayload<K> {
    pub fn new(group: SortableGroupId, item: K, position: usize) -> Self {
        Self {
            group,
            item,
            position,
        }
    }
}

impl<K> ReorderEvent<K> {
    pub fn new(
        active: K,
        over: Option<K>,
        from_group: SortableGroupId,
        to_group: SortableGroupId,
        placement: Placement,
    ) -> Self {
        Self {
            active,
            over,
            from_group,
            to_group,
            placement,
        }
    }
}

impl<K> DropPlacement<K> {
    pub fn new(over: Option<K>, placement: Placement) -> Self {
        Self { over, placement }
    }
}

/// Project a stable-ID insertion by mapping each item to the measured slot
/// it would occupy after the move. Because slots come from real rects, this
/// does not assume a uniform row pitch.
pub fn project_layout<K>(
    items: &[K],
    rects: &HashMap<K, Rect>,
    active: &K,
    target: &DropPlacement<K>,
) -> Vec<ItemTransform<K>>
where
    K: Clone + Eq + Hash,
{
    let Some(active_index) = items.iter().position(|item| item == active) else {
        return Vec::new();
    };
    let mut projected = items.to_vec();
    let moved = projected.remove(active_index);
    let mut insertion = match target.over.as_ref() {
        Some(over) => {
            let Some(over_index) = items.iter().position(|item| item == over) else {
                return Vec::new();
            };
            match target.placement {
                Placement::Before | Placement::On => over_index,
                Placement::After => over_index + 1,
            }
        }
        None => items.len(),
    };
    if active_index < insertion {
        insertion = insertion.saturating_sub(1);
    }
    insertion = insertion.min(projected.len());
    projected.insert(insertion, moved);

    projected
        .iter()
        .enumerate()
        .filter_map(|(new_index, item)| {
            let old = rects.get(item)?;
            let slot_id = items.get(new_index)?;
            let slot = rects.get(slot_id)?;
            Some(ItemTransform {
                id: item.clone(),
                x: slot.x - old.x,
                y: slot.y - old.y,
            })
        })
        .collect()
}

/// Apply a same-list stable-ID reorder.
pub fn apply_reorder<K: PartialEq>(items: &mut Vec<K>, event: &ReorderEvent<K>) -> bool {
    if event.from_group != event.to_group {
        return false;
    }
    let Some(from) = items.iter().position(|item| item == &event.active) else {
        return false;
    };
    let item = items.remove(from);
    let mut to = match event.over.as_ref() {
        Some(over_item) => {
            let Some(over) = items.iter().position(|candidate| candidate == over_item) else {
                items.insert(from, item);
                return false;
            };
            match event.placement {
                Placement::Before | Placement::On => over,
                Placement::After => over + 1,
            }
        }
        None => items.len(),
    };
    to = to.min(items.len());
    if to == from {
        items.insert(from, item);
        return false;
    }
    items.insert(to, item);
    true
}

#[derive(Clone)]
struct GroupContext<K: 'static> {
    id: SortableGroupId,
    zone: ZoneId,
    strategy: Memo<SortStrategy>,
    activation: Memo<Option<ActivationPolicy>>,
    on_reorder: Callback<ReorderEvent<K>>,
}

/// Shared provider for one or more sortable groups. Place sibling groups
/// under the same provider to enable cross-group moves.
#[component]
pub fn SortableProvider<K: Clone + PartialEq + 'static>(
    #[props(default)] phantom: std::marker::PhantomData<K>,
    children: Element,
) -> Element {
    let _ = phantom;
    rsx! {
        DndProvider::<SortablePayload<K>> { {children} }
    }
}

/// Provide one stable-ID sortable group and a layout boundary for its
/// [`SortableItem`] children.
#[component]
pub fn SortableGroup<K: Clone + PartialEq + 'static>(
    on_reorder: EventHandler<ReorderEvent<K>>,
    #[props(default)] id: Option<SortableGroupId>,
    #[props(default)] strategy: SortStrategy,
    #[props(default)] activation: Option<ActivationPolicy>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let auto_group_id = use_hook(SortableGroupId::auto);
    let group_id = id.unwrap_or(auto_group_id);
    rsx! {
        for keyed_group_id in [group_id] {
            SortableGroupInstance::<K> {
                key: "{keyed_group_id}",
                group_id: keyed_group_id,
                strategy,
                activation: activation.clone(),
                on_reorder,
                attributes: attributes.clone(),
                {children.clone()}
            }
        }
    }
}

#[component]
fn SortableGroupInstance<K: Clone + PartialEq + 'static>(
    group_id: SortableGroupId,
    strategy: SortStrategy,
    activation: Option<ActivationPolicy>,
    on_reorder: EventHandler<ReorderEvent<K>>,
    attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let zone = use_hook(ZoneId::auto);
    let strategy_state = use_memo(use_reactive!(|strategy| strategy));
    let activation_state = use_memo(use_reactive!(|activation| activation));
    let reorder = use_callback(move |event| on_reorder.call(event));
    use_context_provider(|| GroupContext {
        id: group_id,
        zone,
        strategy: strategy_state,
        activation: activation_state,
        on_reorder: reorder,
    });

    rsx! {
        FlatDropZone::<SortablePayload<K>> {
            zone_id: zone,
            on_drop: move |outcome: DropOutcome<SortablePayload<K>>| {
                let active = outcome.payload;
                reorder.call(ReorderEvent {
                    active: active.item,
                    over: None,
                    from_group: active.group,
                    to_group: group_id,
                    placement: Placement::After,
                });
            },
            attributes,
            "data-sortable-group": "true",
            {children}
        }
    }
}

/// Render a complete stable-ID collection with one item wrapper per id.
/// Use [`SortableGroup`] and [`SortableItem`] directly for custom layouts.
#[component]
pub fn SortableCollection<K: Clone + Eq + Hash + 'static>(
    items: Vec<K>,
    render: Callback<K, Element>,
    /// Stable, unique Dioxus key for each semantic item id.
    item_key: Callback<K, String>,
    on_reorder: EventHandler<ReorderEvent<K>>,
    #[props(default)] id: Option<SortableGroupId>,
    #[props(default)] strategy: SortStrategy,
    #[props(default)] activation: Option<ActivationPolicy>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let render_keys = checked_render_keys(&items, |item| item_key.call(item));
    rsx! {
        SortableGroup::<K> {
            id,
            strategy,
            activation,
            on_reorder,
            attributes,
            for (position, (item, render_key)) in items.into_iter().zip(render_keys).enumerate() {
                SortableItem::<K> {
                    key: "{render_key}",
                    id: item.clone(),
                    position,
                    {render.call(item)}
                }
            }
        }
    }
}

/// One stable-ID sortable item. Use directly inside a custom group layout,
/// or let [`SortableGroup`] create it for every item.
#[component]
pub fn SortableItem<K: Clone + PartialEq + 'static>(
    id: K,
    /// Current zero-based position in the group.
    position: usize,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let group = use_context::<GroupContext<K>>();
    let payload = SortablePayload {
        group: group.id,
        item: id.clone(),
        position,
    };
    let target = id.clone();
    let accepts_target = id.clone();
    let strategy = *group.strategy.read();
    let activation = group.activation.read().clone();

    rsx! {
        div {
            "data-sortable-item": "true",
            ..attributes,
            DropZone::<SortablePayload<K>> {
                edge: strategy.edges(),
                accepts_query: move |query: DropQuery<SortablePayload<K>>| {
                    query.mode == DragMode::Pointer || query.payload.item != accepts_target
                },
                on_drop: move |outcome: DropOutcome<SortablePayload<K>>| {
                    let active = outcome.payload;
                    if active.group == group.id && active.item == target {
                        return;
                    }
                    let strategy = *group.strategy.peek();
                    let placement = item_drop_placement(
                        strategy,
                        outcome.mode,
                        active.group,
                        group.id,
                        active.position,
                        position,
                        outcome.edge,
                    );
                    group.on_reorder.call(ReorderEvent {
                        active: active.item,
                        over: Some(target.clone()),
                        from_group: active.group,
                        to_group: group.id,
                        placement,
                    });
                },
                Draggable::<SortablePayload<K>> {
                    payload,
                    zone: group.zone,
                    activation,
                    {children}
                }
            }
        }
    }
}

/// Semantic alias for the general core drag handle in sortable UIs.
pub use crate::core::DragHandle as SortableHandle;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_keys_come_directly_from_item_identity() {
        let initial = checked_render_keys(&["a", "b", "c"], str::to_owned);
        let reordered = checked_render_keys(&["c", "a", "b"], str::to_owned);

        assert_eq!(initial, ["a", "b", "c"]);
        assert_eq!(reordered, ["c", "a", "b"]);
    }

    #[test]
    #[should_panic(expected = "SortableCollection item ids must be unique")]
    fn duplicate_item_identity_is_rejected() {
        let _ = checked_render_keys(&["same", "same"], str::to_owned);
    }

    #[test]
    #[should_panic(expected = "SortableCollection render keys must be unique")]
    fn duplicate_render_key_is_rejected() {
        let _ = checked_render_keys(&["a", "b"], |_| "same".to_string());
    }

    #[test]
    fn stable_reorder_uses_identity_and_placement() {
        let group = SortableGroupId::new(1);
        let mut items = vec!["a", "b", "c", "d"];
        assert!(apply_reorder(
            &mut items,
            &ReorderEvent {
                active: "b",
                over: Some("d"),
                from_group: group,
                to_group: group,
                placement: Placement::After,
            },
        ));
        assert_eq!(items, ["a", "c", "d", "b"]);
    }

    #[test]
    fn dropping_after_self_is_a_no_op() {
        let group = SortableGroupId::new(1);
        let mut items = vec!["a", "b", "c"];
        let original = items.clone();

        assert!(!apply_reorder(
            &mut items,
            &ReorderEvent::new("b", Some("b"), group, group, Placement::After),
        ));
        assert_eq!(items, original);
    }

    #[test]
    fn projection_uses_measured_slots_for_variable_rows() {
        let items = vec![1, 2, 3];
        let rects = HashMap::from([
            (1, Rect::new(0.0, 0.0, 100.0, 20.0)),
            (2, Rect::new(0.0, 24.0, 100.0, 60.0)),
            (3, Rect::new(0.0, 88.0, 100.0, 30.0)),
        ]);
        let projected = project_layout(
            &items,
            &rects,
            &1,
            &DropPlacement {
                over: Some(3),
                placement: Placement::After,
            },
        );
        assert_eq!(
            projected[0],
            ItemTransform {
                id: 2,
                x: 0.0,
                y: -24.0
            }
        );
        assert_eq!(
            projected[2],
            ItemTransform {
                id: 1,
                x: 0.0,
                y: 88.0
            }
        );
    }

    #[test]
    fn automatic_and_explicit_group_ids_are_disjoint() {
        let explicit = SortableGroupId::new(1);
        let automatic = SortableGroupId::auto();
        assert_ne!(explicit, automatic);
        assert!(automatic.get() >= AUTO_GROUP_BASE);
    }

    #[test]
    fn keyboard_placement_uses_pickup_and_target_positions() {
        let group = SortableGroupId::new(1);
        assert_eq!(
            item_drop_placement(
                SortStrategy::LinearVertical,
                DragMode::Keyboard,
                group,
                group,
                3,
                0,
                None,
            ),
            Placement::Before
        );
        assert_eq!(
            item_drop_placement(
                SortStrategy::LinearVertical,
                DragMode::Keyboard,
                group,
                group,
                0,
                3,
                None,
            ),
            Placement::After
        );
        assert_eq!(
            item_drop_placement(
                SortStrategy::LinearVertical,
                DragMode::Keyboard,
                group,
                SortableGroupId::new(2),
                0,
                0,
                None,
            ),
            Placement::Before
        );
    }

    #[test]
    fn group_background_appends() {
        let group = SortableGroupId::new(1);
        let mut items = vec!["a", "b", "c"];
        assert!(apply_reorder(
            &mut items,
            &ReorderEvent::new("a", None, group, group, Placement::After),
        ));
        assert_eq!(items, ["b", "c", "a"]);
    }
}

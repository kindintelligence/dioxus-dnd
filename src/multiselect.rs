#![doc = include_str!("../docs/api/multiselect.md")]

use dioxus::prelude::*;

use crate::core::{use_dnd, DragMode, Draggable, DropEffect, ZoneId};

fn suppress_click_for_mode(mode: DragMode) -> bool {
    mode == DragMode::Pointer
}

/// Selection state for keys of type `K`. Cheap to copy.
pub struct Selection<K: Clone + PartialEq + 'static> {
    items: Signal<Vec<K>>,
    anchor: Option<Signal<Option<K>>>,
}

impl<K: Clone + PartialEq + 'static> Copy for Selection<K> {}
impl<K: Clone + PartialEq + 'static> Clone for Selection<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: Clone + PartialEq + 'static> PartialEq for Selection<K> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items && self.anchor == other.anchor
    }
}

impl<K: Clone + PartialEq + 'static> Selection<K> {
    /// Wrap an existing item signal without allocating hook state.
    ///
    /// This retains the pre-range-selection constructor contract and is safe
    /// outside component renders. Range operations derive their anchor from
    /// the first selected item; use [`use_selection_from_signal`] or
    /// [`Self::from_signals`] when the anchor must persist independently.
    pub fn from_signal(items: Signal<Vec<K>>) -> Self {
        Self {
            items,
            anchor: None,
        }
    }

    /// Wrap existing item and anchor signals.
    ///
    /// This is the non-hook constructor for state owned outside the current
    /// component. Supplying both signals makes range-anchor lifetime
    /// explicit and prevents it from being reset by reconstruction.
    pub fn from_signals(items: Signal<Vec<K>>, anchor: Signal<Option<K>>) -> Self {
        Self {
            items,
            anchor: Some(anchor),
        }
    }

    /// Is `key` currently selected?
    pub fn is_selected(&self, key: &K) -> bool {
        self.items.read().contains(key)
    }

    /// Replace the selection with just `key`.
    pub fn select_only(&mut self, key: K) {
        self.items.set(vec![key.clone()]);
        if let Some(mut anchor) = self.anchor {
            anchor.set(Some(key));
        }
    }

    /// Add or remove `key` (Ctrl/Cmd+click semantics). The range anchor
    /// moves to `key` either way, so a following Shift+click ranges from
    /// the item just toggled - the file-manager convention.
    pub fn toggle(&mut self, key: K) {
        {
            let mut items = self.items.write();
            if let Some(ix) = items.iter().position(|k| *k == key) {
                items.remove(ix);
            } else {
                items.push(key.clone());
            }
        }
        if let Some(mut anchor) = self.anchor {
            anchor.set(Some(key));
        }
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.items.write().clear();
        if let Some(mut anchor) = self.anchor {
            anchor.set(None);
        }
    }

    /// Snapshot of the selected keys, in selection order.
    pub fn items(&self) -> Vec<K> {
        self.items.read().clone()
    }

    /// Number of selected keys.
    pub fn len(&self) -> usize {
        self.items.read().len()
    }

    /// Is nothing selected?
    pub fn is_empty(&self) -> bool {
        self.items.read().is_empty()
    }

    /// Apply the standard click convention: plain click selects only this
    /// key; a click with Ctrl or Cmd held toggles it.
    pub fn click(&mut self, key: K, modifiers: Modifiers) {
        if modifiers.contains(Modifiers::CONTROL) || modifiers.contains(Modifiers::META) {
            self.toggle(key);
        } else {
            self.select_only(key);
        }
    }

    /// Select the inclusive range from the current anchor to `to` in the
    /// caller's stable visual order. Returns false when either endpoint is
    /// absent from `ordered`.
    pub fn select_range(&mut self, ordered: &[K], to: &K, additive: bool) -> bool {
        let anchor = self
            .anchor
            .and_then(|anchor| anchor.peek().clone())
            .or_else(|| self.items.peek().first().cloned())
            .unwrap_or_else(|| to.clone());
        let Some(from_index) = ordered.iter().position(|item| item == &anchor) else {
            return false;
        };
        let Some(to_index) = ordered.iter().position(|item| item == to) else {
            return false;
        };
        let (start, end) = if from_index <= to_index {
            (from_index, to_index)
        } else {
            (to_index, from_index)
        };
        let range = &ordered[start..=end];
        if additive {
            let mut selected = self.items.write();
            for item in range {
                if !selected.contains(item) {
                    selected.push(item.clone());
                }
            }
        } else {
            self.items.set(range.to_vec());
        }
        if let Some(mut anchor_state) = self.anchor {
            anchor_state.set(Some(anchor));
        }
        true
    }

    /// Standard click behavior plus Shift-range selection.
    pub fn click_in_order(&mut self, key: K, modifiers: Modifiers, ordered: &[K]) {
        if modifiers.contains(Modifiers::SHIFT) {
            let additive =
                modifiers.contains(Modifiers::CONTROL) || modifiers.contains(Modifiers::META);
            if self.select_range(ordered, &key, additive) {
                return;
            }
        }
        self.click(key, modifiers);
    }

    /// Move a keyboard focus index and optionally extend selection from the
    /// anchor. Returns the new index, clamped to the collection.
    pub fn keyboard_range(
        &mut self,
        ordered: &[K],
        current: usize,
        step: isize,
        extend: bool,
    ) -> Option<usize> {
        if ordered.is_empty() {
            return None;
        }
        let current = current.min(ordered.len() - 1);
        let next = current.saturating_add_signed(step).min(ordered.len() - 1);
        if extend {
            if let Some(mut anchor) = self.anchor {
                if anchor.peek().is_none() {
                    anchor.set(Some(ordered[current].clone()));
                }
            } else if self.items.peek().is_empty() {
                // Give the stateless compatibility wrapper an anchor its
                // ordinary first-selected fallback can derive.
                self.items.set(vec![ordered[current].clone()]);
            }
            self.select_range(ordered, &ordered[next], false);
        } else {
            self.select_only(ordered[next].clone());
        }
        Some(next)
    }
}

/// Wrap an existing item signal with a range anchor owned by this component.
/// Call this unconditionally during render, like [`use_selection`].
pub fn use_selection_from_signal<K: Clone + PartialEq + 'static>(
    items: Signal<Vec<K>>,
) -> Selection<K> {
    Selection::from_signals(items, use_signal(|| None))
}

/// Create selection state owned by this component.
pub fn use_selection<K: Clone + PartialEq + 'static>() -> Selection<K> {
    Selection {
        items: use_signal(Vec::new),
        anchor: Some(use_signal(|| None)),
    }
}

/// A draggable list/grid item participating in a selection.
///
/// - Click / Ctrl+click manage the selection (via [`Selection::click`]).
/// - Dragging a selected item picks up **the whole selection**; dragging an
///   unselected one picks up just that item (the selection is unchanged).
/// - Works with mouse, touch, pen and keyboard.
/// - The wrapper exposes `data-selected="true"` for styling (absent when
///   unselected, so presence-based selectors like Tailwind
///   `data-selected:ring-2` work directly).
///
/// Requires a `DndProvider::<Vec<K>>` ancestor.
#[component]
pub fn SelectableDraggable<K: Clone + PartialEq + 'static>(
    /// This item's key.
    item: K,
    /// Shared selection state from [`use_selection`].
    selection: Selection<K>,
    /// The zone this item lives in.
    #[props(default)]
    zone: Option<ZoneId>,
    /// Drop effect. Defaults to `Move`.
    #[props(default)]
    effect: DropEffect,
    /// Label for screen-reader announcements.
    #[props(default)]
    label: Option<String>,
    #[props(extends = div, extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let dnd = use_dnd::<Vec<K>>(); // fail fast with a clear panic if unprovided
    let selected = selection.is_selected(&item);
    // Payload resolved from *current* selection each render: a selected item
    // drags the group, an unselected one drags itself.
    let payload = if selected {
        selection.items()
    } else {
        vec![item.clone()]
    };
    let click_key = item.clone();
    let mut selection = selection;
    // The browser fires a trailing `click` on the source after a completed
    // pointer drag; letting it through would collapse the just-dragged
    // multi-selection to this one item. Drag start arms the flag, the next
    // click consumes it - exactly one trailing click is swallowed.
    let mut suppress_pointer_click = use_signal(|| false);
    let mut attributes = attributes;
    crate::core::components::protect_attributes(
        &mut attributes,
        &["data-selected", "onclick", "onpointerdown"],
    );

    rsx! {
        div {
            "data-selected": if selected { "true" },
            onclick: move |evt: MouseEvent| {
                if *suppress_pointer_click.peek() {
                    suppress_pointer_click.set(false);
                    return;
                }
                selection.click(click_key.clone(), evt.modifiers());
            },
            ..attributes,
            Draggable::<Vec<K>> {
                payload,
                zone,
                effect,
                label,
                on_drag_start: move |_| {
                    if suppress_click_for_mode(dnd.mode()) {
                        suppress_pointer_click.set(true);
                    }
                },
                on_drag_end: move |dropped: bool| {
                    if !dropped {
                        suppress_pointer_click.set(false);
                    }
                },
                div {
                    onpointerdown: move |_| {
                        // This surface runs before Draggable's root stops
                        // pointerdown propagation. The outer selection
                        // surface owns click because pointer capture retargets
                        // pointerup (and therefore click) to that root.
                        if !dnd.dragging() && *suppress_pointer_click.peek() {
                            suppress_pointer_click.set(false);
                        }
                    },
                    {children}
                }
            }
        }
    }
}

/// A "N items" badge for the drag ghost. Render inside
/// `DragOverlay::<Vec<K>>`; shows the size of the payload being dragged.
#[component]
pub fn SelectionCount<K: Clone + PartialEq + 'static>(
    /// Internal marker; never set this.
    #[props(default)]
    phantom: std::marker::PhantomData<K>,
) -> Element {
    let _ = phantom;
    let dnd = use_dnd::<Vec<K>>();
    let strings = crate::core::use_dnd_strings();
    let n = dnd.payload().map(|p| p.len()).unwrap_or(0);
    let text = (strings.selection_count)(n);
    rsx! {
        span { "{text}" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pointer_drags_arm_browser_click_suppression() {
        assert!(suppress_click_for_mode(DragMode::Pointer));
        assert!(!suppress_click_for_mode(DragMode::Keyboard));
    }

    fn ctrl_click_anchor_probe() -> Element {
        let mut selection = use_selection::<u8>();
        let ordered = [1u8, 2, 3, 4, 5];
        selection.click_in_order(1, Modifiers::empty(), &ordered);
        // Ctrl+click adds 3 and moves the anchor there...
        selection.click_in_order(3, Modifiers::CONTROL, &ordered);
        assert_eq!(selection.items(), vec![1, 3]);
        // ...so the Shift+click ranges 3..=5, not 1..=5.
        selection.click_in_order(5, Modifiers::SHIFT, &ordered);
        assert_eq!(selection.items(), vec![3, 4, 5]);
        // Toggling an item off also re-anchors on it.
        selection.click_in_order(4, Modifiers::CONTROL, &ordered);
        assert_eq!(selection.items(), vec![3, 5]);
        selection.click_in_order(2, Modifiers::SHIFT, &ordered);
        assert_eq!(selection.items(), vec![2, 3, 4]);
        rsx! {}
    }

    #[test]
    fn ctrl_click_moves_the_range_anchor() {
        let mut dom = VirtualDom::new(ctrl_click_anchor_probe);
        dom.rebuild_in_place();
    }
}

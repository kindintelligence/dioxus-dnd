//! The zone registry: every mounted [`crate::core::DropZone`] records itself
//! here (id, label, drop callback, acceptance filter, and its mounted DOM
//! handle). Pointer drags hit-test against cached client rects; keyboard
//! navigation walks the zones in spatial order (top-to-bottom, left-to-right,
//! with unmeasured zones last in registration order).

use std::rc::Rc;

use dioxus::html::MountedData;
use dioxus::prelude::*;

use super::types::{DropOutcome, Point, Rect, ZoneId};

/// One registered drop zone.
pub struct ZoneRecord<T: Clone + 'static> {
    pub id: ZoneId,
    /// The enclosing zone, when this zone is nested inside another
    /// `DropZone` (discovered automatically via context).
    pub parent: Option<ZoneId>,
    /// Human label used in screen-reader announcements.
    pub label: Option<String>,
    /// Delivers a completed drop to the zone's owner.
    pub on_drop: Callback<DropOutcome<T>>,
    /// The zone's acceptance filter, if any.
    pub accepts: Option<Callback<T, bool>>,
    /// The zone's mounted element, once available.
    pub mounted: Signal<Option<Rc<MountedData>>>,
    /// Cached client rect (refreshed via [`ZoneRegistry::refresh_rects`]).
    pub rect: Signal<Option<Rect>>,
}

impl<T: Clone + 'static> Clone for ZoneRecord<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            parent: self.parent,
            label: self.label.clone(),
            on_drop: self.on_drop,
            accepts: self.accepts,
            mounted: self.mounted,
            rect: self.rect,
        }
    }
}

impl<T: Clone + 'static> ZoneRecord<T> {
    /// Does this zone accept the payload?
    pub fn accepts_payload(&self, payload: &T) -> bool {
        match self.accepts {
            Some(cb) => cb.call(payload.clone()),
            None => true,
        }
    }
}

/// Registry of the currently mounted drop zones, in mount order.
pub struct ZoneRegistry<T: Clone + 'static> {
    zones: Signal<Vec<ZoneRecord<T>>>,
}

impl<T: Clone + 'static> Copy for ZoneRegistry<T> {}
impl<T: Clone + 'static> Clone for ZoneRegistry<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: Clone + 'static> PartialEq for ZoneRegistry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.zones == other.zones
    }
}

impl<T: Clone + 'static> ZoneRegistry<T> {
    /// Wrap an existing signal. Prefer [`crate::core::hooks::use_dnd_provider`].
    pub fn from_signal(zones: Signal<Vec<ZoneRecord<T>>>) -> Self {
        Self { zones }
    }

    /// Add (or replace, by id) a zone.
    pub fn register(&mut self, record: ZoneRecord<T>) {
        let mut zones = self.zones.write();
        if let Some(existing) = zones.iter_mut().find(|z| z.id == record.id) {
            *existing = record;
        } else {
            zones.push(record);
        }
    }

    /// Update a zone's label in place (no-op if unchanged or unknown).
    pub fn sync_label(&mut self, id: ZoneId, label: Option<String>) {
        let needs = self
            .zones
            .peek()
            .iter()
            .any(|z| z.id == id && z.label != label);
        if needs {
            if let Some(z) = self.zones.write().iter_mut().find(|z| z.id == id) {
                z.label = label;
            }
        }
    }

    /// Remove a zone (call when its component unmounts).
    pub fn unregister(&mut self, id: ZoneId) {
        self.zones.write().retain(|z| z.id != id);
    }

    /// Look up a zone by id.
    pub fn get(&self, id: ZoneId) -> Option<ZoneRecord<T>> {
        self.zones.peek().iter().find(|z| z.id == id).cloned()
    }

    /// Is a zone with this id registered *here*? The parent-zone context is
    /// shared across payload types, so a record's `parent` can name a zone
    /// living in another type's registry - check before navigating to one.
    pub fn contains(&self, id: ZoneId) -> bool {
        self.zones.peek().iter().any(|z| z.id == id)
    }

    /// The zone keyboard navigation should enter when ascending from
    /// `current`: its parent, but only when that parent is registered in
    /// this registry. A `DropZone<A>` nested inside a `DropZone<B>` records
    /// B's id as its parent, and entering an id this registry can't resolve
    /// would leave the drag hovering a zone that can never receive it.
    pub fn ascend(&self, current: ZoneId) -> Option<ZoneId> {
        self.parent_of(current).filter(|pid| self.contains(*pid))
    }

    /// All zones accepting `payload`, in registration order.
    pub fn acceptable(&self, payload: &T) -> Vec<ZoneRecord<T>> {
        self.zones
            .peek()
            .iter()
            .filter(|z| z.accepts_payload(payload))
            .cloned()
            .collect()
    }

    /// The next/previous zone (cyclic) relative to `current` among zones that
    /// accept `payload`. `step` is `+1` or `-1`.
    ///
    /// Order is **spatial** (top-to-bottom, then left-to-right) for zones
    /// with measured rects - call [`Self::refresh_rects`] first, as the
    /// built-in keyboard interaction does on pickup. Unmeasured zones keep
    /// registration order, after the measured ones.
    pub fn step_zone(&self, current: Option<ZoneId>, payload: &T, step: isize) -> Option<ZoneId> {
        let mut zones = self.acceptable(payload);
        spatial_sort(&mut zones);
        let current_ix = current.and_then(|c| zones.iter().position(|z| z.id == c));
        cycle(zones.len(), current_ix, step).map(|ix| zones[ix].id)
    }

    /// The parent of a zone, if it's nested.
    pub fn parent_of(&self, id: ZoneId) -> Option<ZoneId> {
        self.zones.peek().iter().find(|z| z.id == id)?.parent
    }

    /// Zones directly inside `parent` (`None` = root level) that accept
    /// `payload`, in spatial order (top-to-bottom, left-to-right; unmeasured
    /// zones keep registration order at the end).
    pub fn children_of(&self, parent: Option<ZoneId>, payload: &T) -> Vec<ZoneRecord<T>> {
        let mut zones: Vec<_> = self
            .zones
            .peek()
            .iter()
            .filter(|z| z.parent == parent && z.accepts_payload(payload))
            .cloned()
            .collect();
        spatial_sort(&mut zones);
        zones
    }

    /// Next/previous zone (cyclic) among the *siblings* of `current` -
    /// zones sharing its parent. With no `current`, cycles the root level.
    pub fn step_sibling(
        &self,
        current: Option<ZoneId>,
        payload: &T,
        step: isize,
    ) -> Option<ZoneId> {
        let parent = current.and_then(|c| self.parent_of(c));
        let siblings = self.children_of(parent, payload);
        let current_ix = current.and_then(|c| siblings.iter().position(|z| z.id == c));
        cycle(siblings.len(), current_ix, step).map(|ix| siblings[ix].id)
    }

    /// The first (spatially) acceptable zone nested inside `id`.
    pub fn first_child(&self, id: ZoneId, payload: &T) -> Option<ZoneId> {
        self.children_of(Some(id), payload).first().map(|z| z.id)
    }

    /// Topmost zone containing `point` (client coordinates), using cached
    /// rects - call [`Self::refresh_rects`] when a drag starts. Later-mounted
    /// zones win, approximating DOM paint order.
    pub fn hit_test(&self, point: Point) -> Option<ZoneId> {
        self.zones
            .peek()
            .iter()
            .rev()
            .find(|z| (*z.rect.peek()).map(|r| r.contains(point)).unwrap_or(false))
            .map(|z| z.id)
    }

    /// Like [`Self::hit_test`], but acceptance-aware: it returns the topmost
    /// zone that both contains the point **and** accepts `payload`, and when no
    /// such zone contains the point, falls back to the acceptable zone whose
    /// center is nearest - within `max_distance` CSS px. Skipping zones that
    /// reject the payload lets a drop land on an accepting zone sitting *under*
    /// a rejecting (or decorative) one, and is friendlier for imprecise (touch)
    /// drops that land in the gutter between zones.
    pub fn hit_test_closest(&self, point: Point, payload: &T, max_distance: f64) -> Option<ZoneId> {
        if let Some(hit) = self
            .zones
            .peek()
            .iter()
            .rev()
            .find(|z| {
                z.accepts_payload(payload)
                    && (*z.rect.peek()).map(|r| r.contains(point)).unwrap_or(false)
            })
            .map(|z| z.id)
        {
            return Some(hit);
        }
        let mut best: Option<(ZoneId, f64)> = None;
        for z in self.acceptable(payload) {
            let Some(r) = *z.rect.peek() else { continue };
            let c = r.center();
            let (dx, dy) = (c.x - point.x, c.y - point.y);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= max_distance && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((z.id, d));
            }
        }
        best.map(|(id, _)| id)
    }

    /// Re-measure every mounted zone's client rect and **wait** for the
    /// measurements to land - unlike [`Self::refresh_rects`], which fires
    /// and forgets. Use before a hit-test that must see fresh geometry
    /// (e.g. retrying a missed touch drop after a layout change).
    pub async fn measure_all(&self) {
        let zones: Vec<_> = self
            .zones
            .peek()
            .iter()
            .map(|z| (z.mounted.peek().clone(), z.rect))
            .collect();
        for (mounted, mut rect) in zones {
            if let Some(m) = mounted {
                if let Ok(r) = m.get_client_rect().await {
                    rect.set(Some(Rect::new(
                        r.origin.x,
                        r.origin.y,
                        r.size.width,
                        r.size.height,
                    )));
                }
            }
        }
    }

    /// Re-measure every mounted zone's client rect (async, spawned).
    pub fn refresh_rects(&self) {
        for zone in self.zones.peek().iter() {
            let mounted = zone.mounted.peek().clone();
            let mut rect = zone.rect;
            if let Some(m) = mounted {
                spawn(async move {
                    if let Ok(r) = m.get_client_rect().await {
                        rect.set(Some(Rect::new(
                            r.origin.x,
                            r.origin.y,
                            r.size.width,
                            r.size.height,
                        )));
                    }
                });
            }
        }
    }
}

/// Sort zones spatially: measured rects by (top, left), unmeasured last in
/// their original relative order.
fn spatial_sort<T: Clone + 'static>(zones: &mut [ZoneRecord<T>]) {
    zones.sort_by(|a, b| match (*a.rect.peek(), *b.rect.peek()) {
        (Some(ra), Some(rb)) => (ra.y, ra.x)
            .partial_cmp(&(rb.y, rb.x))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}

/// Cyclic index stepping: `None` current starts at the first (or last)
/// element depending on direction. Pure, for testability.
pub(crate) fn cycle(len: usize, current: Option<usize>, step: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match current {
        None => {
            if step >= 0 {
                0
            } else {
                len - 1
            }
        }
        Some(ix) => (ix as isize + step).rem_euclid(len as isize) as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::cycle;

    #[test]
    fn cycle_steps_and_wraps() {
        assert_eq!(cycle(0, None, 1), None);
        assert_eq!(cycle(3, None, 1), Some(0));
        assert_eq!(cycle(3, None, -1), Some(2));
        assert_eq!(cycle(3, Some(2), 1), Some(0));
        assert_eq!(cycle(3, Some(0), -1), Some(2));
        assert_eq!(cycle(3, Some(1), 1), Some(2));
    }
}

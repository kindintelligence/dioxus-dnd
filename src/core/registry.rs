//! The zone registry: every mounted [`crate::core::DropZone`] records itself
//! here (id, label, drop callback, acceptance filter, and its mounted DOM
//! handle). Pointer drags hit-test against cached client rects; keyboard
//! navigation walks the zones in spatial order (top-to-bottom, left-to-right,
//! with unmeasured zones last in registration order).

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::html::MountedData;
use dioxus::prelude::*;

use super::types::{Direction, DropOutcome, Point, Rect, ZoneId};

// Identity freshness only: Relaxed is sufficient because the counter carries
// no synchronization. Correctness assumes this process-lifetime u64 never
// wraps; do not narrow it.
static NEXT_ZONE_REGISTRATION: AtomicU64 = AtomicU64::new(1);

fn trace_registry_failure(
    operation: &'static str,
    storage: &'static str,
    zone: Option<ZoneId>,
    generation: Option<u64>,
    error: &impl std::fmt::Display,
) {
    tracing::trace!(
        target: "dioxus_dnd::registry",
        operation,
        storage,
        zone_id = ?zone,
        registration_generation = ?generation,
        error = %error,
        "zone registry operation skipped"
    );
}

/// Identifies one particular registration of a [`ZoneId`].
///
/// A zone id can be replaced in place. Async measurements carry this token
/// so a result started for the old registration cannot land in its
/// same-id replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZoneRegistration {
    id: ZoneId,
    generation: u64,
}

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
    /// The zone's mounted element, once available. This plain value lives in
    /// the provider-owned registry storage; zones update it through
    /// [`ZoneRegistry::set_mounted`].
    pub mounted: Option<Rc<MountedData>>,
    /// Cached client rect (refreshed via [`ZoneRegistry::refresh_rects`]).
    /// This plain value lives in the provider-owned registry storage; zones
    /// update it through [`ZoneRegistry::set_rect_if_present`].
    pub rect: Option<Rect>,
}

impl<T: Clone + 'static> Clone for ZoneRecord<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            parent: self.parent,
            label: self.label.clone(),
            on_drop: self.on_drop,
            accepts: self.accepts,
            mounted: self.mounted.clone(),
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

    /// The cached client rect in this registry snapshot.
    pub fn cached_rect(&self) -> Option<Rect> {
        self.rect
    }

    /// The mounted element in this registry snapshot.
    pub fn mounted_handle(&self) -> Option<Rc<MountedData>> {
        self.mounted.clone()
    }
}

/// Registry of the currently registered drop zones, in registration order.
pub struct ZoneRegistry<T: Clone + 'static> {
    zones: Signal<Vec<ZoneRecord<T>>>,
    /// Current generation for each id in `zones`. Kept separately so
    /// `ZoneRecord` remains constructible with a public struct literal.
    registrations: Signal<Vec<(ZoneId, u64)>>,
    /// Changes only when the zone set or a mounted handle changes. The debug
    /// overlay subscribes here so rect writes cannot retrigger measurement.
    mount_revision: Signal<u64>,
    /// Layout direction for spatial ordering (keyboard navigation).
    dir: Signal<Direction>,
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
            && self.registrations == other.registrations
            && self.mount_revision == other.mount_revision
            && self.dir == other.dir
    }
}

impl<T: Clone + 'static> ZoneRegistry<T> {
    /// Wrap an existing signal. Prefer [`crate::core::hooks::use_dnd_provider`].
    pub fn from_signal(zones: Signal<Vec<ZoneRecord<T>>>) -> Self {
        Self {
            zones,
            registrations: Signal::new(Vec::new()),
            mount_revision: Signal::new(0),
            dir: Signal::new(Direction::default()),
        }
    }

    /// Layout direction spatial ordering follows.
    pub fn direction(&self) -> Direction {
        self.dir.try_peek().map(|dir| *dir).unwrap_or_default()
    }

    /// Set the layout direction (no-op if unchanged; safe to call every
    /// render). `DndProvider`'s `dir` prop calls this for you.
    pub fn set_direction(&mut self, dir: Direction) {
        let changed = match self.dir.try_peek() {
            Ok(current) => *current != dir,
            Err(error) => {
                trace_registry_failure("set_direction", "dir", None, None, &error);
                return;
            }
        };
        if changed {
            match self.dir.try_write() {
                Ok(mut current) => *current = dir,
                Err(error) => trace_registry_failure("set_direction", "dir", None, None, &error),
            }
        }
    }

    /// Add (or replace, by id) a zone.
    pub fn register(&mut self, record: ZoneRecord<T>) -> ZoneRegistration {
        let registration = ZoneRegistration {
            id: record.id,
            generation: NEXT_ZONE_REGISTRATION.fetch_add(1, Ordering::Relaxed),
        };
        // Acquire both halves before mutating either. A runtime borrow
        // collision must not leave `zones` and `registrations` disagreeing.
        let mut zones = match self.zones.try_write() {
            Ok(zones) => zones,
            Err(error) => {
                trace_registry_failure(
                    "register",
                    "zones",
                    Some(registration.id),
                    Some(registration.generation),
                    &error,
                );
                return registration;
            }
        };
        let mut registrations = match self.registrations.try_write() {
            Ok(registrations) => registrations,
            Err(error) => {
                trace_registry_failure(
                    "register",
                    "registrations",
                    Some(registration.id),
                    Some(registration.generation),
                    &error,
                );
                return registration;
            }
        };
        if let Some(existing) = zones.iter_mut().find(|z| z.id == record.id) {
            *existing = record;
        } else {
            zones.push(record);
        }
        if let Some(existing) = registrations
            .iter_mut()
            .find(|(id, _)| *id == registration.id)
        {
            existing.1 = registration.generation;
        } else {
            registrations.push((registration.id, registration.generation));
        }
        drop(registrations);
        drop(zones);
        self.bump_mount_revision();
        registration
    }

    /// Update a zone's label in place (no-op if unchanged or unknown).
    pub fn sync_label(&mut self, id: ZoneId, label: Option<String>) {
        let needs = match self.zones.try_peek() {
            Ok(zones) => zones.iter().any(|z| z.id == id && z.label != label),
            Err(error) => {
                trace_registry_failure("sync_label", "zones", Some(id), None, &error);
                return;
            }
        };
        if needs {
            match self.zones.try_write() {
                Ok(mut zones) => {
                    if let Some(z) = zones.iter_mut().find(|z| z.id == id) {
                        z.label = label;
                    }
                }
                Err(error) => trace_registry_failure("sync_label", "zones", Some(id), None, &error),
            }
        }
    }

    /// Remove a zone (call when its component unmounts).
    pub fn unregister(&mut self, id: ZoneId) {
        // Structural state is a pair; acquire both guards before changing it.
        let mut zones = match self.zones.try_write() {
            Ok(zones) => zones,
            Err(error) => {
                trace_registry_failure("unregister", "zones", Some(id), None, &error);
                return;
            }
        };
        let mut registrations = match self.registrations.try_write() {
            Ok(registrations) => registrations,
            Err(error) => {
                trace_registry_failure("unregister", "registrations", Some(id), None, &error);
                return;
            }
        };
        let old_len = zones.len();
        zones.retain(|z| z.id != id);
        let removed = zones.len() != old_len;
        registrations.retain(|(registered_id, _)| *registered_id != id);
        drop(registrations);
        drop(zones);
        if removed {
            self.bump_mount_revision();
        }
    }

    /// Attach the mounted element to this exact registration. A stale
    /// registration token is ignored.
    pub fn set_mounted(&mut self, registration: ZoneRegistration, mounted: Rc<MountedData>) {
        if !self.is_current(registration, "set_mounted") {
            return;
        }
        let mut changed = false;
        match self.zones.try_write() {
            Ok(mut zones) => {
                if let Some(zone) = zones.iter_mut().find(|z| z.id == registration.id) {
                    zone.mounted = Some(mounted);
                    changed = true;
                }
            }
            Err(error) => {
                trace_registry_failure(
                    "set_mounted",
                    "zones",
                    Some(registration.id),
                    Some(registration.generation),
                    &error,
                );
            }
        }
        if changed {
            self.bump_mount_revision();
        }
    }

    /// Store a rect only while the registration that requested it is still
    /// current. This never inserts a missing zone and therefore cannot
    /// resurrect one that unmounted during an async measurement.
    pub fn set_rect_if_present(&mut self, registration: ZoneRegistration, rect: Rect) {
        if !self.is_current(registration, "set_rect_if_present") {
            return;
        }
        match self.zones.try_write() {
            Ok(mut zones) => {
                if let Some(zone) = zones.iter_mut().find(|z| z.id == registration.id) {
                    zone.rect = Some(rect);
                }
            }
            Err(error) => {
                trace_registry_failure(
                    "set_rect_if_present",
                    "zones",
                    Some(registration.id),
                    Some(registration.generation),
                    &error,
                );
            }
        }
    }

    /// Set geometry for the current registration of `id`. This is the
    /// synchronous/manual counterpart to [`Self::set_rect_if_present`], used
    /// by custom layout adapters and the headless test driver.
    pub fn set_rect(&mut self, id: ZoneId, rect: Rect) {
        if let Some(registration) = self.current_registration(id, "set_rect") {
            self.set_rect_if_present(registration, rect);
        }
    }

    /// Look up a zone by id.
    pub fn get(&self, id: ZoneId) -> Option<ZoneRecord<T>> {
        self.zones
            .try_peek()
            .ok()?
            .iter()
            .find(|z| z.id == id)
            .cloned()
    }

    /// The zone's cached client rect, read without subscribing. Returns
    /// `None` when unmeasured, unknown, or the provider is already gone.
    pub fn cached_rect(&self, id: ZoneId) -> Option<Rect> {
        self.zones
            .try_peek()
            .ok()?
            .iter()
            .find(|z| z.id == id)
            .and_then(ZoneRecord::cached_rect)
    }

    /// The zone's mounted element, read without subscribing. Returns `None`
    /// before mount, for an unknown zone, or after provider teardown.
    pub fn mounted_handle(&self, id: ZoneId) -> Option<Rc<MountedData>> {
        self.zones
            .try_peek()
            .ok()?
            .iter()
            .find(|z| z.id == id)
            .and_then(ZoneRecord::mounted_handle)
    }

    /// Every registered zone, in registration order. Unlike the peeking
    /// lookups around it this is a *subscribing* read - a component
    /// rendering from it re-renders when zones mount or unmount - because
    /// its consumers (the debug overlay, your own devtools) are renderers.
    pub fn records(&self) -> Vec<ZoneRecord<T>> {
        self.zones
            .try_read()
            .map(|zones| zones.to_vec())
            .unwrap_or_default()
    }

    /// Is a zone with this id registered *here*? The parent-zone context is
    /// shared across payload types, so a record's `parent` can name a zone
    /// living in another type's registry - check before navigating to one.
    pub fn contains(&self, id: ZoneId) -> bool {
        self.zones
            .try_peek()
            .is_ok_and(|zones| zones.iter().any(|z| z.id == id))
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
            .try_peek()
            .map(|zones| {
                zones
                    .iter()
                    .filter(|z| z.accepts_payload(payload))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The next/previous zone (cyclic) relative to `current` among zones that
    /// accept `payload`. `step` is `+1` or `-1`.
    ///
    /// Order is **spatial** (top-to-bottom, then left-to-right) for zones
    /// with measured rects; tops within one CSS pixel form a row so sub-pixel
    /// layout jitter cannot override horizontal reading order. Call
    /// [`Self::refresh_rects`] first, as the built-in keyboard interaction
    /// does on pickup. Unmeasured zones keep registration order afterwards.
    pub fn step_zone(&self, current: Option<ZoneId>, payload: &T, step: isize) -> Option<ZoneId> {
        let mut zones = self.acceptable(payload);
        spatial_sort(&mut zones, self.direction());
        let current_ix = current.and_then(|c| zones.iter().position(|z| z.id == c));
        cycle(zones.len(), current_ix, step).map(|ix| zones[ix].id)
    }

    /// The parent of a zone, if it's nested.
    pub fn parent_of(&self, id: ZoneId) -> Option<ZoneId> {
        self.zones
            .try_peek()
            .ok()?
            .iter()
            .find(|z| z.id == id)?
            .parent
    }

    /// Zones directly inside `parent` (`None` = root level) that accept
    /// `payload`, in spatial order (top-to-bottom, then left-to-right within
    /// a one-CSS-pixel row band; unmeasured zones keep registration order at
    /// the end).
    pub fn children_of(&self, parent: Option<ZoneId>, payload: &T) -> Vec<ZoneRecord<T>> {
        let mut zones: Vec<_> = self
            .zones
            .try_peek()
            .map(|zones| {
                zones
                    .iter()
                    .filter(|z| z.parent == parent && z.accepts_payload(payload))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        spatial_sort(&mut zones, self.direction());
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

    /// Last record in registry order containing `point` (client coordinates),
    /// using cached rects - call [`Self::refresh_rects`] when a drag starts.
    /// This only approximates DOM paint order; CSS stacking and portals are
    /// not inspected. Replacing a same-id record retains its existing slot.
    pub fn hit_test(&self, point: Point) -> Option<ZoneId> {
        self.zones
            .try_peek()
            .ok()?
            .iter()
            .rev()
            .find(|z| z.cached_rect().map(|r| r.contains(point)).unwrap_or(false))
            .map(|z| z.id)
    }

    /// Like [`Self::hit_test`], but acceptance-aware: it returns the last
    /// record in registry order that both contains the point **and** accepts
    /// `payload`, and when no such zone contains the point, falls back to the
    /// acceptable zone whose *rect* is nearest - within `max_distance` CSS px
    /// of its closest edge, not its center, so a large zone snaps a release
    /// right beside it even though its center sits far away. Skipping zones
    /// that reject the payload lets a drop land on an earlier accepting
    /// overlap, and is friendlier for imprecise (touch) drops that land in the
    /// gutter between zones.
    pub fn hit_test_closest(&self, point: Point, payload: &T, max_distance: f64) -> Option<ZoneId> {
        let zones = self.zones.try_peek().ok()?;
        let mut best: Option<(ZoneId, f64)> = None;
        // One borrowed pass: the former miss path built and cloned an entire
        // `Vec<ZoneRecord<T>>`, then evaluated every acceptance filter twice.
        for z in zones.iter().rev() {
            if !z.accepts_payload(payload) {
                continue;
            }
            let Some(r) = z.cached_rect() else { continue };
            if r.contains(point) {
                return Some(z.id);
            }
            // Distance to the rect's nearest point (zero on either axis the
            // point already overlaps), not to its center.
            let dx = (r.x - point.x).max(point.x - (r.x + r.width)).max(0.0);
            let dy = (r.y - point.y).max(point.y - (r.y + r.height)).max(0.0);
            let d = (dx * dx + dy * dy).sqrt();
            // Reverse iteration preserves direct-hit precedence. Replacing on
            // an equal distance preserves the old fallback tie-break: the
            // earlier record in registry order wins.
            if d <= max_distance && best.map(|(_, bd)| d <= bd).unwrap_or(true) {
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
        let zones = self.measurement_targets();
        for (registration, mounted) in zones {
            if let Ok(r) = mounted.get_client_rect().await {
                // The zone can unmount or be replaced during the await (a
                // closing window mid-drag is the common case). The
                // generation check quietly drops that stale measurement.
                let mut registry = *self;
                registry.set_rect_if_present(
                    registration,
                    Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height),
                );
            }
        }
    }

    /// Re-measure every mounted zone's client rect (async, spawned).
    pub fn refresh_rects(&self) {
        for (registration, mounted) in self.measurement_targets() {
            let mut registry = *self;
            spawn(async move {
                if let Ok(r) = mounted.get_client_rect().await {
                    // See measure_all: the zone can die or be replaced
                    // while this measurement is in flight.
                    registry.set_rect_if_present(
                        registration,
                        Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height),
                    );
                }
            });
        }
    }

    /// Subscribe an effect to registration/mount changes without also
    /// subscribing it to rect writes in the main registry vector.
    pub(crate) fn track_mounts(&self) {
        let _ = self.mount_revision.try_read();
    }

    fn measurement_targets(&self) -> Vec<(ZoneRegistration, Rc<MountedData>)> {
        let registrations = self
            .registrations
            .try_peek()
            .map(|registrations| registrations.clone())
            .unwrap_or_default();
        self.zones
            .try_peek()
            .map(|zones| {
                zones
                    .iter()
                    .filter_map(|zone| {
                        let mounted = zone.mounted_handle()?;
                        let generation = registrations
                            .iter()
                            .find(|(id, _)| *id == zone.id)
                            .map(|(_, generation)| *generation)?;
                        Some((
                            ZoneRegistration {
                                id: zone.id,
                                generation,
                            },
                            mounted,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn is_current(&self, registration: ZoneRegistration, operation: &'static str) -> bool {
        match self.registrations.try_peek() {
            Ok(registrations) => registrations.iter().any(|(id, generation)| {
                *id == registration.id && *generation == registration.generation
            }),
            Err(error) => {
                trace_registry_failure(
                    operation,
                    "registrations",
                    Some(registration.id),
                    Some(registration.generation),
                    &error,
                );
                false
            }
        }
    }

    fn current_registration(
        &self,
        id: ZoneId,
        operation: &'static str,
    ) -> Option<ZoneRegistration> {
        match self.registrations.try_peek() {
            Ok(registrations) => registrations
                .iter()
                .find(|(registered_id, _)| *registered_id == id)
                .map(|(_, generation)| ZoneRegistration {
                    id,
                    generation: *generation,
                }),
            Err(error) => {
                trace_registry_failure(operation, "registrations", Some(id), None, &error);
                None
            }
        }
    }

    fn bump_mount_revision(&mut self) {
        match self.mount_revision.try_write() {
            Ok(mut revision) => *revision = revision.wrapping_add(1),
            Err(error) => {
                trace_registry_failure("bump_mount_revision", "mount_revision", None, None, &error)
            }
        }
    }
}

/// A payload-type-erased "re-measure your zones" channel, shared by every
/// registry under one provider tree.
///
/// Cached client rects go stale the moment layout moves under a live drag -
/// scrolling being the everyday case. Registries are per payload type, but
/// the things that move layout (an auto-scrolling container, your own
/// scroll surface, a collapsing panel) shouldn't need to know any payload
/// type to say "geometry changed". Each provider registers a thunk here
/// that re-measures its own registry **only while it has a drag in
/// flight**, so pinging the channel from every scroll event costs nothing
/// while idle.
///
/// [`crate::autoscroll::AutoScroll`] pings this automatically after every
/// scroll it performs (and on any other scroll of its container); grab the
/// channel with [`crate::core::hooks::use_rect_refresh`] to wire up custom
/// layout mutators.
pub struct RectRefresh {
    thunks: Signal<Vec<(u64, Callback<()>)>>,
}

impl Copy for RectRefresh {}
impl Clone for RectRefresh {
    fn clone(&self) -> Self {
        *self
    }
}
impl PartialEq for RectRefresh {
    fn eq(&self, other: &Self) -> bool {
        self.thunks == other.thunks
    }
}

impl RectRefresh {
    /// Wrap an existing signal. Prefer [`crate::core::hooks::use_dnd_provider`],
    /// which creates one per provider *tree* (nested providers inherit and
    /// re-provide the outermost channel).
    pub fn from_signal(thunks: Signal<Vec<(u64, Callback<()>)>>) -> Self {
        Self { thunks }
    }

    /// Ask every provider in the tree to re-measure its zones. Providers
    /// without a drag in flight ignore the ping, so this is safe to call
    /// from high-frequency sources like scroll events.
    pub fn refresh_all(&self) {
        for (_, thunk) in self.thunks.peek().iter() {
            thunk.call(());
        }
    }

    /// Number of registered providers. Diagnostics and tests.
    pub fn len(&self) -> usize {
        self.thunks.peek().len()
    }

    /// Whether any provider is registered.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Add (or replace, by key) a provider's re-measure thunk.
    pub(crate) fn register(&mut self, key: u64, thunk: Callback<()>) {
        let mut thunks = self.thunks.write();
        if let Some(existing) = thunks.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = thunk;
        } else {
            thunks.push((key, thunk));
        }
    }

    /// Remove a provider's thunk (call when the provider unmounts).
    pub(crate) fn unregister(&mut self, key: u64) {
        self.thunks.write().retain(|(k, _)| *k != key);
    }
}

/// Sort zones spatially: measured rects by row then reading order, unmeasured
/// last in their original relative order. Tops within one CSS pixel form a
/// row so sub-pixel layout jitter cannot override horizontal reading order.
/// Reading order is left-to-right in LTR and right-to-left in RTL.
fn spatial_sort<T: Clone + 'static>(zones: &mut [ZoneRecord<T>], dir: Direction) {
    const ROW_TOP_SLOP: f64 = 1.0;

    // First establish a total, stable vertical order and move unmeasured
    // records to the end. Row tolerance cannot live inside this comparator:
    // pairwise "close enough" comparisons are non-transitive.
    zones.sort_by(|a, b| match (a.cached_rect(), b.cached_rect()) {
        (Some(ra), Some(rb)) => ra.y.total_cmp(&rb.y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    let measured = zones
        .iter()
        .position(|zone| zone.cached_rect().is_none())
        .unwrap_or(zones.len());
    let mut row_start = 0;
    while row_start < measured {
        let row_y = zones[row_start].cached_rect().unwrap().y;
        let mut row_end = row_start + 1;
        while row_end < measured {
            let y = zones[row_end].cached_rect().unwrap().y;
            if !row_y.is_finite() || !y.is_finite() || (y - row_y).abs() > ROW_TOP_SLOP {
                break;
            }
            row_end += 1;
        }
        zones[row_start..row_end].sort_by(|a, b| {
            let ax = a.cached_rect().unwrap().x;
            let bx = b.cached_rect().unwrap().x;
            match dir {
                Direction::Ltr => ax.total_cmp(&bx),
                Direction::Rtl => bx.total_cmp(&ax),
            }
        });
        row_start = row_end;
    }
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
    use dioxus::prelude::*;

    use super::{cycle, Direction, Rect, ZoneId, ZoneRecord, ZoneRegistry};

    #[test]
    fn cycle_steps_and_wraps() {
        assert_eq!(cycle(0, None, 1), None);
        assert_eq!(cycle(3, None, 1), Some(0));
        assert_eq!(cycle(3, None, -1), Some(2));
        assert_eq!(cycle(3, Some(2), 1), Some(0));
        assert_eq!(cycle(3, Some(0), -1), Some(2));
        assert_eq!(cycle(3, Some(1), 1), Some(2));
    }

    fn equality_probe() -> Element {
        let zones = use_signal(Vec::<ZoneRecord<u8>>::new);
        let registrations = use_signal(Vec::<(ZoneId, u64)>::new);
        let other_registrations = use_signal(Vec::<(ZoneId, u64)>::new);
        let mount_revision = use_signal(|| 0u64);
        let other_mount_revision = use_signal(|| 0u64);
        let dir = use_signal(Direction::default);
        let registry = ZoneRegistry {
            zones,
            registrations,
            mount_revision,
            dir,
        };
        let copy = registry;

        assert!(registry == copy, "a copied handle must compare equal");
        assert!(
            registry
                != ZoneRegistry {
                    registrations: other_registrations,
                    ..registry
                },
            "registration identity is part of registry identity"
        );
        assert!(
            registry
                != ZoneRegistry {
                    mount_revision: other_mount_revision,
                    ..registry
                },
            "mount-revision identity is part of registry identity"
        );
        rsx! {}
    }

    #[test]
    fn equality_covers_every_registry_storage_handle() {
        let mut dom = VirtualDom::new(equality_probe);
        dom.rebuild_in_place();
    }

    fn structural_borrow_probe() -> Element {
        let zones = use_signal(Vec::<ZoneRecord<u8>>::new);
        let mut registry = ZoneRegistry::from_signal(zones);
        let record = |id: u64| ZoneRecord {
            id: ZoneId(id),
            parent: None,
            label: None,
            on_drop: Callback::new(|_| {}),
            accepts: None,
            mounted: None,
            rect: Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
        };
        registry.register(record(1));

        // If the zone half is borrowed, registration changes neither half.
        {
            let zones = registry.zones;
            let _zones = zones.read();
            registry.register(record(2));
        }
        assert!(registry.get(ZoneId(2)).is_none());
        assert!(registry.current_registration(ZoneId(2), "test").is_none());

        // If the generation half is borrowed, the already-acquired zone
        // guard must still be dropped without mutating either vector.
        {
            let registrations = registry.registrations;
            let _registrations = registrations.read();
            registry.register(record(3));
        }
        assert!(registry.get(ZoneId(3)).is_none());
        assert!(registry.current_registration(ZoneId(3), "test").is_none());

        // Unregister has the same all-or-nothing structural contract.
        {
            let registrations = registry.registrations;
            let _registrations = registrations.read();
            registry.unregister(ZoneId(1));
        }
        assert!(registry.get(ZoneId(1)).is_some());
        assert!(registry.current_registration(ZoneId(1), "test").is_some());
        rsx! {}
    }

    #[test]
    fn structural_borrow_failures_cannot_split_registry_state() {
        let mut dom = VirtualDom::new(structural_borrow_probe);
        dom.rebuild_in_place();
    }
}

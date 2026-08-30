//! Application-wide drag lifecycle monitoring.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

use super::{
    DragId, DragMode, DragSessionId, DropEffect, DropOutcome, Point, PointerKind, Rect, ZoneId,
};

static NEXT_MONITOR: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DragSnapshot<T> {
    pub id: DragId,
    pub session: Option<DragSessionId>,
    pub payload: T,
    pub source: Option<ZoneId>,
    pub over: Option<ZoneId>,
    pub pointer: Point,
    pub grab: Point,
    pub effect: DropEffect,
    pub mode: DragMode,
    pub pointer_kind: PointerKind,
    pub source_rect: Option<Rect>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DropReceipt<T> {
    pub drag: DragSnapshot<T>,
    pub outcome: DropOutcome<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CancelReason {
    User,
    PointerCancelled,
    NoTarget,
    SourceUnmounted,
    Replaced,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DndEvent<T> {
    Started(DragSnapshot<T>),
    Moved(DragSnapshot<T>),
    TargetChanged {
        drag: DragSnapshot<T>,
        previous: Option<ZoneId>,
        current: Option<ZoneId>,
    },
    Dropped(DropReceipt<T>),
    Cancelled {
        drag: DragSnapshot<T>,
        reason: CancelReason,
    },
}

pub(crate) struct DndMonitor<T: 'static> {
    listeners: Signal<Vec<(u64, Callback<DndEvent<T>>)>>,
    pending: Signal<VecDeque<DndEvent<T>>>,
    dispatching: Signal<bool>,
}

impl<T> Copy for DndMonitor<T> {}
impl<T> Clone for DndMonitor<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for DndMonitor<T> {
    fn eq(&self, other: &Self) -> bool {
        self.listeners == other.listeners
            && self.pending == other.pending
            && self.dispatching == other.dispatching
    }
}

impl<T: Clone + 'static> DndMonitor<T> {
    pub(crate) fn new() -> Self {
        Self {
            listeners: Signal::new(Vec::new()),
            pending: Signal::new(VecDeque::new()),
            dispatching: Signal::new(false),
        }
    }

    pub(crate) fn subscribe(&mut self, callback: Callback<DndEvent<T>>) -> u64 {
        let id = NEXT_MONITOR.fetch_add(1, Ordering::Relaxed);
        self.listeners.write().push((id, callback));
        id
    }

    pub(crate) fn unsubscribe(&mut self, id: u64) {
        if let Ok(mut listeners) = self.listeners.try_write() {
            listeners.retain(|(candidate, _)| *candidate != id);
        }
    }

    pub(crate) fn has_listeners(&self) -> bool {
        self.listeners
            .try_peek()
            .is_ok_and(|listeners| !listeners.is_empty())
    }

    pub(crate) fn emit_lazy(&self, build: impl FnOnce() -> Option<DndEvent<T>>) {
        if !self.has_listeners() {
            return;
        }
        if let Some(event) = build() {
            self.emit(event);
        }
    }

    pub(crate) fn emit(&self, event: DndEvent<T>) {
        if !self.has_listeners() {
            return;
        }
        let mut pending = self.pending;
        let Ok(mut queue) = pending.try_write() else {
            return;
        };
        queue.push_back(event);
        drop(queue);

        if self
            .dispatching
            .try_peek()
            .is_ok_and(|dispatching| *dispatching)
        {
            return;
        }

        let mut dispatching = self.dispatching;
        let Ok(mut active) = dispatching.try_write() else {
            return;
        };
        *active = true;
        drop(active);
        let _guard = DispatchGuard(dispatching);

        loop {
            let mut pending = self.pending;
            let event = pending
                .try_write()
                .ok()
                .and_then(|mut queue| queue.pop_front());
            let Some(event) = event else {
                break;
            };
            let callbacks: Vec<_> = self
                .listeners
                .try_peek()
                .map(|listeners| listeners.iter().map(|(_, callback)| *callback).collect())
                .unwrap_or_default();
            for callback in callbacks {
                callback.call(event.clone());
            }
        }
    }
}

struct DispatchGuard(Signal<bool>);

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        if let Ok(mut dispatching) = self.0.try_write() {
            *dispatching = false;
        }
    }
}

/// Observe every drag lifecycle event from the nearest provider.
pub fn use_dnd_monitor<T: Clone + 'static>(handler: impl FnMut(DndEvent<T>) + 'static) {
    let mut dnd = crate::core::use_dnd::<T>();
    let callback = use_callback(handler);
    let id = use_hook(move || dnd.monitor_mut().subscribe(callback));
    use_drop(move || dnd.monitor_mut().unsubscribe(id));
}

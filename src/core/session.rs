//! Internal pointer-session completion: commit a result before receiver
//! code, then notify the originating source exactly once.

use dioxus::prelude::*;

use super::monitor::CancelReason;
use super::state::{DndContext, DragIdentity, DragPhase, DragStart};
use super::types::{DragId, DragSessionId, DropEffect, Point, ZoneId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragCompletion {
    Dropped,
    Cancelled(CancelReason),
}

impl DragCompletion {
    pub(crate) fn dropped(self) -> bool {
        matches!(self, Self::Dropped)
    }
}

#[derive(Clone, Copy)]
pub(super) struct SourceCompletion {
    id: DragSessionId,
    callback: Callback<bool>,
    committed: Option<bool>,
}

impl<T: Clone + 'static> DndContext<T> {
    /// Begin a pointer drag whose source must be completed exactly once.
    pub(crate) fn start_tracked(
        &mut self,
        payload: T,
        source: Option<ZoneId>,
        pointer: Point,
        grab: Point,
        effect: DropEffect,
        callback: Callback<bool>,
    ) -> DragSessionId {
        self.start_tracked_with_metadata(
            DragId::auto(),
            DragStart::new(payload, pointer)
                .with_source(source)
                .with_grab(grab)
                .with_effect(effect),
            callback,
        )
    }

    /// Begin a tracked pointer drag with its complete initial monitor
    /// snapshot. The session, pointer kind, and any press-time source rect
    /// are installed before `Started` is emitted.
    pub(crate) fn start_tracked_with_metadata(
        &mut self,
        drag_id: DragId,
        start: DragStart<T>,
        callback: Callback<bool>,
    ) -> DragSessionId {
        let id = DragSessionId::auto();
        if !self.prepare_start() {
            // The cancellation boundary synchronously started a replacement.
            // This attempted source promoted but never owned shared state, so
            // retire its local gesture without touching the replacement.
            callback.call(false);
            return id;
        }
        let mut completion = self.runtime.completion;
        completion.set(Some(SourceCompletion {
            id,
            callback,
            committed: None,
        }));
        self.start_with_metadata(DragIdentity::Explicit(drag_id), Some(id), start);
        // A monitor may synchronously start a replacement from `Started`.
        // Do not leave this source generation claiming that replacement.
        if self.drag_session_id() != Some(id) && self.active_session() == Some(id) {
            self.cancel_session(id, CancelReason::Replaced);
        }
        id
    }

    /// Retire the current drag before a start. Returns false when cancellation
    /// user code synchronously started a replacement, which owns the context
    /// and must not be overwritten by the outer start operation.
    pub(super) fn prepare_start(&mut self) -> bool {
        if let Some(previous) = self.active_session() {
            self.cancel_session(previous, CancelReason::Replaced);
        } else if self.dragging() {
            self.cancel_state(CancelReason::Replaced);
        } else if self.phase_peek() == DragPhase::Settling {
            // A completed drop owns its terminal outcome already. Starting a
            // replacement may interrupt only the visual glide.
            self.finish_settle();
        }
        self.active_session().is_none() && !self.dragging()
    }

    /// Current pointer-gesture generation, if the source registered one.
    pub(crate) fn active_session(&self) -> Option<DragSessionId> {
        self.runtime
            .completion
            .try_peek()
            .ok()?
            .as_ref()
            .map(|completion| completion.id)
    }

    pub(crate) fn is_session(&self, id: DragSessionId) -> bool {
        self.active_session() == Some(id)
    }

    pub(crate) fn session_result(&self, id: DragSessionId) -> Option<bool> {
        self.runtime
            .completion
            .try_peek()
            .ok()?
            .as_ref()
            .filter(|completion| completion.id == id)?
            .committed
    }

    /// Commit the result before receiver user code runs, without firing the
    /// public source callback yet. If receiver code unmounts the source, its
    /// cleanup finalizes this committed result instead of changing it.
    pub(crate) fn commit_source(&mut self, id: DragSessionId, dropped: bool) -> bool {
        if self.active_session() != Some(id) {
            return false;
        }
        let mut slot = self.runtime.completion;
        let mut completion = slot.write();
        let Some(completion) = completion.as_mut() else {
            return false;
        };
        if completion.committed.is_none() {
            completion.committed = Some(dropped);
        }
        true
    }

    /// Fire a previously committed result exactly once.
    pub(crate) fn finalize_source(&mut self, id: DragSessionId) -> bool {
        let Some(result) = self.session_result(id) else {
            return false;
        };
        let Some(completion) = self.runtime.completion.take() else {
            return false;
        };
        completion.callback.call(result);
        true
    }

    /// Commit and immediately notify the source.
    pub(crate) fn finish_source(&mut self, id: DragSessionId, dropped: bool) -> bool {
        if !self.commit_source(id, dropped) {
            return false;
        }
        self.finalize_source(id)
    }

    /// Cancel this generation and notify its source exactly once.
    pub(crate) fn cancel_session(&mut self, id: DragSessionId, reason: CancelReason) -> bool {
        if !self.is_session(id) {
            return false;
        }
        if self.session_result(id).is_none() {
            self.cancel_state(reason);
            self.commit_source(id, false);
        }
        self.finalize_source(id)
    }

    /// Retire a source generation without calling back into a runtime that
    /// is already being torn down. Built-in sources cancel from their own
    /// cleanup first; this is the provider/window-close safety net for a
    /// custom source that omitted equivalent cleanup.
    pub(crate) fn abandon_session(&mut self, id: DragSessionId, reason: CancelReason) -> bool {
        if !self.is_session(id) {
            return false;
        }
        if self.session_result(id).is_none() {
            self.cancel_state(reason);
        }
        self.runtime.completion.take();
        true
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::core::state::DragState;
    use crate::core::types::Rect;

    thread_local! {
        static CONTEXT: RefCell<Option<DndContext<String>>> = const { RefCell::new(None) };
        static CALLBACK: RefCell<Option<Callback<bool>>> = const { RefCell::new(None) };
        static REPLACEMENT_CALLBACK: RefCell<Option<Callback<bool>>> = const { RefCell::new(None) };
        static COMPLETIONS: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    }

    fn probe() -> Element {
        let state = use_store(DragState::<String>::default);
        let announcement = use_signal(String::new);
        let context = use_hook(|| DndContext::from_parts(state, announcement));
        let callback =
            use_callback(|dropped| COMPLETIONS.with_borrow_mut(|calls| calls.push(dropped)));
        let replacement_callback = use_callback(|dropped: bool| {
            assert!(!dropped);
            CONTEXT.with_borrow_mut(|slot| {
                slot.as_mut().expect("probe context").start_with_id(
                    DragId(777),
                    DragStart::new("callback replacement".to_string(), Point::default()),
                );
            });
        });
        CONTEXT.with_borrow_mut(|slot| *slot = Some(context));
        CALLBACK.with_borrow_mut(|slot| *slot = Some(callback));
        REPLACEMENT_CALLBACK.with_borrow_mut(|slot| *slot = Some(replacement_callback));
        rsx! {}
    }

    fn context() -> DndContext<String> {
        CONTEXT.with_borrow(|slot| slot.expect("probe context"))
    }

    fn completion_callback() -> Callback<bool> {
        CALLBACK.with_borrow(|slot| slot.expect("probe callback"))
    }

    fn replacement_callback() -> Callback<bool> {
        REPLACEMENT_CALLBACK.with_borrow(|slot| slot.expect("replacement callback"))
    }

    #[test]
    fn tracked_source_completion_is_exactly_once() {
        COMPLETIONS.with_borrow_mut(|calls| calls.clear());
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        let mut dnd = context();

        let first = dom.in_runtime(|| {
            dnd.start_tracked(
                "first".into(),
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                completion_callback(),
            )
        });
        dom.in_runtime(|| {
            assert!(dnd.take().is_some());
            assert!(dnd.finish_source(first, true));
            assert!(!dnd.finish_source(first, false));
        });
        COMPLETIONS.with_borrow(|calls| assert_eq!(calls.as_slice(), &[true]));

        let second = dom.in_runtime(|| {
            dnd.start_tracked(
                "second".into(),
                None,
                Point::new(20.0, 20.0),
                Point::default(),
                DropEffect::Move,
                completion_callback(),
            )
        });
        dom.in_runtime(|| {
            assert!(
                !dnd.finish_source(first, true),
                "stale generation completed"
            );
            assert!(dnd.cancel_session(second, CancelReason::User));
            assert!(!dnd.cancel_session(second, CancelReason::User));
        });
        COMPLETIONS.with_borrow(|calls| assert_eq!(calls.as_slice(), &[true, false]));
    }

    #[test]
    fn successful_source_completion_preserves_settle_payload() {
        COMPLETIONS.with_borrow_mut(|calls| calls.clear());
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        let mut dnd = context();
        let session = dom.in_runtime(|| {
            dnd.start_tracked(
                "card".into(),
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                completion_callback(),
            )
        });
        dom.in_runtime(|| {
            assert!(dnd
                .take_settling(Rect::new(100.0, 100.0, 40.0, 40.0))
                .is_some());
            assert!(dnd.finish_source(session, true));
            assert!(!dnd.dragging());
            assert!(dnd.settling().is_some());
            assert_eq!(dnd.payload().as_deref(), Some("card"));
            dnd.finish_settle();
            assert!(dnd.payload().is_none());
        });
        COMPLETIONS.with_borrow(|calls| assert_eq!(calls.as_slice(), &[true]));
    }

    #[test]
    fn cancellation_callback_replacement_is_not_overwritten_by_outer_start() {
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        let mut dnd = context();

        dom.in_runtime(|| {
            dnd.start_tracked(
                "first".into(),
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                replacement_callback(),
            );
            dnd.start_with_id(
                DragId(999),
                DragStart::new("outer start".to_string(), Point::default()),
            );

            assert_eq!(dnd.drag_id(), Some(DragId(777)));
            assert_eq!(dnd.payload().as_deref(), Some("callback replacement"));
            assert!(dnd.dragging());
            dnd.cancel();
        });
    }

    #[test]
    fn committed_success_survives_source_cleanup_during_delivery() {
        COMPLETIONS.with_borrow_mut(|calls| calls.clear());
        let mut dom = VirtualDom::new(probe);
        dom.rebuild_in_place();
        let mut dnd = context();
        let session = dom.in_runtime(|| {
            dnd.start_tracked(
                "card".into(),
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                completion_callback(),
            )
        });
        dom.in_runtime(|| {
            assert!(dnd.take().is_some());
            assert!(dnd.commit_source(session, true));
        });
        COMPLETIONS.with_borrow(|calls| assert!(calls.is_empty()));

        // This is what Draggable's cleanup calls if receiver user code
        // synchronously removes the source. It must finalize the committed
        // success, not overwrite it with cancellation.
        dom.in_runtime(|| assert!(dnd.cancel_session(session, CancelReason::SourceUnmounted)));
        COMPLETIONS.with_borrow(|calls| assert_eq!(calls.as_slice(), &[true]));
        dom.in_runtime(|| assert!(!dnd.finalize_source(session)));
    }
}

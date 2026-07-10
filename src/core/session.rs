//! Internal pointer-session completion: commit a result before receiver
//! code, then notify the originating source exactly once.

use dioxus::prelude::*;

use super::state::DndContext;
use super::types::{DragMode, DragSessionId, DropEffect, Point, ZoneId};

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
        if let Some(previous) = self.active_session() {
            self.cancel_session(previous);
        }
        self.start(payload, source, pointer, grab, effect, DragMode::Pointer);
        let id = DragSessionId::auto();
        let mut completion = self.completion;
        completion.set(Some(SourceCompletion {
            id,
            callback,
            committed: None,
        }));
        id
    }

    /// Current pointer-gesture generation, if the source registered one.
    pub(crate) fn active_session(&self) -> Option<DragSessionId> {
        self.completion
            .try_peek()
            .ok()?
            .as_ref()
            .map(|completion| completion.id)
    }

    pub(crate) fn is_session(&self, id: DragSessionId) -> bool {
        self.active_session() == Some(id)
    }

    pub(crate) fn session_result(&self, id: DragSessionId) -> Option<bool> {
        self.completion
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
        let mut slot = self.completion;
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
        let Some(completion) = self.completion.take() else {
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
    pub(crate) fn cancel_session(&mut self, id: DragSessionId) -> bool {
        if !self.is_session(id) {
            return false;
        }
        if self.session_result(id).is_none() {
            self.cancel();
            self.commit_source(id, false);
        }
        self.finalize_source(id)
    }

    /// Retire a source generation without calling back into a runtime that
    /// is already being torn down. Built-in sources cancel from their own
    /// cleanup first; this is the provider/window-close safety net for a
    /// custom source that omitted equivalent cleanup.
    pub(crate) fn abandon_session(&mut self, id: DragSessionId) -> bool {
        if !self.is_session(id) {
            return false;
        }
        if self.session_result(id).is_none() {
            self.cancel();
        }
        self.completion.take();
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
        static COMPLETIONS: RefCell<Vec<bool>> = const { RefCell::new(Vec::new()) };
    }

    fn probe() -> Element {
        let state = use_store(DragState::<String>::default);
        let announcement = use_signal(String::new);
        let context = use_hook(|| DndContext::from_parts(state, announcement));
        let callback =
            use_callback(|dropped| COMPLETIONS.with_borrow_mut(|calls| calls.push(dropped)));
        CONTEXT.with_borrow_mut(|slot| *slot = Some(context));
        CALLBACK.with_borrow_mut(|slot| *slot = Some(callback));
        rsx! {}
    }

    fn context() -> DndContext<String> {
        CONTEXT.with_borrow(|slot| slot.expect("probe context"))
    }

    fn completion_callback() -> Callback<bool> {
        CALLBACK.with_borrow(|slot| slot.expect("probe callback"))
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
            assert!(dnd.cancel_session(second));
            assert!(!dnd.cancel_session(second));
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
        dom.in_runtime(|| assert!(dnd.cancel_session(session)));
        COMPLETIONS.with_borrow(|calls| assert_eq!(calls.as_slice(), &[true]));
        dom.in_runtime(|| assert!(!dnd.finalize_source(session)));
    }
}

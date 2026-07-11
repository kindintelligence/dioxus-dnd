//! Settle-glide presentation routing: which window's overlay presents a
//! cross-window drop's settle.

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

use super::geometry::WindowKey;
use super::state::DndWorld;

// Identity freshness only: Relaxed is sufficient because the counter carries
// no synchronization. Correctness assumes this process-lifetime u64 never
// wraps; do not narrow it.
static NEXT_SETTLE_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SettleClaim {
    presenter: WindowKey,
    generation: u64,
}

impl<T: Clone + 'static> DndWorld<T> {
    /// Elect `key` to present the next world settle. Custom world delivery
    /// calls this before [`crate::core::DndContext::take_settling`]; built-in
    /// delivery claims automatically.
    ///
    /// The claim is required, not advisory: in a joined world, only the
    /// elected window's overlay presents and finishes a settle. A custom
    /// source that calls `take_settling` without claiming gets no glide
    /// anywhere; that claimless settle is only cleaned up when its origin
    /// window closes or the next drag begins.
    pub fn claim_settle(&self, key: WindowKey) {
        let mut claim = self.settle_claim;
        claim.set(Some(SettleClaim {
            presenter: key,
            generation: NEXT_SETTLE_GENERATION.fetch_add(1, Ordering::Relaxed),
        }));
    }

    // Both token reads intersect the claim with the context's actual settle
    // state (like `settling_in` does): custom code may cancel or reset the
    // shared context mid-settle without world cleanup, and a lingering claim
    // must not keep a `SettleSlot` hidden or an overlay in its settle state.

    pub(crate) fn settle_token(&self, key: WindowKey) -> Option<u64> {
        self.ctx.settling()?;
        (*self.settle_claim.read())
            .filter(|claim| claim.presenter == key)
            .map(|claim| claim.generation)
    }

    pub(crate) fn peek_settle_token(&self, key: WindowKey) -> Option<u64> {
        if !self.ctx.settling_peek() {
            return None;
        }
        (*self.settle_claim.peek())
            .filter(|claim| claim.presenter == key)
            .map(|claim| claim.generation)
    }

    pub(super) fn settle_presenter_is(&self, key: WindowKey) -> bool {
        self.peek_settle_token(key).is_some()
    }

    pub(super) fn settle_presenter(&self) -> Option<WindowKey> {
        self.settle_claim
            .peek()
            .as_ref()
            .map(|claim| claim.presenter)
    }

    /// Finish a custom or built-in settle from its elected window. Custom
    /// world overlays should use this rather than finishing the shared
    /// context directly, so world metadata is cleared with it.
    pub fn finish_settle_from(&self, key: WindowKey) -> bool {
        let Some(generation) = self.peek_settle_token(key) else {
            return false;
        };
        self.finish_settle_generation(key, generation)
    }

    pub(crate) fn finish_settle_generation(&self, key: WindowKey, generation: u64) -> bool {
        if *self.settle_claim.peek()
            != Some(SettleClaim {
                presenter: key,
                generation,
            })
            || self.ctx.settling().is_none()
        {
            return false;
        }
        let mut ctx = self.ctx;
        ctx.finish_settle();
        self.clear_world_state();
        true
    }

    /// The window elected to present the current settle glide.
    pub fn settling_in(&self) -> Option<WindowKey> {
        self.ctx
            .settling()
            .and_then(|_| (*self.settle_claim.read()).map(|claim| claim.presenter))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::core::{DragMode, DropEffect, Point, Rect};

    thread_local! {
        static WORLD: RefCell<Option<DndWorld<String>>> = const { RefCell::new(None) };
    }

    fn test_app() -> Element {
        let world = use_hook(DndWorld::<String>::new);
        WORLD.with_borrow_mut(|slot| *slot = Some(world));
        rsx! {}
    }

    #[test]
    fn stale_generation_cannot_finish_its_successor() {
        let mut dom = VirtualDom::new(test_app);
        dom.rebuild_in_place();
        let world = WORLD.with_borrow(|slot| slot.expect("test world"));
        dom.in_runtime(|| {
            let origin = WindowKey::auto();
            let presenter = WindowKey::auto();
            let mut ctx = world.context();
            ctx.start(
                "payload".to_string(),
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                DragMode::Pointer,
            );
            world.begin_from(origin);
            world.claim_settle(presenter);
            assert!(ctx.take_settling(Rect::new(0.0, 0.0, 10.0, 10.0)).is_some());
            let stale = world.settle_token(presenter).unwrap();

            // A successor may elect the same presenter. Only its fresh
            // generation may finish the shared context.
            world.claim_settle(presenter);
            let successor = world.settle_token(presenter).unwrap();
            assert_ne!(stale, successor);
            assert!(!world.finish_settle_generation(presenter, stale));
            assert!(ctx.settling().is_some());
            assert!(world.finish_settle_generation(presenter, successor));
            assert!(ctx.payload().is_none());
        });
    }

    #[test]
    fn cancelled_context_settle_retires_the_claim_tokens() {
        let mut dom = VirtualDom::new(test_app);
        dom.rebuild_in_place();
        let world = WORLD.with_borrow(|slot| slot.expect("test world"));
        dom.in_runtime(|| {
            let origin = WindowKey::auto();
            let presenter = WindowKey::auto();
            let mut ctx = world.context();
            ctx.start(
                "payload".to_string(),
                None,
                Point::new(10.0, 10.0),
                Point::default(),
                DropEffect::Move,
                DragMode::Pointer,
            );
            world.begin_from(origin);
            world.claim_settle(presenter);
            assert!(ctx.take_settling(Rect::new(0.0, 0.0, 10.0, 10.0)).is_some());
            assert!(world.settle_token(presenter).is_some());

            // Custom code may reset the shared context mid-settle without
            // world cleanup. The claim must stop presenting immediately, so
            // a `SettleSlot` cannot stay hidden on a settle that no longer
            // exists.
            ctx.cancel();
            assert_eq!(world.settle_token(presenter), None);
            assert_eq!(world.peek_settle_token(presenter), None);
            assert_eq!(world.settling_in(), None);
            assert!(!world.finish_settle_from(presenter));
        });
    }
}

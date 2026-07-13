//! The liveness engine: one logical ticker advances every widget state,
//! and it survives any window close order.
//!
//! Exactly one window holds the ticker claim and drives ticks; every other
//! window retries the claim on a slow poll. A closing owner releases the
//! claim in `use_drop`, so a survivor adopts the ticker within ~500ms - the
//! widgets keep animating no matter which windows close, including Mission
//! Control itself.

use dioxus::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use crate::model::{Model, Widget, WidgetState};

const TICK: Duration = Duration::from_millis(50);
const RETRY_CLAIM: Duration = Duration::from_millis(500);

impl WidgetState {
    /// One 50ms step, pure and deterministic given `seed` (xorshift64), so
    /// liveness is unit-testable without a UI or a `rand` dependency.
    pub fn advanced(&self) -> WidgetState {
        let mut seed = self.seed;
        let mut rand01 = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed, (seed >> 11) as f64 / (1u64 << 53) as f64)
        };

        let (seed_a, step) = rand01();
        let last = self.samples.last().copied().unwrap_or(0.5);
        // Random walk gently attracted to mid-scale so the trace stays lively
        // without hugging the rails.
        let next_sample = (last + (step - 0.5) * 0.09 + (0.5 - last) * 0.012).clamp(0.0, 1.0);
        let mut samples = self.samples.clone();
        samples.push(next_sample);
        if samples.len() > 60 {
            samples.remove(0);
        }

        let mut seed2 = seed_a;
        let mut rand01b = move || {
            seed2 ^= seed2 << 13;
            seed2 ^= seed2 >> 7;
            seed2 ^= seed2 << 17;
            (seed2, (seed2 >> 11) as f64 / (1u64 << 53) as f64)
        };
        let (seed_b, drift) = rand01b();
        let bpm = (self.bpm + (drift - 0.5) * 0.7).clamp(58.0, 102.0);

        WidgetState {
            ticks: self.ticks + 1,
            samples,
            level: (self.level + 0.004) % 1.0,
            bpm,
            seed: seed_b,
        }
    }
}

/// Install the failover ticker in this window. Every window calls this; the
/// claim decides who actually drives.
pub fn use_ticker(model: Model) {
    let held = use_hook(|| Rc::new(Cell::new(false)));
    let release_model = model.clone();
    let release_held = held.clone();
    use_drop(move || {
        if release_held.get() {
            release_model.release_ticker();
        }
    });
    use_future(move || {
        let model = model.clone();
        let held = held.clone();
        async move {
            loop {
                if held.get() {
                    tick_all(&model);
                    tokio::time::sleep(TICK).await;
                } else if model.claim_ticker() {
                    held.set(true);
                } else {
                    tokio::time::sleep(RETRY_CLAIM).await;
                }
            }
        }
    });
}

fn tick_all(model: &Model) {
    let mut widgets: Vec<Widget> = model.dock.peek().clone();
    for satellite in model.satellites.peek().iter() {
        widgets.extend(satellite.widgets.peek().iter().copied());
    }
    for widget in widgets {
        advance_signal(widget.state);
    }
}

fn advance_signal(mut state: Signal<WidgetState>) {
    // A widget snapshot can race a satellite teardown by one tick; degrade
    // by skipping the sample, never by panicking.
    if let Ok(mut current) = state.try_write() {
        let next = current.advanced();
        *current = next;
    }
}

#[cfg(test)]
mod tests {
    use crate::model::WidgetState;

    #[test]
    fn advanced_is_deterministic_and_increments() {
        let start = WidgetState::seeded(42, 0.3);
        let a = start.advanced();
        let b = start.advanced();
        assert_eq!(a, b, "same seed must produce the same next state");
        assert_eq!(a.ticks, 1);
        assert_ne!(a.seed, start.seed);
    }

    #[test]
    fn samples_stay_capped_and_bounded() {
        let mut state = WidgetState::seeded(7, 0.0);
        for _ in 0..10_000 {
            state = state.advanced();
            assert_eq!(state.samples.len(), 60);
            let last = *state.samples.last().unwrap();
            assert!((0.0..=1.0).contains(&last), "sample {last} out of range");
            assert!(state.level < 1.0, "level must wrap below 1.0");
            assert!(
                (58.0..=102.0).contains(&state.bpm),
                "bpm {} out of band",
                state.bpm
            );
        }
        assert_eq!(state.ticks, 10_000);
    }
}

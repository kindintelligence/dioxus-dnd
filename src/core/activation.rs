//! Reusable activation policy for pointer and keyboard drag sources.

/// Which element is allowed to activate a drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Activator {
    /// The whole draggable surface activates the drag.
    #[default]
    Surface,
    /// Only a nested [`crate::core::DragHandle`] activates the drag.
    Handle,
    /// No built-in pointer or keyboard input activates the drag.
    Manual,
}

/// A condition that promotes a pointer press into a drag.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ActivationConstraint {
    /// Promote after travelling this many CSS pixels.
    Distance(f64),
    /// Promote after a delay, provided movement stays inside `tolerance`.
    Delay { duration_ms: u32, tolerance: f64 },
    /// Promote when any contained constraint succeeds.
    Either(Vec<ActivationConstraint>),
    /// Never promote automatically. Custom code may start the drag through
    /// [`crate::core::DndContext`].
    Manual,
}

impl Default for ActivationConstraint {
    fn default() -> Self {
        Self::Distance(8.0)
    }
}

impl ActivationConstraint {
    fn collect_delays(&self, delays: &mut Vec<(u32, f64)>) {
        match self {
            Self::Delay {
                duration_ms,
                tolerance,
            } => delays.push((*duration_ms, tolerance.max(0.0))),
            Self::Either(items) => {
                for item in items {
                    item.collect_delays(delays);
                }
            }
            Self::Distance(_) | Self::Manual => {}
        }
    }

    pub(crate) fn delays(&self) -> Vec<(u32, f64)> {
        let mut delays = Vec::new();
        self.collect_delays(&mut delays);
        delays
    }

    /// Smallest configured distance threshold, when one exists.
    pub fn distance(&self) -> Option<f64> {
        match self {
            Self::Distance(distance) => Some(distance.max(0.0)),
            Self::Either(items) => items
                .iter()
                .filter_map(Self::distance)
                .min_by(f64::total_cmp),
            Self::Delay { .. } | Self::Manual => None,
        }
    }

    /// Shortest configured delay and its movement tolerance.
    pub fn delay(&self) -> Option<(u32, f64)> {
        self.delays()
            .into_iter()
            .min_by_key(|(duration, _)| *duration)
    }

    /// Whether no built-in event may promote this policy.
    pub fn is_manual(&self) -> bool {
        match self {
            Self::Manual => true,
            // An empty disjunction can never promote, just like `Manual`.
            Self::Either(items) => items.iter().all(Self::is_manual),
            Self::Distance(_) | Self::Delay { .. } => false,
        }
    }

    /// Whether a movement delta exceeded a delay constraint's tolerance.
    pub fn exceeded_delay_tolerance(&self, dx: f64, dy: f64) -> bool {
        let distance = dx.hypot(dy);
        let delays = self.delays();
        !delays.is_empty() && delays.iter().all(|(_, tolerance)| distance > *tolerance)
    }
}

/// Complete activation policy for a draggable source.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ActivationPolicy {
    pub activator: Activator,
    pub constraint: ActivationConstraint,
}

impl Default for ActivationPolicy {
    fn default() -> Self {
        Self {
            activator: Activator::Surface,
            constraint: ActivationConstraint::default(),
        }
    }
}

impl ActivationPolicy {
    pub fn surface(constraint: ActivationConstraint) -> Self {
        Self {
            activator: Activator::Surface,
            constraint,
        }
    }

    pub fn handle(constraint: ActivationConstraint) -> Self {
        Self {
            activator: Activator::Handle,
            constraint,
        }
    }

    pub fn manual() -> Self {
        Self {
            activator: Activator::Manual,
            constraint: ActivationConstraint::Manual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_exposes_earliest_distance_and_delay() {
        let policy = ActivationConstraint::Either(vec![
            ActivationConstraint::Distance(12.0),
            ActivationConstraint::Delay {
                duration_ms: 300,
                tolerance: 5.0,
            },
            ActivationConstraint::Distance(7.0),
            ActivationConstraint::Delay {
                duration_ms: 180,
                tolerance: 3.0,
            },
        ]);

        assert_eq!(policy.distance(), Some(7.0));
        assert_eq!(policy.delay(), Some((180, 3.0)));
        assert!(!policy.exceeded_delay_tolerance(4.0, 0.0));
        assert!(policy.exceeded_delay_tolerance(6.0, 0.0));
        assert_eq!(
            policy.delays(),
            vec![(300, 5.0), (180, 3.0)],
            "nested delay alternatives retain their independent clocks and tolerances"
        );
    }

    #[test]
    fn empty_or_manual_only_disjunctions_are_manual() {
        assert!(ActivationConstraint::Either(Vec::new()).is_manual());
        assert!(ActivationConstraint::Either(vec![
            ActivationConstraint::Manual,
            ActivationConstraint::Either(Vec::new()),
        ])
        .is_manual());
    }
}

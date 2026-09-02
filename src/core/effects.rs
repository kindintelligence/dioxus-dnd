//! Target-side acceptance and drop-effect negotiation.

use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use super::{DragId, DragMode, DropEffect, PointerKind, ZoneId};

/// A compact set of drop effects a target supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DropEffects(u8);

impl DropEffects {
    pub const EMPTY: Self = Self(0);
    pub const MOVE: Self = Self(1 << 0);
    pub const COPY: Self = Self(1 << 1);
    pub const LINK: Self = Self(1 << 2);
    pub const STANDARD: Self = Self(Self::MOVE.0 | Self::COPY.0 | Self::LINK.0);
    pub const ALL: Self = Self::STANDARD;

    pub fn contains(self, effect: DropEffect) -> bool {
        let flag = match effect {
            DropEffect::Move => Self::MOVE,
            DropEffect::Copy => Self::COPY,
            DropEffect::Link => Self::LINK,
            DropEffect::None => return false,
        };
        self.0 & flag.0 != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Accept the proposed effect or choose a deterministic supported
    /// fallback. `None` is never chosen as a fallback for an active drop.
    pub fn negotiate(self, proposed: DropEffect) -> Option<DropEffect> {
        if proposed == DropEffect::None {
            return None;
        }
        if self.contains(proposed) {
            return Some(proposed);
        }
        [DropEffect::Move, DropEffect::Copy, DropEffect::Link]
            .into_iter()
            .find(|effect| self.contains(*effect))
    }
}

impl Default for DropEffects {
    fn default() -> Self {
        Self::ALL
    }
}

impl BitOr for DropEffects {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for DropEffects {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for DropEffects {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for DropEffects {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for DropEffects {
    type Output = Self;

    /// The complement within [`DropEffects::STANDARD`]: `!DropEffects::COPY`
    /// is `MOVE | LINK`. Bits outside the standard set are never produced.
    fn not(self) -> Self::Output {
        Self(!self.0 & Self::STANDARD.0)
    }
}

impl From<DropEffect> for DropEffects {
    /// The single-flag set for an effect; [`DropEffect::None`] maps to
    /// [`DropEffects::EMPTY`].
    fn from(effect: DropEffect) -> Self {
        match effect {
            DropEffect::Move => Self::MOVE,
            DropEffect::Copy => Self::COPY,
            DropEffect::Link => Self::LINK,
            DropEffect::None => Self::EMPTY,
        }
    }
}

impl FromIterator<DropEffect> for DropEffects {
    fn from_iter<I: IntoIterator<Item = DropEffect>>(iter: I) -> Self {
        iter.into_iter()
            .map(Self::from)
            .fold(Self::EMPTY, BitOr::bitor)
    }
}

/// Full target-side acceptance input.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DropQuery<T> {
    pub payload: T,
    pub source: Option<ZoneId>,
    pub proposed_effect: DropEffect,
    pub mode: DragMode,
    pub pointer_kind: PointerKind,
    pub drag_id: Option<DragId>,
}

impl<T> DropQuery<T> {
    pub fn new(payload: T) -> Self {
        Self {
            payload,
            source: None,
            proposed_effect: DropEffect::default(),
            mode: DragMode::default(),
            pointer_kind: PointerKind::default(),
            drag_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_keeps_supported_effect_and_falls_back_explicitly() {
        let effects = DropEffects::MOVE | DropEffects::COPY;
        assert_eq!(effects.negotiate(DropEffect::Copy), Some(DropEffect::Copy));
        assert_eq!(effects.negotiate(DropEffect::Link), Some(DropEffect::Move));
        assert_eq!(DropEffects::EMPTY.negotiate(DropEffect::Move), None);
        assert_eq!(DropEffects::ALL.negotiate(DropEffect::None), None);
        assert!(!DropEffects::ALL.contains(DropEffect::None));
    }

    #[test]
    fn set_operators_stay_inside_the_standard_set() {
        let move_copy = DropEffects::MOVE | DropEffects::COPY;
        assert_eq!(move_copy & DropEffects::COPY, DropEffects::COPY);
        assert_eq!(move_copy & DropEffects::LINK, DropEffects::EMPTY);
        assert_eq!(!DropEffects::COPY, DropEffects::MOVE | DropEffects::LINK);
        assert_eq!(!DropEffects::STANDARD, DropEffects::EMPTY);
        assert_eq!(!DropEffects::EMPTY, DropEffects::STANDARD);
        // Complementing twice must not leak bits above the defined flags.
        assert_eq!(!!DropEffects::COPY, DropEffects::COPY);

        let mut effects = DropEffects::STANDARD;
        effects &= DropEffects::MOVE | DropEffects::LINK;
        assert_eq!(effects, DropEffects::MOVE | DropEffects::LINK);
    }

    #[test]
    fn sets_build_from_effect_values() {
        assert_eq!(DropEffects::from(DropEffect::Link), DropEffects::LINK);
        assert_eq!(DropEffects::from(DropEffect::None), DropEffects::EMPTY);
        let collected: DropEffects = [DropEffect::Move, DropEffect::None, DropEffect::Copy]
            .into_iter()
            .collect();
        assert_eq!(collected, DropEffects::MOVE | DropEffects::COPY);
        assert_eq!(
            std::iter::empty::<DropEffect>().collect::<DropEffects>(),
            DropEffects::EMPTY
        );
    }
}

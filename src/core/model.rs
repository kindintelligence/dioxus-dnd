//! Small model helpers for applying completed drops to app-owned state.
//!
//! The crate never touches your data: drops arrive as [`DropOutcome`]s and
//! you decide what they mean. These helpers cover the most common meaning,
//! the remove-from-source, append-to-target dance, without imposing bounds
//! on your item type: no `Clone` (the payload arrives owned), no
//! `PartialEq` (matching is by the key you extract).

use std::collections::HashMap;

use super::{DropEffect, DropOutcome, ZoneId};

/// Apply a drop to a `HashMap<ZoneId, Vec<T>>` model.
///
/// `Move` removes the matching item from `outcome.from` before appending it
/// to `outcome.to`. `Copy` leaves the source alone and passes the payload
/// through `clone_item` first, which is where you should assign a fresh id.
///
/// Semantics worth knowing:
///
/// - Removal matches **every** item in the source whose key equals the
///   payload's key. Keys are expected to be unique within a zone; if they
///   are not, a single `Move` prunes all of them.
/// - A `Move` where `from == Some(to)` removes and re-appends, so dropping
///   an item back onto its own zone sends it to the **end of that list**.
/// - A `Move` with `from: None` (payload from outside any zone, e.g. a
///   palette) skips removal and just appends.
/// - An unknown `to` zone is created on the fly rather than dropping the
///   item on the floor.
pub fn apply_clone_or_move<T, K>(
    zones: &mut HashMap<ZoneId, Vec<T>>,
    outcome: DropOutcome<T>,
    key: impl Fn(&T) -> K,
    mut clone_item: impl FnMut(T) -> T,
) where
    K: PartialEq,
{
    let DropOutcome {
        payload,
        from,
        to,
        effect,
        ..
    } = outcome;
    let item = if effect == DropEffect::Copy {
        clone_item(payload)
    } else {
        if let Some(from) = from {
            let payload_key = key(&payload);
            if let Some(source) = zones.get_mut(&from) {
                source.retain(|item| key(item) != payload_key);
            }
        }
        payload
    };

    zones.entry(to).or_default().push(item);
}

/// Apply a drop between two plain `Vec<T>` lists.
///
/// `Move` removes the matching item from `source` before appending it to
/// `target`. `Copy` leaves the source alone and passes the payload through
/// `clone_item` first, which is where you should assign a fresh id.
///
/// You choose which lists to pass, so the outcome's `from` and `to` fields
/// are **ignored** here; only `payload` and `effect` are consulted. Pass
/// `None` for `source` when the payload came from outside any list. As with
/// [`apply_clone_or_move`], removal matches every item whose key equals the
/// payload's key.
pub fn apply_list_clone_or_move<T, K>(
    source: Option<&mut Vec<T>>,
    target: &mut Vec<T>,
    outcome: DropOutcome<T>,
    key: impl Fn(&T) -> K,
    mut clone_item: impl FnMut(T) -> T,
) where
    K: PartialEq,
{
    let DropOutcome {
        payload, effect, ..
    } = outcome;
    let item = if effect == DropEffect::Copy {
        clone_item(payload)
    } else {
        if let Some(source) = source {
            let payload_key = key(&payload);
            source.retain(|item| key(item) != payload_key);
        }
        payload
    };

    target.push(item);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{DragMode, Point};

    #[derive(Debug, Clone, PartialEq)]
    struct Card {
        id: u32,
        title: &'static str,
    }

    fn outcome(
        payload: Card,
        from: Option<ZoneId>,
        to: ZoneId,
        effect: DropEffect,
    ) -> DropOutcome<Card> {
        DropOutcome {
            payload,
            from,
            to,
            effect,
            mode: DragMode::Pointer,
            client: Point::default(),
            element: Point::default(),
            grab: Point::default(),
            edge: None,
        }
    }

    #[test]
    fn move_removes_from_source_and_appends_to_target() {
        let a = ZoneId(1);
        let b = ZoneId(2);
        let mut zones = HashMap::from([
            (
                a,
                vec![
                    Card {
                        id: 1,
                        title: "one",
                    },
                    Card {
                        id: 2,
                        title: "two",
                    },
                ],
            ),
            (
                b,
                vec![Card {
                    id: 3,
                    title: "three",
                }],
            ),
        ]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 2,
                    title: "two",
                },
                Some(a),
                b,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            zones[&a],
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            zones[&b],
            vec![
                Card {
                    id: 3,
                    title: "three"
                },
                Card {
                    id: 2,
                    title: "two"
                }
            ]
        );
    }

    #[test]
    fn copy_leaves_source_and_allows_new_identity() {
        let a = ZoneId(1);
        let b = ZoneId(2);
        let mut zones = HashMap::from([
            (
                a,
                vec![Card {
                    id: 1,
                    title: "one",
                }],
            ),
            (b, Vec::new()),
        ]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(a),
                b,
                DropEffect::Copy,
            ),
            |card| card.id,
            |mut card| {
                card.id = 10;
                card
            },
        );

        assert_eq!(
            zones[&a],
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            zones[&b],
            vec![Card {
                id: 10,
                title: "one"
            }]
        );
    }

    /// Pins the self-drop semantics documented on `apply_clone_or_move`: a
    /// `Move` back onto the source zone reorders the item to the end.
    #[test]
    fn move_onto_own_zone_reorders_to_end() {
        let a = ZoneId(1);
        let mut zones = HashMap::from([(
            a,
            vec![
                Card {
                    id: 1,
                    title: "one",
                },
                Card {
                    id: 2,
                    title: "two",
                },
            ],
        )]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(a),
                a,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            zones[&a],
            vec![
                Card {
                    id: 2,
                    title: "two"
                },
                Card {
                    id: 1,
                    title: "one"
                }
            ]
        );
    }

    /// A payload from outside any zone (palette, external drop) has no
    /// source to prune; `Move` just appends.
    #[test]
    fn move_without_source_zone_just_appends() {
        let b = ZoneId(2);
        let mut zones = HashMap::from([(b, Vec::new())]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 7,
                    title: "seven",
                },
                None,
                b,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            zones[&b],
            vec![Card {
                id: 7,
                title: "seven"
            }]
        );
    }

    /// An unknown target zone is created rather than losing the item.
    #[test]
    fn unknown_target_zone_is_created() {
        let a = ZoneId(1);
        let ghost = ZoneId(99);
        let mut zones = HashMap::from([(
            a,
            vec![Card {
                id: 1,
                title: "one",
            }],
        )]);

        apply_clone_or_move(
            &mut zones,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(a),
                ghost,
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert!(zones[&a].is_empty());
        assert_eq!(
            zones[&ghost],
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
    }

    #[test]
    fn list_move_removes_from_source_and_appends_to_target() {
        let mut source = vec![
            Card {
                id: 1,
                title: "one",
            },
            Card {
                id: 2,
                title: "two",
            },
        ];
        let mut target = vec![Card {
            id: 3,
            title: "three",
        }];

        apply_list_clone_or_move(
            Some(&mut source),
            &mut target,
            outcome(
                Card {
                    id: 2,
                    title: "two",
                },
                Some(ZoneId(1)),
                ZoneId(2),
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            source,
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            target,
            vec![
                Card {
                    id: 3,
                    title: "three"
                },
                Card {
                    id: 2,
                    title: "two"
                }
            ]
        );
    }

    #[test]
    fn list_copy_leaves_source_and_allows_new_identity() {
        let mut source = vec![Card {
            id: 1,
            title: "one",
        }];
        let mut target = Vec::new();

        apply_list_clone_or_move(
            Some(&mut source),
            &mut target,
            outcome(
                Card {
                    id: 1,
                    title: "one",
                },
                Some(ZoneId(1)),
                ZoneId(2),
                DropEffect::Copy,
            ),
            |card| card.id,
            |mut card| {
                card.id = 10;
                card
            },
        );

        assert_eq!(
            source,
            vec![Card {
                id: 1,
                title: "one"
            }]
        );
        assert_eq!(
            target,
            vec![Card {
                id: 10,
                title: "one"
            }]
        );
    }

    /// `Move` into a list without a source (`None`) skips removal.
    #[test]
    fn list_move_without_source_just_appends() {
        let mut target = Vec::new();

        apply_list_clone_or_move(
            None,
            &mut target,
            outcome(
                Card {
                    id: 7,
                    title: "seven",
                },
                None,
                ZoneId(2),
                DropEffect::Move,
            ),
            |card| card.id,
            |card| card,
        );

        assert_eq!(
            target,
            vec![Card {
                id: 7,
                title: "seven"
            }]
        );
    }
}

//! Ranked collision detection and release policy.

use dioxus::prelude::Callback;

use super::{Point, Rect, ZoneId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CollisionStrategy {
    #[default]
    PointerWithin,
    ClosestCenter,
    ClosestCorners,
    RectIntersection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ZoneCandidate {
    pub id: ZoneId,
    pub rect: Rect,
    pub order: usize,
}

impl ZoneCandidate {
    pub fn new(id: ZoneId, rect: Rect, order: usize) -> Self {
        Self { id, rect, order }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Collision {
    pub zone: ZoneId,
    /// Lower scores rank first.
    pub score: f64,
}

impl Collision {
    pub fn new(zone: ZoneId, score: f64) -> Self {
        Self { zone, score }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CollisionRequest<T> {
    pub pointer: Point,
    pub active_rect: Option<Rect>,
    pub payload: T,
    pub candidates: Vec<ZoneCandidate>,
    pub max_distance: f64,
}

impl<T> CollisionRequest<T> {
    pub fn new(pointer: Point, payload: T, candidates: Vec<ZoneCandidate>) -> Self {
        Self {
            pointer,
            active_rect: None,
            payload,
            candidates,
            max_distance: 0.0,
        }
    }
}

#[non_exhaustive]
pub enum CollisionDetector<T: 'static> {
    BuiltIn(CollisionStrategy),
    Custom(Callback<CollisionRequest<T>, Vec<Collision>>),
}

impl<T> Copy for CollisionDetector<T> {}
impl<T> Clone for CollisionDetector<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for CollisionDetector<T> {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (Self::BuiltIn(a), Self::BuiltIn(b)) => a == b,
            (Self::Custom(a), Self::Custom(b)) => a == b,
            _ => false,
        }
    }
}

impl<T> std::fmt::Debug for CollisionDetector<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuiltIn(strategy) => f.debug_tuple("BuiltIn").field(strategy).finish(),
            Self::Custom(_) => f.write_str("Custom(..)"),
        }
    }
}

impl<T> Default for CollisionDetector<T> {
    fn default() -> Self {
        Self::BuiltIn(CollisionStrategy::PointerWithin)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct ReleasePolicy<T: 'static> {
    pub collision: CollisionDetector<T>,
    pub recovery_radius: f64,
    pub sticky: bool,
}

impl<T> Copy for ReleasePolicy<T> {}
impl<T> Clone for ReleasePolicy<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for ReleasePolicy<T> {
    fn eq(&self, other: &Self) -> bool {
        self.collision == other.collision
            && self.recovery_radius == other.recovery_radius
            && self.sticky == other.sticky
    }
}

impl<T> Default for ReleasePolicy<T> {
    fn default() -> Self {
        Self {
            collision: CollisionDetector::default(),
            recovery_radius: 48.0,
            sticky: false,
        }
    }
}

impl<T> ReleasePolicy<T> {
    pub fn strategy(strategy: CollisionStrategy) -> Self {
        Self {
            collision: CollisionDetector::BuiltIn(strategy),
            ..Self::default()
        }
    }

    pub fn with_recovery_radius(mut self, recovery_radius: f64) -> Self {
        self.recovery_radius = recovery_radius.max(0.0);
        self
    }

    pub fn with_collision(mut self, collision: CollisionDetector<T>) -> Self {
        self.collision = collision;
        self
    }

    pub fn with_sticky(mut self, sticky: bool) -> Self {
        self.sticky = sticky;
        self
    }
}

pub fn rank_collisions<T: Clone + 'static>(
    detector: CollisionDetector<T>,
    request: CollisionRequest<T>,
) -> Vec<Collision> {
    match detector {
        CollisionDetector::BuiltIn(strategy) => rank_builtin(strategy, &request),
        CollisionDetector::Custom(callback) => {
            let orders: Vec<_> = request
                .candidates
                .iter()
                .map(|candidate| (candidate.id, candidate.order))
                .collect();
            let order = |zone| {
                orders
                    .iter()
                    .find(|(candidate, _)| *candidate == zone)
                    .map(|(_, order)| *order)
                    .unwrap_or_default()
            };
            let mut ranked = callback.call(request);
            ranked.sort_by(|a, b| {
                a.score
                    .total_cmp(&b.score)
                    .then_with(|| order(b.zone).cmp(&order(a.zone)))
            });
            ranked
        }
    }
}

pub(crate) fn rank_builtin_candidates(
    strategy: CollisionStrategy,
    pointer: Point,
    active_rect: Option<Rect>,
    candidates: Vec<ZoneCandidate>,
    max_distance: f64,
) -> Vec<Collision> {
    rank_builtin(
        strategy,
        &CollisionRequest {
            pointer,
            active_rect,
            payload: (),
            candidates,
            max_distance,
        },
    )
}

fn rank_builtin<T>(strategy: CollisionStrategy, request: &CollisionRequest<T>) -> Vec<Collision> {
    let mut ranked = Vec::new();
    for candidate in &request.candidates {
        let edge_distance = point_rect_distance(request.pointer, candidate.rect);
        let overlap = request
            .active_rect
            .map(|active| intersection_area(active, candidate.rect))
            .unwrap_or(0.0);
        let eligible = match (strategy, request.active_rect) {
            (CollisionStrategy::RectIntersection, Some(_)) => {
                overlap > 0.0
                    || (request.max_distance > 0.0 && edge_distance <= request.max_distance)
            }
            (CollisionStrategy::ClosestCenter | CollisionStrategy::ClosestCorners, Some(_)) => {
                overlap > 0.0 || edge_distance <= request.max_distance
            }
            _ => edge_distance <= request.max_distance,
        };
        if !eligible {
            continue;
        }
        let score = match strategy {
            CollisionStrategy::PointerWithin => edge_distance,
            CollisionStrategy::ClosestCenter => distance(
                request
                    .active_rect
                    .map(|rect| rect.center())
                    .unwrap_or(request.pointer),
                candidate.rect.center(),
            ),
            CollisionStrategy::ClosestCorners => match request.active_rect {
                Some(active) => closest_corner_distance(active, candidate.rect),
                None => point_corner_distance(request.pointer, candidate.rect),
            },
            CollisionStrategy::RectIntersection => {
                if overlap > 0.0 {
                    -overlap
                } else {
                    edge_distance
                }
            }
        };
        ranked.push((
            Collision {
                zone: candidate.id,
                score,
            },
            candidate.order,
        ));
    }
    ranked.sort_by(|(a, ao), (b, bo)| {
        a.score
            .total_cmp(&b.score)
            // Later registration wins exact ties, preserving the existing
            // overlap contract.
            .then_with(|| bo.cmp(ao))
    });
    ranked.into_iter().map(|(collision, _)| collision).collect()
}

fn distance(a: Point, b: Point) -> f64 {
    let d = a - b;
    (d.x * d.x + d.y * d.y).sqrt()
}

pub fn point_rect_distance(point: Point, rect: Rect) -> f64 {
    let dx = (rect.x - point.x)
        .max(point.x - (rect.x + rect.width))
        .max(0.0);
    let dy = (rect.y - point.y)
        .max(point.y - (rect.y + rect.height))
        .max(0.0);
    (dx * dx + dy * dy).sqrt()
}

fn corners(rect: Rect) -> [Point; 4] {
    [
        Point::new(rect.x, rect.y),
        Point::new(rect.x + rect.width, rect.y),
        Point::new(rect.x + rect.width, rect.y + rect.height),
        Point::new(rect.x, rect.y + rect.height),
    ]
}

fn point_corner_distance(point: Point, rect: Rect) -> f64 {
    corners(rect)
        .into_iter()
        .map(|corner| distance(point, corner))
        .min_by(f64::total_cmp)
        .unwrap_or(f64::INFINITY)
}

fn closest_corner_distance(a: Rect, b: Rect) -> f64 {
    corners(a)
        .into_iter()
        .flat_map(|left| {
            corners(b)
                .into_iter()
                .map(move |right| distance(left, right))
        })
        .min_by(f64::total_cmp)
        .unwrap_or(f64::INFINITY)
}

fn intersection_area(a: Rect, b: Rect) -> f64 {
    let width = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let height = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    width.max(0.0) * height.max(0.0)
}

#[cfg(test)]
mod tests {
    use dioxus::prelude::*;

    use super::*;

    fn request(point: Point) -> CollisionRequest<()> {
        CollisionRequest {
            pointer: point,
            active_rect: None,
            payload: (),
            candidates: vec![
                ZoneCandidate {
                    id: ZoneId(1),
                    rect: Rect::new(0.0, 0.0, 100.0, 100.0),
                    order: 0,
                },
                ZoneCandidate {
                    id: ZoneId(2),
                    rect: Rect::new(50.0, 0.0, 100.0, 100.0),
                    order: 1,
                },
            ],
            max_distance: 0.0,
        }
    }

    #[test]
    fn pointer_within_preserves_later_overlap_precedence() {
        let ranked = rank_collisions(
            CollisionDetector::BuiltIn(CollisionStrategy::PointerWithin),
            request(Point::new(75.0, 50.0)),
        );
        assert_eq!(ranked[0].zone, ZoneId(2));
    }

    #[test]
    fn recovery_radius_uses_distance_to_rect_edge() {
        let mut request = request(Point::new(160.0, 50.0));
        request.max_distance = 12.0;
        let ranked = rank_collisions(CollisionDetector::default(), request);
        assert_eq!(ranked[0].zone, ZoneId(2));
    }

    #[test]
    fn rect_intersection_does_not_require_pointer_inside_target() {
        let ranked = rank_collisions(
            CollisionDetector::BuiltIn(CollisionStrategy::RectIntersection),
            CollisionRequest {
                pointer: Point::new(25.0, 50.0),
                active_rect: Some(Rect::new(25.0, 25.0, 50.0, 50.0)),
                payload: (),
                candidates: vec![ZoneCandidate {
                    id: ZoneId(1),
                    rect: Rect::new(60.0, 25.0, 50.0, 50.0),
                    order: 0,
                }],
                max_distance: 0.0,
            },
        );
        assert_eq!(ranked[0].zone, ZoneId(1));
    }

    #[test]
    fn rect_intersection_exact_hover_requires_shape_overlap() {
        let ranked = rank_collisions(
            CollisionDetector::BuiltIn(CollisionStrategy::RectIntersection),
            CollisionRequest {
                pointer: Point::new(75.0, 50.0),
                active_rect: Some(Rect::new(0.0, 0.0, 20.0, 20.0)),
                payload: (),
                candidates: vec![ZoneCandidate {
                    id: ZoneId(1),
                    rect: Rect::new(50.0, 0.0, 100.0, 100.0),
                    order: 0,
                }],
                max_distance: 0.0,
            },
        );
        assert!(ranked.is_empty());
    }

    #[test]
    fn closest_center_uses_the_active_shape_and_can_rank_pointer_outside() {
        let ranked = rank_collisions(
            CollisionDetector::BuiltIn(CollisionStrategy::ClosestCenter),
            CollisionRequest {
                pointer: Point::new(10.0, 10.0),
                active_rect: Some(Rect::new(40.0, 0.0, 40.0, 40.0)),
                payload: (),
                candidates: vec![
                    ZoneCandidate {
                        id: ZoneId(1),
                        rect: Rect::new(60.0, 0.0, 40.0, 40.0),
                        order: 0,
                    },
                    ZoneCandidate {
                        id: ZoneId(2),
                        rect: Rect::new(500.0, 0.0, 40.0, 40.0),
                        order: 1,
                    },
                ],
                max_distance: 0.0,
            },
        );
        assert_eq!(ranked[0].zone, ZoneId(1));
    }

    fn custom_ranking_probe() -> Element {
        let ranked = rank_collisions(
            CollisionDetector::Custom(Callback::new(|_| {
                vec![
                    Collision::new(ZoneId(1), 10.0),
                    Collision::new(ZoneId(2), 1.0),
                    Collision::new(ZoneId(3), 1.0),
                ]
            })),
            CollisionRequest {
                pointer: Point::default(),
                active_rect: None,
                payload: (),
                candidates: vec![
                    ZoneCandidate::new(ZoneId(1), Rect::default(), 0),
                    ZoneCandidate::new(ZoneId(2), Rect::default(), 1),
                    ZoneCandidate::new(ZoneId(3), Rect::default(), 2),
                ],
                max_distance: 0.0,
            },
        );
        assert_eq!(
            ranked
                .iter()
                .map(|collision| collision.zone)
                .collect::<Vec<_>>(),
            [ZoneId(3), ZoneId(2), ZoneId(1)]
        );
        rsx! {}
    }

    #[test]
    fn custom_results_are_sorted_by_score_then_registration_order() {
        let mut dom = VirtualDom::new(custom_ranking_probe);
        dom.rebuild_in_place();
    }
}

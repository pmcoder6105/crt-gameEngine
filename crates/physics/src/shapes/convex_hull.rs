use elderforge_core::math::Vec3;
use serde::{Deserialize, Serialize};

use crate::broadphase::Aabb;

/// Convex hull defined by its vertices in local space.
/// // TODO: precomputed face/edge adjacency for SAT.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConvexHull {
    pub points: Vec<Vec3>,
}

impl ConvexHull {
    pub fn aabb(&self, position: Vec3) -> Aabb {
        let mut min = Vec3::ZERO;
        let mut max = Vec3::ZERO;
        for point in &self.points {
            min = min.min(*point);
            max = max.max(*point);
        }
        Aabb::new(position + min, position + max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_bounds_all_points() {
        let hull = ConvexHull {
            points: vec![Vec3::new(-1.0, 0.0, 0.0), Vec3::new(2.0, 3.0, -4.0)],
        };
        let aabb = hull.aabb(Vec3::ZERO);
        assert_eq!(aabb.min, Vec3::new(-1.0, 0.0, -4.0));
        assert_eq!(aabb.max, Vec3::new(2.0, 3.0, 0.0));
    }
}

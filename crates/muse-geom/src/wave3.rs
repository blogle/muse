//! Shared numerical conventions for Wave 3 surface-vector operators.
use glam::DVec3;

/// Named f64 radial/tangency tolerance for unit-sphere fields.
pub const TANGENCY_TOLERANCE: f64 = 1.0e-10;
pub const DEGENERACY_EPSILON: f64 = 1.0e-12;

/// Projects to the unit-position tangent plane, returning zero for invalid geometry.
pub fn project_tangent(position: DVec3, vector: DVec3) -> DVec3 {
    let length = position.length();
    if !position.is_finite()
        || !vector.is_finite()
        || !length.is_finite()
        || length <= DEGENERACY_EPSILON
    {
        return DVec3::ZERO;
    }
    let p = position / length;
    vector - p * vector.dot(p)
}

/// Parallel transport by shortest-geodesic rotation. Coincident points use projection;
/// antipodal/degenerate endpoints use deterministic projection fallback, never NaN.
pub fn parallel_transport(from: DVec3, to: DVec3, vector: DVec3) -> DVec3 {
    let a = from.normalize_or_zero();
    let b = to.normalize_or_zero();
    if a == DVec3::ZERO || b == DVec3::ZERO || !vector.is_finite() {
        return DVec3::ZERO;
    }
    let axis = a.cross(b);
    let sin = axis.length();
    let cos = a.dot(b).clamp(-1.0, 1.0);
    if sin <= DEGENERACY_EPSILON {
        return project_tangent(b, project_tangent(a, vector));
    }
    let rotation_axis = axis / sin;
    let angle = sin.atan2(cos);
    let transported = vector * angle.cos()
        + rotation_axis.cross(vector) * angle.sin()
        + rotation_axis * rotation_axis.dot(vector) * (1.0 - angle.cos());
    project_tangent(b, transported)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_and_transport_are_finite_and_tangent() {
        for to in [DVec3::X, DVec3::Y, -DVec3::X, DVec3::ZERO] {
            let moved = parallel_transport(DVec3::X, to, DVec3::Y);
            assert!(moved.is_finite());
            if to != DVec3::ZERO {
                assert!(to.normalize().dot(moved).abs() < TANGENCY_TOLERANCE);
            }
        }
    }
}

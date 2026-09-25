//! Tangent projection and deterministic shortest-geodesic vector transport.
use glam::DVec3;

/// Named f64 radial/tangency tolerance for unit-sphere fields.
pub const TANGENCY_TOLERANCE: f64 = 1.0e-10;
/// Epsilon used for degenerate sphere positions and transport fallbacks.
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

/// Signed normal motion along a canonical source-to-target edge. Positive is approach.
pub fn boundary_normal_relative_motion(
    source_position: DVec3,
    target_position: DVec3,
    source_velocity: DVec3,
    target_velocity: DVec3,
    edge_tangent_source_to_target: DVec3,
) -> f64 {
    let target_at_source = parallel_transport(target_position, source_position, target_velocity);
    let source_velocity = project_tangent(source_position, source_velocity);
    let tangent =
        project_tangent(source_position, edge_tangent_source_to_target).normalize_or_zero();
    (source_velocity - target_at_source).dot(tangent)
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

    #[test]
    fn boundary_normal_is_positive_for_approach_and_reverses_under_velocity_negation() {
        let source = DVec3::X;
        let target = DVec3::Y;
        let tangent = DVec3::Y;
        // Source moves toward target (+Y); target moves toward source (+X).
        let approach = boundary_normal_relative_motion(source, target, DVec3::Y, DVec3::X, tangent);
        let separation =
            boundary_normal_relative_motion(source, target, -DVec3::Y, -DVec3::X, tangent);
        assert!(approach > 0.0);
        assert!(separation < 0.0);
        assert!((approach + separation).abs() < TANGENCY_TOLERANCE);
    }
}

//! Signed boundary-normal interaction over cross-label mesh edges.
use glam::DVec3;
use muse_types::Mesh;

use crate::OperatorError;

const TANGENCY_TOLERANCE: f64 = 1.0e-10;
const DEGENERACY_EPSILON: f64 = 1.0e-12;

fn project_tangent(position: DVec3, vector: DVec3) -> DVec3 {
    vector - position * vector.dot(position)
}

fn parallel_transport(from: DVec3, to: DVec3, vector: DVec3) -> DVec3 {
    let axis = from.cross(to);
    let sin = axis.length();
    let cos = from.dot(to).clamp(-1.0, 1.0);
    if sin <= DEGENERACY_EPSILON {
        return project_tangent(to, project_tangent(from, vector));
    }
    let rotation_axis = axis / sin;
    let angle = sin.atan2(cos);
    let moved = vector * angle.cos()
        + rotation_axis.cross(vector) * angle.sin()
        + rotation_axis * rotation_axis.dot(vector) * (1.0 - angle.cos());
    project_tangent(to, moved)
}

/// Compute signed approach/separation at cells incident to a category boundary.
///
/// Each undirected edge is considered once in ascending cell-ID order. Incident
/// edge contributions are averaged by angular edge length, with sequential
/// canonical accumulation so results do not depend on neighbor-vector order or
/// Rayon scheduling.
pub fn boundary_normal_component(
    mesh: &Mesh,
    labels: &[u32],
    velocity: &[DVec3],
) -> crate::Result<Vec<f64>> {
    crate::validate_component_mesh(mesh)?;
    let n = mesh.positions.len();
    if labels.len() != n || velocity.len() != n {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }

    for (index, (&position, &vector)) in mesh.positions.iter().zip(velocity).enumerate() {
        if !position.is_finite()
            || !vector.is_finite()
            || (position.length() - 1.0).abs() > TANGENCY_TOLERANCE
            || position.dot(vector).abs() > TANGENCY_TOLERANCE
        {
            return Err(OperatorError::Invalid(format!(
                "boundary inputs must be finite unit-sphere tangent vectors (cell {index})"
            )));
        }
    }

    let mut edges = Vec::new();
    for source in 0..n {
        for &target_id in &mesh.neighbors[source] {
            let target = target_id as usize;
            if source >= target || labels[source] == labels[target] {
                continue;
            }
            let source_position = mesh.positions[source];
            let target_position = mesh.positions[target];
            let edge_length = source_position.dot(target_position).clamp(-1.0, 1.0).acos();
            if !edge_length.is_finite() || edge_length <= 0.0 {
                return Err(OperatorError::Invalid(
                    "boundary edge has invalid geometry".into(),
                ));
            }
            let edge_tangent =
                project_tangent(source_position, target_position).normalize_or_zero();
            let target_velocity =
                parallel_transport(target_position, source_position, velocity[target]);
            let contribution = (velocity[source] - target_velocity).dot(edge_tangent);
            if !contribution.is_finite() {
                return Err(OperatorError::Invalid(
                    "boundary result is not finite".into(),
                ));
            }
            edges.push((source, target, contribution, edge_length));
        }
    }

    edges.sort_unstable_by_key(|&(source, target, _, _)| (source, target));
    let mut sums = vec![0.0; n];
    let mut weights = vec![0.0; n];
    for (source, target, contribution, weight) in edges {
        sums[source] += contribution * weight;
        sums[target] += contribution * weight;
        weights[source] += weight;
        weights[target] += weight;
    }
    let output: Vec<_> = sums
        .iter()
        .zip(weights)
        .map(|(&sum, weight)| if weight == 0.0 { 0.0 } else { sum / weight })
        .collect();
    if output.iter().any(|value| !value.is_finite()) {
        return Err(OperatorError::Invalid(
            "boundary result is not finite".into(),
        ));
    }
    Ok(output)
}

pub(crate) fn dispatch(operator_id: &str) -> OperatorError {
    super::not_implemented(operator_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn pair_mesh() -> Mesh {
        Mesh {
            positions: vec![DVec3::X, DVec3::Y, DVec3::Z],
            triangles: Vec::new(),
            neighbors: vec![vec![1], vec![0], Vec::new()],
        }
    }

    #[test]
    fn normal_motion_has_expected_sign_and_zero_interior() {
        let mesh = pair_mesh();
        let labels = [1, 2, 1];
        let approaching = [DVec3::Y, DVec3::X, DVec3::ZERO];
        let separated = [-DVec3::Y, -DVec3::X, DVec3::ZERO];
        let convergence = boundary_normal_component(&mesh, &labels, &approaching).unwrap();
        let divergence = boundary_normal_component(&mesh, &labels, &separated).unwrap();
        assert!(convergence[0] > 0.0 && convergence[1] > 0.0);
        assert!(divergence[0] < 0.0 && divergence[1] < 0.0);
        assert_eq!(convergence[2], 0.0);
        assert_eq!(
            boundary_normal_component(&mesh, &labels, &[DVec3::ZERO; 3]).unwrap(),
            vec![0.0; 3]
        );
        assert_eq!(
            boundary_normal_component(&mesh, &labels, &separated).unwrap(),
            convergence.iter().map(|value| -*value).collect::<Vec<_>>()
        );
    }

    #[test]
    fn aggregation_is_canonical_and_validates_fields() {
        let mesh = Mesh {
            positions: vec![
                DVec3::X,
                DVec3::Y,
                (DVec3::X + DVec3::Y).normalize(),
                DVec3::Z,
            ],
            triangles: Vec::new(),
            neighbors: vec![vec![2, 1, 3], vec![0], vec![0], vec![0]],
        };
        let labels = [1, 2, 3, 4];
        let velocity = [DVec3::Y, DVec3::X, DVec3::ZERO, DVec3::ZERO];
        let expected = boundary_normal_component(&mesh, &labels, &velocity).unwrap();
        let mut reordered = mesh.clone();
        reordered.neighbors[0].reverse();
        assert_eq!(
            expected,
            boundary_normal_component(&reordered, &labels, &velocity).unwrap()
        );
        assert!(boundary_normal_component(&mesh, &labels[..3], &velocity).is_err());
        assert!(boundary_normal_component(&mesh, &labels, &[DVec3::ZERO; 3]).is_err());
        let mut invalid = velocity;
        invalid[0].x = f64::NAN;
        assert!(boundary_normal_component(&mesh, &labels, &invalid).is_err());
        let mut nontangent = velocity;
        nontangent[0] = DVec3::X;
        assert!(boundary_normal_component(&mesh, &labels, &nontangent).is_err());
    }
}

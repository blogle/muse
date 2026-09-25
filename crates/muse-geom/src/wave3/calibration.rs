//! Immutable geometric calibration for generic unit-sphere spatial operators.
//!
//! Construction traverses the input cells, neighbor lists, and triangles once. It
//! allocates normalized positions and cell areas (O(cells)), and one canonical
//! record per undirected edge (O(edges)); it does not retain or copy topology.
//! Neighbor lists must be sorted, unique, in-range, loop-free, and symmetric.
//! Every triangle must be valid and every cell must be covered by positive-area
//! triangles. Edge records are in lexicographic `(source, target)` order with
//! `source < target`. Distances are angular radians on the unit sphere, not hops.

use std::fmt;

use glam::DVec3;
use muse_types::{CellId, Mesh};

const POSITION_EPSILON: f64 = 1.0e-15;
const GEOMETRY_EPSILON: f64 = 1.0e-15;

/// Explicit failure modes for malformed or degenerate spherical meshes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationError {
    /// No cells were supplied.
    EmptyMesh,
    /// A position is non-finite or has zero length.
    InvalidPosition(CellId),
    /// Neighbor lists do not conform to the sorted symmetric graph contract.
    InvalidNeighbors(CellId),
    /// A triangle has an invalid, repeated, or out-of-range cell ID.
    InvalidTriangle(usize),
    /// A triangle has zero or numerically degenerate spherical area.
    DegenerateTriangle(usize),
    /// A mesh cell has no positive accumulated spherical area.
    UncoveredCell(CellId),
    /// An adjacent pair is coincident or antipodal, making its shortest edge
    /// direction undefined.
    DegenerateEdge(CellId, CellId),
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMesh => write!(f, "mesh must contain cells"),
            Self::InvalidPosition(cell) => write!(f, "invalid spherical position at cell {cell}"),
            Self::InvalidNeighbors(cell) => write!(f, "invalid neighbor list for cell {cell}"),
            Self::InvalidTriangle(index) => write!(f, "invalid triangle at index {index}"),
            Self::DegenerateTriangle(index) => {
                write!(f, "degenerate spherical triangle at index {index}")
            }
            Self::UncoveredCell(cell) => write!(f, "cell {cell} has no positive spherical area"),
            Self::DegenerateEdge(a, b) => write!(f, "degenerate edge ({a}, {b})"),
        }
    }
}

impl std::error::Error for CalibrationError {}

/// Geometry for one undirected mesh edge. Tangents point source-to-target at
/// their respective endpoints; `inverse_distance` is radians⁻¹.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CalibratedEdge {
    pub source: CellId,
    pub target: CellId,
    pub angular_distance: f64,
    pub inverse_distance: f64,
    pub tangent_at_source: DVec3,
    pub tangent_at_target: DVec3,
}

/// Reusable immutable cell and edge geometry for generic spatial derivatives.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshCalibration {
    positions: Vec<DVec3>,
    cell_areas: Vec<f64>,
    edges: Vec<CalibratedEdge>,
}

impl MeshCalibration {
    /// Calibrate validated unit-sphere geometry in deterministic mesh order.
    pub fn new(mesh: &Mesh) -> Result<Self, CalibrationError> {
        let cell_count = mesh.positions.len();
        if cell_count == 0 {
            return Err(CalibrationError::EmptyMesh);
        }

        let mut positions = Vec::with_capacity(cell_count);
        for (index, point) in mesh.positions.iter().enumerate() {
            let length = point.length();
            if !point.is_finite() || !length.is_finite() || length <= POSITION_EPSILON {
                return Err(CalibrationError::InvalidPosition(index as CellId));
            }
            positions.push(*point / length);
        }

        if mesh.neighbors.len() != cell_count {
            return Err(CalibrationError::InvalidNeighbors(0));
        }
        let mut edge_count = 0;
        for (index, adjacent) in mesh.neighbors.iter().enumerate() {
            let cell = index as CellId;
            if adjacent.windows(2).any(|pair| pair[0] >= pair[1])
                || adjacent.iter().any(|&neighbor| {
                    neighbor as usize >= cell_count
                        || neighbor == cell
                        || mesh.neighbors[neighbor as usize]
                            .binary_search(&cell)
                            .is_err()
                })
            {
                return Err(CalibrationError::InvalidNeighbors(cell));
            }
            edge_count += adjacent.iter().filter(|&&neighbor| neighbor > cell).count();
        }

        let mut cell_areas = vec![0.0; cell_count];
        for (triangle_index, triangle) in mesh.triangles.iter().enumerate() {
            let [a, b, c] = *triangle;
            if [a, b, c].iter().any(|&cell| cell as usize >= cell_count)
                || a == b
                || b == c
                || a == c
            {
                return Err(CalibrationError::InvalidTriangle(triangle_index));
            }
            let (pa, pb, pc) = (
                positions[a as usize],
                positions[b as usize],
                positions[c as usize],
            );
            let area = 2.0
                * pa.dot(pb.cross(pc))
                    .abs()
                    .atan2(1.0 + pa.dot(pb) + pb.dot(pc) + pc.dot(pa));
            if !area.is_finite() || area <= GEOMETRY_EPSILON {
                return Err(CalibrationError::DegenerateTriangle(triangle_index));
            }
            let third = area / 3.0;
            cell_areas[a as usize] += third;
            cell_areas[b as usize] += third;
            cell_areas[c as usize] += third;
        }
        for (index, area) in cell_areas.iter().enumerate() {
            if !area.is_finite() || *area <= 0.0 {
                return Err(CalibrationError::UncoveredCell(index as CellId));
            }
        }

        let mut edges = Vec::with_capacity(edge_count);
        for (source_index, adjacent) in mesh.neighbors.iter().enumerate() {
            let source = source_index as CellId;
            for &target in adjacent.iter().filter(|&&target| target > source) {
                let (a, b) = (positions[source as usize], positions[target as usize]);
                let cross = a.cross(b);
                let sine = cross.length();
                let cosine = a.dot(b).clamp(-1.0, 1.0);
                let angular_distance = sine.atan2(cosine);
                if !sine.is_finite()
                    || sine <= GEOMETRY_EPSILON
                    || !angular_distance.is_finite()
                    || angular_distance <= GEOMETRY_EPSILON
                    || (std::f64::consts::PI - angular_distance) <= GEOMETRY_EPSILON
                {
                    return Err(CalibrationError::DegenerateEdge(source, target));
                }
                let tangent_at_source = (b - a * cosine).normalize();
                let tangent_at_target = (b * cosine - a).normalize();
                edges.push(CalibratedEdge {
                    source,
                    target,
                    angular_distance,
                    inverse_distance: angular_distance.recip(),
                    tangent_at_source,
                    tangent_at_target,
                });
            }
        }

        Ok(Self {
            positions,
            cell_areas,
            edges,
        })
    }

    /// Unit-normalized world-space cell positions.
    pub fn positions(&self) -> &[DVec3] {
        &self.positions
    }

    /// Per-cell spherical area in steradians, distributed equally from incident
    /// triangle areas. The sum equals the sum of mesh triangle areas.
    pub fn cell_areas(&self) -> &[f64] {
        &self.cell_areas
    }

    /// Canonically ordered unique undirected edges.
    pub fn edges(&self) -> &[CalibratedEdge] {
        &self.edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icosphere;

    #[test]
    fn canonical_mesh_calibration_is_repeatable_and_has_valid_geometric_weights() {
        for level in [0, 5] {
            let mesh = icosphere(level).unwrap();
            let first = MeshCalibration::new(&mesh).unwrap();
            let second = MeshCalibration::new(&mesh).unwrap();
            assert_eq!(first, second);
            assert!(
                first
                    .cell_areas()
                    .iter()
                    .all(|area| area.is_finite() && *area > 0.0)
            );
            assert!(first.edges().windows(2).all(|pair| {
                (pair[0].source, pair[0].target) < (pair[1].source, pair[1].target)
            }));
            assert!(first.edges().iter().all(|edge| {
                edge.source < edge.target
                    && edge.angular_distance.is_finite()
                    && edge.angular_distance > 0.0
                    && edge.inverse_distance.is_finite()
                    && edge.inverse_distance > 0.0
                    && edge.tangent_at_source.is_finite()
                    && edge.tangent_at_target.is_finite()
                    && first.positions()[edge.source as usize]
                        .dot(edge.tangent_at_source)
                        .abs()
                        < 1.0e-10
                    && first.positions()[edge.target as usize]
                        .dot(edge.tangent_at_target)
                        .abs()
                        < 1.0e-10
            }));
        }
    }

    #[test]
    fn malformed_geometry_returns_specific_errors() {
        let mut mesh = icosphere(0).unwrap();
        mesh.positions[0] = DVec3::splat(f64::NAN);
        assert_eq!(
            MeshCalibration::new(&mesh),
            Err(CalibrationError::InvalidPosition(0))
        );

        let mut mesh = icosphere(0).unwrap();
        mesh.neighbors[0].clear();
        assert!(matches!(
            MeshCalibration::new(&mesh),
            Err(CalibrationError::InvalidNeighbors(_))
        ));
    }
}

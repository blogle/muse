//! Canonical spherical geometry built on the approved `hexasphere` mesh.

use glam::DVec3;
use hexasphere::shapes::IcoSphere;
use muse_types::Mesh;

/// Geometry construction errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryError {
    /// Levels above five exceed the supported canonical mesh resolution.
    UnsupportedLevel(u8),
}

impl std::fmt::Display for GeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLevel(level) => write!(f, "unsupported icosphere level {level}"),
        }
    }
}

impl std::error::Error for GeometryError {}

/// Constructs a canonical icosphere at the requested subdivision level.
pub fn icosphere(level: u8) -> Result<Mesh, GeometryError> {
    if level > 5 {
        return Err(GeometryError::UnsupportedLevel(level));
    }

    let subdivisions = (1_usize << level) - 1;
    let sphere = IcoSphere::new(subdivisions, |_| ());
    let positions: Vec<DVec3> = sphere
        .raw_points()
        .iter()
        .map(|point| DVec3::new(point.x as f64, point.y as f64, point.z as f64).normalize())
        .collect();
    let indices = sphere.get_all_indices();
    let triangles: Vec<[u32; 3]> = indices.as_chunks::<3>().0.to_vec();

    let mut neighbors = vec![Vec::new(); positions.len()];
    for [a, b, c] in &triangles {
        for (from, to) in [(*a, *b), (*b, *a), (*b, *c), (*c, *b), (*c, *a), (*a, *c)] {
            if !neighbors[from as usize].contains(&to) {
                neighbors[from as usize].push(to);
            }
        }
    }
    for adjacent in &mut neighbors {
        adjacent.sort_unstable();
    }

    Ok(Mesh {
        positions,
        triangles,
        neighbors,
    })
}

/// Latitude in radians, measured from the equatorial plane.
pub fn latitude(position: DVec3) -> f64 {
    position.y.asin()
}

/// Projects a vector onto the tangent plane at `position`.
pub fn project_tangent(position: DVec3, vector: DVec3) -> DVec3 {
    vector - position * vector.dot(position) / position.length_squared()
}

/// Angular distance between two points on the unit sphere, in radians.
pub fn great_circle_distance(a: DVec3, b: DVec3) -> f64 {
    a.normalize().dot(b.normalize()).clamp(-1.0, 1.0).acos()
}

/// Unit tangent at `a` pointing along the shortest great-circle arc to `b`.
pub fn edge_tangent(a: DVec3, b: DVec3) -> DVec3 {
    project_tangent(a, b).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn canonical_mesh_counts_topology_and_unit_positions() {
        let base = icosphere(0).unwrap();
        assert_eq!(base.positions.len(), 12);
        assert_eq!(base.triangles.len(), 20);

        let mesh = icosphere(5).unwrap();
        assert_eq!(mesh.positions.len(), 10_242);
        assert!(
            mesh.positions
                .iter()
                .all(|p| (p.length() - 1.0).abs() < 1e-10)
        );
        let degree_five = mesh
            .neighbors
            .iter()
            .filter(|neighbors| neighbors.len() == 5)
            .count();
        assert_eq!(degree_five, 12);
        for (cell, neighbors) in mesh.neighbors.iter().enumerate() {
            assert!(neighbors.len() == 5 || neighbors.len() == 6);
            assert!(!neighbors.contains(&(cell as u32)));
            assert!(neighbors.windows(2).all(|pair| pair[0] != pair[1]));
            assert!(
                neighbors
                    .iter()
                    .all(|neighbor| mesh.neighbors[*neighbor as usize].contains(&(cell as u32)))
            );
        }
        assert!(mesh.triangles.iter().all(|triangle| {
            triangle
                .iter()
                .all(|id| (*id as usize) < mesh.positions.len())
                && triangle[0] != triangle[1]
                && triangle[1] != triangle[2]
                && triangle[2] != triangle[0]
        }));
    }

    #[test]
    fn repeated_construction_is_byte_identical() {
        let serialize = |mesh: &Mesh| {
            let mut bytes = Vec::new();
            for point in &mesh.positions {
                bytes.extend(point.x.to_le_bytes());
                bytes.extend(point.y.to_le_bytes());
                bytes.extend(point.z.to_le_bytes());
            }
            for triangle in &mesh.triangles {
                for id in triangle {
                    bytes.extend(id.to_le_bytes());
                }
            }
            for adjacent in &mesh.neighbors {
                bytes.extend((adjacent.len() as u32).to_le_bytes());
                for id in adjacent {
                    bytes.extend(id.to_le_bytes());
                }
            }
            bytes
        };
        let first = serialize(&icosphere(3).unwrap());
        let second = serialize(&icosphere(3).unwrap());
        assert_eq!(first, second);
    }

    #[test]
    fn tangent_projection_is_orthogonal() {
        let position = DVec3::new(1.0, 2.0, 3.0).normalize();
        let result = project_tangent(position, DVec3::new(-2.0, 4.0, 1.0));
        assert!(position.dot(result).abs() < 1e-10);
    }

    fn unit_vectors() -> impl Strategy<Value = DVec3> {
        (-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0)
            .prop_filter("nonzero vector", |(x, y, z)| x * x + y * y + z * z > 1e-8)
            .prop_map(|(x, y, z)| DVec3::new(x, y, z).normalize())
    }

    proptest! {
        #[test]
        fn random_tangent_projection_is_orthogonal(position in unit_vectors(), vector in unit_vectors()) {
            let projected = project_tangent(position, vector);
            prop_assert!(position.dot(projected).abs() < 1e-10);
        }

        #[test]
        fn great_circle_distance_is_symmetric(a in unit_vectors(), b in unit_vectors()) {
            prop_assert!((great_circle_distance(a, b) - great_circle_distance(b, a)).abs() < 1e-12);
        }
    }
}

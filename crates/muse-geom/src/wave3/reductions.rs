//! Stable deterministic reductions over canonical edges and cells.
use glam::DVec3;
use muse_types::CellId;

/// An edge contribution and its non-negative geometry weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedEdge<T> {
    pub cells: [CellId; 2],
    pub weight: f64,
    pub value: T,
}

/// Invalid input to a deterministic reduction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReductionError {
    /// Per-edge value and weight arrays have different lengths.
    LengthMismatch,
    /// An edge references a cell outside the declared cell range.
    InvalidCell(CellId),
    /// Self-edges do not define an edge contribution.
    SelfEdge(CellId),
    /// A weight is negative or non-finite.
    InvalidWeight,
    /// An input value is non-finite.
    NonFiniteValue,
    /// A finite input overflowed during accumulation or normalization.
    NonFiniteResult,
}

impl std::fmt::Display for ReductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid reduction input/result: {self:?}")
    }
}

impl std::error::Error for ReductionError {}

fn canonicalize<T: Copy>(
    cell_count: usize,
    edges: &[WeightedEdge<T>],
    finite: impl Fn(T) -> bool,
    key: impl Fn(T) -> [u64; 3],
) -> Result<Vec<WeightedEdge<T>>, ReductionError> {
    let mut ordered = Vec::with_capacity(edges.len());
    for edge in edges {
        let [a, b] = edge.cells;
        if a as usize >= cell_count {
            return Err(ReductionError::InvalidCell(a));
        }
        if b as usize >= cell_count {
            return Err(ReductionError::InvalidCell(b));
        }
        if a == b {
            return Err(ReductionError::SelfEdge(a));
        }
        if !edge.weight.is_finite() || edge.weight < 0.0 {
            return Err(ReductionError::InvalidWeight);
        }
        if !finite(edge.value) {
            return Err(ReductionError::NonFiniteValue);
        }
        let mut canonical = *edge;
        canonical.cells.sort_unstable();
        ordered.push(canonical);
    }
    // Include contribution data in tie-breaking so equivalent edge multisets have
    // the same summation order regardless of their input order.
    ordered.sort_by(|a, b| {
        a.cells
            .cmp(&b.cells)
            .then_with(|| a.weight.total_cmp(&b.weight))
            .then_with(|| key(a.value).cmp(&key(b.value)))
    });
    Ok(ordered)
}

fn check(value: f64) -> Result<f64, ReductionError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(ReductionError::NonFiniteResult)
}

/// Deterministically sums finite scalar values in stable cell order.
pub fn stable_sum(values: &[f64]) -> Result<f64, ReductionError> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(ReductionError::NonFiniteValue);
    }
    let mut sum = 0.0;
    for &value in values {
        sum = check(sum + value)?;
    }
    Ok(sum)
}

/// Deterministically sums finite vectors in stable cell order.
pub fn stable_vector_sum(values: &[DVec3]) -> Result<DVec3, ReductionError> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(ReductionError::NonFiniteValue);
    }
    let mut sum = DVec3::ZERO;
    for &value in values {
        sum += value;
        if !sum.is_finite() {
            return Err(ReductionError::NonFiniteResult);
        }
    }
    Ok(sum)
}

/// Computes the mean; an empty slice has the explicit zero result.
pub fn stable_mean(values: &[f64]) -> Result<f64, ReductionError> {
    if values.is_empty() {
        return Ok(0.0);
    }
    check(stable_sum(values)? / values.len() as f64)
}

/// Geometry-weighted scalar edge contributions accumulated at both endpoint cells.
/// A cell with zero incident weight has value zero.
pub fn weighted_edge_scalar_sum(
    cell_count: usize,
    edges: &[WeightedEdge<f64>],
) -> Result<Vec<f64>, ReductionError> {
    let ordered = canonicalize(cell_count, edges, f64::is_finite, |v| [v.to_bits(), 0, 0])?;
    let mut sums = vec![0.0; cell_count];
    for edge in ordered {
        let contribution = check(edge.weight * edge.value)?;
        for cell in edge.cells {
            let slot = &mut sums[cell as usize];
            *slot = check(*slot + contribution)?;
        }
    }
    Ok(sums)
}

/// Geometry-weighted scalar edge mean at both endpoint cells. Zero total weight yields zero.
pub fn weighted_edge_scalar_mean(
    cell_count: usize,
    edges: &[WeightedEdge<f64>],
) -> Result<Vec<f64>, ReductionError> {
    let ordered = canonicalize(cell_count, edges, f64::is_finite, |v| [v.to_bits(), 0, 0])?;
    let mut sums = vec![0.0; cell_count];
    let mut weights = vec![0.0; cell_count];
    for edge in ordered {
        let contribution = check(edge.weight * edge.value)?;
        for cell in edge.cells {
            let i = cell as usize;
            sums[i] = check(sums[i] + contribution)?;
            weights[i] = check(weights[i] + edge.weight)?;
        }
    }
    sums.iter()
        .zip(weights)
        .map(|(&sum, weight)| {
            if weight == 0.0 {
                Ok(0.0)
            } else {
                check(sum / weight)
            }
        })
        .collect()
}

/// Geometry-weighted vector edge contributions accumulated at both endpoint cells.
pub fn weighted_edge_vector_sum(
    cell_count: usize,
    edges: &[WeightedEdge<DVec3>],
) -> Result<Vec<DVec3>, ReductionError> {
    let ordered = canonicalize(cell_count, edges, DVec3::is_finite, |v| {
        [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
    })?;
    let mut sums = vec![DVec3::ZERO; cell_count];
    for edge in ordered {
        let contribution = edge.value * edge.weight;
        if !contribution.is_finite() {
            return Err(ReductionError::NonFiniteResult);
        }
        for cell in edge.cells {
            let sum = &mut sums[cell as usize];
            *sum += contribution;
            if !sum.is_finite() {
                return Err(ReductionError::NonFiniteResult);
            }
        }
    }
    Ok(sums)
}

/// Geometry-weighted vector edge mean at both endpoint cells. Zero total weight yields zero.
pub fn weighted_edge_vector_mean(
    cell_count: usize,
    edges: &[WeightedEdge<DVec3>],
) -> Result<Vec<DVec3>, ReductionError> {
    let ordered = canonicalize(cell_count, edges, DVec3::is_finite, |v| {
        [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
    })?;
    let mut sums = vec![DVec3::ZERO; cell_count];
    let mut weights = vec![0.0; cell_count];
    for edge in ordered {
        let contribution = edge.value * edge.weight;
        if !contribution.is_finite() {
            return Err(ReductionError::NonFiniteResult);
        }
        for cell in edge.cells {
            let i = cell as usize;
            sums[i] += contribution;
            if !sums[i].is_finite() {
                return Err(ReductionError::NonFiniteResult);
            }
            weights[i] = check(weights[i] + edge.weight)?;
        }
    }
    sums.iter()
        .zip(weights)
        .map(|(&sum, weight)| {
            if weight == 0.0 {
                Ok(DVec3::ZERO)
            } else {
                let mean = sum / weight;
                mean.is_finite()
                    .then_some(mean)
                    .ok_or(ReductionError::NonFiniteResult)
            }
        })
        .collect()
}

/// Validates the lengths of parallel edge arrays before they are zipped.
pub fn validate_edge_lengths<T>(
    edges: &[[CellId; 2]],
    values: &[T],
    weights: &[f64],
) -> Result<(), ReductionError> {
    (edges.len() == values.len() && edges.len() == weights.len())
        .then_some(())
        .ok_or(ReductionError::LengthMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_scalar_aggregation_matches_hand_calculation_and_permutation() {
        let edges = [
            WeightedEdge {
                cells: [2, 0],
                weight: 2.0,
                value: 3.0,
            },
            WeightedEdge {
                cells: [0, 1],
                weight: 1.0,
                value: 6.0,
            },
            WeightedEdge {
                cells: [2, 1],
                weight: 3.0,
                value: 2.0,
            },
        ];
        let means = weighted_edge_scalar_mean(4, &edges).unwrap();
        assert_eq!(means, vec![4.0, 3.0, 2.4, 0.0]);
        let mut reversed = edges;
        reversed.reverse();
        assert_eq!(means, weighted_edge_scalar_mean(4, &reversed).unwrap());
        assert_eq!(
            weighted_edge_scalar_sum(4, &edges).unwrap(),
            vec![12.0, 12.0, 12.0, 0.0]
        );
    }

    #[test]
    fn vector_mean_and_zero_weight_are_explicit() {
        let edges = [
            WeightedEdge {
                cells: [0, 1],
                weight: 1.0,
                value: DVec3::X,
            },
            WeightedEdge {
                cells: [0, 2],
                weight: 3.0,
                value: DVec3::Y,
            },
        ];
        let means = weighted_edge_vector_mean(4, &edges).unwrap();
        assert_eq!(means[0], (DVec3::X + 3.0 * DVec3::Y) / 4.0);
        assert_eq!(means[3], DVec3::ZERO);
        let zero = [WeightedEdge {
            cells: [0, 1],
            weight: 0.0,
            value: DVec3::X,
        }];
        assert_eq!(
            weighted_edge_vector_mean(2, &zero).unwrap(),
            vec![DVec3::ZERO; 2]
        );
    }

    #[test]
    fn malformed_edges_and_lengths_return_errors() {
        let invalid = [WeightedEdge {
            cells: [0, 3],
            weight: 1.0,
            value: 1.0,
        }];
        assert_eq!(
            weighted_edge_scalar_sum(2, &invalid),
            Err(ReductionError::InvalidCell(3))
        );
        assert_eq!(
            validate_edge_lengths(&[[0, 1]], &[1.0, 2.0], &[1.0]),
            Err(ReductionError::LengthMismatch)
        );
    }

    #[test]
    fn stable_sum_rejects_overflow_and_empty_mean_is_zero() {
        assert_eq!(
            stable_sum(&[f64::MAX, f64::MAX]),
            Err(ReductionError::NonFiniteResult)
        );
        assert_eq!(stable_mean(&[]).unwrap(), 0.0);
    }
}

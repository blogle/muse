//! Bounded, generic fixed-point relaxation and convergence diagnostics.

/// The field-delta norm used to decide whether an iteration has converged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConvergenceCriterion {
    /// Root mean square of the element-wise changes.
    Rms { tolerance: f64 },
    /// Largest absolute element-wise change.
    Max { tolerance: f64 },
}

impl ConvergenceCriterion {
    fn tolerance(self) -> f64 {
        match self {
            Self::Rms { tolerance } | Self::Max { tolerance } => tolerance,
        }
    }

    fn delta(self, previous: &[f64], next: &[f64]) -> f64 {
        match self {
            Self::Rms { .. } => {
                let sum = previous.iter().zip(next).fold(0.0, |sum, (&a, &b)| {
                    let change = b - a;
                    sum + change * change
                });
                (sum / previous.len() as f64).sqrt()
            }
            Self::Max { .. } => previous
                .iter()
                .zip(next)
                .fold(0.0_f64, |largest, (&a, &b)| largest.max((b - a).abs())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquilibriumDiagnostics {
    pub converged: bool,
    pub iterations: usize,
    pub final_delta: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EquilibriumError {
    EmptyField,
    ZeroIterationCap,
    InvalidTolerance,
    NonFiniteInput { index: usize },
    NonFiniteDelta,
    ShapeMismatch { expected: usize, actual: usize },
}

/// Applies a deterministic fixed-point map until its field delta reaches the
/// selected tolerance or `hard_cap` iterations have completed.
///
/// The map receives an immutable current field and returns the complete next
/// field. Reductions traverse cells sequentially in slice order.
pub fn relax<F>(
    initial: &[f64],
    criterion: ConvergenceCriterion,
    hard_cap: usize,
    mut update: F,
) -> Result<(Vec<f64>, EquilibriumDiagnostics), EquilibriumError>
where
    F: FnMut(&[f64]) -> Vec<f64>,
{
    if initial.is_empty() {
        return Err(EquilibriumError::EmptyField);
    }
    if hard_cap == 0 {
        return Err(EquilibriumError::ZeroIterationCap);
    }
    let tolerance = criterion.tolerance();
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(EquilibriumError::InvalidTolerance);
    }
    validate_finite(initial)?;

    let mut current = initial.to_vec();
    let mut final_delta = 0.0;
    for iterations in 1..=hard_cap {
        let next = update(&current);
        if next.len() != current.len() {
            return Err(EquilibriumError::ShapeMismatch {
                expected: current.len(),
                actual: next.len(),
            });
        }
        validate_finite(&next)?;
        final_delta = criterion.delta(&current, &next);
        if !final_delta.is_finite() {
            return Err(EquilibriumError::NonFiniteDelta);
        }
        current = next;
        if final_delta <= tolerance {
            return Ok((
                current,
                EquilibriumDiagnostics {
                    converged: true,
                    iterations,
                    final_delta,
                },
            ));
        }
    }

    Ok((
        current,
        EquilibriumDiagnostics {
            converged: false,
            iterations: hard_cap,
            final_delta,
        },
    ))
}

fn validate_finite(values: &[f64]) -> Result<(), EquilibriumError> {
    if let Some(index) = values.iter().position(|value| !value.is_finite()) {
        return Err(EquilibriumError::NonFiniteInput { index });
    }
    Ok(())
}

pub fn validate_diagnostics(value: EquilibriumDiagnostics, hard_cap: usize) -> bool {
    value.iterations <= hard_cap && value.final_delta.is_finite() && value.final_delta >= 0.0
}

#[cfg(test)]
mod tests {
    use super::{ConvergenceCriterion, EquilibriumError, relax};

    #[test]
    fn convergent_sequence_reports_diagnostics_and_repeats_exactly() {
        let run = || {
            relax(
                &[0.0, 0.0],
                ConvergenceCriterion::Max { tolerance: 0.01 },
                20,
                |field| {
                    field
                        .iter()
                        .map(|value| value + (1.0 - value) * 0.5)
                        .collect()
                },
            )
            .unwrap()
        };
        let first = run();
        assert_eq!(first, run());
        assert!(first.1.converged);
        assert!(first.1.iterations <= 20);
        assert!(first.1.final_delta <= 0.01);
    }

    #[test]
    fn hard_cap_returns_bounded_non_convergence() {
        let (_, diagnostics) = relax(
            &[0.0],
            ConvergenceCriterion::Max { tolerance: 0.0 },
            3,
            |x| vec![x[0] + 1.0],
        )
        .unwrap();
        assert!(!diagnostics.converged);
        assert_eq!(diagnostics.iterations, 3);
        assert_eq!(diagnostics.final_delta, 1.0);
    }

    #[test]
    fn rms_and_max_deltas_use_their_defined_norms() {
        let (_, rms) = relax(
            &[0.0, 0.0],
            ConvergenceCriterion::Rms { tolerance: 2.0 },
            1,
            |_| vec![3.0, 4.0],
        )
        .unwrap();
        assert_eq!(rms.final_delta, 12.5_f64.sqrt());

        let (_, max) = relax(
            &[0.0, 0.0],
            ConvergenceCriterion::Max { tolerance: 5.0 },
            1,
            |_| vec![3.0, -4.0],
        )
        .unwrap();
        assert_eq!(max.final_delta, 4.0);
    }

    #[test]
    fn rejects_empty_or_mismatched_fields() {
        assert_eq!(
            relax(
                &[],
                ConvergenceCriterion::Max { tolerance: 1.0 },
                1,
                |_| vec![]
            ),
            Err(EquilibriumError::EmptyField)
        );
        assert_eq!(
            relax(
                &[1.0],
                ConvergenceCriterion::Max { tolerance: 1.0 },
                1,
                |_| vec![1.0, 2.0]
            ),
            Err(EquilibriumError::ShapeMismatch {
                expected: 1,
                actual: 2
            })
        );
    }

    #[test]
    fn rejects_non_finite_initial_and_updated_fields() {
        assert_eq!(
            relax(
                &[f64::NAN],
                ConvergenceCriterion::Max { tolerance: 1.0 },
                1,
                |_| vec![0.0]
            ),
            Err(EquilibriumError::NonFiniteInput { index: 0 })
        );
        assert_eq!(
            relax(
                &[0.0],
                ConvergenceCriterion::Max { tolerance: 1.0 },
                1,
                |_| vec![f64::INFINITY]
            ),
            Err(EquilibriumError::NonFiniteInput { index: 0 })
        );
    }

    #[test]
    fn zero_cap_is_rejected() {
        assert_eq!(
            relax(
                &[0.0],
                ConvergenceCriterion::Max { tolerance: 1.0 },
                0,
                |x| x.to_vec()
            ),
            Err(EquilibriumError::ZeroIterationCap)
        );
    }
}

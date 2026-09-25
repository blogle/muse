//! Bounded, generic fixed-point relaxation and convergence diagnostics.

/// Summary of a bounded fixed-point relaxation run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquilibriumDiagnostics {
    pub converged: bool,
    pub iterations: usize,
    /// The final RMS or maximum element-wise change, as selected by the criterion.
    pub final_delta: f64,
}

/// Norm used to decide whether one relaxation update has converged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvergenceCriterion {
    Rms,
    Max,
}

/// Invalid input or output encountered during fixed-point relaxation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquilibriumError {
    EmptyField,
    ShapeMismatch { expected: usize, actual: usize },
    InvalidTolerance,
    ZeroIterationCap,
    NonFiniteValue,
}

/// Apply `update` repeatedly until its field delta satisfies the selected criterion
/// or `hard_cap` updates have been performed. The callback must compute a complete
/// next field from the current field; each returned field is validated before use.
///
/// Iteration and reductions are sequential and stable, so identical inputs and
/// callbacks produce identical diagnostics on a canonical build.
pub fn relax_fixed_point<F>(
    initial: &[f64],
    tolerance: f64,
    hard_cap: usize,
    criterion: ConvergenceCriterion,
    mut update: F,
) -> Result<(Vec<f64>, EquilibriumDiagnostics), EquilibriumError>
where
    F: FnMut(&[f64]) -> Vec<f64>,
{
    if initial.is_empty() {
        return Err(EquilibriumError::EmptyField);
    }
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(EquilibriumError::InvalidTolerance);
    }
    if hard_cap == 0 {
        return Err(EquilibriumError::ZeroIterationCap);
    }
    if initial.iter().any(|value| !value.is_finite()) {
        return Err(EquilibriumError::NonFiniteValue);
    }

    let mut current = initial.to_vec();
    let mut final_delta = f64::INFINITY;
    for iteration in 1..=hard_cap {
        let next = update(&current);
        if next.len() != current.len() {
            return Err(EquilibriumError::ShapeMismatch {
                expected: current.len(),
                actual: next.len(),
            });
        }
        if next.iter().any(|value| !value.is_finite()) {
            return Err(EquilibriumError::NonFiniteValue);
        }

        let mut max_delta: f64 = 0.0;
        let mut squared_delta_sum = 0.0;
        for (before, after) in current.iter().zip(&next) {
            let delta = (after - before).abs();
            max_delta = max_delta.max(delta);
            squared_delta_sum += delta * delta;
        }
        let rms_delta = (squared_delta_sum / current.len() as f64).sqrt();
        final_delta = match criterion {
            ConvergenceCriterion::Rms => rms_delta,
            ConvergenceCriterion::Max => max_delta,
        };
        if !final_delta.is_finite() {
            return Err(EquilibriumError::NonFiniteValue);
        }
        current = next;

        if final_delta <= tolerance {
            return Ok((
                current,
                EquilibriumDiagnostics {
                    converged: true,
                    iterations: iteration,
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

/// Check that diagnostics are finite, nonnegative, and within the configured cap.
pub fn validate_diagnostics(value: EquilibriumDiagnostics, hard_cap: usize) -> bool {
    value.iterations <= hard_cap && value.final_delta.is_finite() && value.final_delta >= 0.0
}

#[cfg(test)]
mod tests {
    use super::{
        ConvergenceCriterion, EquilibriumDiagnostics, EquilibriumError, relax_fixed_point,
        validate_diagnostics,
    };

    #[test]
    fn converges_and_is_repeatable() {
        let run = || {
            relax_fixed_point(&[0.0, 4.0], 0.01, 32, ConvergenceCriterion::Max, |field| {
                field.iter().map(|value| value * 0.5).collect()
            })
            .unwrap()
        };
        let first = run();
        let second = run();
        assert_eq!(first, second);
        assert!(first.1.converged);
        assert!(first.1.iterations <= 32);
        assert!(first.1.final_delta <= 0.01);
    }

    #[test]
    fn reports_bounded_non_convergence_at_hard_cap() {
        let (_, diagnostics) =
            relax_fixed_point(&[0.0], 0.0, 3, ConvergenceCriterion::Max, |field| {
                field.iter().map(|value| value + 1.0).collect()
            })
            .unwrap();
        assert_eq!(
            diagnostics,
            EquilibriumDiagnostics {
                converged: false,
                iterations: 3,
                final_delta: 1.0,
            }
        );
        assert!(validate_diagnostics(diagnostics, 3));
    }

    #[test]
    fn computes_rms_and_max_field_deltas() {
        for (criterion, expected) in [
            (ConvergenceCriterion::Rms, 5.0_f64.sqrt()),
            (ConvergenceCriterion::Max, 3.0),
        ] {
            let (_, diagnostics) =
                relax_fixed_point(&[0.0, 0.0], 0.0, 1, criterion, |_| vec![1.0, 3.0]).unwrap();
            assert!((diagnostics.final_delta - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn rejects_empty_mismatched_and_non_finite_fields() {
        assert_eq!(
            relax_fixed_point(&[], 1.0, 1, ConvergenceCriterion::Rms, |_| vec![]),
            Err(EquilibriumError::EmptyField)
        );
        assert_eq!(
            relax_fixed_point(&[1.0], 1.0, 1, ConvergenceCriterion::Rms, |_| vec![]),
            Err(EquilibriumError::ShapeMismatch {
                expected: 1,
                actual: 0
            })
        );
        assert_eq!(
            relax_fixed_point(&[f64::NAN], 1.0, 1, ConvergenceCriterion::Rms, |v| v
                .to_vec()),
            Err(EquilibriumError::NonFiniteValue)
        );
        assert_eq!(
            relax_fixed_point(&[0.0], 1.0, 1, ConvergenceCriterion::Rms, |_| vec![
                f64::INFINITY
            ]),
            Err(EquilibriumError::NonFiniteValue)
        );
    }

    #[test]
    fn enforces_hard_cap_even_when_more_work_would_converge() {
        let (_, diagnostics) =
            relax_fixed_point(&[1.0], 0.01, 1, ConvergenceCriterion::Max, |field| {
                field.iter().map(|value| value * 0.5).collect()
            })
            .unwrap();
        assert_eq!(diagnostics.iterations, 1);
        assert!(!diagnostics.converged);
    }
}

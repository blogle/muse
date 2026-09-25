//! Bounded, generic fixed-point relaxation and equilibrium diagnostics.

/// Convergence rule applied to the per-iteration field delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConvergenceCriterion {
    /// The root-mean-square field delta is at most the tolerance.
    Rms,
    /// The largest absolute field delta is at most the tolerance.
    Max,
    /// Both RMS and maximum deltas are at most the tolerance.
    RmsAndMax,
    /// Either RMS or maximum delta is at most the tolerance.
    RmsOrMax,
}

/// Bounded relaxation parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelaxationConfig {
    pub criterion: ConvergenceCriterion,
    pub tolerance: f64,
    pub hard_iteration_cap: usize,
}

/// Summary of the final fixed-point update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquilibriumDiagnostics {
    pub converged: bool,
    pub iterations: usize,
    /// Delta selected by [`RelaxationConfig::criterion`].
    pub final_delta: f64,
    pub final_rms_delta: f64,
    pub final_max_delta: f64,
}

/// Result of a bounded scalar-field relaxation.
#[derive(Clone, Debug, PartialEq)]
pub struct RelaxationResult {
    pub field: Vec<f64>,
    pub diagnostics: EquilibriumDiagnostics,
}

/// Errors found while validating configuration or a relaxation update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquilibriumError {
    InvalidTolerance,
    ZeroIterationCap,
    EmptyField,
    ShapeMismatch,
    NonFiniteValue,
}

/// Repeatedly applies a deterministic fixed-point update until convergence or
/// the explicit hard iteration cap is reached.
///
/// The update is a mathematical relaxation step, not physical-time stepping.
/// Its output must have the same shape as its input and contain only finite
/// values. Deltas are reduced sequentially in slice order for deterministic
/// results.
pub fn relax_scalar_field<F>(
    initial: &[f64],
    config: RelaxationConfig,
    mut update: F,
) -> Result<RelaxationResult, EquilibriumError>
where
    F: FnMut(&[f64]) -> Vec<f64>,
{
    if !config.tolerance.is_finite() || config.tolerance < 0.0 {
        return Err(EquilibriumError::InvalidTolerance);
    }
    if config.hard_iteration_cap == 0 {
        return Err(EquilibriumError::ZeroIterationCap);
    }
    if initial.is_empty() {
        return Err(EquilibriumError::EmptyField);
    }
    if initial.iter().any(|value| !value.is_finite()) {
        return Err(EquilibriumError::NonFiniteValue);
    }

    let mut field = initial.to_vec();
    let mut diagnostics = EquilibriumDiagnostics {
        converged: false,
        iterations: 0,
        final_delta: 0.0,
        final_rms_delta: 0.0,
        final_max_delta: 0.0,
    };

    for iteration in 1..=config.hard_iteration_cap {
        let next = update(&field);
        if next.len() != field.len() {
            return Err(EquilibriumError::ShapeMismatch);
        }
        if next.iter().any(|value| !value.is_finite()) {
            return Err(EquilibriumError::NonFiniteValue);
        }

        let mut squared_delta_sum = 0.0;
        let mut max_delta: f64 = 0.0;
        for (previous, current) in field.iter().zip(&next) {
            let delta = (current - previous).abs();
            if !delta.is_finite() {
                return Err(EquilibriumError::NonFiniteValue);
            }
            squared_delta_sum += delta * delta;
            if !squared_delta_sum.is_finite() {
                return Err(EquilibriumError::NonFiniteValue);
            }
            max_delta = max_delta.max(delta);
        }
        let rms_delta = (squared_delta_sum / field.len() as f64).sqrt();
        if !rms_delta.is_finite() {
            return Err(EquilibriumError::NonFiniteValue);
        }
        let converged = match config.criterion {
            ConvergenceCriterion::Rms => rms_delta <= config.tolerance,
            ConvergenceCriterion::Max => max_delta <= config.tolerance,
            ConvergenceCriterion::RmsAndMax => {
                rms_delta <= config.tolerance && max_delta <= config.tolerance
            }
            ConvergenceCriterion::RmsOrMax => {
                rms_delta <= config.tolerance || max_delta <= config.tolerance
            }
        };
        let final_delta = match config.criterion {
            ConvergenceCriterion::Rms => rms_delta,
            ConvergenceCriterion::Max => max_delta,
            ConvergenceCriterion::RmsAndMax => rms_delta.max(max_delta),
            ConvergenceCriterion::RmsOrMax => rms_delta.min(max_delta),
        };

        field = next;
        diagnostics = EquilibriumDiagnostics {
            converged,
            iterations: iteration,
            final_delta,
            final_rms_delta: rms_delta,
            final_max_delta: max_delta,
        };
        if converged {
            break;
        }
    }

    Ok(RelaxationResult { field, diagnostics })
}

/// Reports whether diagnostics are finite, nonnegative, and within a hard cap.
pub fn validate_diagnostics(value: EquilibriumDiagnostics, hard_cap: usize) -> bool {
    value.iterations <= hard_cap
        && value.final_delta.is_finite()
        && value.final_delta >= 0.0
        && value.final_rms_delta.is_finite()
        && value.final_rms_delta >= 0.0
        && value.final_max_delta.is_finite()
        && value.final_max_delta >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(criterion: ConvergenceCriterion, tolerance: f64, cap: usize) -> RelaxationConfig {
        RelaxationConfig {
            criterion,
            tolerance,
            hard_iteration_cap: cap,
        }
    }

    #[test]
    fn converges_and_reports_rms_and_max_deltas() {
        let result = relax_scalar_field(
            &[0.0, 0.0],
            config(ConvergenceCriterion::Max, 0.1, 8),
            |field| field.iter().map(|value| value + 0.05).collect(),
        )
        .unwrap();

        assert!(result.diagnostics.converged);
        assert_eq!(result.diagnostics.iterations, 1);
        assert_eq!(result.field, vec![0.05, 0.05]);
        assert!((result.diagnostics.final_rms_delta - 0.05).abs() < 1e-12);
        assert!((result.diagnostics.final_max_delta - 0.05).abs() < 1e-12);
        assert!(validate_diagnostics(result.diagnostics, 8));
    }

    #[test]
    fn reports_bounded_non_convergence_at_the_cap() {
        let result =
            relax_scalar_field(&[0.0], config(ConvergenceCriterion::Rms, 0.0, 3), |field| {
                vec![field[0] + 1.0]
            })
            .unwrap();

        assert!(!result.diagnostics.converged);
        assert_eq!(result.diagnostics.iterations, 3);
        assert_eq!(result.diagnostics.final_delta, 1.0);
        assert_eq!(result.field, vec![3.0]);
        assert!(!validate_diagnostics(result.diagnostics, 2));
    }

    #[test]
    fn max_criterion_detects_a_single_large_delta() {
        let result = relax_scalar_field(
            &[0.0, 0.0, 0.0, 0.0],
            config(ConvergenceCriterion::Max, 1.5, 1),
            |_| vec![2.0, 0.0, 0.0, 0.0],
        )
        .unwrap();
        assert_eq!(result.diagnostics.final_max_delta, 2.0);
        assert_eq!(result.diagnostics.final_rms_delta, 1.0);
        assert!(!result.diagnostics.converged);

        let rms_result = relax_scalar_field(
            &[0.0, 0.0, 0.0, 0.0],
            config(ConvergenceCriterion::Rms, 1.1, 1),
            |_| vec![2.0, 0.0, 0.0, 0.0],
        )
        .unwrap();
        assert!(rms_result.diagnostics.converged);
    }

    #[test]
    fn rejects_empty_mismatched_and_non_finite_fields() {
        let cfg = config(ConvergenceCriterion::Max, 0.1, 2);
        assert_eq!(
            relax_scalar_field(&[], cfg, |_| vec![]),
            Err(EquilibriumError::EmptyField)
        );
        assert_eq!(
            relax_scalar_field(&[1.0], cfg, |_| vec![1.0, 2.0]),
            Err(EquilibriumError::ShapeMismatch)
        );
        assert_eq!(
            relax_scalar_field(&[f64::NAN], cfg, |field| field.to_vec()),
            Err(EquilibriumError::NonFiniteValue)
        );
        assert_eq!(
            relax_scalar_field(&[1.0], cfg, |_| vec![f64::INFINITY]),
            Err(EquilibriumError::NonFiniteValue)
        );
    }

    #[test]
    fn repeated_runs_are_identical() {
        let run = || {
            relax_scalar_field(
                &[1.0, 2.0, 3.0],
                config(ConvergenceCriterion::RmsAndMax, 0.01, 16),
                |field| field.iter().map(|value| value * 0.5).collect(),
            )
            .unwrap()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn rejects_invalid_bounds() {
        assert_eq!(
            relax_scalar_field(
                &[0.0],
                config(ConvergenceCriterion::Max, f64::NAN, 2),
                |field| field.to_vec(),
            ),
            Err(EquilibriumError::InvalidTolerance)
        );
        assert_eq!(
            relax_scalar_field(&[0.0], config(ConvergenceCriterion::Max, 0.0, 0), |field| {
                field.to_vec()
            },),
            Err(EquilibriumError::ZeroIterationCap)
        );
    }
}
